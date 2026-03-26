// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Version-Based Hot Reload - RFC-0007 Extension
//!
//! This module implements version-based hot reload detection:
//! - Reload if plugin version number increases
//! - Reload if version ends with "-dev" (development builds)
//!
//! This replaces the file-watcher based approach with deterministic version checking.
//!
//! ## Safety
//!
//! This module uses file modification time checking instead of loading the
//! plugin library to check for version changes. This avoids:
//!
//! 1. **Segmentation faults** from loading a library while it's in use
//! 2. **Memory corruption** from multiple library mappings
//! 3. **Race conditions** between hot-reload and version checking
//!
//! The approach:
//! - Store file modification time when plugin is first loaded
//! - Compare modification time to detect changes
//! - Only load library when actually performing hot-reload
//! - Use proper synchronization with the plugin manager for safe unloading

#![allow(dead_code)]

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use skylet_abi::AbiV2PluginLoader;

/// Stored version info for a loaded plugin
#[derive(Debug, Clone)]
pub struct PluginVersionInfo {
    pub plugin_id: String,
    pub version: String,
    pub path: PathBuf,
    pub modified_time: std::time::SystemTime,
    /// Inode number at load time - used to detect file replacement (cp/mv)
    /// vs in-place modification. When a file is replaced, its inode changes,
    /// and loading the new .so while the old one is mapped causes segfaults.
    #[cfg(unix)]
    pub inode: u64,
}

/// Result of a version check for a single plugin
#[derive(Debug, Clone)]
pub struct VersionCheckResult {
    pub plugin_id: String,
    pub current_version: String,
    pub new_version: String,
    pub needs_reload: bool,
    pub reason: ReloadReason,
}

/// Reason why a plugin needs reload
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadReason {
    VersionIncreased,
    DevBuild,
    VersionChanged,
    FileModified,
}

impl std::fmt::Display for ReloadReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReloadReason::VersionIncreased => write!(f, "version increased"),
            ReloadReason::DevBuild => write!(f, "development build"),
            ReloadReason::VersionChanged => write!(f, "version changed"),
            ReloadReason::FileModified => write!(f, "file modified"),
        }
    }
}

/// Version-based hot reload manager
pub struct VersionBasedHotReload {
    /// Map of plugin_id -> version info for loaded plugins
    loaded_versions: Arc<RwLock<HashMap<String, PluginVersionInfo>>>,
}

impl Default for VersionBasedHotReload {
    fn default() -> Self {
        Self::new()
    }
}

