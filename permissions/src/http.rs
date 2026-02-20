// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Authentication HTTP Handlers - RFC-0023
//!
//! HTTP endpoints for authentication operations:
//! - POST /auth/login - Authenticate user
//! - GET /auth/validate - Validate session token
//! - POST /auth/logout - Invalidate session

use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    response::Json,
    Json as JsonRequest,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::auth::{AuthProvider, Credentials, LocalAuthProvider};
use crate::types::{AuthResult, Session};

// ============================================================================
// Rate Limiting
// ============================================================================

/// Simple in-memory rate limiter to prevent brute-force attacks
pub struct RateLimiter {
    /// Map of client key -> list of attempt timestamps
    attempts: RwLock<HashMap<String, Vec<Instant>>>,
    /// Maximum number of attempts allowed within the window
    max_attempts: usize,
    /// Time window for rate limiting
    window: Duration,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `max_attempts` - Maximum attempts allowed within the window
    /// * `window_secs` - Time window in seconds
    pub fn new(max_attempts: usize, window_secs: u64) -> Self {
        Self {
            attempts: RwLock::new(HashMap::new()),
            max_attempts,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Check if a request from the given key should be allowed
    ///
    /// Returns `true` if the request is allowed, `false` if rate limited
    pub fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut attempts = self.attempts.write();
        let entry = attempts.entry(key.to_string()).or_default();

        // Remove old attempts outside the window
        entry.retain(|t| now.duration_since(*t) < self.window);

        if entry.len() >= self.max_attempts {
            false // Rate limited
        } else {
            entry.push(now);
            true // Allowed
        }
    }

    /// Clean up old entries to prevent memory growth
    /// Should be called periodically in production
    pub fn cleanup(&self) {
        let now = Instant::now();
        let mut attempts = self.attempts.write();
        attempts.retain(|_, timestamps| {
            timestamps.retain(|t| now.duration_since(*t) < self.window);
            !timestamps.is_empty()
        });
    }
}

// ============================================================================
// HTTP Request/Response Types
// ============================================================================

/// Login request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    /// Authentication method
    pub method: AuthMethod,
    /// Username for password auth
    pub username: Option<String>,
    /// Password for password auth
    pub password: Option<String>,
    /// AGE public key for AGE auth
    pub age_public_key: Option<String>,
    /// Signature for AGE auth
    pub signature: Option<String>,
    /// Challenge for AGE auth
    pub challenge: Option<String>,
    /// API key for API key auth
    pub api_key: Option<String>,
    /// API secret (optional)
    pub api_secret: Option<String>,
    /// JWT token for JWT auth
    pub jwt_token: Option<String>,
}

/// Authentication method
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    Password,
    AgeKey,
    ApiKey,
    Jwt,
}

/// Login response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub success: bool,
    pub token: Option<String>,
    pub expires_at: Option<String>,
    pub user: Option<UserInfo>,
    pub error: Option<String>,
}

/// User info in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub user_id: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

impl From<&Session> for UserInfo {
    fn from(session: &Session) -> Self {
        Self {
            user_id: session.user.user_id.0.clone(),
            display_name: session.user.display_name.clone(),
            email: session.user.email.clone(),
            roles: session.roles.iter().map(|r| r.name.clone()).collect(),
            permissions: session.permissions.iter().map(|p| p.as_str()).collect(),
        }
    }
}

/// Validate response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateResponse {
    pub valid: bool,
    pub user: Option<UserInfo>,
    pub expires_at: Option<String>,
    pub error: Option<String>,
}

/// Logout response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoutResponse {
    pub success: bool,
    pub message: String,
}

/// Generic error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: u16,
}

// ============================================================================
// Auth State
// ============================================================================

/// Shared authentication state for HTTP handlers
pub struct AuthState {
    pub provider: Arc<LocalAuthProvider>,
    /// Rate limiter for authentication endpoints (login, register)
    pub rate_limiter: Arc<RateLimiter>,
}

impl AuthState {
    /// Create new auth state with default TTL of 24 hours
    /// Rate limits: 5 attempts per 60 seconds per IP
    pub fn new() -> Self {
        Self {
            provider: Arc::new(LocalAuthProvider::new(86400)), // 24 hours
            rate_limiter: Arc::new(RateLimiter::new(5, 60)),   // 5 attempts per minute
        }
    }

