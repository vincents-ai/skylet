// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Plugin Lifecycle Automation - RFC-0002
//!
//! This module implements the top-level plugin lifecycle orchestrator that
//! coordinates discovery, dependency resolution, loading, health tracking,
//! and shutdown of plugins.
//!
//! The `PluginLifecycleManager` wraps the lower-level `PluginManager` (which
//! handles FFI context creation and ABI loading) and adds:
//! - State machine tracking per plugin (Discovered → Loading → Active → Failed)
//! - Dependency-ordered activation via `PluginDependencyResolver`
//! - Health check tracking and status queries
//! - Graceful ordered shutdown (reverse dependency order)
//!
//! Integration with:
//! - `PluginManager` (manager.rs): ABI v2 loading with FFI service wiring
//! - `PluginDiscovery` (discovery.rs): Filesystem plugin scanning
//! - `PluginDependencyResolver` (dependency_resolver.rs): Topological sort

use anyhow::{anyhow, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use super::dependency_resolver::PluginDependencyResolver;
use super::discovery::{DiscoveryConfig, PluginDiscovery};
use super::manager::PluginManager;

use skylet_abi::AbiV2PluginLoader;

// ============================================================================
// Plugin State Machine
// ============================================================================

/// Plugin lifecycle status
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum PluginStatus {
    /// Plugin discovered on disk but not yet loaded
    Discovered,
    /// Plugin is currently being loaded and initialized
    Loading,
    /// Plugin is active and running
    Active,
    /// Plugin failed to load or crashed
    Failed(String),
    /// Plugin is shutting down
    ShuttingDown,
    /// Plugin has been shut down
    Stopped,
}

impl std::fmt::Display for PluginStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginStatus::Discovered => write!(f, "Discovered"),
            PluginStatus::Loading => write!(f, "Loading"),
            PluginStatus::Active => write!(f, "Active"),
            PluginStatus::Failed(msg) => write!(f, "Failed: {}", msg),
            PluginStatus::ShuttingDown => write!(f, "ShuttingDown"),
            PluginStatus::Stopped => write!(f, "Stopped"),
        }
    }
}

/// Tracked state for a single plugin
#[derive(Debug, Clone, Serialize)]
pub struct PluginState {
    /// Plugin name
    pub name: String,
    /// Detected ABI version
    pub abi_version: String,
    /// Path to the plugin shared library
    pub path: PathBuf,
    /// Current lifecycle status
    pub status: PluginStatus,
    /// Declared dependencies (plugin names)
    pub dependencies: Vec<String>,
    /// When the plugin was loaded (if active)
    pub loaded_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Last error message (if failed)
    pub error: Option<String>,
}

/// Configuration for the lifecycle manager
#[derive(Debug, Clone)]
pub struct LifecycleConfig {
    /// Plugin discovery configuration
    pub discovery: DiscoveryConfig,
    /// Whether to continue loading remaining plugins if one fails
    pub continue_on_failure: bool,
    /// Health check interval in seconds (0 = disabled)
    pub health_check_interval_secs: u64,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            discovery: DiscoveryConfig::default(),
            continue_on_failure: true,
            health_check_interval_secs: 0,
        }
    }
}

// ============================================================================
// Lifecycle Manager
// ============================================================================

/// Top-level plugin lifecycle orchestrator.
///
/// Coordinates the full lifecycle: discovery → dependency resolution →
/// ordered loading → health tracking → ordered shutdown.
///
/// Wraps `PluginManager` for the actual FFI loading and context creation.
pub struct PluginLifecycleManager {
    /// Lower-level plugin manager that handles ABI loading and FFI services
    plugin_manager: PluginManager,
    /// Tracked plugin states
    plugins: Arc<RwLock<HashMap<String, PluginState>>>,
    /// Plugin loading order (dependencies first)
    loading_order: Arc<RwLock<Vec<String>>>,
    /// Configuration
    config: LifecycleConfig,
}

