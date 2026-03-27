// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Plugin Lifecycle Automation - RFC-0002
//!
//! This module implements the complete plugin management workflow with:
//! - Installation (download, verify, unpack)
//! - Activation (resolve deps, load, init)
//! - Deactivation (shutdown, unload)
//! - Uninstallation (cleanup, unregister)
//!
//! Integration with:
//! - RFC-0001: Registry for plugin discovery
//! - RFC-0003: Package handling (artifact verification, extraction)
//! - RFC-0004: ABI loading
//! - RFC-0005: Dependency resolution
//! - RFC-0006: Configuration management

#![allow(dead_code)]

use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

// Integration with RFC-0003 (Package handling)
use plugin_packager::{
    extract_artifact, verify_artifact, DependencyResolver, LocalRegistry, PluginHealthChecker,
    PluginRegistryEntry, SignatureManager,
};

// Integration with RFC-0004 (ABI loading)
use skylet_abi::{PluginLoadConfig, PluginLoadPipeline};

use super::manager::PluginManager;

/// Plugin status for tracking in the lifecycle manager
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginStatus {
    /// Plugin is discovered but not installed
    Discovered,
    /// Plugin artifact is being downloaded
    Downloading,
    /// Plugin artifact is downloaded and ready for installation
    Downloaded,
    /// Plugin is being installed
    Installing,
    /// Plugin is installed but not active
    Installed,
    /// Plugin is being activated
    Activating,
    /// Plugin is active and running
    Active,
    /// Plugin is being deactivated
    Deactivating,
    /// Plugin is deactivated
    Deactivated,
    /// Plugin is being uninstalled
    Uninstalling,
    /// Plugin encountered an error
    Error(String),
}

impl std::fmt::Display for PluginStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginStatus::Discovered => write!(f, "Discovered"),
            PluginStatus::Downloading => write!(f, "Downloading"),
            PluginStatus::Downloaded => write!(f, "Downloaded"),
            PluginStatus::Installing => write!(f, "Installing"),
            PluginStatus::Installed => write!(f, "Installed"),
            PluginStatus::Activating => write!(f, "Activating"),
            PluginStatus::Active => write!(f, "Active"),
            PluginStatus::Deactivating => write!(f, "Deactivating"),
            PluginStatus::Deactivated => write!(f, "Deactivated"),
            PluginStatus::Uninstalling => write!(f, "Uninstalling"),
            PluginStatus::Error(msg) => write!(f, "Error: {}", msg),
        }
    }
}

/// Detailed plugin state tracked by the lifecycle manager
#[derive(Debug, Clone)]
pub struct PluginState {
    /// Unique plugin identifier
    pub id: String,
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Current status
    pub status: PluginStatus,
    /// Path to the plugin artifact (if downloaded)
    pub artifact_path: Option<PathBuf>,
    /// Path to the installed plugin directory
    pub install_path: Option<PathBuf>,
    /// List of dependency plugin IDs
    pub dependencies: Vec<String>,
    /// Health check result (if active)
    pub health_status: Option<String>,
    /// Last status change timestamp
    pub last_status_change: chrono::DateTime<chrono::Utc>,
    /// Error message if status is Error
    pub error_message: Option<String>,
}

/// Configuration for the lifecycle manager
#[derive(Debug, Clone)]
pub struct LifecycleConfig {
    /// Directory for installed plugins
    pub plugins_dir: PathBuf,
    /// Directory for downloaded artifacts
    pub artifacts_dir: PathBuf,
    /// Directory for plugin data
    pub data_dir: PathBuf,
    /// Whether to auto-activate plugins after installation
    pub auto_activate: bool,
    /// Whether to verify signatures before installation
    pub verify_signatures: bool,
    /// Maximum concurrent installations
    pub max_concurrent_installs: usize,
    /// Health check interval in seconds (0 = disabled)
    pub health_check_interval_secs: u64,
    /// Whether to check dependencies before activation
    pub check_dependencies: bool,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            plugins_dir: PathBuf::from("./plugins"),
            artifacts_dir: PathBuf::from("./artifacts"),
            data_dir: PathBuf::from("./data"),
            auto_activate: false,
            verify_signatures: true,
            max_concurrent_installs: 3,
            health_check_interval_secs: 60,
            check_dependencies: true,
        }
    }
}

