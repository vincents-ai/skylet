// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::ConfigEnvironment;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    pub environment: ConfigEnvironment,
    pub overrides: HashMap<String, String>,
    pub env_specific_paths: HashMap<ConfigEnvironment, PathBuf>,
    pub default_values: HashMap<String, serde_json::Value>,
    pub enabled_features: Vec<String>,
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self {
            environment: ConfigEnvironment::Development,
            overrides: HashMap::new(),
            env_specific_paths: HashMap::new(),
            default_values: HashMap::new(),
            enabled_features: Vec::new(),
        }
    }
}

#[allow(dead_code)] // Phase 2 config infrastructure — not yet wired up
impl EnvironmentConfig {
    pub fn new(environment: ConfigEnvironment) -> Self {
        Self {
            environment,
            ..Default::default()
        }
    }

    #[allow(dead_code)] // Phase 2 config infrastructure — not yet wired up
    pub fn with_environment(mut self, env: ConfigEnvironment) -> Self {
        self.environment = env;
        self
    }

    pub fn with_base_dir(mut self, base_dir: PathBuf) -> Self {
        self.env_specific_paths
            .insert(ConfigEnvironment::Development, base_dir.join("dev"));
        self.env_specific_paths
            .insert(ConfigEnvironment::Staging, base_dir.join("staging"));
        self.env_specific_paths
            .insert(ConfigEnvironment::Production, base_dir.join("prod"));
        self
    }

    pub fn add_override(&mut self, key: String, value: String) {
        self.overrides.insert(key, value);
    }

    #[allow(dead_code)] // Phase 2 config infrastructure — not yet wired up
    pub fn add_default_value(&mut self, key: String, value: serde_json::Value) {
        self.default_values.insert(key, value);
    }

    pub fn enable_feature(&mut self, feature: String) {
        if !self.enabled_features.contains(&feature) {
            self.enabled_features.push(feature);
        }
    }

    pub fn disable_feature(&mut self, feature: &str) {
        self.enabled_features.retain(|f| f != feature);
    }

    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        self.enabled_features.contains(&feature.to_string())
    }

    #[allow(dead_code)] // Phase 2 config infrastructure — not yet wired up
    pub fn get_config_path(&self, plugin_name: &str) -> PathBuf {
        if let Some(path) = self.env_specific_paths.get(&self.environment) {
            path.join(format!("{}.toml", plugin_name))
        } else {
            PathBuf::from(format!("{}.toml", plugin_name))
        }
    }

    pub fn get_override(&self, key: &str) -> Option<String> {
        let env_prefix = format!("{}.", self.environment.as_str());
        self.overrides
            .get(&format!("{}{}", env_prefix, key))
            .or_else(|| self.overrides.get(key))
            .cloned()
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Phase 2 config infrastructure — not yet wired up
pub struct MultiEnvConfigManager {
    base_dir: PathBuf,
    environments: HashMap<ConfigEnvironment, EnvironmentConfig>,
    current_environment: ConfigEnvironment,
}

#[allow(dead_code)] // Phase 2 config infrastructure — not yet wired up
impl MultiEnvConfigManager {
    pub fn new(base_dir: PathBuf) -> Self {
        let mut manager = Self {
            base_dir: base_dir.clone(),
            environments: HashMap::new(),
            current_environment: ConfigEnvironment::Development,
        };

        for env in [
            ConfigEnvironment::Development,
            ConfigEnvironment::Staging,
            ConfigEnvironment::Production,
        ] {
            let env_config = EnvironmentConfig::new(env).with_base_dir(base_dir.clone());
            manager.environments.insert(env, env_config);
        }

        manager
    }

    pub fn with_environment(mut self, env: ConfigEnvironment) -> Result<Self> {
        if !self.environments.contains_key(&env) {
            return Err(anyhow!("Environment {:?} not configured", env));
        }
        self.current_environment = env;
        Ok(self)
    }

    pub fn current_environment(&self) -> ConfigEnvironment {
        self.current_environment
    }

    pub fn switch_environment(&mut self, env: ConfigEnvironment) -> Result<()> {
        if !self.environments.contains_key(&env) {
            return Err(anyhow!("Environment {:?} not configured", env));
        }
        self.current_environment = env;
        Ok(())
    }

    pub fn get_env_config(&self, env: ConfigEnvironment) -> Option<&EnvironmentConfig> {
        self.environments.get(&env)
    }

    pub fn get_current_env_config(&self) -> Option<&EnvironmentConfig> {
        self.environments.get(&self.current_environment)
    }

    pub fn get_current_env_config_mut(&mut self) -> Option<&mut EnvironmentConfig> {
        self.environments.get_mut(&self.current_environment)
    }

    pub async fn load_plugin_config(&self, plugin_name: &str) -> Result<serde_json::Value> {
        let env_config = self
            .get_current_env_config()
            .ok_or_else(|| anyhow!("No environment configured"))?;

        let config_path = env_config.get_config_path(plugin_name);

        if !config_path.exists() {
            let base_config = self.base_dir.join(format!("{}.toml", plugin_name));
            if base_config.exists() {
                return Self::load_config_file(&base_config).await;
            }
            return Ok(serde_json::json!({}));
        }

        let mut config = Self::load_config_file(&config_path).await?;

        if let Some(obj) = config.as_object_mut() {
            for (key, value) in &env_config.default_values {
                if !obj.contains_key(key) {
                    obj.insert(key.clone(), value.clone());
                }
            }

            for (key, override_value) in &env_config.overrides {
                if let Some(stripped_key) =
                    key.strip_prefix(&format!("{}.", env_config.environment.as_str()))
                {
                    obj.insert(
                        stripped_key.to_string(),
                        serde_json::Value::String(override_value.clone()),
                    );
                }
            }
        }

        Ok(config)
    }

    async fn load_config_file(path: &PathBuf) -> Result<serde_json::Value> {
        let content = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read config file: {:?}", path))?;

        if path.extension().map(|e| e == "toml").unwrap_or(false) {
            let toml_value: toml::Value = toml::from_str(&content)
                .with_context(|| format!("Failed to parse TOML config: {:?}", path))?;
            Ok(Self::toml_to_json(toml_value))
        } else if path.extension().map(|e| e == "json").unwrap_or(false) {
            Ok(serde_json::from_str(&content)?)
        } else {
            Err(anyhow::anyhow!(
                "Unsupported config file format: {:?}",
                path
            ))
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
                serde_json::Value::Array(arr.into_iter().map(Self::toml_to_json).collect())
            }
            toml::Value::Table(table) => serde_json::Value::Object(
                table
                    .into_iter()
                    .map(|(k, v)| (k, Self::toml_to_json(v)))
                    .collect(),
            ),
            toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
        }
    }

    pub fn compare_env_configs(
        &self,
        plugin_name: &str,
    ) -> HashMap<ConfigEnvironment, serde_json::Value> {
        let mut configs = HashMap::new();

        for (env, env_config) in &self.environments {
            let config_path = env_config.get_config_path(plugin_name);
            let base_config = self.base_dir.join(format!("{}.toml", plugin_name));

            if config_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_path) {
                    if let Ok(toml_value) = toml::from_str::<toml::Value>(&content) {
                        configs.insert(*env, Self::toml_to_json(toml_value));
                    }
                }
            } else if base_config.exists() {
                if let Ok(content) = std::fs::read_to_string(&base_config) {
                    if let Ok(toml_value) = toml::from_str::<toml::Value>(&content) {
                        configs.insert(*env, Self::toml_to_json(toml_value));
                    }
                }
            } else {
                configs.insert(*env, serde_json::json!({}));
            }
        }

        configs
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)] // Phase 2 config infrastructure — not yet wired up
pub struct EnvironmentComparison {
    pub plugin_name: String,
    pub environments: HashMap<String, serde_json::Value>,
    pub differences: Vec<ConfigDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)] // Phase 2 config infrastructure — not yet wired up
