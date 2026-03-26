// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Permission Types - RFC-0023 User-Level Permissions
//!
//! Core types for authentication, authorization, and user context.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// User Identity Types
// ============================================================================

/// Unique user identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub String);

impl UserId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn try_parse(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

/// User identity with AGE public key for authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserIdentity {
    pub user_id: UserId,
    pub age_public_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ed25519_public_key: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

impl UserIdentity {
    pub fn new(age_public_key: String) -> Self {
        let now = Utc::now();
        Self {
            user_id: UserId::new(),
            age_public_key,
            ed25519_public_key: None,
            display_name: None,
            email: None,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        }
    }

    pub fn with_ed25519_key(mut self, public_key_hex: String) -> Self {
        self.ed25519_public_key = Some(public_key_hex);
        self
    }

    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }
}

// ============================================================================
// Session Types
// ============================================================================

/// Session token for authenticated user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionToken {
    pub token: String,
    pub user_id: UserId,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub tenant_id: Option<TenantId>,
}

impl SessionToken {
    pub fn new(user_id: UserId, ttl_seconds: i64) -> Self {
        let now = Utc::now();
        Self {
            token: Uuid::new_v4().to_string(),
            user_id,
            issued_at: now,
            expires_at: now + chrono::Duration::seconds(ttl_seconds),
            tenant_id: None,
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    pub fn with_tenant(mut self, tenant_id: TenantId) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }
}

/// Session with full context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub token: SessionToken,
    pub user: UserIdentity,
    pub claims: Claims,
    pub roles: Vec<Role>,
    pub permissions: Vec<Permission>,
}

// ============================================================================
// Claims
// ============================================================================

/// JWT-style claims for authorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,         // Subject (user_id)
    pub iss: Option<String>, // Issuer
    pub aud: Option<String>, // Audience
    pub exp: i64,            // Expiration timestamp
    pub iat: i64,            // Issued at timestamp
    pub nbf: Option<i64>,    // Not before timestamp
    pub custom: HashMap<String, serde_json::Value>,
}

impl Claims {
    pub fn new(user_id: &UserId, ttl_seconds: i64) -> Self {
        let now = Utc::now().timestamp();
        Self {
            sub: user_id.0.clone(),
            iss: None,
            aud: None,
            exp: now + ttl_seconds,
            iat: now,
            nbf: None,
            custom: HashMap::new(),
        }
    }

    pub fn with_claim(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.custom.insert(key.into(), value);
        self
    }
}

// ============================================================================
// Permission Types
// ============================================================================

/// A permission grants access to a specific capability
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Permission {
    pub namespace: String,
    pub resource: String,
    pub action: String,
}

impl Permission {
    pub fn new(
        namespace: impl Into<String>,
        resource: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            namespace: namespace.into(),
            resource: resource.into(),
            action: action.into(),
        }
    }

    pub fn as_str(&self) -> String {
        format!("{}:{}:{}", self.namespace, self.resource, self.action)
    }

    pub fn try_parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 3 {
            Some(Self {
                namespace: parts[0].to_string(),
                resource: parts[1].to_string(),
                action: parts[2].to_string(),
            })
        } else {
            None
        }
    }

    /// Check if this permission matches a pattern (wildcards supported)
    pub fn matches(&self, pattern: &Permission) -> bool {
        fn matches_part(value: &str, pattern: &str) -> bool {
            pattern == "*" || value == pattern
        }

        matches_part(&self.namespace, &pattern.namespace)
            && matches_part(&self.resource, &pattern.resource)
            && matches_part(&self.action, &pattern.action)
    }
}

/// Role containing multiple permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<Permission>,
}

impl Role {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            permissions: Vec::new(),
        }
    }

    pub fn with_permission(mut self, permission: Permission) -> Self {
        self.permissions.push(permission);
        self
    }

    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.iter().any(|p| permission.matches(p))
    }
}

// ============================================================================
// Tenant Types (Multi-tenancy)
// ============================================================================

/// Tenant identifier for multi-tenant isolation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(pub String);

impl TenantId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn try_parse(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl Default for TenantId {
    fn default() -> Self {
        Self::new()
    }
}

/// Tenant configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: TenantId,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub settings: TenantSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantSettings {
    pub max_users: Option<usize>,
    pub session_ttl_seconds: i64,
    pub allowed_auth_providers: Vec<String>,
}

impl Default for TenantSettings {
    fn default() -> Self {
        Self {
            max_users: None,
            session_ttl_seconds: 3600, // 1 hour
            allowed_auth_providers: vec!["local".to_string()],
        }
    }
}

// ============================================================================
// User Context
// ============================================================================

/// User context for plugin call chain propagation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    pub user_id: UserId,
    pub session_id: String,
    pub tenant_id: Option<TenantId>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl UserContext {
    pub fn from_session(session: &Session) -> Self {
        Self {
            user_id: session.user.user_id.clone(),
            session_id: session.token.token.clone(),
            tenant_id: session.token.tenant_id.clone(),
            roles: session.roles.iter().map(|r| r.name.clone()).collect(),
            permissions: session.permissions.iter().map(|p| p.as_str()).collect(),
            metadata: session.user.metadata.clone(),
        }
    }

    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.iter().any(|p| {
            Permission::try_parse(p)
                .map(|parsed| permission.matches(&parsed))
                .unwrap_or(false)
        })
    }
}

// ============================================================================
// Authentication Result
// ============================================================================

/// Result of authentication attempt
#[derive(Debug)]
pub enum AuthResult {
    Success(Box<Session>),
    InvalidCredentials,
    AccountLocked,
    AccountExpired,
    ProviderUnavailable,
    TokenExpired,
    TokenInvalid,
    RateLimited,
}

// ============================================================================
// Built-in Roles
// ============================================================================

/// Create the admin role with full permissions
pub fn admin_role() -> Role {
    Role::new("admin")
        .with_description("Full administrative access")
        .with_permission(Permission::new("*", "*", "*"))
}

/// Create the user role with basic permissions
pub fn user_role() -> Role {
    Role::new("user")
        .with_description("Standard user access")
        .with_permission(Permission::new("self", "*", "read"))
        .with_permission(Permission::new("self", "*", "write"))
}

/// Create the guest role with minimal permissions
pub fn guest_role() -> Role {
    Role::new("guest")
        .with_description("Guest access")
        .with_permission(Permission::new("public", "*", "read"))
}

impl Role {
    fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}
