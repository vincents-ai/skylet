// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Filesystem Permission Enforcement - RFC-0008
//!
//! Enforces filesystem access permissions for plugins based on declared
//! capabilities and approved paths.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use super::capabilities::{CapabilityStatus, FilesystemAccessMode};

/// A single path permission entry
#[derive(Debug, Clone)]
pub struct PathPermission {
    /// The path this permission applies to (can include wildcards)
    pub path: String,
    /// Access mode granted
    pub mode: FilesystemAccessMode,
    /// Status of this permission
    pub status: CapabilityStatus,
    /// Source of this permission (plugin ID or "system")
    pub source: String,
}

/// Filesystem permission enforcer
///
/// Maintains a registry of approved path permissions per plugin
/// and validates access requests against them.
#[derive(Debug)]
pub struct FilesystemEnforcer {
    /// Map of plugin_id -> list of approved path permissions
    permissions: Arc<RwLock<HashMap<String, Vec<PathPermission>>>>,
    /// Global read-only paths available to all plugins
    global_read_paths: Vec<String>,
    /// VFS mount points
    vfs_mounts: HashMap<String, PathBuf>,
}

impl FilesystemEnforcer {
    /// Create a new filesystem enforcer
    pub fn new() -> Self {
        let mut vfs_mounts = HashMap::new();

        // Standard VFS mount points
        vfs_mounts.insert("plugin-data".to_string(), PathBuf::from("./data/plugins"));
        vfs_mounts.insert("shared-data".to_string(), PathBuf::from("./data/shared"));
        vfs_mounts.insert("config".to_string(), PathBuf::from("./config"));
        vfs_mounts.insert("cache".to_string(), PathBuf::from("./cache"));
        vfs_mounts.insert("logs".to_string(), PathBuf::from("./logs"));
        vfs_mounts.insert("temp".to_string(), PathBuf::from("./tmp"));

        Self {
            permissions: Arc::new(RwLock::new(HashMap::new())),
            global_read_paths: vec![
                "/etc/ssl/certs".to_string(),      // SSL certificates
                "/usr/share/zoneinfo".to_string(), // Timezone data
            ],
            vfs_mounts,
        }
    }

    /// Register path permissions for a plugin
    pub fn register_permissions(&self, plugin_id: &str, permissions: Vec<PathPermission>) {
        let mut perms = self.permissions.write().unwrap();
        perms.insert(plugin_id.to_string(), permissions);
    }

    /// Add a single path permission for a plugin
    pub fn add_permission(&self, plugin_id: &str, permission: PathPermission) {
        let mut perms = self.permissions.write().unwrap();
        perms
            .entry(plugin_id.to_string())
            .or_default()
            .push(permission);
    }

    /// Remove all permissions for a plugin
    pub fn remove_plugin(&self, plugin_id: &str) {
        let mut perms = self.permissions.write().unwrap();
        perms.remove(plugin_id);
    }

    /// Check if a plugin can access a path
    pub fn check_access(
        &self,
        plugin_id: &str,
        path: &str,
        requested_mode: FilesystemAccessMode,
    ) -> Result<(), FilesystemAccessError> {
        // First, resolve VFS URI to actual path if needed
        let resolved_path = self.resolve_vfs_path(path);

        // Check global read-only paths
        if requested_mode == FilesystemAccessMode::Read {
            for global_path in &self.global_read_paths {
                if resolved_path.starts_with(global_path) {
                    return Ok(());
                }
            }
        }

        // Check plugin-specific permissions
        let perms = self.permissions.read().unwrap();

        if let Some(plugin_perms) = perms.get(plugin_id) {
            for perm in plugin_perms {
                // Skip non-approved permissions
                if perm.status != CapabilityStatus::Approved
                    && perm.status != CapabilityStatus::AutoApproved
                {
                    continue;
                }

                // Check if path matches (resolve both the request path and the permission path)
                let resolved_perm_path = self.resolve_vfs_path(&perm.path);
                if self.path_matches(&resolved_path, &resolved_perm_path) {
                    // Check if mode is compatible
                    if self.mode_compatible(perm.mode, requested_mode) {
                        return Ok(());
                    }
                }
            }
        }

        Err(FilesystemAccessError::PermissionDenied {
            plugin_id: plugin_id.to_string(),
            path: path.to_string(),
            mode: requested_mode,
        })
    }

    /// Resolve a VFS URI to a filesystem path
    pub fn resolve_vfs_path(&self, uri: &str) -> String {
        // Check if it's a VFS URI (scheme://path format)
        if let Some(colon_pos) = uri.find("://") {
            let scheme = &uri[..colon_pos];
            let path = &uri[colon_pos + 3..];

            if let Some(mount_point) = self.vfs_mounts.get(scheme) {
                return mount_point.join(path).to_string_lossy().to_string();
            }
        }

        // Return as-is if not a VFS URI
        uri.to_string()
    }

    /// Check if a path matches a pattern (supports * wildcard and directory prefixes)
    fn path_matches(&self, path: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if let Some(prefix) = pattern.strip_suffix("/*") {
            return path.starts_with(prefix);
        }

        if pattern.ends_with('/') {
            return path.starts_with(pattern);
        }

        // Exact match
        if path == pattern {
            return true;
        }

        // Directory-like match: pattern "foo/bar" also matches "foo/bar/baz"
        if path.starts_with(pattern) && path[pattern.len()..].starts_with('/') {
            return true;
        }

        false
    }