pub struct ConfigDiff {
    pub key: String,
    pub env1: String,
    pub env2: String,
    pub value1: serde_json::Value,
    pub value2: serde_json::Value,
}

#[allow(dead_code)] // Phase 2 config infrastructure — not yet wired up
impl MultiEnvConfigManager {
    pub fn compare_configs_for_plugin(&self, plugin_name: &str) -> EnvironmentComparison {
        let configs = self.compare_env_configs(plugin_name);
        let mut differences = Vec::new();

        let envs: Vec<ConfigEnvironment> = configs.keys().cloned().collect();

        for i in 0..envs.len() {
            for j in (i + 1)..envs.len() {
                let env1 = &envs[i];
                let env2 = &envs[j];

                if let Some(config1) = configs.get(env1) {
                    if let Some(config2) = configs.get(env2) {
                        if let Some(obj1) = config1.as_object() {
                            if let Some(obj2) = config2.as_object() {
                                for (key, value1) in obj1 {
                                    if let Some(value2) = obj2.get(key) {
                                        if value1 != value2 {
                                            differences.push(ConfigDiff {
                                                key: key.clone(),
                                                env1: env1.as_str().to_string(),
                                                env2: env2.as_str().to_string(),
                                                value1: value1.clone(),
                                                value2: value2.clone(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        EnvironmentComparison {
            plugin_name: plugin_name.to_string(),
            environments: configs
                .into_iter()
                .map(|(k, v)| (k.as_str().to_string(), v))
                .collect(),
            differences,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_config() {
        let config = EnvironmentConfig::new(ConfigEnvironment::Development);
        assert_eq!(config.environment, ConfigEnvironment::Development);

        let mut config = config;
        config.enable_feature("test_feature".to_string());
        assert!(config.is_feature_enabled("test_feature"));

        config.disable_feature("test_feature");
        assert!(!config.is_feature_enabled("test_feature"));
    }

    #[test]
    fn test_multi_env_config_manager() {
        let base_dir = PathBuf::from("/tmp/test");
        let manager = MultiEnvConfigManager::new(base_dir);

        assert_eq!(
            manager.current_environment(),
            ConfigEnvironment::Development
        );

        assert!(manager
            .environments
            .contains_key(&ConfigEnvironment::Development));
        assert!(manager
            .environments
            .contains_key(&ConfigEnvironment::Staging));
        assert!(manager
            .environments
            .contains_key(&ConfigEnvironment::Production));
    }

    #[test]
    fn test_switch_environment() {
        let base_dir = PathBuf::from("/tmp/test");
        let mut manager = MultiEnvConfigManager::new(base_dir);

        let result = manager.switch_environment(ConfigEnvironment::Production);
        assert!(result.is_ok());
        assert_eq!(manager.current_environment(), ConfigEnvironment::Production);
    }

    #[test]
    fn test_config_override() {
        let mut config = EnvironmentConfig::new(ConfigEnvironment::Production);

        config.add_override(
            "api_url".to_string(),
            "https://api.prod.example.com".to_string(),
        );
        config.add_override("production.api_key".to_string(), "prod_secret".to_string());

        assert_eq!(
            config.get_override("api_url"),
            Some("https://api.prod.example.com".to_string())
        );
        assert_eq!(
            config.get_override("api_key"),
            Some("prod_secret".to_string())
        );
    }
}
