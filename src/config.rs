// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]
use anyhow::{anyhow, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: Option<usize>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            workers: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "json".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataConfig {
    pub directory: PathBuf,
}

impl Default for DataConfig {
    fn default() -> Self {
        Self {
            directory: PathBuf::from("./data"),
        }
    }
}

/// Helper function to provide default for probe_abi_version
fn default_probe_abi() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub directory: PathBuf,
    pub bootstrap: Vec<String>,
    /// Patterns to exclude from plugin discovery (supports * wildcard)
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    /// Patterns to include exclusively (if empty, include all non-excluded)
    #[serde(default)]
    pub include_patterns: Vec<String>,
    /// Whether to probe .so files to detect ABI version
    #[serde(default = "default_probe_abi")]
    pub probe_abi_version: bool,
    /// Whether to include debug builds (target/debug) in discovery
    #[serde(default)]
    pub include_debug_builds: bool,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            directory: PathBuf::from("./target/release"),
            bootstrap: vec![
                "config-manager".to_string(),
                "logging".to_string(),
                "registry".to_string(),
                "secrets-manager".to_string(),
            ],
            exclude_patterns: vec![
                "test_plugin".to_string(),
                "simple_v2_plugin".to_string(),
                "skylet_sdk_macros".to_string(), // proc-macro, not a plugin
            ],
            include_patterns: vec![],
            probe_abi_version: true,
            include_debug_builds: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub logging: LoggingConfig,
    pub data: DataConfig,
    pub plugins: PluginConfig,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        Self::load_with_args(ConfigArgs::try_parse_from(vec!["app"]).ok())
    }

    pub fn load_with_args(args: Option<ConfigArgs>) -> Result<Self> {
        let args = args.unwrap_or_default();
        let mut app_config = AppConfig::default();

        // Auto-discover config.toml or config.json from working directory
        for candidate in &["config.toml", "config.json"] {
            let path = std::path::Path::new(candidate);
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("toml");
                    match ext {
                        "toml" => {
                            if let Ok(file_config) = toml::from_str::<AppConfig>(&content) {
                                app_config = file_config;
                                break;
                            }
                        }
                        "json" => {
                            if let Ok(file_config) = serde_json::from_str::<AppConfig>(&content) {
                                app_config = file_config;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Load from environment variables
        if let Ok(host) = std::env::var("SKYLET_SERVER_HOST") {
            app_config.server.host = host;
        }
        if let Ok(port) = std::env::var("SKYLET_SERVER_PORT") {
            if let Ok(port) = port.parse() {
                app_config.server.port = port;
            }
        }

        // Load from CLI arguments
        if let Some(path) = args.config {
            let config_content = std::fs::read_to_string(&path)
                .map_err(|e| anyhow!("Failed to read config file: {}", e))?;

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("toml");

            let override_config: AppConfig = match ext {
                "toml" => toml::from_str(&config_content)
                    .map_err(|e| anyhow!("Failed to parse TOML config: {}", e))?,
                "json" => serde_json::from_str(&config_content)
                    .map_err(|e| anyhow!("Failed to parse JSON config: {}", e))?,
                _ => return Err(anyhow!("Unsupported config format: {}", ext)),
            };

            app_config = override_config;
        }

        if let Some(host) = args.server_host {
            app_config.server.host = host;
        }

        if let Some(port) = args.server_port {
            app_config.server.port = port;
        }

        if let Some(workers) = args.server_workers {
            app_config.server.workers = Some(workers);
        }

        if let Some(path) = args.data_directory {
            app_config.data.directory = path;
        }

        if let Some(plugin_dir) = args.plugins_directory {
            app_config.plugins.directory = plugin_dir;
        }

        Ok(app_config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.server.port == 0 {
            return Err(anyhow!("Server port must be greater than 0"));
        }

        if self.server.host.is_empty() {
            return Err(anyhow!("Server host cannot be empty"));
        }

        Ok(())
    }

    pub fn export_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| anyhow!("Failed to export config as TOML: {}", e))
    }

    pub fn export_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| anyhow!("Failed to export config as JSON: {}", e))
    }
}

#[derive(Debug, Default, Parser)]
pub struct ConfigArgs {
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    #[arg(long)]
    pub server_host: Option<String>,

    #[arg(long)]
    pub server_port: Option<u16>,

    #[arg(long)]
    pub server_workers: Option<usize>,

    #[arg(long)]
    pub data_directory: Option<PathBuf>,

    #[arg(long)]
    pub plugins_directory: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.logging.level, "info");
        assert_eq!(config.data.directory, PathBuf::from("./data"));
        assert!(config.plugins.bootstrap.len() == 4);
    }

    #[test]
    fn test_config_validation() {
        let mut config = AppConfig::default();
        config.server.port = 0;
        assert!(config.validate().is_err());

        let mut config = AppConfig::default();
        config.server.host = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_export_toml() {
        let config = AppConfig::default();
        let toml = config.export_toml().unwrap();
        assert!(toml.contains("[server]"));
        assert!(toml.contains("[logging]"));
    }

    #[test]
    fn test_config_export_json() {
        let config = AppConfig::default();
        let json = config.export_json().unwrap();
        assert!(json.contains("\"server\""));
        assert!(json.contains("\"logging\""));
    }
}
