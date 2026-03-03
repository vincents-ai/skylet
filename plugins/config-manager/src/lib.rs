// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Config Manager Plugin - Configuration loading and management service
//!
//! This plugin provides centralized configuration management including:
//! - Configuration loading from TOML/JSON files
//! - Environment variable overrides
//! - CLI argument parsing
//! - Service registry integration
//!
//! ## Migration to v2 ABI
//!
//! This plugin has been migrated from v1 ABI (manual C string handling, unsafe static mut)
//! to v2 ABI (RFC-0004 Phase 1).
//!
//! ### Changes:
//! - Removed v1 ABI functions (plugin_init, plugin_shutdown, plugin_get_info)
//! - Removed unsafe `static mut CONFIG_SERVICE`
//! - Removed manual C string handling functions (config_load, config_get, etc.)
//! - All plugin ABI functions now implemented in v2_ffi.rs module
//! - ConfigService uses thread-safe Arc<RwLock> pattern
//! - PluginInfoV2 structure with 40+ metadata fields
//! - SafePluginContext for type-safe service access
//! - Uses skylet-plugin-common for RFC-0006 compliant config paths

// Export v2 ABI implementation
mod v2_ffi;
pub use v2_ffi::*;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use skylet_plugin_common::config_paths;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tracing::info;

// ============================================================================
// Configuration Types
// ============================================================================

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: PathBuf,
    pub node_id: u64,
    pub data_dir: PathBuf,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("./data/skylet.db"),
            node_id: 1,
            data_dir: PathBuf::from("./data"),
        }
    }
}

/// Application configuration (main config struct)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub database: DatabaseConfig,
}

// ============================================================================
// Configuration Service
// ============================================================================

/// ConfigService provides configuration management
#[derive(Debug, Clone)]
pub struct ConfigService {
    config: Arc<RwLock<AppConfig>>,
}

impl ConfigService {
    /// Create a new configuration service with defaults
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(AppConfig::default())),
        }
    }

    /// Load configuration from defaults
    pub fn load_defaults() -> Result<Self> {
        info!("Loading default configuration");
        Ok(Self::new())
    }

    /// Load configuration using RFC-0006 compliant config paths
    /// Searches in order: local -> user -> system
    pub fn load_auto() -> Result<Self> {
        if let Some(path) = config_paths::find_config("config-manager") {
            info!("Found configuration at: {:?}", path);
            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("toml");
            match extension {
                "json" => Self::load_from_json(path.to_str().unwrap_or("")),
                "yaml" | "yml" => Self::load_from_yaml(path.to_str().unwrap_or("")),
                _ => Self::load_from_toml(path.to_str().unwrap_or("")),
            }
        } else {
            info!("No configuration file found, using defaults");
            Self::load_defaults()
        }
    }

    /// Load configuration from TOML file
    pub fn load_from_toml(path: &str) -> Result<Self> {
        info!("Loading configuration from TOML file: {}", path);
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read config file: {}", e))?;
        let config: AppConfig =
            toml::from_str(&content).map_err(|e| anyhow!("Failed to parse TOML config: {}", e))?;
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
        })
    }

    /// Load configuration from JSON file
    pub fn load_from_json(path: &str) -> Result<Self> {
        info!("Loading configuration from JSON file: {}", path);
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read config file: {}", e))?;
        let config: AppConfig = serde_json::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse JSON config: {}", e))?;
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
        })
    }

    /// Load configuration from YAML file
    pub fn load_from_yaml(path: &str) -> Result<Self> {
        info!("Loading configuration from YAML file: {}", path);
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read config file: {}", e))?;
        let config: AppConfig = serde_yaml::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse YAML config: {}", e))?;
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
        })
    }

    /// Get current configuration
    pub fn get_config(&self) -> Result<AppConfig> {
        self.config
            .read()
            .map(|lock| lock.clone())
            .map_err(|e| anyhow!("Failed to read config: {}", e))
    }

    /// Update configuration
    pub fn set_config(&self, config: AppConfig) -> Result<()> {
        *self
            .config
            .write()
            .map_err(|e| anyhow!("Failed to write config: {}", e))? = config;
        info!("Configuration updated");
        Ok(())
    }

    /// Get database configuration
    pub fn get_database_config(&self) -> Result<DatabaseConfig> {
        Ok(self.get_config()?.database)
    }

    /// Set database configuration
    pub fn set_database_config(&self, db_config: DatabaseConfig) -> Result<()> {
        let mut config = self.get_config()?;
        config.database = db_config;
        self.set_config(config)?;
        info!("Database configuration updated");
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        let config = self.get_config()?;

        // Validate database config
        if config.database.path.as_os_str().is_empty() {
            return Err(anyhow!("Database path cannot be empty"));
        }
        if config.database.node_id == 0 {
            return Err(anyhow!("Database node_id must be greater than 0"));
        }

        info!("Configuration validation successful");
        Ok(())
    }

    /// Export configuration as TOML
    pub fn export_toml(&self) -> Result<String> {
        let config = self.get_config()?;
        toml::to_string_pretty(&config)
            .map_err(|e| anyhow!("Failed to export config as TOML: {}", e))
    }

    /// Export configuration as JSON
    pub fn export_json(&self) -> Result<String> {
        let config = self.get_config()?;
        serde_json::to_string_pretty(&config)
            .map_err(|e| anyhow!("Failed to export config as JSON: {}", e))
    }

    /// Export configuration as YAML
    pub fn export_yaml(&self) -> Result<String> {
        let config = self.get_config()?;
        serde_yaml::to_string(&config)
            .map_err(|e| anyhow!("Failed to export config as YAML: {}", e))
    }
}

impl Default for ConfigService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.database.node_id, 1);
    }

    #[test]
    fn test_config_service_creation() {
        let service = ConfigService::new();
        let config = service.get_config().unwrap();
        assert_eq!(config.database.node_id, 1);
    }

    #[test]
    fn test_config_validation() {
        let service = ConfigService::new();
        assert!(service.validate().is_ok());
    }

    #[test]
    fn test_config_export_json() {
        let service = ConfigService::new();
        let json = service.export_json().unwrap();
        assert!(!json.is_empty());
        assert!(json.contains("database"));
    }

    #[test]
    fn test_config_export_toml() {
        let service = ConfigService::new();
        let toml = service.export_toml().unwrap();
        assert!(!toml.is_empty());
        assert!(toml.contains("[database]"));
    }

    #[test]
    fn test_database_config_update() {
        let service = ConfigService::new();
        let mut db_config = service.get_database_config().unwrap();
        db_config.node_id = 42;
        service.set_database_config(db_config).unwrap();

        let updated = service.get_database_config().unwrap();
        assert_eq!(updated.node_id, 42);
    }
}
