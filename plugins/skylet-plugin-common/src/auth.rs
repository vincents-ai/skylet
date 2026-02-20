// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// Authentication plugin abstraction for Skylet
// Provides common interface for authentication and authorization plugins

use anyhow::Result;
use serde::{Deserialize, Serialize};
use skylet_abi::*;
use std::collections::HashMap;

/// Common authentication methods supported by plugins
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuthMethod {
    /// OAuth2 authentication flow
    OAuth2,
    /// API key authentication
    ApiKey,
    /// Basic username/password authentication
    Basic,
    /// Bearer token authentication
    Bearer,
    /// JWT token authentication
    JWT,
    /// Custom authentication method
    Custom(String),
}

/// Authentication credentials for different methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthCredentials {
    /// Authentication method
    pub method: AuthMethod,
    /// Credential data (varies by method)
    pub credentials: HashMap<String, String>,
    /// Additional metadata
    pub metadata: Option<HashMap<String, String>>,
}

/// Authentication result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResult {
    /// Whether authentication was successful
    pub success: bool,
    /// Access token (if applicable)
    pub access_token: Option<String>,
    /// Refresh token (if applicable)
    pub refresh_token: Option<String>,
    /// Token expiration time (Unix timestamp)
    pub expires_at: Option<u64>,
    /// Additional user/session data
    pub user_data: Option<serde_json::Value>,
    /// Error message (if authentication failed)
    pub error: Option<String>,
}

/// User/identity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// Unique user identifier
    pub id: String,
    /// Username
    pub username: String,
    /// Email address
    pub email: Option<String>,
    /// Display name
    pub display_name: Option<String>,
    /// Profile URL
    pub profile_url: Option<String>,
    /// Avatar URL
    pub avatar_url: Option<String>,
    /// Additional user attributes
    pub attributes: HashMap<String, String>,
}

/// Authentication provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProviderConfig {
    /// Provider name (e.g., "github", "google", "microsoft")
    pub provider: String,
    /// OAuth2 client ID
    pub client_id: Option<String>,
    /// OAuth2 client secret
    pub client_secret: Option<String>,
    /// OAuth2 authorization URL
    pub auth_url: Option<String>,
    /// OAuth2 token URL
    pub token_url: Option<String>,
    /// OAuth2 scope(s)
    pub scope: Option<String>,
    /// Redirect URI for OAuth2
    pub redirect_uri: Option<String>,
    /// Additional provider-specific configuration
    pub config: HashMap<String, String>,
}

/// Main authentication plugin trait
pub trait AuthPlugin: Send + Sync {
    /// Get plugin information
    fn get_provider_name(&self) -> &str;

    /// Get supported authentication methods
    fn get_supported_methods(&self) -> Vec<AuthMethod>;

    /// Get provider configuration requirements
    fn get_config_requirements(&self) -> Vec<String>;

    /// Initialize the authentication provider (synchronous)
    fn initialize(&mut self, config: AuthProviderConfig) -> Result<()>;

    /// Authenticate with provided credentials
    fn authenticate(&self, credentials: AuthCredentials) -> Result<AuthResult>;

    /// Validate an existing token
    fn validate_token(&self, token: &str) -> Result<bool>;

    /// Refresh an expired token
    fn refresh_token(&self, refresh_token: &str) -> Result<AuthResult>;

    /// Get user information from token
    fn get_user_info(&self, token: &str) -> Result<UserInfo>;

    /// Revoke/remove authentication
    fn revoke_token(&self, token: &str) -> Result<bool>;

    /// Generate authorization URL (for OAuth2)
    fn get_auth_url(&self, state: Option<&str>) -> Result<String>;

    /// Exchange authorization code for token (for OAuth2)
    fn exchange_code_for_token(&self, code: &str, state: Option<&str>) -> Result<AuthResult>;

    /// Check if provider supports token introspection
    fn supports_token_introspection(&self) -> bool {
        false
    }

    /// Introspect token to get metadata
    fn introspect_token(&self, token: &str) -> Result<serde_json::Value> {
        Err(anyhow::anyhow!("Token introspection not supported"))
    }
}

/// HTTP client extension for authentication
pub trait AuthenticatedClient {
    /// Add authentication headers to request
    fn add_auth_headers(&self, headers: &mut HashMap<String, String>, token: &str);

    /// Check if request needs authentication
    fn needs_auth(&self, url: &str) -> bool;

    /// Handle authentication errors
    fn handle_auth_error(&self, status: u16, headers: &HashMap<String, String>) -> bool;
}

/// Common authentication error types
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Unsupported authentication method: {0}")]
    UnsupportedMethod(String),

    #[error("Invalid credentials: {0}")]
    InvalidCredentials(String),

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Provider error: {0}")]
    ProviderError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,
}

impl From<AuthError> for super::PluginCommonError {
    fn from(err: AuthError) -> Self {
        match err {
            AuthError::NetworkError(msg) => super::PluginCommonError::NetworkError(msg),
            AuthError::RateLimitExceeded => super::PluginCommonError::RateLimitExceeded,
            _ => super::PluginCommonError::HttpRequestFailed(err.to_string()),
        }
    }
}

/// Common OAuth2 utilities
pub mod oauth2_utils {
    use super::*;
    use url::Url;

    /// Generate OAuth2 authorization URL
    pub fn generate_auth_url(
        base_url: &str,
        client_id: &str,
        redirect_uri: &str,
        scope: &str,
        state: Option<&str>,
    ) -> Result<String> {
        let mut url =
            Url::parse(base_url).map_err(|e| anyhow::anyhow!("Invalid auth URL: {}", e))?;

        url.query_pairs_mut()
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("scope", scope)
            .append_pair("response_type", "code");

        if let Some(s) = state {
            url.query_pairs_mut().append_pair("state", s);
        }

        Ok(url.to_string())
    }

    /// Parse OAuth2 callback parameters
    pub fn parse_callback_params(query: &str) -> Result<(String, Option<String>)> {
        let params = url::form_urlencoded::parse(query.as_bytes());

        let mut code = None;
        let mut state = None;
        let mut error = None;

        for (key, value) in params {
            match key.as_ref() {
                "code" => code = Some(value.into_owned()),
                "state" => state = Some(value.into_owned()),
                "error" => error = Some(value.into_owned()),
                _ => {}
            }
        }

        if let Some(err) = error {
            return Err(anyhow::anyhow!("OAuth2 error: {}", err));
        }

        let code = code.ok_or_else(|| anyhow::anyhow!("Missing authorization code"))?;
        Ok((code, state))
    }
}