    /// Create auth state with custom TTL
    pub fn with_ttl(ttl_seconds: i64) -> Self {
        Self {
            provider: Arc::new(LocalAuthProvider::new(ttl_seconds)),
            rate_limiter: Arc::new(RateLimiter::new(5, 60)),
        }
    }

    /// Create auth state with existing provider
    pub fn with_provider(provider: Arc<LocalAuthProvider>) -> Self {
        Self {
            provider,
            rate_limiter: Arc::new(RateLimiter::new(5, 60)),
        }
    }

    /// Create auth state with custom rate limiting
    pub fn with_rate_limit(
        provider: Arc<LocalAuthProvider>,
        max_attempts: usize,
        window_secs: u64,
    ) -> Self {
        Self {
            provider,
            rate_limiter: Arc::new(RateLimiter::new(max_attempts, window_secs)),
        }
    }
}

impl Default for AuthState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// POST /auth/login - Authenticate user
///
/// Supports multiple authentication methods:
/// - Password: username + password
/// - AGE Key: age_public_key + signature + challenge
/// - API Key: api_key + optional api_secret
/// - JWT: jwt_token
///
/// Rate limited to 5 attempts per minute per IP address.
pub async fn login_handler(
    State(state): State<Arc<AuthState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    JsonRequest(req): JsonRequest<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check rate limit using client IP
    let client_ip = addr.ip().to_string();
    if !state.rate_limiter.check(&client_ip) {
        return Err(error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many authentication attempts. Please try again later.",
        ));
    }

    // Build credentials based on auth method
    let credentials = match req.method {
        AuthMethod::Password => {
            let username = req.username.as_ref().ok_or_else(|| {
                error_response(
                    StatusCode::BAD_REQUEST,
                    "Username required for password auth",
                )
            })?;
            let password = req.password.as_ref().ok_or_else(|| {
                error_response(
                    StatusCode::BAD_REQUEST,
                    "Password required for password auth",
                )
            })?;

            Credentials::Password {
                username: username.clone(),
                password: password.clone(),
            }
        }

        AuthMethod::AgeKey => {
            let age_public_key = req.age_public_key.as_ref().ok_or_else(|| {
                error_response(StatusCode::BAD_REQUEST, "AGE public key required")
            })?;
            let signature = req.signature.as_ref().ok_or_else(|| {
                error_response(StatusCode::BAD_REQUEST, "Signature required for AGE auth")
            })?;
            let challenge = req.challenge.as_ref().ok_or_else(|| {
                error_response(StatusCode::BAD_REQUEST, "Challenge required for AGE auth")
            })?;

            Credentials::AgeKey {
                age_public_key: age_public_key.clone(),
                signature: signature.clone(),
                challenge: challenge.clone(),
            }
        }

        AuthMethod::ApiKey => {
            let api_key = req
                .api_key
                .as_ref()
                .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "API key required"))?;

            Credentials::ApiKey {
                key: api_key.clone(),
                secret: req.api_secret.clone(),
            }
        }

        AuthMethod::Jwt => {
            let jwt_token = req
                .jwt_token
                .as_ref()
                .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "JWT token required"))?;

            Credentials::Jwt {
                token: jwt_token.clone(),
            }
        }
    };

    // Authenticate
    let result = state.provider.authenticate(&credentials);

    match result {
        AuthResult::Success(session) => Ok(Json(LoginResponse {
            success: true,
            token: Some(session.token.token.clone()),
            expires_at: Some(session.token.expires_at.to_rfc3339()),
            user: Some(UserInfo::from(&session)),
            error: None,
        })),

        AuthResult::InvalidCredentials => Ok(Json(LoginResponse {
            success: false,
            token: None,
            expires_at: None,
            user: None,
            error: Some("Invalid credentials".to_string()),
        })),

        AuthResult::AccountLocked => Ok(Json(LoginResponse {
            success: false,
            token: None,
            expires_at: None,
            user: None,
            error: Some("Account is locked".to_string()),
        })),

        AuthResult::AccountExpired => Ok(Json(LoginResponse {
            success: false,
            token: None,
            expires_at: None,
            user: None,
            error: Some("Account has expired".to_string()),
        })),

        AuthResult::ProviderUnavailable => Err(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Authentication provider unavailable",
        )),

        AuthResult::TokenExpired | AuthResult::TokenInvalid => Ok(Json(LoginResponse {
            success: false,
            token: None,
            expires_at: None,
            user: None,
            error: Some("Token invalid or expired".to_string()),
        })),
    }
}