    /// Check if a granted mode satisfies a requested mode
    fn mode_compatible(
        &self,
        granted: FilesystemAccessMode,
        requested: FilesystemAccessMode,
    ) -> bool {
        match (granted, requested) {
            // ReadWrite grants everything
            (FilesystemAccessMode::ReadWrite, _) => true,
            // Read only grants read
            (FilesystemAccessMode::Read, FilesystemAccessMode::Read) => true,
            // WriteOnly grants write and append
            (FilesystemAccessMode::WriteOnly, FilesystemAccessMode::WriteOnly) => true,
            (FilesystemAccessMode::WriteOnly, FilesystemAccessMode::Append) => true,
            // Append only grants append
            (FilesystemAccessMode::Append, FilesystemAccessMode::Append) => true,
            _ => false,
        }
    }

    /// Get all permissions for a plugin
    pub fn get_permissions(&self, plugin_id: &str) -> Option<Vec<PathPermission>> {
        let perms = self.permissions.read().unwrap();
        perms.get(plugin_id).cloned()
    }

    /// Add a VFS mount point
    pub fn add_vfs_mount(&mut self, scheme: &str, path: PathBuf) {
        self.vfs_mounts.insert(scheme.to_string(), path);
    }

    /// List all VFS mount points
    pub fn list_vfs_mounts(&self) -> &HashMap<String, PathBuf> {
        &self.vfs_mounts
    }
}

impl Default for FilesystemEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

/// Filesystem access errors
#[derive(Debug, Clone)]
pub enum FilesystemAccessError {
    /// Permission denied for plugin
    PermissionDenied {
        plugin_id: String,
        path: String,
        mode: FilesystemAccessMode,
    },
    /// Path not found
    PathNotFound(String),
    /// Invalid path
    InvalidPath(String),
}

impl fmt::Display for FilesystemAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilesystemAccessError::PermissionDenied {
                plugin_id,
                path,
                mode,
            } => {
                write!(
                    f,
                    "Permission denied for plugin {} to {:?} {}",
                    plugin_id, mode, path
                )
            }
            FilesystemAccessError::PathNotFound(path) => write!(f, "Path not found: {}", path),
            FilesystemAccessError::InvalidPath(path) => write!(f, "Invalid path: {}", path),
        }
    }
}

impl std::error::Error for FilesystemAccessError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filesystem_enforcer_new() {
        let enforcer = FilesystemEnforcer::new();
        assert!(enforcer.vfs_mounts.contains_key("plugin-data"));
        assert!(enforcer.vfs_mounts.contains_key("config"));
    }

    #[test]
    fn test_resolve_vfs_path() {
        let enforcer = FilesystemEnforcer::new();

        let resolved = enforcer.resolve_vfs_path("plugin-data://cache/file.db");
        assert!(resolved.contains("data/plugins"));
        assert!(resolved.contains("cache/file.db"));

        // Non-VFS path should pass through
        let resolved = enforcer.resolve_vfs_path("/etc/passwd");
        assert_eq!(resolved, "/etc/passwd");
    }

    #[test]
    fn test_path_matches() {
        let enforcer = FilesystemEnforcer::new();

        assert!(enforcer.path_matches("/data/plugins/test/file.txt", "/data/plugins/test/file.txt"));
        assert!(enforcer.path_matches("/data/plugins/test/file.txt", "/data/plugins/test/"));
        assert!(enforcer.path_matches("/data/plugins/test/file.txt", "/data/plugins/*"));
        assert!(enforcer.path_matches("/any/path", "*"));
        assert!(!enforcer.path_matches("/data/plugins/test/file.txt", "/data/other/"));
    }

    #[test]
    fn test_mode_compatible() {
        let enforcer = FilesystemEnforcer::new();

        // ReadWrite grants all
        assert!(
            enforcer.mode_compatible(FilesystemAccessMode::ReadWrite, FilesystemAccessMode::Read)
        );
        assert!(enforcer.mode_compatible(
            FilesystemAccessMode::ReadWrite,
            FilesystemAccessMode::ReadWrite
        ));
        assert!(enforcer.mode_compatible(
            FilesystemAccessMode::ReadWrite,
            FilesystemAccessMode::WriteOnly
        ));

        // Read only grants read
        assert!(enforcer.mode_compatible(FilesystemAccessMode::Read, FilesystemAccessMode::Read));
        assert!(
            !enforcer.mode_compatible(FilesystemAccessMode::Read, FilesystemAccessMode::WriteOnly)
        );
    }

    #[test]
    fn test_check_access_granted() {
        let enforcer = FilesystemEnforcer::new();

        enforcer.add_permission(
            "test-plugin",
            PathPermission {
                path: "plugin-data://cache".to_string(),
                mode: FilesystemAccessMode::ReadWrite,
                status: CapabilityStatus::Approved,
                source: "admin".to_string(),
            },
        );

        // Should grant access to approved path
        assert!(enforcer
            .check_access(
                "test-plugin",
                "plugin-data://cache/file.txt",
                FilesystemAccessMode::Read
            )
            .is_ok());
        assert!(enforcer
            .check_access(
                "test-plugin",
                "plugin-data://cache/file.txt",
                FilesystemAccessMode::WriteOnly
            )
            .is_ok());
    }

    #[test]
    fn test_check_access_denied() {
        let enforcer = FilesystemEnforcer::new();

        // No permissions registered - should deny
        let result =
            enforcer.check_access("unknown-plugin", "/etc/shadow", FilesystemAccessMode::Read);
        assert!(result.is_err());
    }
}