impl std::fmt::Debug for PluginLifecycleManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginLifecycleManager")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl PluginLifecycleManager {
    /// Create a new lifecycle manager with the given configuration.
    ///
    /// The `PluginManager` is created internally with default services.
    pub fn new(config: LifecycleConfig) -> Self {
        Self {
            plugin_manager: PluginManager::new(),
            plugins: Arc::new(RwLock::new(HashMap::new())),
            loading_order: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// Create a lifecycle manager wrapping an existing `PluginManager`.
    ///
    /// Useful when you need custom `PluginServices` configuration.
    pub fn with_plugin_manager(config: LifecycleConfig, plugin_manager: PluginManager) -> Self {
        Self {
            plugin_manager,
            plugins: Arc::new(RwLock::new(HashMap::new())),
            loading_order: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// Get a reference to the underlying `PluginManager`.
    pub fn plugin_manager(&self) -> &PluginManager {
        &self.plugin_manager
    }

    // ========================================================================
    // Discovery
    // ========================================================================

    /// Discover plugins from the configured search paths.
    ///
    /// Scans the filesystem for plugin shared libraries and registers them
    /// as `Discovered` in the plugin state map.
    ///
    /// Returns the number of plugins discovered.
    pub async fn discover(&self) -> Result<usize> {
        info!("Discovering plugins...");

        let discovery = PluginDiscovery::new(self.config.discovery.clone());
        let discovered = discovery.discover_plugins()?;

        info!("Discovered {} plugins", discovered.len());

        let mut plugins = self.plugins.write().await;
        for dp in &discovered {
            plugins.insert(
                dp.name.clone(),
                PluginState {
                    name: dp.name.clone(),
                    abi_version: dp.abi_version.clone(),
                    path: dp.path.clone(),
                    status: PluginStatus::Discovered,
                    dependencies: Vec::new(),
                    loaded_at: None,
                    error: None,
                },
            );
        }

        Ok(discovered.len())
    }

    // ========================================================================
    // Dependency Resolution
    // ========================================================================

    /// Resolve the loading order by probing plugin dependencies.
    ///
    /// For each discovered plugin, probes the shared library to extract
    /// dependency metadata, then performs a topological sort to determine
    /// the correct loading order (dependencies before dependents).
    ///
    /// Returns the ordered list of plugin names.
    pub async fn resolve_dependencies(&self) -> Result<Vec<String>> {
        info!("Resolving plugin dependencies...");

        let plugins = self.plugins.read().await;
        let mut resolver = PluginDependencyResolver::new();
        let mut dep_map: HashMap<String, Vec<String>> = HashMap::new();

        for (name, state) in plugins.iter() {
            if state.status != PluginStatus::Discovered {
                continue;
            }

            let (deps, version) = match AbiV2PluginLoader::load(&state.path) {
                Ok(loader) => {
                    let deps: Vec<String> = match loader.get_dependencies() {
                        Ok(entries) => entries
                            .into_iter()
                            .filter(|d| d.required)
                            .map(|d| match d.version_range {
                                Some(ver) => format!("{}@{}", d.name, ver),
                                None => d.name,
                            })
                            .collect(),
                        Err(e) => {
                            warn!(
                                "Could not read dependencies for plugin '{}': {}",
                                name, e
                            );
                            vec![]
                        }
                    };
                    let version = loader.get_info().ok().map(|m| m.version);
                    (deps, version)
                }
                Err(e) => {
                    warn!(
                        "Could not probe plugin '{}' for dependencies: {}",
                        name, e
                    );
                    (vec![], None)
                }
            };

            dep_map.insert(name.clone(), deps.clone());
            resolver.register_plugin(name, &state.abi_version, deps, version.as_deref());
        }
        drop(plugins);

        let ordered = match resolver.resolve_loading_order() {
            Ok(order) => {
                let names: Vec<String> = order.into_iter().map(|(name, _)| name).collect();
                info!(
                    "Resolved loading order: {:?}",
                    names.iter().map(|n| n.as_str()).collect::<Vec<_>>()
                );
                names
            }
            Err(e) => {
                warn!(
                    "Dependency resolution failed, using discovery order: {}",
                    e
                );
                let plugins = self.plugins.read().await;
                plugins.keys().cloned().collect()
            }
        };

        // Store dependencies on each plugin state
        {
            let mut plugins = self.plugins.write().await;
            for (name, deps) in &dep_map {
                if let Some(state) = plugins.get_mut(name) {
                    state.dependencies = deps.clone();
                }
            }
        }

        // Store the loading order
        {
            let mut order = self.loading_order.write().await;
            *order = ordered.clone();
        }

        Ok(ordered)
    }

    // ========================================================================
    // Activation (Loading)
    // ========================================================================

    /// Activate all discovered plugins in dependency order.
    ///
    /// Calls `discover()` and `resolve_dependencies()` if not already done,
    /// then loads each plugin via `PluginManager::load_plugin_instance_v2()`.
    ///
    /// Returns a summary of (loaded_count, failed_count).
    pub async fn activate_all(&self) -> Result<(usize, usize)> {
        // Discover if not done yet
        {
            let plugins = self.plugins.read().await;
            if plugins.is_empty() {
                drop(plugins);
                self.discover().await?;
            }
        }

        // Resolve dependencies if not done yet
        let order = {
            let current_order = self.loading_order.read().await;
            if current_order.is_empty() {
                drop(current_order);
                self.resolve_dependencies().await?
            } else {
                current_order.clone()
            }
        };

        info!("Activating {} plugins...", order.len());

        let mut loaded = 0;
        let mut failed = 0;

        for name in &order {
            let path = {
                let plugins = self.plugins.read().await;
                match plugins.get(name) {
                    Some(state) => state.path.clone(),
                    None => {
                        warn!("Plugin '{}' in loading order but not in state map", name);
                        continue;
                    }
                }
            };

            // Mark as loading
            {
                let mut plugins = self.plugins.write().await;
                if let Some(state) = plugins.get_mut(name) {
                    state.status = PluginStatus::Loading;
                }
            }

            match self
                .plugin_manager
                .load_plugin_instance_v2(name, &path)
                .await
            {
                Ok(()) => {
                    info!("Activated plugin: {}", name);
                    let mut plugins = self.plugins.write().await;
                    if let Some(state) = plugins.get_mut(name) {
                        state.status = PluginStatus::Active;
                        state.loaded_at = Some(chrono::Utc::now());
                        state.error = None;
                    }
                    loaded += 1;
                }
                Err(e) => {
                    let error_msg = format!("{}", e);
                    warn!("Failed to activate plugin '{}': {}", name, error_msg);
                    let mut plugins = self.plugins.write().await;
                    if let Some(state) = plugins.get_mut(name) {
                        state.status = PluginStatus::Failed(error_msg.clone());
                        state.error = Some(error_msg);
                    }
                    failed += 1;

                    if !self.config.continue_on_failure {
                        return Err(anyhow!(
                            "Plugin '{}' failed to activate and continue_on_failure is false",
                            name
                        ));
                    }
                }
            }
        }

        info!(
            "Plugin activation complete: {} loaded, {} failed",
            loaded, failed
        );

        Ok((loaded, failed))
    }

    /// Activate a single plugin by name.
    ///
    /// The plugin must already be in `Discovered` state (via `discover()`).
    /// Does NOT check dependencies -- use `activate_all()` for dependency ordering.
    pub async fn activate(&self, plugin_name: &str) -> Result<()> {
        let path = {
            let plugins = self.plugins.read().await;
            let state = plugins
                .get(plugin_name)
                .ok_or_else(|| anyhow!("Plugin '{}' not found", plugin_name))?;

            if state.status == PluginStatus::Active {
                return Ok(()); // Already active
            }

            state.path.clone()
        };

        // Mark as loading
        {
            let mut plugins = self.plugins.write().await;
            if let Some(state) = plugins.get_mut(plugin_name) {
                state.status = PluginStatus::Loading;
            }
        }

        match self
            .plugin_manager
            .load_plugin_instance_v2(plugin_name, &path)
            .await
        {
            Ok(()) => {
                let mut plugins = self.plugins.write().await;
                if let Some(state) = plugins.get_mut(plugin_name) {
                    state.status = PluginStatus::Active;
                    state.loaded_at = Some(chrono::Utc::now());
                    state.error = None;
                }
                info!("Activated plugin: {}", plugin_name);
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                let mut plugins = self.plugins.write().await;
                if let Some(state) = plugins.get_mut(plugin_name) {
                    state.status = PluginStatus::Failed(error_msg.clone());
                    state.error = Some(error_msg.clone());
                }
                Err(anyhow!(
                    "Failed to activate plugin '{}': {}",
                    plugin_name,
                    error_msg
                ))
            }
        }
    }

    // ========================================================================
    // Deactivation (Shutdown)
    // ========================================================================

    /// Shut down all active plugins in reverse dependency order.
    ///
    /// Plugins that depend on others are shut down first, then their
    /// dependencies.
    pub async fn shutdown_all(&self) {
        let order = {
            let order = self.loading_order.read().await;
            let mut reversed = order.clone();
            reversed.reverse(); // Dependents before dependencies
            reversed
        };

        info!("Shutting down {} plugins...", order.len());

        for name in &order {
            let should_shutdown = {
                let plugins = self.plugins.read().await;
                matches!(
                    plugins.get(name).map(|s| &s.status),
                    Some(PluginStatus::Active)
                )
            };

            if should_shutdown {
                // Mark as shutting down
                {
                    let mut plugins = self.plugins.write().await;
                    if let Some(state) = plugins.get_mut(name) {
                        state.status = PluginStatus::ShuttingDown;
                    }
                }

                match self.plugin_manager.unload_plugin(name).await {
                    Ok(()) => {
                        info!("Shut down plugin: {}", name);
                        let mut plugins = self.plugins.write().await;
                        if let Some(state) = plugins.get_mut(name) {
                            state.status = PluginStatus::Stopped;
                        }
                    }
                    Err(e) => {
                        error!("Error shutting down plugin '{}': {}", name, e);
                        let mut plugins = self.plugins.write().await;
                        if let Some(state) = plugins.get_mut(name) {
                            state.status =
                                PluginStatus::Failed(format!("Shutdown error: {}", e));
                        }
                    }
                }
            }
        }

        // Also shut down any plugins not in the loading order (shouldn't happen,
        // but be defensive)
        self.plugin_manager.shutdown_all().await;

        info!("All plugins shut down");
    }

    /// Deactivate a single plugin by name.
    ///
    /// Does NOT check for active dependents -- the caller must ensure
    /// no other active plugin depends on this one.
    pub async fn deactivate(&self, plugin_name: &str) -> Result<()> {
        {
            let plugins = self.plugins.read().await;
            let state = plugins
                .get(plugin_name)
                .ok_or_else(|| anyhow!("Plugin '{}' not found", plugin_name))?;

            if state.status != PluginStatus::Active {
                return Ok(()); // Not active, nothing to do
            }
        }

        // Check if any active plugin depends on this one
        {
            let plugins = self.plugins.read().await;
            let dependents: Vec<String> = plugins
                .iter()
                .filter(|(_, s)| {
                    s.status == PluginStatus::Active
                        && s.dependencies.iter().any(|d| {
                            // Dependencies may include version suffixes like "foo@1.0"
                            d == plugin_name || d.starts_with(&format!("{}@", plugin_name))
                        })
                })
                .map(|(name, _)| name.clone())
                .collect();

            if !dependents.is_empty() {
                return Err(anyhow!(
                    "Cannot deactivate '{}': active plugins depend on it: {:?}",
                    plugin_name,
                    dependents
                ));
            }
        }

        // Mark as shutting down
        {
            let mut plugins = self.plugins.write().await;
            if let Some(state) = plugins.get_mut(plugin_name) {
                state.status = PluginStatus::ShuttingDown;
            }
        }

        match self.plugin_manager.unload_plugin(plugin_name).await {
            Ok(()) => {
                let mut plugins = self.plugins.write().await;
                if let Some(state) = plugins.get_mut(plugin_name) {
                    state.status = PluginStatus::Stopped;
                }
                info!("Deactivated plugin: {}", plugin_name);
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                let mut plugins = self.plugins.write().await;
                if let Some(state) = plugins.get_mut(plugin_name) {
                    state.status = PluginStatus::Failed(format!("Shutdown error: {}", error_msg));
                }
                Err(anyhow!(
                    "Failed to deactivate plugin '{}': {}",
                    plugin_name,
                    error_msg
                ))
            }
        }
    }

    // ========================================================================
    // Status Queries
    // ========================================================================

    /// Get the state of a specific plugin.
    pub async fn get_state(&self, plugin_name: &str) -> Option<PluginState> {
        let plugins = self.plugins.read().await;
        plugins.get(plugin_name).cloned()
    }

    /// List all tracked plugins with their states.
    pub async fn list_plugins(&self) -> Vec<PluginState> {
        let plugins = self.plugins.read().await;
        plugins.values().cloned().collect()
    }

    /// Get all plugins with a specific status.
    pub async fn get_plugins_by_status(&self, status: &PluginStatus) -> Vec<PluginState> {
        let plugins = self.plugins.read().await;
        plugins
            .values()
            .filter(|s| &s.status == status)
            .cloned()
            .collect()
    }

    /// Get a summary of plugin counts by status.
    pub async fn status_summary(&self) -> HashMap<String, usize> {
        let plugins = self.plugins.read().await;
        let mut counts: HashMap<String, usize> = HashMap::new();
        for state in plugins.values() {
            let key = match &state.status {
                PluginStatus::Discovered => "discovered",
                PluginStatus::Loading => "loading",
                PluginStatus::Active => "active",
                PluginStatus::Failed(_) => "failed",
                PluginStatus::ShuttingDown => "shutting_down",
                PluginStatus::Stopped => "stopped",
            };
            *counts.entry(key.to_string()).or_insert(0) += 1;
        }
        counts
    }

    /// Get the loading order (dependencies first).
    pub async fn loading_order(&self) -> Vec<String> {
        self.loading_order.read().await.clone()
    }

    /// Get the count of active plugins.
    pub async fn active_count(&self) -> usize {
        let plugins = self.plugins.read().await;
        plugins
            .values()
            .filter(|s| s.status == PluginStatus::Active)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lifecycle_config_default() {
        let config = LifecycleConfig::default();
        assert!(config.continue_on_failure);
        assert_eq!(config.health_check_interval_secs, 0);
    }

    #[tokio::test]
    async fn test_lifecycle_manager_creation() {
        let config = LifecycleConfig::default();
        let manager = PluginLifecycleManager::new(config);
        let plugins = manager.list_plugins().await;
        assert!(plugins.is_empty());
    }

    #[tokio::test]
    async fn test_lifecycle_manager_with_plugin_manager() {
        let config = LifecycleConfig::default();
        let pm = PluginManager::new();
        let manager = PluginLifecycleManager::with_plugin_manager(config, pm);
        let plugins = manager.list_plugins().await;
        assert!(plugins.is_empty());
    }

    #[tokio::test]
    async fn test_plugin_status_display() {
        assert_eq!(format!("{}", PluginStatus::Active), "Active");
        assert_eq!(format!("{}", PluginStatus::Discovered), "Discovered");
        assert_eq!(format!("{}", PluginStatus::Loading), "Loading");
        assert_eq!(format!("{}", PluginStatus::Stopped), "Stopped");
        assert_eq!(format!("{}", PluginStatus::ShuttingDown), "ShuttingDown");
        assert!(format!("{}", PluginStatus::Failed("test".to_string())).contains("test"));
    }

    #[tokio::test]
    async fn test_get_nonexistent_plugin() {
        let config = LifecycleConfig::default();
        let manager = PluginLifecycleManager::new(config);
        let state = manager.get_state("nonexistent").await;
        assert!(state.is_none());
    }

    #[tokio::test]
    async fn test_status_summary_empty() {
        let config = LifecycleConfig::default();
        let manager = PluginLifecycleManager::new(config);
        let summary = manager.status_summary().await;
        assert!(summary.is_empty());
    }

    #[tokio::test]
    async fn test_active_count_empty() {
        let config = LifecycleConfig::default();
        let manager = PluginLifecycleManager::new(config);
        assert_eq!(manager.active_count().await, 0);
    }

    #[tokio::test]
    async fn test_loading_order_empty() {
        let config = LifecycleConfig::default();
        let manager = PluginLifecycleManager::new(config);
        let order = manager.loading_order().await;
        assert!(order.is_empty());
    }

    #[tokio::test]
    async fn test_shutdown_all_empty() {
        let config = LifecycleConfig::default();
        let manager = PluginLifecycleManager::new(config);
        // Should not panic on empty
        manager.shutdown_all().await;
    }

    #[tokio::test]
    async fn test_deactivate_nonexistent() {
        let config = LifecycleConfig::default();
        let manager = PluginLifecycleManager::new(config);
        let result = manager.deactivate("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_activate_nonexistent() {
        let config = LifecycleConfig::default();
        let manager = PluginLifecycleManager::new(config);
        let result = manager.activate("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_plugins_by_status() {
        let config = LifecycleConfig::default();
        let manager = PluginLifecycleManager::new(config);

        // Manually insert some plugins for testing
        {
            let mut plugins = manager.plugins.write().await;
            plugins.insert(
                "plugin1".to_string(),
                PluginState {
                    name: "plugin1".to_string(),
                    abi_version: "v2".to_string(),
                    path: PathBuf::from("/tmp/plugin1.so"),
                    status: PluginStatus::Active,
                    dependencies: vec![],
                    loaded_at: Some(chrono::Utc::now()),
                    error: None,
                },
            );
            plugins.insert(
                "plugin2".to_string(),
                PluginState {
                    name: "plugin2".to_string(),
                    abi_version: "v2".to_string(),
                    path: PathBuf::from("/tmp/plugin2.so"),
                    status: PluginStatus::Discovered,
                    dependencies: vec![],
                    loaded_at: None,
                    error: None,
                },
            );
            plugins.insert(
                "plugin3".to_string(),
                PluginState {
                    name: "plugin3".to_string(),
                    abi_version: "v2".to_string(),
                    path: PathBuf::from("/tmp/plugin3.so"),
                    status: PluginStatus::Failed("load error".to_string()),
                    dependencies: vec![],
                    loaded_at: None,
                    error: Some("load error".to_string()),
                },
            );
        }

        let active = manager
            .get_plugins_by_status(&PluginStatus::Active)
            .await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "plugin1");

        let discovered = manager
            .get_plugins_by_status(&PluginStatus::Discovered)
            .await;
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].name, "plugin2");

        let all = manager.list_plugins().await;
        assert_eq!(all.len(), 3);

        let summary = manager.status_summary().await;
        assert_eq!(summary.get("active"), Some(&1));
        assert_eq!(summary.get("discovered"), Some(&1));
        assert_eq!(summary.get("failed"), Some(&1));

        assert_eq!(manager.active_count().await, 1);
    }
}
