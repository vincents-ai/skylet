// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Plugin Testing Utilities
//!
//! Provides helper functions to load and initialize plugins during testing.
//! Enables integration tests to verify plugins load correctly with the Skylet instance.

use crate::bootstrap::{load_bootstrap_plugins, BootstrapContext, DynamicPluginLoader};
use crate::config::AppConfig;
use crate::plugin_manager::discovery::{DiscoveryConfig, PluginDiscovery};
use anyhow::Result;
use std::path::PathBuf;
use tracing::info;

/// Configuration for test plugin loading
#[derive(Debug, Clone)]
pub struct TestPluginConfig {
    /// Plugin directory to search for plugins
    pub plugin_dir: PathBuf,
    /// Whether to include debug builds
    pub include_debug: bool,
    /// Patterns to exclude from loading
    pub exclude_patterns: Vec<String>,
    /// Patterns to include in loading
    pub include_patterns: Vec<String>,
}

impl TestPluginConfig {
    /// Create a test config from AppConfig
    pub fn from_app_config(config: &AppConfig) -> Self {
        Self {
            plugin_dir: config.plugins.directory.clone(),
            include_debug: true,
            exclude_patterns: vec!["test_plugin".to_string(), "simple_v2_plugin".to_string()],
            include_patterns: vec![],
        }
    }

    /// Create a test config with custom plugin directory
    pub fn with_plugin_dir(plugin_dir: PathBuf) -> Self {
        Self {
            plugin_dir,
            include_debug: true,
            exclude_patterns: vec!["test_plugin".to_string(), "simple_v2_plugin".to_string()],
            include_patterns: vec![],
        }
    }
}

/// Test plugin loader that handles bootstrap and application plugins
pub struct TestPluginLoader {
    config: TestPluginConfig,
    loader: DynamicPluginLoader,
}

impl TestPluginLoader {
    /// Create a new test plugin loader
    pub fn new(config: TestPluginConfig) -> Self {
        Self {
            config,
            loader: DynamicPluginLoader::new(),
        }
    }

    /// Load all available plugins for testing
    ///
    /// Returns a tuple of (bootstrap_context, loaded_plugins)
    pub fn load_all_plugins(&self) -> Result<(BootstrapContext, Vec<(String, String)>)> {
        // Load bootstrap plugins
        let bootstrap_context = match load_bootstrap_plugins(None) {
            Ok(ctx) => {
                info!("✅ Bootstrap plugins loaded successfully");
                ctx
            }
            Err(e) => {
                info!("⚠️  Some bootstrap plugins failed (non-fatal): {}", e);
                BootstrapContext::new()
            }
        };

        // Discover application plugins
        let discovery_config = DiscoveryConfig {
            search_paths: vec![self.config.plugin_dir.clone()],
            exclude_patterns: self.config.exclude_patterns.clone(),
            include_patterns: self.config.include_patterns.clone(),
            probe_abi_version: true,
            include_debug_builds: self.config.include_debug,
        };

        let discovery = PluginDiscovery::new(discovery_config);
        let app_plugins = match discovery.discover_for_loading() {
            Ok(plugins) => {
                info!("✅ Discovered {} application plugins", plugins.len());
                plugins
            }
            Err(e) => {
                info!("⚠️  Plugin discovery failed: {}", e);
                vec![]
            }
        };

        // Load discovered plugins
        let mut loaded_plugins = Vec::new();
        for (plugin_name, abi_version) in app_plugins {
            match self.loader.load_plugin(&plugin_name) {
                Ok(_) => {
                    info!("✅ Loaded plugin: {} ({})", plugin_name, abi_version);
                    loaded_plugins.push((plugin_name, abi_version));
                }
                Err(e) => {
                    info!("⚠️  Failed to load plugin '{}': {}", plugin_name, e);
                }
            }
        }

        Ok((bootstrap_context, loaded_plugins))
    }

    /// Load specific plugins by name
    pub fn load_plugins(&self, plugin_names: &[&str]) -> Result<Vec<String>> {
        let mut loaded = Vec::new();

        for name in plugin_names {
            match self.loader.load_plugin(name) {
                Ok(_) => {
                    info!("✅ Loaded plugin: {}", name);
                    loaded.push(name.to_string());
                }
                Err(e) => {
                    info!("❌ Failed to load plugin '{}': {}", name, e);
                    return Err(e);
                }
            }
        }

        Ok(loaded)
    }

    /// Load bootstrap plugins only
    pub fn load_bootstrap_only(&self) -> Result<BootstrapContext> {
        let ctx = load_bootstrap_plugins(None)?;
        info!("✅ Bootstrap plugins loaded");
        Ok(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_config_creation() {
        let config = TestPluginConfig::with_plugin_dir(PathBuf::from("./plugins"));
        assert_eq!(config.plugin_dir, PathBuf::from("./plugins"));
        assert!(config.include_debug);
        assert!(!config.exclude_patterns.is_empty());
    }

    #[test]
    fn test_exclude_patterns() {
        let config = TestPluginConfig::with_plugin_dir(PathBuf::from("./plugins"));
        assert!(config.exclude_patterns.contains(&"test_plugin".to_string()));
        assert!(config
            .exclude_patterns
            .contains(&"simple_v2_plugin".to_string()));
    }
}
