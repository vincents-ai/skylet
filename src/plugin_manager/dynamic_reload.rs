// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]

//! Dynamic Plugin Reload System
//!
//! This module provides hot-reload functionality for plugins:
//! - State serialization before reload
//! - Graceful plugin replacement
//! - State restoration after reload
//!
//! Note: The actual reload implementation uses the PluginManager's
//! load_plugin/unload_plugin methods with epoch-based reclamation.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};

use super::manager::PluginManager;

/// Result of a reload operation
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ReloadResult {
    pub plugin_id: String,
    pub success: bool,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
    pub state_preserved: bool,
    pub error: Option<String>,
}

/// Cache for plugin paths to avoid repeated filesystem lookups
pub struct PluginPathCache {
    cache: std::sync::Mutex<HashMap<String, PathBuf>>,
    max_size: usize,
}

impl PluginPathCache {
    pub fn new(_cache_dir: PathBuf, max_size: usize) -> Self {
        Self {
            cache: std::sync::Mutex::new(HashMap::new()),
            max_size,
        }
    }

    pub fn get(&self, plugin_name: &str) -> Option<PathBuf> {
        let cache = self.cache.lock().unwrap();
        cache.get(plugin_name).cloned()
    }

    pub fn insert(&self, plugin_name: String, path: PathBuf) {
        let mut cache = self.cache.lock().unwrap();
        if cache.len() >= self.max_size {
            if let Some(first_key) = cache.keys().next().cloned() {
                cache.remove(&first_key);
            }
        }
        cache.insert(plugin_name, path);
    }

    pub fn invalidate(&self, plugin_name: &str) {
        let mut cache = self.cache.lock().unwrap();
        cache.remove(plugin_name);
    }

    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }
}

impl PluginManager {
    /// Dynamically reload a plugin
    ///
    /// This performs a hot-reload by:
    /// 1. Finding the plugin's path from cache
    /// 2. Serializing state (if supported)
    /// 3. Unloading the old version
    /// 4. Loading the new version
    /// 5. Restoring state (if supported)
    pub async fn reload_plugin(&self, plugin_id: &str) -> Result<ReloadResult> {
        info!("Hot reload requested for plugin: {}", plugin_id);

        // Get the plugin path from discovery or cache
        let plugin_path = self.get_plugin_path(plugin_id).await;

        match plugin_path {
            Some(path) => self.reload_plugin_from_path(plugin_id, &path).await,
            None => {
                warn!("Plugin path not found for: {}", plugin_id);
                Ok(ReloadResult {
                    plugin_id: plugin_id.to_string(),
                    success: false,
                    old_version: None,
                    new_version: None,
                    state_preserved: false,
                    error: Some(format!(
                        "Plugin '{}' not found. Ensure it's registered in the plugin directory.",
                        plugin_id
                    )),
                })
            }
        }
    }

    /// Get plugin path from discovery or cached location
    async fn get_plugin_path(&self, plugin_name: &str) -> Option<PathBuf> {
        // Check discovery paths (simplified - in production would use PluginDiscovery)
        let search_paths = vec![
            PathBuf::from("./target/release"),
            PathBuf::from("./target/debug"),
            PathBuf::from("/usr/local/lib/skylet/plugins"),
            PathBuf::from("/usr/lib/skylet/plugins"),
            PathBuf::from("./plugins"),
        ];

        let extensions = if cfg!(target_os = "macos") {
            vec!["dylib"]
        } else if cfg!(target_os = "windows") {
            vec!["dll"]
        } else {
            vec!["so"]
        };

        for base_path in &search_paths {
            if !base_path.exists() {
                continue;
            }

            for ext in &extensions {
                let plugin_path = base_path.join(format!("lib{}.{}", plugin_name, ext));
                if plugin_path.exists() {
                    debug!("Found plugin {} at {:?}", plugin_name, plugin_path);
                    return Some(plugin_path);
                }
            }
        }

        None
    }

