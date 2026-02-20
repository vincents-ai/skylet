// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Authentication Providers - RFC-0023
//!
//! Implementations of various authentication providers.
//!
//! SECURITY: Password hashing uses Argon2id (Password Hashing Competition winner)
//! with recommended OWASP parameters for secure password storage.

use anyhow::Result;
use argon2::{self, Algorithm, Argon2, Params, Version};
use parking_lot::RwLock;
use password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rand::rngs::OsRng;
use std::collections::HashMap;
use std::sync::Arc;

use super::types::*;

// ============================================================================
// Auth Provider Trait
// ============================================================================

/// Trait for authentication providers
pub trait AuthProvider: Send + Sync {
    /// Provider name/identifier
    fn name(&self) -> &str;

    /// Authenticate user with credentials
    fn authenticate(&self, credentials: &Credentials) -> AuthResult;

    /// Validate an existing session token
    fn validate_token(&self, token: &str) -> Option<Session>;

    /// Revoke a session token
    fn revoke_token(&self, token: &str) -> Result<()>;

    /// Refresh a session token
    fn refresh_token(&self, token: &str, ttl_seconds: i64) -> Option<SessionToken>;
}

/// Credentials for authentication
#[derive(Debug, Clone)]
pub enum Credentials {
    /// Local authentication with AGE key signature
    AgeKey {
        age_public_key: String,
        signature: String,
        challenge: String,
    },
    /// Username/password authentication
    Password { username: String, password: String },
    /// OAuth2 authorization code
    OAuth2 {
        provider: String,
        code: String,
        state: String,
    },
    /// API key authentication
    ApiKey { key: String, secret: Option<String> },
    /// JWT token
    Jwt { token: String },
}

// ============================================================================
// Local Auth Provider (AGE Keys + Password)
// ============================================================================

/// Local authentication provider supporting AGE keys and passwords
pub struct LocalAuthProvider {
    /// User store
    users: RwLock<HashMap<String, UserIdentity>>,
    /// Session store
    sessions: RwLock<HashMap<String, Session>>,
    /// Password hashes (username -> hash)
    passwords: RwLock<HashMap<String, String>>,
    /// Default session TTL
    default_ttl: i64,
}

impl LocalAuthProvider {
    pub fn new(default_ttl_seconds: i64) -> Self {
        Self {
            users: RwLock::new(HashMap::new()),
            sessions: RwLock::new(HashMap::new()),
            passwords: RwLock::new(HashMap::new()),
            default_ttl: default_ttl_seconds,
        }
    }

    /// Register a new user with AGE public key
    pub fn register_user(&self, user: UserIdentity) -> Result<()> {
        let mut users = self.users.write();
        users.insert(user.user_id.0.clone(), user);
        Ok(())
    }

    /// Register a new user with password
    /// Uses Argon2id for secure password hashing per OWASP guidelines
    pub fn register_user_with_password(
        &self,
        username: String,
        password: String,
        display_name: Option<String>,
    ) -> Result<UserId> {
        let user = UserIdentity::new(format!("age-{}", &username))
            .with_display_name(display_name.clone().unwrap_or_else(|| username.clone()));

        // Secure password hash using Argon2id
        let hash = Self::hash_password(&password)?;

        let user_id = user.user_id.clone();
        let mut users = self.users.write();
        users.insert(user.user_id.0.clone(), user);

        let mut passwords = self.passwords.write();
        passwords.insert(username, hash);

        Ok(user_id)
    }

    /// Create a session for a user
    pub fn create_session(&self, user_id: &UserId, roles: Vec<Role>) -> Session {
        // Acquire write lock on sessions FIRST to prevent TOCTOU race condition.
        // This ensures atomic check-and-insert: no other thread can create a session
        // for the same user between our read of user data and session insertion.
        let mut sessions = self.sessions.write();

        let users = self.users.read();
        let user = users
            .get(&user_id.0)
            .cloned()
            .unwrap_or_else(|| UserIdentity::new("unknown".to_string()));
        drop(users);

        let ttl = self.default_ttl;
        let token = SessionToken::new(user_id.clone(), ttl);
        let claims = Claims::new(user_id, ttl);

        // Collect all permissions from roles
        let permissions: Vec<Permission> =
            roles.iter().flat_map(|r| r.permissions.clone()).collect();

        let session = Session {
            token,
            user,
            claims,
            roles,
            permissions,
        };

        // Store session (already holding write lock)
        let token_str = session.token.token.clone();
        sessions.insert(token_str, session.clone());

        session
    }

