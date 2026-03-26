// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Authorization Layer - RFC-0023
//!
//! Permission checking, caching, and RBAC implementation.

use anyhow::Result;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::types::*;

// ============================================================================
// Permission Checker
// ============================================================================

/// Core permission checker
pub struct PermissionChecker {
    /// Role store
    roles: RwLock<HashMap<String, Role>>,
    /// User role assignments
    user_roles: RwLock<HashMap<String, Vec<String>>>,
    /// Permission cache
    cache: RwLock<PermissionCache>,
}

impl PermissionChecker {
    pub fn new() -> Self {
        let mut roles = HashMap::new();

        // Register built-in roles
        roles.insert("admin".to_string(), admin_role());
        roles.insert("user".to_string(), user_role());
        roles.insert("guest".to_string(), guest_role());

        Self {
            roles: RwLock::new(roles),
            user_roles: RwLock::new(HashMap::new()),
            cache: RwLock::new(PermissionCache::new(Duration::from_secs(300))),
        }
    }

    /// Register a custom role
    pub fn register_role(&self, role: Role) {
        let mut roles = self.roles.write();
        roles.insert(role.name.clone(), role);
    }

    /// Assign roles to a user
    pub fn assign_roles(&self, user_id: &UserId, role_names: Vec<String>) {
        let mut user_roles = self.user_roles.write();
        user_roles.insert(user_id.0.clone(), role_names);

        // Invalidate cache for this user
        let mut cache = self.cache.write();
        cache.invalidate_user(&user_id.0);
    }

    /// Get roles for a user
    pub fn get_user_roles(&self, user_id: &UserId) -> Vec<Role> {
        let user_roles = self.user_roles.read();
        let role_names = user_roles.get(&user_id.0).cloned().unwrap_or_default();
        drop(user_roles);

        let roles = self.roles.read();
        role_names
            .iter()
            .filter_map(|name| roles.get(name).cloned())
            .collect()
    }

    /// Check if user has permission
    pub fn has_permission(&self, user_id: &UserId, permission: &Permission) -> bool {
        // Check cache first
        {
            let cache = self.cache.read();
            if let Some(cached) = cache.get(&user_id.0, permission) {
                return cached;
            }
        }

        // Compute permission
        let has_access = self.compute_permission(user_id, permission);

        // Cache result
        {
            let mut cache = self.cache.write();
            cache.set(&user_id.0, permission.clone(), has_access);
        }

        has_access
    }

    /// Compute permission without cache
    fn compute_permission(&self, user_id: &UserId, permission: &Permission) -> bool {
        let roles = self.get_user_roles(user_id);
        roles.iter().any(|role| role.has_permission(permission))
    }

    /// Check if user has any of the specified permissions
    pub fn has_any_permission(&self, user_id: &UserId, permissions: &[Permission]) -> bool {
        permissions.iter().any(|p| self.has_permission(user_id, p))
    }

    /// Check if user has all of the specified permissions
    pub fn has_all_permissions(&self, user_id: &UserId, permissions: &[Permission]) -> bool {
        permissions.iter().all(|p| self.has_permission(user_id, p))
    }

    /// Invalidate all cached permissions for a user
    pub fn invalidate_cache(&self, user_id: &UserId) {
        let mut cache = self.cache.write();
        cache.invalidate_user(&user_id.0);
    }

    /// Invalidate all cached permissions
    pub fn invalidate_all(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }
}

impl Default for PermissionChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Permission Cache
// ============================================================================

/// Cache entry for permission checks
#[derive(Debug, Clone)]
struct CacheEntry {
    granted: bool,
    cached_at: Instant,
}

/// Permission cache with TTL
struct PermissionCache {
    entries: HashMap<String, HashMap<String, CacheEntry>>,
    ttl: Duration,
}