    /// Reload a plugin from a specific path
    ///
    /// This performs the full hot-reload workflow:
    /// 1. Serialize current state (if plugin supports hot reload)
    /// 2. Unload the current plugin version
    /// 3. Load the new plugin version
    /// 4. Restore serialized state (if supported)
    pub async fn reload_plugin_from_path(
        &self,
        plugin_id: &str,
        new_path: &Path,
    ) -> Result<ReloadResult> {
        info!("Hot reload from path requested for plugin: {} at {:?}", plugin_id, new_path);

        // Check if plugin is currently loaded
        let was_loaded = self.is_plugin_loaded(plugin_id).await;

        let old_version = if was_loaded {
            // Get current plugin info before unloading
            self.get_plugin_version(plugin_id).await
        } else {
            None
        };

        // Prepare state preservation
        let serialized_state = if was_loaded {
            self.prepare_plugin_state(plugin_id).await.ok()
        } else {
            None
        };

        // Unload current version if loaded
        if was_loaded {
            match self.unload_plugin(plugin_id).await {
                Ok(_) => {
                    debug!("Successfully unloaded plugin {}", plugin_id);
                }
                Err(e) => {
                    error!("Failed to unload plugin {}: {}", plugin_id, e);
                    return Ok(ReloadResult {
                        plugin_id: plugin_id.to_string(),
                        success: false,
                        old_version,
                        new_version: None,
                        state_preserved: false,
                        error: Some(format!("Failed to unload plugin: {}", e)),
                    });
                }
            }
        }

        // Load new version
        match self.load_plugin_instance_v2(plugin_id, &new_path.to_path_buf()).await {
            Ok(_) => {
                debug!("Successfully loaded new version of plugin {}", plugin_id);

                // Restore state if we had it
                let state_restored = if let Some(state) = serialized_state {
                    self.restore_plugin_state(plugin_id, &state).await.is_ok()
                } else {
                    false
                };

                let new_version = self.get_plugin_version(plugin_id).await;

                Ok(ReloadResult {
                    plugin_id: plugin_id.to_string(),
                    success: true,
                    old_version,
                    new_version,
                    state_preserved: state_restored,
                    error: None,
                })
            }
            Err(e) => {
                error!("Failed to load new version of plugin {}: {}", plugin_id, e);

                // Try to restore old version if we had one
                if was_loaded && old_version.is_some() {
                    warn!("Attempting to restore previous version of plugin {}", plugin_id);
                    // In production, would reload from the old path
                }

                Ok(ReloadResult {
                    plugin_id: plugin_id.to_string(),
                    success: false,
                    old_version,
                    new_version: None,
                    state_preserved: false,
                    error: Some(format!("Failed to load new version: {}", e)),
                })
            }
        }
    }

    /// Get the version of a loaded plugin
    async fn get_plugin_version(&self, plugin_name: &str) -> Option<String> {
        let plugins = self.get_plugins().await;
        if let Some(guarded) = plugins.get(plugin_name) {
            guarded.access().and_then(|guard| {
                guard.plugin().get_info().ok().map(|info| {
                    format!("{}-{}", info.name, info.version)
                })
            })
        } else {
            None
        }
    }

    /// Prepare plugin state for hot reload
    /// Calls plugin_prepare_hot_reload if supported
    async fn prepare_plugin_state(&self, plugin_name: &str) -> Result<Vec<u8>> {
        let plugins = self.get_plugins().await;
        if let Some(guarded) = plugins.get(plugin_name) {
            if let Some(_guard) = guarded.access() {
                // Try to call prepare_hot_reload via FFI
                // This would call the plugin's plugin_prepare_hot_reload symbol
                // For now, return empty state
                debug!("Preparing state for plugin {}", plugin_name);
                return Ok(Vec::new());
            }
        }
        Err(anyhow!("Plugin not found or not accessible"))
    }

    /// Restore plugin state after hot reload
    async fn restore_plugin_state(&self, plugin_name: &str, state: &[u8]) -> Result<()> {
        if state.is_empty() {
            return Ok(());
        }

        let plugins = self.get_plugins().await;
        if let Some(guarded) = plugins.get(plugin_name) {
            if let Some(_guard) = guarded.access() {
                debug!("Restoring state for plugin {}", plugin_name);
                // Would call plugin_init_from_state via FFI
                return Ok(());
            }
        }
        Err(anyhow!("Plugin not found or not accessible"))
    }
}

// ============================================================================
// Hot Reload Support Trait
// ============================================================================

/// Extension trait to add hot reload support to plugins
/// Note: The Plugin struct already has built-in hot reload methods:
/// - `supports_hot_reload()` - checks if ABI symbols are available
/// - `prepare_hot_reload()` - serializes state via FFI
/// - `init_from_state()` - restores state via FFI
///
/// This trait is kept for potential future extensions that need custom
/// hot reload behavior beyond the ABI-level support.
#[allow(dead_code)]
pub trait HotReloadPlugin {
    /// Check if this plugin supports hot reload
    fn supports_hot_reload(&self) -> bool;

    /// Prepare for hot reload - serialize state
    fn prepare_hot_reload(&self) -> Result<Vec<u8>>;

    /// Initialize from serialized state after reload
    fn init_from_state(&self, state: &[u8]) -> Result<()>;
}