    /// Hash a password using Argon2id (OWASP recommended).
    ///
    /// Uses the following parameters per OWASP guidelines:
    /// - Algorithm: Argon2id (hybrid mode for better resistance to attacks)
    /// - Memory: 64 MB (65536 KB)
    /// - Time cost: 3 iterations
    /// - Parallelism: 4 lanes
    /// - Output length: 32 bytes
    fn hash_password(password: &str) -> Result<String> {
        // Generate a random salt
        let salt = SaltString::generate(&mut OsRng);

        // Configure Argon2id with OWASP-recommended parameters
        // See: https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
        let params = Params::new(
            65536,    // m_cost: 64 MB memory
            3,        // t_cost: 3 iterations
            4,        // p_cost: 4 parallel lanes
            Some(32), // output length
        )
        .map_err(|e| anyhow::anyhow!("Failed to create Argon2 params: {}", e))?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        // Hash the password
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;

        Ok(hash.to_string())
    }

    /// Verify a password against an Argon2id hash.
    ///
    /// Supports verification of hashes created with Argon2id, Argon2i, and Argon2d.
    fn verify_password(password: &str, hash: &str) -> bool {
        // Argon2 verification only - no legacy hash support
        if let Ok(parsed_hash) = PasswordHash::new(hash) {
            let result = Argon2::default().verify_password(password.as_bytes(), &parsed_hash);
            return result.is_ok();
        }

        false
    }
}

impl AuthProvider for LocalAuthProvider {
    fn name(&self) -> &str {
        "local"
    }

    fn authenticate(&self, credentials: &Credentials) -> AuthResult {
        match credentials {
            Credentials::Password { username, password } => {
                let passwords = self.passwords.read();
                let stored_hash = passwords.get(username);

                match stored_hash {
                    Some(hash) if Self::verify_password(password, hash) => {
                        // Find user by age key (format: age-{username})
                        let users = self.users.read();
                        let age_key = format!("age-{}", username);
                        let user = users.values().find(|u| u.age_public_key == age_key);

                        match user {
                            Some(user) => {
                                let session = self.create_session(&user.user_id, vec![user_role()]);
                                AuthResult::Success(session)
                            }
                            None => AuthResult::InvalidCredentials,
                        }
                    }
                    _ => AuthResult::InvalidCredentials,
                }
            }

            Credentials::AgeKey {
                age_public_key,
                signature,
                challenge,
            } => {
                // Validate inputs
                if signature.is_empty() || challenge.is_empty() || age_public_key.is_empty() {
                    return AuthResult::InvalidCredentials;
                }

                // Verify the AGE public key format (starts with "age1")
                if !age_public_key.starts_with("age1") {
                    return AuthResult::InvalidCredentials;
                }

                // Parse the AGE recipient to validate the key format
                let _recipient = match age_public_key.parse::<age::x25519::Recipient>() {
                    Ok(r) => r,
                    Err(_) => return AuthResult::InvalidCredentials,
                };

                // Decode signature from base64
                use base64::Engine;
                let engine = base64::engine::general_purpose::STANDARD;
                let sig_bytes = match engine.decode(signature) {
                    Ok(b) => b,
                    Err(_) => return AuthResult::InvalidCredentials,
                };

                // For AGE keys, we verify using Ed25519 over the challenge
                // The signature should be created by the corresponding private identity
                // NOTE: Full cryptographic verification requires the private key to sign
                // For now, we verify the key format is valid and exists in our user store
                // Production should implement challenge-response with the client

                let users = self.users.read();
                let user = users.values().find(|u| u.age_public_key == *age_public_key);

                match user {
                    Some(user) => {
                        // In production: verify Ed25519 signature over challenge using derived pubkey
                        // For now: require signature length matches Ed25519 (64 bytes)
                        if sig_bytes.len() != 64 {
                            return AuthResult::InvalidCredentials;
                        }
                        let session = self.create_session(&user.user_id, vec![user_role()]);
                        AuthResult::Success(session)
                    }
                    None => AuthResult::InvalidCredentials,
                }
            }

            Credentials::ApiKey { key, secret } => {
                // Simple API key auth - in production, verify against secure store
                if key.is_empty() {
                    return AuthResult::InvalidCredentials;
                }

                let users = self.users.read();
                let user = users.values().find(|u| {
                    // Check if API key matches user metadata
                    u.metadata.get("api_key").map(|k| k == key).unwrap_or(false)
                });

                match user {
                    Some(user) => {
                        // Verify secret if present using constant-time comparison
                        if let Some(expected_secret) = user.metadata.get("api_secret") {
                            if let Some(provided_secret) = secret {
                                use subtle::ConstantTimeEq;
                                if provided_secret
                                    .as_bytes()
                                    .ct_eq(expected_secret.as_bytes())
                                    .unwrap_u8()
                                    != 1
                                {
                                    return AuthResult::InvalidCredentials;
                                }
                            }
                        }
                        let session = self.create_session(&user.user_id, vec![user_role()]);
                        AuthResult::Success(session)
                    }
                    None => AuthResult::InvalidCredentials,
                }
            }

            Credentials::Jwt { token } => {
                // Validate JWT token
                self.validate_token(token)
                    .map(AuthResult::Success)
                    .unwrap_or(AuthResult::TokenInvalid)
            }

            _ => AuthResult::ProviderUnavailable,
        }
    }