impl PermissionCache {
    fn new(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
        }
    }

    fn get(&self, user_id: &str, permission: &Permission) -> Option<bool> {
        let user_cache = self.entries.get(user_id)?;
        let entry = user_cache.get(&permission.as_str())?;

        if entry.cached_at.elapsed() > self.ttl {
            None
        } else {
            Some(entry.granted)
        }
    }

    fn set(&mut self, user_id: &str, permission: Permission, granted: bool) {
        let user_cache = self.entries.entry(user_id.to_string()).or_default();
        user_cache.insert(
            permission.as_str(),
            CacheEntry {
                granted,
                cached_at: Instant::now(),
            },
        );
    }

    fn invalidate_user(&mut self, user_id: &str) {
        self.entries.remove(user_id);
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

// ============================================================================
// Authorization Middleware
// ============================================================================

/// Authorization middleware for checking permissions
pub struct AuthorizationMiddleware {
    checker: Arc<PermissionChecker>,
}

impl AuthorizationMiddleware {
    pub fn new(checker: Arc<PermissionChecker>) -> Self {
        Self { checker }
    }

    /// Check if request is authorized
    pub fn authorize(
        &self,
        context: &UserContext,
        required_permission: &Permission,
    ) -> Result<(), AuthzError> {
        if self
            .checker
            .has_permission(&context.user_id, required_permission)
        {
            Ok(())
        } else {
            Err(AuthzError::PermissionDenied {
                user_id: context.user_id.0.clone(),
                permission: required_permission.as_str(),
            })
        }
    }

    /// Check if request is authorized for any of the permissions
    pub fn authorize_any(
        &self,
        context: &UserContext,
        permissions: &[Permission],
    ) -> Result<(), AuthzError> {
        if self
            .checker
            .has_any_permission(&context.user_id, permissions)
        {
            Ok(())
        } else {
            Err(AuthzError::PermissionDenied {
                user_id: context.user_id.0.clone(),
                permission: permissions
                    .iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            })
        }
    }
}

// ============================================================================
// Authorization Error
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum AuthzError {
    #[error("Permission denied for user {user_id}: {permission}")]
    PermissionDenied { user_id: String, permission: String },

    #[error("Invalid user context")]
    InvalidContext,

    #[error("Session expired")]
    SessionExpired,

    #[error("Tenant mismatch")]
    TenantMismatch,
}

// ============================================================================
// Resource-Based Access Control (ReBAC)
// ============================================================================

/// Resource-based permission check
#[derive(Debug, Clone)]
pub struct ResourcePermission {
    pub resource_type: String,
    pub resource_id: String,
    pub action: String,
}

impl ResourcePermission {
    pub fn new(
        resource_type: impl Into<String>,
        resource_id: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            resource_type: resource_type.into(),
            resource_id: resource_id.into(),
            action: action.into(),
        }
    }

    pub fn as_permission(&self) -> Permission {
        Permission::new(&self.resource_type, &self.resource_id, &self.action)
    }
}

/// Resource-based access control checker
pub struct ResourceAccessControl {
    /// Direct resource permissions (user_id -> resource -> actions)
    permissions: RwLock<HashMap<String, HashMap<String, Vec<String>>>>,
}

impl ResourceAccessControl {
    pub fn new() -> Self {
        Self {
            permissions: RwLock::new(HashMap::new()),
        }
    }

    /// Grant permission on a resource to a user
    pub fn grant(&self, user_id: &UserId, resource: &ResourcePermission) {
        let mut perms = self.permissions.write();
        let user_perms = perms.entry(user_id.0.clone()).or_default();
        let resource_key = format!("{}:{}", resource.resource_type, resource.resource_id);
        let actions = user_perms.entry(resource_key).or_default();

        if !actions.contains(&resource.action) {
            actions.push(resource.action.clone());
        }
    }

    /// Revoke permission on a resource from a user
    pub fn revoke(&self, user_id: &UserId, resource: &ResourcePermission) {
        let mut perms = self.permissions.write();
        if let Some(user_perms) = perms.get_mut(&user_id.0) {
            let resource_key = format!("{}:{}", resource.resource_type, resource.resource_id);
            if let Some(actions) = user_perms.get_mut(&resource_key) {
                actions.retain(|a| a != &resource.action);
            }
        }
    }

