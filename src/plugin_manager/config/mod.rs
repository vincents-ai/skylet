// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Advanced Configuration Management Module
//!
//! Provides comprehensive plugin configuration management with:
//! - Advanced schema validation with custom validators
//! - Environment variable integration
//! - Configuration hot-reload with file watching
//! - Multi-environment support (dev, staging, prod)
//! - Configuration migration and versioning

pub mod env_integration;
pub mod hot_reload;
pub mod multi_env;
pub mod schema;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration environment types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfigEnvironment {
    Development,
    Staging,
    Production,
}

impl ConfigEnvironment {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "dev" | "development" => Some(ConfigEnvironment::Development),
            "staging" => Some(ConfigEnvironment::Staging),
            "prod" | "production" => Some(ConfigEnvironment::Production),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ConfigEnvironment::Development => "development",
            ConfigEnvironment::Staging => "staging",
            ConfigEnvironment::Production => "production",
        }
    }
}

impl Default for ConfigEnvironment {
    fn default() -> Self {
        ConfigEnvironment::Development
    }
}

/// Advanced configuration backend with multi-environment support
pub struct AdvancedConfigBackend {
    base_dir: PathBuf,
    environment: ConfigEnvironment,
    configs: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    schema_validators: Arc<RwLock<HashMap<String, schema::SchemaValidator>>>,
    env_prefix: String,
}

impl AdvancedConfigBackend {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            environment: ConfigEnvironment::default(),
            configs: Arc::new(RwLock::new(HashMap::new())),
            schema_validators: Arc::new(RwLock::new(HashMap::new())),
            env_prefix: "SKYLET_".to_string(),
        }
    }

    pub fn with_environment(mut self, env: ConfigEnvironment) -> Self {
        self.environment = env;
        self
    }

    pub fn with_env_prefix(mut self, prefix: String) -> Self {
        self.env_prefix = prefix;
        self
    }

    pub async fn load_plugin_config(&self, plugin_name: &str) -> Result<serde_json::Value> {
        let mut configs = self.configs.write().await;

        if configs.contains_key(plugin_name) {
            return Ok(configs.get(plugin_name).cloned().unwrap());
        }

        let config_path = self.get_config_path(plugin_name)?;
        let config = self.load_config_file(&config_path).await?;

        configs.insert(plugin_name.to_string(), config.clone());
        Ok(config)
    }

    pub async fn get_config_value(&self, plugin_name: &str, key: &str) -> Result<Option<String>> {
        let configs = self.configs.read().await;

        if let Some(config) = configs.get(plugin_name) {
            if let Some(value) = config.get(key) {
                return Ok(Some(serde_json::to_string(value)?));
            }
        }

        Ok(None)
    }

    pub async fn set_config_value(&self, plugin_name: &str, key: &str, value: &str) -> Result<()> {
        let mut configs = self.configs.write().await;

        let config = configs.entry(plugin_name.to_string()).or_insert_with(|| {
            serde_json::json!({})
        });

        let json_value: serde_json::Value = serde_json::from_str(value)?;
        config[key] = json_value;

        Ok(())
    }

    pub async fn reload_plugin_config(&self, plugin_name: &str) -> Result<()> {
        let config_path = self.get_config_path(plugin_name)?;
        let config = self.load_config_file(&config_path).await?;

        let mut configs = self.configs.write().await;
        configs.insert(plugin_name.to_string(), config);

        Ok(())
    }

    pub async fn validate_plugin_config(
        &self,
        plugin_name: &str,
        schema_json: &str,
    ) -> Result<schema::ValidationResult> {
        let config = self.load_plugin_config(plugin_name).await?;

        let validators = self.schema_validators.read().await;
        if let Some(validator) = validators.get(plugin_name) {
            return validator.validate(&serde_json::to_string(&config)?);
        }

        let validator = schema::SchemaValidator::from_json(schema_json)?;
        drop(validators);

        let mut validators = self.schema_validators.write().await;
        validators.insert(plugin_name.to_string(), validator.clone());

        validator.validate(&serde_json::to_string(&config)?)
    }

    async fn load_config_file(&self, path: &PathBuf) -> Result<serde_json::Value> {
        if !path.exists() {
            return Ok(serde_json::json!({}));
        }

        let content = tokio::fs::read_to_string(path).await
            .with_context(|| format!("Failed to read config file: {:?}", path))?;

        if path.extension().map(|e| e == "toml").unwrap_or(false) {
            let toml_value: toml::Value = toml::from_str(&content)
                .with_context(|| format!("Failed to parse TOML config: {:?}", path))?;
            Ok(self::toml_to_json(toml_value))
        } else if path.extension().map(|e| e == "json").unwrap_or(false) {
            Ok(serde_json::from_str(&content)?)
        } else {
            Err(anyhow::anyhow!("Unsupported config file format: {:?}", path))
        }
    }

    fn get_config_path(&self, plugin_name: &str) -> Result<PathBuf> {
        Ok(self.base_dir.join(format!("{}.toml", plugin_name)))
    }

    pub fn environment(&self) -> ConfigEnvironment {
        self.environment
    }

    pub fn env_prefix(&self) -> &str {
        &self.env_prefix
    }
}

fn toml_to_json(toml: toml::Value) -> serde_json::Value {
    match toml {
        toml::Value::String(s) => serde_json::Value::String(s),
        toml::Value::Integer(i) => serde_json::Value::Number(i.into()),
        toml::Value::Float(f) => {
            if let Some(n) = serde_json::Number::from_f64(f) {
                serde_json::Value::Number(n)
            } else {
                serde_json::Value::Null
            }
        }
        toml::Value::Boolean(b) => serde_json::Value::Bool(b),
        toml::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(toml_to_json).collect())
        }
        toml::Value::Table(table) => serde_json::Value::Object(
            table
                .into_iter()
                .map(|(k, v)| (k, toml_to_json(v)))
                .collect(),
        ),
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_environment_from_str() {
        assert_eq!(
            ConfigEnvironment::from_str("dev"),
            Some(ConfigEnvironment::Development)
        );
        assert_eq!(
            ConfigEnvironment::from_str("development"),
            Some(ConfigEnvironment::Development)
        );
        assert_eq!(
            ConfigEnvironment::from_str("staging"),
            Some(ConfigEnvironment::Staging)
        );
        assert_eq!(
            ConfigEnvironment::from_str("prod"),
            Some(ConfigEnvironment::Production)
        );
        assert_eq!(
            ConfigEnvironment::from_str("production"),
            Some(ConfigEnvironment::Production)
        );
        assert_eq!(ConfigEnvironment::from_str("invalid"), None);
    }

    #[test]
    fn test_config_environment_as_str() {
        assert_eq!(ConfigEnvironment::Development.as_str(), "development");
        assert_eq!(ConfigEnvironment::Staging.as_str(), "staging");
        assert_eq!(ConfigEnvironment::Production.as_str(), "production");
    }
}