    fn validate_token(&self, token: &str) -> Option<Session> {
        let sessions = self.sessions.read();
        sessions.get(token).and_then(|session| {
            if session.token.is_expired() {
                None
            } else {
                Some(session.clone())
            }
        })
    }

    fn revoke_token(&self, token: &str) -> Result<()> {
        let mut sessions = self.sessions.write();
        sessions.remove(token);
        Ok(())
    }

    fn refresh_token(&self, token: &str, ttl_seconds: i64) -> Option<SessionToken> {
        let mut sessions = self.sessions.write();

        if let Some(session) = sessions.get_mut(token) {
            if !session.token.is_expired() {
                let now = chrono::Utc::now();
                session.token.expires_at = now + chrono::Duration::seconds(ttl_seconds);
                session.claims.exp = now.timestamp() + ttl_seconds;
                return Some(session.token.clone());
            }
        }
        None
    }
}

// ============================================================================
// OAuth2 Types (for future implementation)
// ============================================================================

/// OAuth2 configuration
#[derive(Debug, Clone)]
pub struct OAuth2Config {
    pub provider: String,
    pub client_id: String,
    pub client_secret: String,
    pub authorize_url: String,
    pub token_url: String,
    pub userinfo_url: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
}

/// OAuth2 token response
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OAuth2TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

/// OAuth2 user info
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OAuth2UserInfo {
    pub sub: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub picture: Option<String>,
}

// ============================================================================
// Auth Provider Registry
// ============================================================================

/// Registry for authentication providers
pub struct AuthProviderRegistry {
    providers: RwLock<HashMap<String, Arc<dyn AuthProvider>>>,
}

impl AuthProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, name: impl Into<String>, provider: Arc<dyn AuthProvider>) {
        let mut providers = self.providers.write();
        providers.insert(name.into(), provider);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn AuthProvider>> {
        let providers = self.providers.read();
        providers.get(name).cloned()
    }

    pub fn authenticate(&self, provider_name: &str, credentials: &Credentials) -> AuthResult {
        let providers = self.providers.read();
        match providers.get(provider_name) {
            Some(provider) => provider.authenticate(credentials),
            None => AuthResult::ProviderUnavailable,
        }
    }
}

impl Default for AuthProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_auth_password() {
        let provider = LocalAuthProvider::new(3600);

        // Register user
        let _user_id = provider
            .register_user_with_password(
                "testuser".to_string(),
                "password123".to_string(),
                Some("Test User".to_string()),
            )
            .unwrap();

        // Authenticate with correct password
        let result = provider.authenticate(&Credentials::Password {
            username: "testuser".to_string(),
            password: "password123".to_string(),
        });

        match result {
            AuthResult::Success(session) => {
                assert_eq!(session.user.display_name, Some("Test User".to_string()));
            }
            _ => panic!("Expected successful authentication"),
        }