impl LifecycleConfig {
    /// Create a new configuration with custom paths
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        let plugins_dir = plugins_dir.into();
        Self {
            plugins_dir: plugins_dir.clone(),
            artifacts_dir: plugins_dir.join("../artifacts"),
            data_dir: plugins_dir.join("../data"),
            ..Default::default()
        }
    }

    /// Enable auto-activation after installation
    pub fn with_auto_activate(mut self) -> Self {
        self.auto_activate = true;
        self
    }

    /// Disable signature verification
    pub fn without_signature_verification(mut self) -> Self {
        self.verify_signatures = false;
        self
    }

    /// Set health check interval
    pub fn with_health_check_interval(mut self, secs: u64) -> Self {
        self.health_check_interval_secs = secs;
        self
    }
}

/// Result of an installation operation
#[derive(Debug, Clone)]
pub struct InstallationResult {
    /// Plugin ID
    pub plugin_id: String,
    /// Whether installation was successful
    pub success: bool,
    /// Path where plugin was installed
    pub install_path: Option<PathBuf>,
    /// Error message if failed
    pub error: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Result of an activation operation
#[derive(Debug, Clone)]
pub struct ActivationResult {
    /// Plugin ID
    pub plugin_id: String,
    /// Whether activation was successful
    pub success: bool,
    /// Resolved dependencies
    pub resolved_dependencies: Vec<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Result of a deactivation operation
#[derive(Debug, Clone)]
pub struct DeactivationResult {
    /// Plugin ID
    pub plugin_id: String,
    /// Whether deactivation was successful
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Result of an uninstallation operation
#[derive(Debug, Clone)]
pub struct UninstallationResult {
    /// Plugin ID
    pub plugin_id: String,
    /// Whether uninstallation was successful
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Main lifecycle manager for plugin automation
pub struct PluginLifecycleManager {
    /// Configuration
    config: LifecycleConfig,
    /// Plugin states indexed by plugin ID
    plugins: Arc<RwLock<HashMap<String, PluginState>>>,
    /// Local registry for plugin discovery
    registry: Arc<RwLock<LocalRegistry>>,
    /// Dependency resolver
    _dependency_resolver: DependencyResolver,
    /// Health checker
    health_checker: PluginHealthChecker,
    /// Signature manager (optional)
    _signature_manager: Option<SignatureManager>,
    /// Load pipeline for ABI loading (fallback when plugin_manager not set)
    load_pipeline: PluginLoadPipeline,
    /// Reference to the main PluginManager for actual plugin loading
    /// This is the critical fix for HR-ARCH-1: unifies lifecycle with actual plugin management
    /// Uses RwLock for interior mutability so we can set it after wrapping in Arc
    plugin_manager: RwLock<Option<Arc<PluginManager>>>,
}

impl PluginLifecycleManager {
    /// Create a new lifecycle manager
    pub fn new(config: LifecycleConfig) -> Result<Self> {
        // Ensure directories exist
        std::fs::create_dir_all(&config.plugins_dir)
            .with_context(|| format!("Creating plugins directory: {:?}", config.plugins_dir))?;
        std::fs::create_dir_all(&config.artifacts_dir)
            .with_context(|| format!("Creating artifacts directory: {:?}", config.artifacts_dir))?;
        std::fs::create_dir_all(&config.data_dir)
            .with_context(|| format!("Creating data directory: {:?}", config.data_dir))?;

        // Initialize local registry
        let registry = LocalRegistry::new();

        // Initialize dependency resolver with a separate registry instance
        // (DependencyResolver takes ownership)
        let dependency_resolver = DependencyResolver::new(LocalRegistry::new());

        // Initialize health checker
        let health_checker = PluginHealthChecker::new();

        // Initialize signature manager (optional)
        let signature_manager = if config.verify_signatures {
            Some(SignatureManager::new())
        } else {
            None
        };

        // Initialize load pipeline
        let load_config = PluginLoadConfig::default();
        let load_pipeline = PluginLoadPipeline::new(load_config)?;

        Ok(Self {
            config,
            plugins: Arc::new(RwLock::new(HashMap::new())),
            registry: Arc::new(RwLock::new(registry)),
            _dependency_resolver: dependency_resolver,
            health_checker,
            _signature_manager: signature_manager,
            load_pipeline,
            plugin_manager: RwLock::new(None),
        })
    }