    /// Check if user has permission on a resource
    pub fn check(&self, user_id: &UserId, resource: &ResourcePermission) -> bool {
        let perms = self.permissions.read();

        if let Some(user_perms) = perms.get(&user_id.0) {
            let resource_key = format!("{}:{}", resource.resource_type, resource.resource_id);
            if let Some(actions) = user_perms.get(&resource_key) {
                return actions.contains(&resource.action) || actions.contains(&"*".to_string());
            }
        }

        false
    }
}

impl Default for ResourceAccessControl {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Audit Logging for Auth/Authz
// ============================================================================

/// Audit event for authorization decisions
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthzAuditEvent {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub event_type: AuthzEventType,
    pub user_id: String,
    pub permission: Option<String>,
    pub resource: Option<String>,
    pub granted: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum AuthzEventType {
    PermissionCheck,
    RoleAssignment,
    RoleRemoval,
    ResourceGrant,
    ResourceRevoke,
}

/// Audit logger for authorization events
pub struct AuthzAuditLog {
    events: RwLock<Vec<AuthzAuditEvent>>,
    max_events: usize,
}

impl AuthzAuditLog {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: RwLock::new(Vec::new()),
            max_events,
        }
    }

    pub fn log(&self, event: AuthzAuditEvent) {
        let mut events = self.events.write();

        // Trim if over limit
        if events.len() >= self.max_events {
            events.remove(0);
        }

        events.push(event);
    }

    pub fn get_events(&self) -> Vec<AuthzAuditEvent> {
        self.events.read().clone()
    }

    pub fn get_events_for_user(&self, user_id: &UserId) -> Vec<AuthzAuditEvent> {
        self.events
            .read()
            .iter()
            .filter(|e| e.user_id == user_id.0)
            .cloned()
            .collect()
    }
}

impl Default for AuthzAuditLog {
    fn default() -> Self {
        Self::new(10000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_checker() {
        let checker = PermissionChecker::new();

        let user_id = UserId::new();
        checker.assign_roles(&user_id, vec!["admin".to_string()]);

        // Admin should have all permissions
        let perm = Permission::new("system", "config", "write");
        assert!(checker.has_permission(&user_id, &perm));

        // Guest should not have admin permissions
        let guest_id = UserId::new();
        checker.assign_roles(&guest_id, vec!["guest".to_string()]);
        assert!(!checker.has_permission(&guest_id, &perm));
    }

    #[test]
    fn test_permission_cache() {
        let checker = PermissionChecker::new();

        let user_id = UserId::new();
        checker.assign_roles(&user_id, vec!["user".to_string()]);

        let perm = Permission::new("self", "profile", "read");

        // First check (not cached)
        assert!(checker.has_permission(&user_id, &perm));

        // Second check (should be cached)
        assert!(checker.has_permission(&user_id, &perm));

        // Invalidate and recheck
        checker.invalidate_cache(&user_id);
        assert!(checker.has_permission(&user_id, &perm));
    }

    #[test]
    fn test_resource_access_control() {
        let rac = ResourceAccessControl::new();

        let user_id = UserId::new();
        let resource = ResourcePermission::new("document", "doc-123", "read");

        // Initially no access
        assert!(!rac.check(&user_id, &resource));

        // Grant access
        rac.grant(&user_id, &resource);
        assert!(rac.check(&user_id, &resource));

        // Revoke access
        rac.revoke(&user_id, &resource);
        assert!(!rac.check(&user_id, &resource));
    }

    #[test]
    fn test_authorization_middleware() {
        let checker = Arc::new(PermissionChecker::new());
        let middleware = AuthorizationMiddleware::new(checker.clone());

        let user_id = UserId::new();
        checker.assign_roles(&user_id, vec!["user".to_string()]);

        let context = UserContext {
            user_id: user_id.clone(),
            session_id: "test-session".to_string(),
            tenant_id: None,
            roles: vec!["user".to_string()],
            permissions: vec!["self:*:read".to_string()],
            metadata: HashMap::new(),
        };

        // Should allow self read
        let perm = Permission::new("self", "profile", "read");
        assert!(middleware.authorize(&context, &perm).is_ok());

        // Should deny system write
        let perm = Permission::new("system", "config", "write");
        assert!(middleware.authorize(&context, &perm).is_err());
    }
}