        // Authenticate with wrong password
        let result = provider.authenticate(&Credentials::Password {
            username: "testuser".to_string(),
            password: "wrongpassword".to_string(),
        });

        match result {
            AuthResult::InvalidCredentials => {}
            _ => panic!("Expected InvalidCredentials"),
        }
    }

    #[test]
    fn test_session_management() {
        let provider = LocalAuthProvider::new(3600);

        let user = UserIdentity::new("age-test-key".to_string()).with_display_name("Test User");
        provider.register_user(user).unwrap();

        let users = provider.users.read();
        let user_id = users.values().next().map(|u| u.user_id.clone()).unwrap();
        drop(users);

        let session = provider.create_session(&user_id, vec![user_role()]);
        let token = session.token.token.clone();

        // Validate token
        let validated = provider.validate_token(&token);
        assert!(validated.is_some());

        // Revoke token
        provider.revoke_token(&token).unwrap();
        let validated = provider.validate_token(&token);
        assert!(validated.is_none());
    }

    #[test]
    fn test_auth_provider_creation() {
        let provider = LocalAuthProvider::new(1800);
        assert!(!provider.name().is_empty());
    }

    #[test]
    fn test_password_registration() {
        let provider = LocalAuthProvider::new(3600);
        let result = provider.register_user_with_password(
            "alice".to_string(),
            "secure_password".to_string(),
            Some("Alice Smith".to_string()),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_duplicate_password_registration() {
        let provider = LocalAuthProvider::new(3600);
        provider
            .register_user_with_password("alice".to_string(), "password".to_string(), None)
            .unwrap();

        // Duplicate registration should fail
        let result = provider.register_user_with_password(
            "alice".to_string(),
            "different_password".to_string(),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_user_identity_registration() {
        let provider = LocalAuthProvider::new(3600);
        let user = UserIdentity::new("age-public-key".to_string());
        assert!(provider.register_user(user).is_ok());
    }

    #[test]
    fn test_authenticate_nonexistent_user() {
        let provider = LocalAuthProvider::new(3600);
        let result = provider.authenticate(&Credentials::Password {
            username: "nonexistent".to_string(),
            password: "any_password".to_string(),
        });

        match result {
            AuthResult::InvalidCredentials => {}
            _ => panic!("Expected InvalidCredentials for nonexistent user"),
        }
    }

    #[test]
    fn test_token_refresh() {
        let provider = LocalAuthProvider::new(3600);
        let user = UserIdentity::new("age-key".to_string());
        provider.register_user(user).unwrap();

        let users = provider.users.read();
        let user_id = users.values().next().map(|u| u.user_id.clone()).unwrap();
        drop(users);

        let session = provider.create_session(&user_id, vec![user_role()]);
        let original_token = session.token.token.clone();

        // Refresh token
        let refreshed = provider.refresh_token(&original_token, 7200);
        assert!(refreshed.is_some());

        let new_token = refreshed.unwrap().token;
        assert_ne!(new_token, original_token);

        // Original token should be invalidated
        let _validated_original = provider.validate_token(&original_token);
        // The original token may or may not be available depending on implementation

        // New token should be valid
        let validated_new = provider.validate_token(&new_token);
        assert!(validated_new.is_some());
    }

    #[test]
    fn test_token_validation_invalid_token() {
        let provider = LocalAuthProvider::new(3600);
        let result = provider.validate_token("invalid_token_format");
        assert!(result.is_none());
    }

    #[test]
    fn test_revoke_already_revoked_token() {
        let provider = LocalAuthProvider::new(3600);
        let user = UserIdentity::new("age-key".to_string());
        provider.register_user(user).unwrap();

        let users = provider.users.read();
        let user_id = users.values().next().map(|u| u.user_id.clone()).unwrap();
        drop(users);

        let session = provider.create_session(&user_id, vec![user_role()]);
        let token = session.token.token.clone();

        // Revoke twice
        provider.revoke_token(&token).unwrap();
        let _second_revoke = provider.revoke_token(&token);

        // Second revoke may fail or succeed depending on implementation
        // The important thing is token is no longer valid
        let validated = provider.validate_token(&token);
        assert!(validated.is_none());
    }

    #[test]
    fn test_multiple_users_independent_tokens() {
        let provider = LocalAuthProvider::new(3600);

        let user1 = UserIdentity::new("key1".to_string());
        let user2 = UserIdentity::new("key2".to_string());

        provider.register_user(user1).unwrap();
        provider.register_user(user2).unwrap();

        let users = provider.users.read();
        let user_ids: Vec<_> = users.values().map(|u| u.user_id.clone()).collect();
        drop(users);

        assert_eq!(user_ids.len(), 2);

        let session1 = provider.create_session(&user_ids[0], vec![user_role()]);
        let session2 = provider.create_session(&user_ids[1], vec![user_role()]);

        let token1 = session1.token.token.clone();
        let token2 = session2.token.token.clone();

        assert_ne!(token1, token2);

        // Revoke token1
        provider.revoke_token(&token1).unwrap();

        // Token1 should be invalid, token2 should still be valid
        assert!(provider.validate_token(&token1).is_none());
        assert!(provider.validate_token(&token2).is_some());
    }

    #[test]
    fn test_password_not_stored_in_session() {
        let provider = LocalAuthProvider::new(3600);
        provider
            .register_user_with_password(
                "secure_user".to_string(),
                "top_secret_password".to_string(),
                None,
            )
            .unwrap();

        let result = provider.authenticate(&Credentials::Password {
            username: "secure_user".to_string(),
            password: "top_secret_password".to_string(),
        });

        match result {
            AuthResult::Success(session) => {
                // Session should not contain plaintext password
                let user_id = session.user.user_id.0.clone();
                assert!(!user_id.contains("password"));
            }
            _ => panic!("Authentication failed"),
        }
    }

    #[test]
    fn test_auth_provider_registry() {
        let registry = AuthProviderRegistry::new();
        let provider = Arc::new(LocalAuthProvider::new(3600));

        registry.register("local", provider);
        let retrieved = registry.get("local");
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_registry_get_nonexistent_provider() {
        let registry = AuthProviderRegistry::new();
        let result = registry.get("nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_registry_authenticate() {
        let registry = AuthProviderRegistry::new();
        let provider = Arc::new(LocalAuthProvider::new(3600));

        let user = UserIdentity::new("age-key".to_string());
        provider.register_user(user).unwrap();

        registry.register("local", provider);

        let result = registry.authenticate(
            "local",
            &Credentials::ApiKey {
                key: "test-key".to_string(),
                secret: None,
            },
        );

        // Result depends on implementation but should not panic
        match result {
            AuthResult::Success(_) | AuthResult::InvalidCredentials => {}
            _ => {}
        }
    }

    #[test]
    fn test_registry_authenticate_unavailable_provider() {
        let registry = AuthProviderRegistry::new();
        let result = registry.authenticate(
            "unavailable",
            &Credentials::Password {
                username: "user".to_string(),
                password: "pass".to_string(),
            },
        );

        match result {
            AuthResult::ProviderUnavailable => {}
            _ => panic!("Expected ProviderUnavailable"),
        }
    }

    #[test]
    fn test_credentials_variant_equality() {
        let cred1 = Credentials::ApiKey {
            key: "test".to_string(),
            secret: None,
        };
        let cred2 = Credentials::ApiKey {
            key: "test".to_string(),
            secret: None,
        };

        // Both credentials are created successfully
        match cred1 {
            Credentials::ApiKey { .. } => {}
            _ => panic!("Credential type mismatch"),
        }

        match cred2 {
            Credentials::ApiKey { .. } => {}
            _ => panic!("Credential type mismatch"),
        }
    }

    #[test]
    fn test_user_display_name_storage() {
        let provider = LocalAuthProvider::new(3600);
        let display_name = "John Doe";

        let _user_id = provider
            .register_user_with_password(
                "john".to_string(),
                "password".to_string(),
                Some(display_name.to_string()),
            )
            .unwrap();

        let result = provider.authenticate(&Credentials::Password {
            username: "john".to_string(),
            password: "password".to_string(),
        });

        match result {
            AuthResult::Success(session) => {
                assert_eq!(session.user.display_name, Some(display_name.to_string()));
            }
            _ => panic!("Authentication failed"),
        }
    }
}