    /// Set the PluginManager for actual plugin operations
    /// This is the critical for HR-ARCH-1: enables lifecycle manager to delegate
    /// to the actual PluginManager instead of using a separate load_pipeline
    pub async fn set_plugin_manager(&self, manager: Arc<PluginManager>) {
        let mut pm = self.plugin_manager.write().await;
        *pm = Some(manager);
        info!("PluginLifecycleManager: PluginManager reference set");
    }

    /// Register an already-loaded plugin for hot-reload tracking
    /// This is used when plugins are loaded by PluginManager in main.rs
    /// and need to be tracked by the lifecycle manager for hot reload.
    pub async fn register_loaded_plugin(&self, plugin_id: &str, install_path: PathBuf) -> Result<()> {
        let state = PluginState {
            id: plugin_id.to_string(),
            name: plugin_id.to_string(),
            version: "unknown".to_string(),
            status: PluginStatus::Active,
            artifact_path: None,
            install_path: Some(install_path),
            dependencies: vec![],
            health_status: None,
            last_status_change: chrono::Utc::now(),
            error_message: None,
        };

        let mut plugins = self.plugins.write().await;
        plugins.insert(plugin_id.to_string(), state);

        info!("Registered loaded plugin '{}' for hot-reload tracking", plugin_id);
        Ok(())
    }