impl VersionBasedHotReload {
    pub fn new() -> Self {
        Self {
            loaded_versions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a plugin's version when it's loaded
    pub async fn register_plugin(&self, plugin_id: &str, version: &str, path: PathBuf) {
        let metadata = std::fs::metadata(&path).ok();
        let modified_time = metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        #[cfg(unix)]
        let inode = metadata
            .as_ref()
            .map(|m| std::os::unix::fs::MetadataExt::ino(m))
            .unwrap_or(0);

        let mut versions = self.loaded_versions.write().await;
        versions.insert(
            plugin_id.to_string(),
            PluginVersionInfo {
                plugin_id: plugin_id.to_string(),
                version: version.to_string(),
                path,
                modified_time,
                #[cfg(unix)]
                inode,
            },
        );
        debug!("Registered plugin '{}' with version '{}' for version-based-reload (mtime: {:?})", plugin_id, version, modified_time);
    }

    /// Unregister a plugin when it's unloaded
    pub async fn unregister_plugin(&self, plugin_id: &str) {
        let mut versions = self.loaded_versions.write().await;
        versions.remove(plugin_id);
        debug!("Unregistered plugin '{}' from version-based-reload", plugin_id);
    }

    /// Check all loaded plugins for version changes
    ///
    /// Returns a list of plugins that need to be reloaded.
    /// Uses file modification time instead of loading the library
    /// to avoid segfaults during hot-reload.
    pub async fn check_versions(&self) -> Vec<VersionCheckResult> {
        let versions = self.loaded_versions.read().await;
        let mut results = Vec::new();

        for (_, info) in versions.iter() {
            match self.check_single_plugin(info).await {
                Ok(Some(result)) => {
                    if result.needs_reload {
                        info!(
                            "Plugin '{}' needs reload: {} -> {} ({})",
                            result.plugin_id,
                            result.current_version,
                            result.new_version,
                            result.reason
                        );
                    }
                    results.push(result);
                }
                Ok(None) => {}
                Err(e) => {
                    warn!("Failed to check version for plugin '{}': {}", info.plugin_id, e);
                }
            }
        }

        results.into_iter().filter(|r| r.needs_reload).collect()
    }

    /// Check a single plugin for version changes using file mtime
    async fn check_single_plugin(&self, info: &PluginVersionInfo) -> Result<Option<VersionCheckResult>> {
        if !info.path.exists() {
            debug!("Plugin path no longer exists: {:?}", info.path);
            return Ok(None);
        }

        // Check file modification time first (fast, no library load)
        let fs_metadata = std::fs::metadata(&info.path)
            .map_err(|e| anyhow!("Failed to get file metadata for plugin '{}': {}", info.plugin_id, e))?;
        
        if let Ok(modified) = fs_metadata.modified() {
            if modified <= info.modified_time {
                // File not modified, skip version check
                debug!(
                    "Plugin '{}' file not modified: {:?} <= {:?}",
                    info.plugin_id,
                    modified,
                    info.modified_time
                );
                return Ok(None);
            }
        }

        // SAFETY: Check if the file was replaced (new inode) vs modified in-place.
        // When a .so file is replaced via cp/mv, the inode changes. Loading the new
        // .so while the old one is still mapped causes segfaults due to conflicting
        // symbol tables and static initializers. In this case, we report that a
        // restart is needed instead of attempting an unsafe hot reload.
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let new_inode = fs_metadata.ino();
            if new_inode != info.inode {
                warn!(
                    "Plugin '{}' file was replaced (inode changed: {} -> {}). \
                     Hot reload skipped - server restart required for safety.",
                    info.plugin_id, info.inode, new_inode
                );
                return Ok(Some(VersionCheckResult {
                    plugin_id: info.plugin_id.clone(),
                    current_version: info.version.clone(),
                    new_version: format!("{} (restart required)", info.version),
                    needs_reload: false, // Don't auto-reload replaced files
                    reason: ReloadReason::FileModified,
                }));
            }
        }

        debug!(
            "Plugin '{}' file modified: {:?} > {:?}",
            info.plugin_id,
            fs_metadata.modified().ok(),
            Some(info.modified_time)
        );

        // File modified in-place (same inode), safe to load and check version
        let loader = AbiV2PluginLoader::load(&info.path)
            .map_err(|e| anyhow!("Failed to load plugin for version check: {}", e))?;

        let metadata = loader
            .get_info()
            .map_err(|e| anyhow!("Failed to get plugin info for version check: {}", e))?;

        let new_version = metadata.version;
        let current_version = &info.version;

        if new_version == *current_version {
            debug!(
                "Plugin '{}' version unchanged: {}",
                info.plugin_id, current_version
            );
            return Ok(None);
        }

        let (needs_reload, reason) = self.should_reload(current_version, &new_version);

        debug!(
            "Plugin '{}' version changed: {} -> {} (needs_reload: {}, reason: {:?})",
            info.plugin_id, current_version, new_version, needs_reload, reason
        );

        Ok(Some(VersionCheckResult {
            plugin_id: info.plugin_id.clone(),
            current_version: current_version.clone(),
            new_version: new_version.clone(),
            needs_reload,
            reason,
        }))
    }

    /// Determine if a plugin should be reloaded based on version change
    ///
    /// Rules:
    /// 1. If new version ends with "-dev", always reload (development builds)
    /// 2. If new version > current version, reload (upgrade)
    /// 3. If new version < current version, reload (downgrade - version changed)
    fn should_reload(&self, current: &str, new: &str) -> (bool, ReloadReason) {
        if new.ends_with("-dev") {
            return (true, ReloadReason::DevBuild);
        }

        let current_semver = parse_semver(current);
        let new_semver = parse_semver(new);

        match (current_semver, new_semver) {
            (Some(current_v), Some(new_v)) => {
                if new_v > current_v {
                    (true, ReloadReason::VersionIncreased)
                } else if new_v < current_v {
                    (true, ReloadReason::VersionChanged)
                } else {
                    (false, ReloadReason::VersionChanged)
                }
            }
            _ => {
                if current != new {
                    (true, ReloadReason::VersionChanged)
                } else {
                    (false, ReloadReason::VersionChanged)
                }
            }
        }
    }

    /// Get version info for a specific plugin
    pub async fn get_version_info(&self, plugin_id: &str) -> Option<PluginVersionInfo> {
        let versions = self.loaded_versions.read().await;
        versions.get(plugin_id).cloned()
    }

    /// Update the stored version for a plugin (after successful reload)
    pub async fn update_version(&self, plugin_id: &str, new_version: &str) {
        let mut versions = self.loaded_versions.write().await;
        if let Some(info) = versions.get_mut(plugin_id) {
            info.version = new_version.to_string();
            debug!(
                "Updated plugin '{}' version to '{}'",
                plugin_id, new_version
            );
        }
    }

    /// Update stored modified time after successful reload
    pub async fn update_modified_time(&self, plugin_id: &str) {
        let mut versions = self.loaded_versions.write().await;
        if let Some(info) = versions.get_mut(plugin_id) {
            if let Ok(fs_metadata) = std::fs::metadata(&info.path) {
                if let Ok(modified) = fs_metadata.modified() {
                    info.modified_time = modified;
                    debug!(
                        "Updated plugin '{}' modified time to {:?}",
                        plugin_id, modified
                    );
                }
            }
        }
    }

    /// List all registered plugins
    pub async fn list_registered(&self) -> Vec<(String, String, PathBuf)> {
        let versions = self.loaded_versions.read().await;
        versions
            .values()
            .map(|info| (info.plugin_id.clone(), info.version.clone(), info.path.clone()))
            .collect()
    }
}

/// Simple semver representation for version comparison
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SemVer {
    major: u64,
    minor: u64,
    patch: u64,
}

/// Parse a semver string into a SemVer struct
fn parse_semver(version: &str) -> Option<SemVer> {
    let version = version.trim();

    let version_part = if let Some(idx) = version.find('-') {
        &version[..idx]
    } else {
        version
    };

    let parts: Vec<&str> = version_part.split('.').collect();
    if parts.len() < 2 {
        return None;
    }

    let major = parts.get(0)?.parse().ok()?;
    let minor = parts.get(1)?.parse().ok()?;
    let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

    Some(SemVer {
        major,
        minor,
        patch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_parse_semver() {
        let v = parse_semver("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);

        let v = parse_semver("0.1.0").unwrap();
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 1);
        assert_eq!(v.patch, 0);

        let v = parse_semver("1.0").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);

        let v = parse_semver("2.0.0-dev").unwrap();
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_semver_comparison() {
        let v1 = parse_semver("1.0.0").unwrap();
        let v2 = parse_semver("1.0.1").unwrap();
        let v3 = parse_semver("1.1.0").unwrap();
        let v4 = parse_semver("2.0.0").unwrap();

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v3 < v4);
        assert!(v1 < v4);
    }

    #[test]
    fn test_should_reload_dev_build() {
        let reload = VersionBasedHotReload::new();

        let (needs_reload, reason) = reload.should_reload("1.0.0", "1.0.0-dev");
        assert!(needs_reload);
        assert_eq!(reason, ReloadReason::DevBuild);

        let (needs_reload, reason) = reload.should_reload("1.0.0-dev", "1.0.0-dev");
        assert!(needs_reload);
        assert_eq!(reason, ReloadReason::DevBuild);
    }

    #[test]
    fn test_should_reload_version_increased() {
        let reload = VersionBasedHotReload::new();

        let (needs_reload, reason) = reload.should_reload("1.0.0", "1.0.1");
        assert!(needs_reload);
        assert_eq!(reason, ReloadReason::VersionIncreased);

        let (needs_reload, reason) = reload.should_reload("1.0.0", "1.1.0");
        assert!(needs_reload);
        assert_eq!(reason, ReloadReason::VersionIncreased);

        let (needs_reload, reason) = reload.should_reload("1.0.0", "2.0.0");
        assert!(needs_reload);
        assert_eq!(reason, ReloadReason::VersionIncreased);
    }

    #[test]
    fn test_should_reload_version_decreased() {
        let reload = VersionBasedHotReload::new();

        let (needs_reload, reason) = reload.should_reload("2.0.0", "1.0.0");
        assert!(needs_reload);
        assert_eq!(reason, ReloadReason::VersionChanged);
    }

    #[test]
    fn test_should_reload_same_version() {
        let reload = VersionBasedHotReload::new();

        let (needs_reload, _) = reload.should_reload("1.0.0", "1.0.0");
        assert!(!needs_reload);
    }

    #[tokio::test]
    async fn test_register_and_unregister() {
        let reload = VersionBasedHotReload::new();
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("test.so");
        std::fs::write(&path, b"test").unwrap();

        reload
            .register_plugin("test-plugin", "1.0.0", path.clone())
            .await;

        let info = reload.get_version_info("test-plugin").await;
        assert!(info.is_some());
        assert_eq!(info.unwrap().version, "1.0.0");

        reload.unregister_plugin("test-plugin").await;

        let info = reload.get_version_info("test-plugin").await;
        assert!(info.is_none());
    }

    #[tokio::test]
    async fn test_list_registered() {
        let reload = VersionBasedHotReload::new();
        let temp_dir = tempdir().unwrap();
        let path_a = temp_dir.path().join("a.so");
        let path_b = temp_dir.path().join("b.so");
        std::fs::write(&path_a, b"test").unwrap();
        std::fs::write(&path_b, b"test").unwrap();

        reload
            .register_plugin("plugin-a", "1.0.0", path_a)
            .await;
        reload
            .register_plugin("plugin-b", "2.0.0", path_b)
            .await;

        let list = reload.list_registered().await;
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_update_version() {
        let reload = VersionBasedHotReload::new();
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("test.so");
        std::fs::write(&path, b"test").unwrap();

        reload
            .register_plugin("test-plugin", "1.0.0", path)
            .await;

        reload.update_version("test-plugin", "2.0.0").await;

        let info = reload.get_version_info("test-plugin").await;
        assert_eq!(info.unwrap().version, "2.0.0");
    }
}
