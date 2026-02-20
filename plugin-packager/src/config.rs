// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Configuration management for plugin-packager
//!
//! This module provides a configuration system for plugin-packager that allows
//! users to customize installation paths, registry behavior, and other settings.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Configuration for plugin-packager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default plugin installation directory
    #[serde(default = "Config::default_plugin_dir")]
    pub plugin_dir: PathBuf,

    /// Registry file path
    #[serde(default = "Config::default_registry_file")]
    pub registry_file: PathBuf,

    /// Whether to auto-verify artifacts on install
    #[serde(default = "Config::default_verify_on_install")]
    pub verify_on_install: bool,

    /// Whether to auto-register plugins in local registry
    #[serde(default = "Config::default_auto_register")]
    pub auto_register: bool,

    /// Whether to check dependencies before installation
    #[serde(default = "Config::default_check_dependencies")]
    pub check_dependencies: bool,

    /// Maximum concurrent plugin installations
    #[serde(default = "Config::default_max_concurrent")]
    pub max_concurrent_installs: usize,

    /// Cache directory for downloaded packages
    #[serde(default = "Config::default_cache_dir")]
    pub cache_dir: PathBuf,

    /// Enable verbose logging
    #[serde(default = "Config::default_verbose")]
    pub verbose: bool,

    /// Backup existing plugins before upgrade
    #[serde(default = "Config::default_backup_on_upgrade")]
    pub backup_on_upgrade: bool,
}

impl Config {
    /// Default plugin installation directory
    fn default_plugin_dir() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        home.join(".skylet").join("plugins")
    }

    /// Default registry file path
    fn default_registry_file() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        home.join(".skylet").join("registry.json")
    }

    /// Default verify on install
    fn default_verify_on_install() -> bool {
        true
    }

    /// Default auto register
    fn default_auto_register() -> bool {
        true
    }

    /// Default check dependencies
    fn default_check_dependencies() -> bool {
        false // Can be expensive, opt-in
    }

    /// Default max concurrent installs
    fn default_max_concurrent() -> usize {
        4
    }

    /// Default cache directory
    fn default_cache_dir() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        home.join(".skylet").join("cache")
    }

    /// Default verbose logging
    fn default_verbose() -> bool {
        false
    }

    /// Default backup on upgrade
    fn default_backup_on_upgrade() -> bool {
        true
    }

    /// Load configuration from file, or return default if not found
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if path.exists() {
            Self::load(path)
        } else {
            Ok(Self::default())
        }
    }

    /// Load configuration from file
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).context("reading config file")?;
        let config: Self = toml::from_str(&content).context("parsing config file")?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent();
        if let Some(parent_path) = parent {
            fs::create_dir_all(parent_path).context("creating config directory")?;
        }

        let content = toml::to_string_pretty(self).context("serializing config")?;
        fs::write(path, content).context("writing config file")?;
        Ok(())
    }

    /// Get the configuration file path (creating parent directories if needed)
    pub fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("could not determine home directory")?;
        let config_path = home.join(".skylet").join("config.toml");
        Ok(config_path)
    }

    /// Ensure plugin directory exists
    pub fn ensure_plugin_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.plugin_dir).context("failed to create plugin directory")?;
        Ok(())
    }

    /// Ensure cache directory exists
    pub fn ensure_cache_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.cache_dir).context("failed to create cache directory")?;
        Ok(())
    }

    /// Ensure registry directory exists
    pub fn ensure_registry_dir(&self) -> Result<()> {
        if let Some(parent) = self.registry_file.parent() {
            fs::create_dir_all(parent).context("failed to create registry directory")?;
        }
        Ok(())
    }

    /// Ensure all required directories exist
    pub fn ensure_all_dirs(&self) -> Result<()> {
        self.ensure_plugin_dir()?;
        self.ensure_cache_dir()?;
        self.ensure_registry_dir()?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            plugin_dir: Config::default_plugin_dir(),
            registry_file: Config::default_registry_file(),
            verify_on_install: Config::default_verify_on_install(),
            auto_register: Config::default_auto_register(),
            check_dependencies: Config::default_check_dependencies(),
            max_concurrent_installs: Config::default_max_concurrent(),
            cache_dir: Config::default_cache_dir(),
            verbose: Config::default_verbose(),
            backup_on_upgrade: Config::default_backup_on_upgrade(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.verify_on_install);
        assert!(config.auto_register);
        assert!(!config.check_dependencies);
        assert_eq!(config.max_concurrent_installs, 4);
        assert!(!config.verbose);
        assert!(config.backup_on_upgrade);
    }

    #[test]
    fn test_config_save_and_load() -> Result<()> {
        let temp_dir = tempdir()?;
        let config_path = temp_dir.path().join("config.toml");

        // Create custom config
        let mut config = Config::default();
        config.verbose = true;
        config.max_concurrent_installs = 8;
        config.check_dependencies = true;

        // Save
        config.save(&config_path)?;
        assert!(config_path.exists());

        // Load
        let loaded = Config::load(&config_path)?;
        assert_eq!(loaded.verbose, true);
        assert_eq!(loaded.max_concurrent_installs, 8);
        assert_eq!(loaded.check_dependencies, true);

        Ok(())
    }

    #[test]
    fn test_config_load_or_default() -> Result<()> {
        let temp_dir = tempdir()?;
        let config_path = temp_dir.path().join("nonexistent.toml");

        // Should return default if file doesn't exist
        let config = Config::load_or_default(&config_path)?;
        assert_eq!(config.max_concurrent_installs, 4);

        Ok(())
    }

    #[test]
    fn test_config_ensure_dirs() -> Result<()> {
        let temp_dir = tempdir()?;
        let mut config = Config::default();
        config.plugin_dir = temp_dir.path().join("plugins");
        config.cache_dir = temp_dir.path().join("cache");
        config.registry_file = temp_dir.path().join("registry.json");

        config.ensure_all_dirs()?;

        assert!(config.plugin_dir.exists());
        assert!(config.cache_dir.exists());

        Ok(())
    }

    #[test]
    fn test_config_toml_serialization() -> Result<()> {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config)?;

        // Should contain key settings
        assert!(toml_str.contains("plugin_dir"));
        assert!(toml_str.contains("registry_file"));
        assert!(toml_str.contains("verify_on_install"));

        Ok(())
    }
}