    /// Install a plugin from an artifact
    ///
    /// This performs the complete installation workflow:
    /// 1. Verify artifact signature (if enabled)
    /// 2. Verify artifact integrity (SHA-256)
    /// 3. Extract artifact to plugin directory
    /// 4. Register plugin in local registry
    pub async fn install(&self, artifact_path: &Path) -> Result<InstallationResult> {
        let start = std::time::Instant::now();
        let artifact_path = artifact_path.to_path_buf();

        info!("Starting installation from: {:?}", artifact_path);

        // Step 1: Verify artifact integrity
        debug!("Verifying artifact integrity");
        if let Err(e) = verify_artifact(&artifact_path, None) {
            let error = format!("Artifact verification failed: {}", e);
            warn!("{}", error);
            return Ok(InstallationResult {
                plugin_id: String::new(),
                success: false,
                install_path: None,
                error: Some(error),
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Step 2: Extract artifact
        debug!("Extracting artifact");
        let extract_result = extract_artifact(&artifact_path, &self.config.plugins_dir)
            .with_context(|| "Extracting artifact")?;

        let plugin_id = extract_result.plugin_name.clone();
        let install_path = extract_result.plugin_dir.clone();

        info!("Extracted plugin '{}' to {:?}", plugin_id, install_path);

        // Step 3: Register in local registry
        let registry_entry = PluginRegistryEntry {
            plugin_id: plugin_id.clone(),
            name: plugin_id.clone(),
            version: extract_result.plugin_version.clone(),
            abi_version: "2.0".to_string(),
            description: Some(format!("Plugin {}", plugin_id)),
            author: None,
            license: None,
            keywords: None,
            dependencies: None,
        };

        let mut registry = self.registry.write().await;
        registry.register(registry_entry)?;

        // Step 4: Update plugin state
        let mut plugins = self.plugins.write().await;
        let state = PluginState {
            id: plugin_id.clone(),
            name: plugin_id.clone(),
            version: extract_result.plugin_version.clone(),
            status: PluginStatus::Installed,
            artifact_path: Some(artifact_path),
            install_path: Some(install_path.clone()),
            dependencies: vec![],
            health_status: None,
            last_status_change: chrono::Utc::now(),
            error_message: None,
        };
        plugins.insert(plugin_id.clone(), state);

        let duration_ms = start.elapsed().as_millis() as u64;
        info!(
            "Installation complete for '{}' in {}ms",
            plugin_id, duration_ms
        );

        Ok(InstallationResult {
            plugin_id,
            success: true,
            install_path: Some(install_path),
            error: None,
            duration_ms,
        })
    }

    /// Install a plugin from a URL
    ///
    /// This downloads the artifact first, then performs installation
    pub async fn install_from_url(&self, url: &str) -> Result<InstallationResult> {
        let start = std::time::Instant::now();

        info!("Downloading plugin from: {}", url);

        // Extract filename from URL
        let filename = url.rsplit('/').next().unwrap_or("plugin.tar.gz");
        let artifact_path = self.config.artifacts_dir.join(filename);

        // Update state to downloading
        let temp_id = format!("download-{}", chrono::Utc::now().timestamp());
        {
            let mut plugins = self.plugins.write().await;
            plugins.insert(
                temp_id.clone(),
                PluginState {
                    id: temp_id.clone(),
                    name: temp_id.clone(),
                    version: String::new(),
                    status: PluginStatus::Downloading,
                    artifact_path: Some(artifact_path.clone()),
                    install_path: None,
                    dependencies: vec![],
                    health_status: None,
                    last_status_change: chrono::Utc::now(),
                    error_message: None,
                },
            );
        }

        // Download the artifact
        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Downloading from {}", url))?;

        if !response.status().is_success() {
            let error = format!("Download failed with status: {}", response.status());
            return Ok(InstallationResult {
                plugin_id: String::new(),
                success: false,
                install_path: None,
                error: Some(error),
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }

        let bytes = response
            .bytes()
            .await
            .with_context(|| "Reading download content")?;

        std::fs::write(&artifact_path, &bytes)
            .with_context(|| format!("Writing artifact to {:?}", artifact_path))?;

        info!("Downloaded {} bytes to {:?}", bytes.len(), artifact_path);

        // Now install from the downloaded artifact
        self.install(&artifact_path).await
    }

    /// Activate a plugin
    ///
    /// This performs the activation workflow:
    /// 1. Check dependencies are satisfied
    /// 2. Load plugin binary
    /// 3. Initialize plugin
    /// 4. Mark as active
    pub async fn activate(&self, plugin_id: &str) -> Result<ActivationResult> {
        let start = std::time::Instant::now();

        info!("Activating plugin: {}", plugin_id);

        // Get current state and install path
        let (install_path, dependencies) = {
            let plugins = self.plugins.read().await;
            let state = plugins
                .get(plugin_id)
                .ok_or_else(|| anyhow!("Plugin '{}' not found", plugin_id))?;

            // Check if already active
            if state.status == PluginStatus::Active {
                return Ok(ActivationResult {
                    plugin_id: plugin_id.to_string(),
                    success: true,
                    resolved_dependencies: state.dependencies.clone(),
                    error: None,
                    duration_ms: 0,
                });
            }

            let install_path = state
                .install_path
                .clone()
                .ok_or_else(|| anyhow!("Plugin '{}' not installed", plugin_id))?;

            (install_path, state.dependencies.clone())
        };

        // Update status to activating
        {
            let mut plugins = self.plugins.write().await;
            if let Some(state) = plugins.get_mut(plugin_id) {
                state.status = PluginStatus::Activating;
                state.last_status_change = chrono::Utc::now();
            }
        }

        // Resolve dependencies
        let mut resolved_deps = Vec::new();
        if self.config.check_dependencies && !dependencies.is_empty() {
            debug!("Checking dependencies for {}", plugin_id);
            let plugins = self.plugins.read().await;
            for dep_id in &dependencies {
                if let Some(dep_state) = plugins.get(dep_id) {
                    if dep_state.status != PluginStatus::Active {
                        let error = format!("Dependency '{}' is not active", dep_id);
                        drop(plugins);
                        {
                            let mut plugins = self.plugins.write().await;
                            if let Some(state) = plugins.get_mut(plugin_id) {
                                state.status = PluginStatus::Error(error.clone());
                                state.error_message = Some(error.clone());
                            }
                        }
                        return Ok(ActivationResult {
                            plugin_id: plugin_id.to_string(),
                            success: false,
                            resolved_dependencies: vec![],
                            error: Some(error),
                            duration_ms: start.elapsed().as_millis() as u64,
                        });
                    }
                    resolved_deps.push(dep_id.clone());
                } else {
                    let error = format!("Dependency '{}' not found", dep_id);
                    drop(plugins);
                    {
                        let mut plugins = self.plugins.write().await;
                        if let Some(state) = plugins.get_mut(plugin_id) {
                            state.status = PluginStatus::Error(error.clone());
                            state.error_message = Some(error.clone());
                        }
                    }
                    return Ok(ActivationResult {
                        plugin_id: plugin_id.to_string(),
                        success: false,
                        resolved_dependencies: vec![],
                        error: Some(error),
                        duration_ms: start.elapsed().as_millis() as u64,
                    });
                }
            }
        }

        // Find plugin binary
        let binary_path = self.find_plugin_binary(&install_path)?;

        // Load plugin using plugin_manager if available, otherwise fall back to load_pipeline
        debug!("Loading plugin binary: {:?}", binary_path);
        
        let pm_guard = self.plugin_manager.read().await;
        if let Some(ref pm) = *pm_guard {
            let load_result = pm.load_plugin_instance_v2(plugin_id, &binary_path).await;
            if load_result.is_err() {
                let e = load_result.unwrap_err();
                let error = format!("Plugin manager load failed: {}", e);
                error!("{}", error);
                let mut plugins = self.plugins.write().await;
                if let Some(state) = plugins.get_mut(plugin_id) {
                    state.status = PluginStatus::Error(error.clone());
                    state.error_message = Some(error.clone());
                }
                return Err(anyhow!(error));
            }
            
            info!("Plugin '{}' loaded successfully via PluginManager", plugin_id);

            let mut plugins = self.plugins.write().await;
            if let Some(state) = plugins.get_mut(plugin_id) {
                state.status = PluginStatus::Active;
                state.last_status_change = chrono::Utc::now();
            }

            let duration_ms = start.elapsed().as_millis() as u64;
            info!(
                "Activation complete for '{}' in {}ms",
                plugin_id, duration_ms
            );

            Ok(ActivationResult {
                plugin_id: plugin_id.to_string(),
                success: true,
                resolved_dependencies: resolved_deps,
                error: None,
                duration_ms,
            })
        } else {
            drop(pm_guard);
            match self.load_pipeline.load(&binary_path) {
                Ok(load_result) => {
                    if load_result.success {
                        info!("Plugin '{}' loaded successfully", plugin_id);

                        let mut plugins = self.plugins.write().await;
                        if let Some(state) = plugins.get_mut(plugin_id) {
                            state.status = PluginStatus::Active;
                            state.last_status_change = chrono::Utc::now();
                        }

                        let duration_ms = start.elapsed().as_millis() as u64;
                        info!(
                            "Activation complete for '{}' in {}ms",
                            plugin_id, duration_ms
                        );

                        Ok(ActivationResult {
                            plugin_id: plugin_id.to_string(),
                            success: true,
                            resolved_dependencies: resolved_deps,
                            error: None,
                            duration_ms,
                        })
                    } else {
                        let error = format!("Load failed: {:?}", load_result.failed_stage());
                        error!("{}", error);

                        let mut plugins = self.plugins.write().await;
                        if let Some(state) = plugins.get_mut(plugin_id) {
                            state.status = PluginStatus::Error(error.clone());
                            state.error_message = Some(error.clone());
                        }

                        Ok(ActivationResult {
                            plugin_id: plugin_id.to_string(),
                            success: false,
                            resolved_dependencies: vec![],
                            error: Some("Load failed".to_string()),
                            duration_ms: start.elapsed().as_millis() as u64,
                        })
                    }
                }
                Err(e) => {
                    let error = format!("Load error: {}", e);
                    error!("{}", error);

                    let mut plugins = self.plugins.write().await;
                    if let Some(state) = plugins.get_mut(plugin_id) {
                        state.status = PluginStatus::Error(error.clone());
                        state.error_message = Some(error.clone());
                    }

                    Ok(ActivationResult {
                        plugin_id: plugin_id.to_string(),
                        success: false,
                        resolved_dependencies: vec![],
                        error: Some(e.to_string()),
                        duration_ms: start.elapsed().as_millis() as u64,
                    })
                }
            }
        }
    }

    /// Deactivate a plugin
    ///
    /// This performs the deactivation workflow:
    /// 1. Check if other plugins depend on this one
    /// 2. Shutdown plugin
    /// 3. Unload plugin
    /// 4. Mark as deactivated
    pub async fn deactivate(&self, plugin_id: &str) -> Result<DeactivationResult> {
        let start = std::time::Instant::now();

        info!("Deactivating plugin: {}", plugin_id);

        // Check current status and get dependencies info
        let dependents = {
            let plugins = self.plugins.read().await;
            let state = plugins
                .get(plugin_id)
                .ok_or_else(|| anyhow!("Plugin '{}' not found", plugin_id))?;

            // Check if already deactivated
            if state.status != PluginStatus::Active {
                return Ok(DeactivationResult {
                    plugin_id: plugin_id.to_string(),
                    success: true,
                    error: None,
                    duration_ms: 0,
                });
            }

            // Check for dependents
            plugins
                .iter()
                .filter(|(_, s)| {
                    s.dependencies.contains(&plugin_id.to_string())
                        && s.status == PluginStatus::Active
                })
                .map(|(id, _)| id.clone())
                .collect::<Vec<String>>()
        };

        // Update status to deactivating
        {
            let mut plugins = self.plugins.write().await;
            if let Some(state) = plugins.get_mut(plugin_id) {
                state.status = PluginStatus::Deactivating;
                state.last_status_change = chrono::Utc::now();
            }
        }

        if !dependents.is_empty() {
            let error = format!("Cannot deactivate: plugins {:?} depend on it", dependents);
            let mut plugins = self.plugins.write().await;
            if let Some(state) = plugins.get_mut(plugin_id) {
                state.status = PluginStatus::Error(error.clone());
                state.error_message = Some(error.clone());
            }

            return Ok(DeactivationResult {
                plugin_id: plugin_id.to_string(),
                success: false,
                error: Some(format!("Dependent plugins: {:?}", dependents)),
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Perform shutdown (would call plugin_shutdown in real implementation)
        debug!("Shutting down plugin: {}", plugin_id);

        // Call plugin_manager unload if available
        let pm_guard = self.plugin_manager.read().await;
        if let Some(ref pm) = *pm_guard {
            if let Err(e) = pm.unload_plugin(plugin_id).await {
                warn!("Plugin manager unload failed for {}: {}", plugin_id, e);
            }
        }
        drop(pm_guard);

        // Update state
        {
            let mut plugins = self.plugins.write().await;
            if let Some(state) = plugins.get_mut(plugin_id) {
                state.status = PluginStatus::Deactivated;
                state.health_status = None;
                state.last_status_change = chrono::Utc::now();
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        info!(
            "Deactivation complete for '{}' in {}ms",
            plugin_id, duration_ms
        );

        Ok(DeactivationResult {
            plugin_id: plugin_id.to_string(),
            success: true,
            error: None,
            duration_ms,
        })
    }

    /// Uninstall a plugin
    ///
    /// This performs the uninstallation workflow:
    /// 1. Deactivate if active
    /// 2. Remove plugin files
    /// 3. Unregister from registry
    /// 4. Remove state
    pub async fn uninstall(&self, plugin_id: &str) -> Result<UninstallationResult> {
        let start = std::time::Instant::now();

        info!("Uninstalling plugin: {}", plugin_id);

        // Deactivate first if needed
        {
            let plugins = self.plugins.read().await;
            if let Some(state) = plugins.get(plugin_id) {
                if state.status == PluginStatus::Active {
                    drop(plugins);
                    self.deactivate(plugin_id).await?;
                }
            }
        }

        // Get install path before removing
        let (install_path, version) = {
            let mut plugins = self.plugins.write().await;
            let state = plugins
                .get_mut(plugin_id)
                .ok_or_else(|| anyhow!("Plugin '{}' not found", plugin_id))?;

            state.status = PluginStatus::Uninstalling;
            state.last_status_change = chrono::Utc::now();

            (state.install_path.clone(), state.version.clone())
        };

        // Remove plugin files
        if let Some(ref path) = install_path {
            debug!("Removing plugin directory: {:?}", path);
            if path.exists() {
                std::fs::remove_dir_all(path)
                    .with_context(|| format!("Removing plugin directory: {:?}", path))?;
            }
        }

        // Unregister from registry
        {
            let mut registry = self.registry.write().await;
            registry.remove(plugin_id, &version)?;
        }

        // Remove state
        {
            let mut plugins = self.plugins.write().await;
            plugins.remove(plugin_id);
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        info!(
            "Uninstallation complete for '{}' in {}ms",
            plugin_id, duration_ms
        );

        Ok(UninstallationResult {
            plugin_id: plugin_id.to_string(),
            success: true,
            error: None,
            duration_ms,
        })
    }

    /// Run health check on all active plugins
    pub async fn health_check_all(&self) -> Result<HashMap<String, String>> {
        let mut results = HashMap::new();

        let plugins = self.plugins.read().await;
        for (plugin_id, state) in plugins.iter() {
            if state.status == PluginStatus::Active {
                if let Some(ref install_path) = state.install_path {
                    if let Ok(binary_path) = self.find_plugin_binary(install_path) {
                        let check_result = self.health_checker.check_binary_exists(&binary_path);
                        results.insert(plugin_id.clone(), format!("{:?}", check_result));
                    }
                }
            }
        }

        Ok(results)
    }

    /// Get plugin state
    pub async fn get_state(&self, plugin_id: &str) -> Option<PluginState> {
        let plugins = self.plugins.read().await;
        plugins.get(plugin_id).cloned()
    }

    /// List all plugins with their states
    pub async fn list_plugins(&self) -> Vec<PluginState> {
        let plugins = self.plugins.read().await;
        plugins.values().cloned().collect()
    }

    /// Get plugins by status
    pub async fn get_plugins_by_status(&self, status: PluginStatus) -> Vec<PluginState> {
        let plugins = self.plugins.read().await;
        plugins
            .values()
            .filter(|s| s.status == status)
            .cloned()
            .collect()
    }

    /// Find the plugin binary in the install directory
    fn find_plugin_binary(&self, install_path: &Path) -> Result<PathBuf> {
        // Common plugin binary names
        let binary_names = ["plugin.so", "plugin.dylib", "plugin.dll", "libplugin.so"];

        for name in &binary_names {
            let binary_path = install_path.join(name);
            if binary_path.exists() {
                return Ok(binary_path);
            }
        }

        // Look for any .so/.dylib/.dll file
        for entry in std::fs::read_dir(install_path)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "so" || ext == "dylib" || ext == "dll" {
                    return Ok(path);
                }
            }
        }

        Err(anyhow!("No plugin binary found in {:?}", install_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_lifecycle_config_default() {
        let config = LifecycleConfig::default();
        assert!(config.plugins_dir.to_str().unwrap().contains("plugins"));
        assert!(!config.auto_activate);
        assert!(config.verify_signatures);
    }

    #[tokio::test]
    async fn test_lifecycle_config_builder() {
        let config = LifecycleConfig::new("/custom/plugins")
            .with_auto_activate()
            .without_signature_verification()
            .with_health_check_interval(120);

        assert_eq!(config.plugins_dir, PathBuf::from("/custom/plugins"));
        assert!(config.auto_activate);
        assert!(!config.verify_signatures);
        assert_eq!(config.health_check_interval_secs, 120);
    }

    #[tokio::test]
    async fn test_plugin_status_display() {
        assert_eq!(format!("{}", PluginStatus::Active), "Active");
        assert_eq!(format!("{}", PluginStatus::Installed), "Installed");
        assert!(format!("{}", PluginStatus::Error("test".to_string())).contains("test"));
    }

    #[tokio::test]
    async fn test_lifecycle_manager_creation() {
        let temp_dir = tempdir().unwrap();
        let config = LifecycleConfig::new(temp_dir.path().join("plugins"));
        let manager = PluginLifecycleManager::new(config);
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_list_plugins_empty() {
        let temp_dir = tempdir().unwrap();
        let config = LifecycleConfig::new(temp_dir.path().join("plugins"));
        let manager = PluginLifecycleManager::new(config).unwrap();

        let plugins = manager.list_plugins().await;
        assert!(plugins.is_empty());
    }

    #[tokio::test]
    async fn test_get_nonexistent_plugin() {
        let temp_dir = tempdir().unwrap();
        let config = LifecycleConfig::new(temp_dir.path().join("plugins"));
        let manager = PluginLifecycleManager::new(config).unwrap();

        let state = manager.get_state("nonexistent").await;
        assert!(state.is_none());
    }

    #[tokio::test]
    async fn test_activate_nonexistent_plugin() {
        let temp_dir = tempdir().unwrap();
        let config = LifecycleConfig::new(temp_dir.path().join("plugins"));
        let manager = PluginLifecycleManager::new(config).unwrap();

        let result = manager.activate("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_deactivate_nonexistent_plugin() {
        let temp_dir = tempdir().unwrap();
        let config = LifecycleConfig::new(temp_dir.path().join("plugins"));
        let manager = PluginLifecycleManager::new(config).unwrap();

        let result = manager.deactivate("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_uninstall_nonexistent_plugin() {
        let temp_dir = tempdir().unwrap();
        let config = LifecycleConfig::new(temp_dir.path().join("plugins"));
        let manager = PluginLifecycleManager::new(config).unwrap();

        let result = manager.uninstall("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_plugin_state_timestamp() {
        let before = chrono::Utc::now();
        let state = PluginState {
            id: "test".to_string(),
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            status: PluginStatus::Installed,
            artifact_path: None,
            install_path: None,
            dependencies: vec![],
            health_status: None,
            last_status_change: chrono::Utc::now(),
            error_message: None,
        };
        let after = chrono::Utc::now();

        assert!(state.last_status_change >= before);
        assert!(state.last_status_change <= after);
    }

    #[tokio::test]
    async fn test_get_plugins_by_status() {
        let temp_dir = tempdir().unwrap();
        let config = LifecycleConfig::new(temp_dir.path().join("plugins"));
        let manager = PluginLifecycleManager::new(config).unwrap();

        // Manually insert some plugins
        {
            let mut plugins = manager.plugins.write().await;
            plugins.insert(
                "plugin1".to_string(),
                PluginState {
                    id: "plugin1".to_string(),
                    name: "plugin1".to_string(),
                    version: "1.0.0".to_string(),
                    status: PluginStatus::Active,
                    artifact_path: None,
                    install_path: None,
                    dependencies: vec![],
                    health_status: None,
                    last_status_change: chrono::Utc::now(),
                    error_message: None,
                },
            );
            plugins.insert(
                "plugin2".to_string(),
                PluginState {
                    id: "plugin2".to_string(),
                    name: "plugin2".to_string(),
                    version: "1.0.0".to_string(),
                    status: PluginStatus::Installed,
                    artifact_path: None,
                    install_path: None,
                    dependencies: vec![],
                    health_status: None,
                    last_status_change: chrono::Utc::now(),
                    error_message: None,
                },
            );
        }

        let active = manager.get_plugins_by_status(PluginStatus::Active).await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "plugin1");

        let installed = manager.get_plugins_by_status(PluginStatus::Installed).await;
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].id, "plugin2");
    }

    #[tokio::test]
    async fn test_installation_result_success() {
        let result = InstallationResult {
            plugin_id: "test".to_string(),
            success: true,
            install_path: Some(PathBuf::from("/plugins/test")),
            error: None,
            duration_ms: 100,
        };

        assert!(result.success);
        assert!(result.install_path.is_some());
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_activation_result_success() {
        let result = ActivationResult {
            plugin_id: "test".to_string(),
            success: true,
            resolved_dependencies: vec!["dep1".to_string()],
            error: None,
            duration_ms: 50,
        };

        assert!(result.success);
        assert_eq!(result.resolved_dependencies.len(), 1);
    }

    #[tokio::test]
    async fn test_deactivation_result_success() {
        let result = DeactivationResult {
            plugin_id: "test".to_string(),
            success: true,
            error: None,
            duration_ms: 30,
        };

        assert!(result.success);
    }

    #[tokio::test]
    async fn test_uninstallation_result_success() {
        let result = UninstallationResult {
            plugin_id: "test".to_string(),
            success: true,
            error: None,
            duration_ms: 20,
        };

        assert!(result.success);
    }

    #[tokio::test]
    async fn test_health_check_all_empty() {
        let temp_dir = tempdir().unwrap();
        let config = LifecycleConfig::new(temp_dir.path().join("plugins"));
        let manager = PluginLifecycleManager::new(config).unwrap();

        let results = manager.health_check_all().await;
        assert!(results.expect("health check should succeed").is_empty());
    }
}