/// GET /auth/validate - Validate session token
///
/// Returns user info if token is valid
pub async fn validate_handler(
    State(state): State<Arc<AuthState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<ValidateResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Extract token from Authorization header
    let token = extract_token(&headers)?;

    // Validate token
    match state.provider.validate_token(&token) {
        Some(session) => Ok(Json(ValidateResponse {
            valid: true,
            user: Some(UserInfo::from(&session)),
            expires_at: Some(session.token.expires_at.to_rfc3339()),
            error: None,
        })),
        None => Ok(Json(ValidateResponse {
            valid: false,
            user: None,
            expires_at: None,
            error: Some("Invalid or expired token".to_string()),
        })),
    }
}

/// POST /auth/logout - Invalidate session
///
/// Revokes the provided token
pub async fn logout_handler(
    State(state): State<Arc<AuthState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<LogoutResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Extract token from Authorization header
    let token = extract_token(&headers)?;

    // Revoke token
    match state.provider.revoke_token(&token) {
        Ok(()) => Ok(Json(LogoutResponse {
            success: true,
            message: "Successfully logged out".to_string(),
        })),
        Err(e) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Logout failed: {}", e),
        )),
    }
}

/// POST /auth/register - Register new user
///
/// Creates a new user with the specified credentials.
/// Rate limited to 5 attempts per minute per IP address.
pub async fn register_handler(
    State(state): State<Arc<AuthState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    JsonRequest(req): JsonRequest<RegisterRequest>,
) -> Result<Json<RegisterResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check rate limit using client IP
    let client_ip = addr.ip().to_string();
    if !state.rate_limiter.check(&client_ip) {
        return Err(error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many registration attempts. Please try again later.",
        ));
    }

    let username = req
        .username
        .as_ref()
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "Username required"))?;
    let password = req
        .password
        .as_ref()
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "Password required"))?;

    // Validate password strength
    if password.len() < 8 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Password must be at least 8 characters",
        ));
    }

    // Register user
    match state.provider.register_user_with_password(
        username.clone(),
        password.clone(),
        req.display_name.clone(),
    ) {
        Ok(user_id) => Ok(Json(RegisterResponse {
            success: true,
            user_id: user_id.0,
            message: "User registered successfully".to_string(),
        })),
        Err(e) => Err(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Registration failed: {}", e),
        )),
    }
}

/// Registration request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub username: Option<String>,
    pub password: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
}

/// Registration response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub success: bool,
    pub user_id: String,
    pub message: String,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract Bearer token from Authorization header
fn extract_token(
    headers: &axum::http::HeaderMap,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| error_response(StatusCode::UNAUTHORIZED, "Missing Authorization header"))?;

    if !auth_header.starts_with("Bearer ") {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "Invalid Authorization header format. Expected 'Bearer <token>'",
        ));
    }

    Ok(auth_header[7..].to_string())
}

/// Create an error response tuple
fn error_response(status: StatusCode, message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: message.to_string(),
            code: status.as_u16(),
        }),
    )
}

// ============================================================================
// Router Builder
// ============================================================================

use axum::routing::{get, post};

/// Build the auth router with the given state
pub fn auth_router(state: Arc<AuthState>) -> axum::Router {
    axum::Router::new()
        .route("/login", post(login_handler))
        .route("/validate", get(validate_handler))
        .route("/logout", post(logout_handler))
        .route("/register", post(register_handler))
        .with_state(state)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_state_creation() {
        let state = AuthState::new();
        assert_eq!(state.provider.name(), "local");
    }

    #[test]
    fn test_auth_state_with_ttl() {
        let state = AuthState::with_ttl(3600);
        assert_eq!(state.provider.name(), "local");
    }

    #[test]
    fn test_user_info_from_session() {
        use super::super::types::*;

        let user = UserIdentity::new("age-test".to_string()).with_display_name("Test User");
        let session = Session {
            token: SessionToken::new(user.user_id.clone(), 3600),
            user,
            claims: Claims::new(&UserId::new(), 3600),
            roles: vec![],
            permissions: vec![],
        };

        let info = UserInfo::from(&session);
        assert_eq!(info.display_name, Some("Test User".to_string()));
    }
}
