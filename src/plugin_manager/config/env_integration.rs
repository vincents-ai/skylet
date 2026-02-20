// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(dead_code)] // Infrastructure for env var integration - not yet wired into production

use anyhow::{anyhow, Result};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct EnvVarConfig {
    pub prefix: String,
    pub separator: String,
    #[allow(dead_code)] // Builder pattern field — not yet used in config loading
    pub overwrite_files: bool,
    pub mappings: HashMap<String, String>,
}

impl Default for EnvVarConfig {
    fn default() -> Self {
        Self {
            prefix: "SKYLET_".to_string(),
            separator: "_".to_string(),
            overwrite_files: false,
            mappings: HashMap::new(),
        }
    }
}

#[allow(dead_code)] // Future: env var integration not yet wired into config loading
impl EnvVarConfig {
    pub fn new(prefix: String) -> Self {
        Self {
            prefix,
            ..Default::default()
        }
    }

    #[allow(dead_code)] // Public API — not yet called from production code
    pub fn with_prefix(mut self, prefix: String) -> Self {
        self.prefix = prefix;
        self
    }

    #[allow(dead_code)] // Public API — not yet called from production code
    pub fn with_separator(mut self, separator: String) -> Self {
        self.separator = separator;
        self
    }

    #[allow(dead_code)] // Public API — not yet called from production code
    pub fn with_overwrite_files(mut self, overwrite: bool) -> Self {
        self.overwrite_files = overwrite;
        self
    }

    pub fn add_mapping(&mut self, key: String, env_var: String) {
        self.mappings.insert(key, env_var);
    }

    #[allow(dead_code)] // Public API — not yet called from production code
    pub fn get_env_value(&self, plugin_name: &str, config_key: &str) -> Option<String> {
        if let Some(mapped) = self.mappings.get(config_key) {
            return std::env::var(mapped).ok();
        }

        let env_key = format!(
            "{}{}{}{}{}",
            self.prefix,
            plugin_name,
            self.separator,
            config_key.replace(".", &self.separator),
            ""
        );

        std::env::var(&env_key)
            .or_else(|_| {
                let alt_key = format!(
                    "{}{}",
                    self.prefix,
                    config_key.replace(".", &self.separator)
                );
                std::env::var(&alt_key)
            })
            .ok()
    }

    pub fn load_all_env_vars(&self, plugin_name: &str) -> HashMap<String, String> {
        let mut values = HashMap::new();

        let plugin_prefix = format!("{}{}{}", self.prefix, plugin_name, self.separator);

        for (key, value) in std::env::vars() {
            if key.starts_with(&plugin_prefix) {
                let config_key = key
                    .strip_prefix(&plugin_prefix)
                    .unwrap()
                    .replace(&self.separator, ".");
                values.insert(config_key, value);
            }
        }

        for (config_key, env_var) in &self.mappings {
            if let Ok(value) = std::env::var(env_var) {
                values.insert(config_key.clone(), value);
            }
        }

        values
    }
}

pub struct EnvVarIntegrator {
    config: EnvVarConfig,
}

impl EnvVarIntegrator {
    pub fn new(config: EnvVarConfig) -> Self {
        Self { config }
    }

    pub fn with_prefix(prefix: String) -> Self {
        Self::new(EnvVarConfig::new(prefix))
    }

    pub fn merge_into_config(
        &self,
        plugin_name: &str,
        mut config_json: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let env_values = self.config.load_all_env_vars(plugin_name);

        if env_values.is_empty() {
            return Ok(config_json);
        }

        if !config_json.is_object() {
            return Err(anyhow!("Config must be a JSON object"));
        }

        let config_obj = config_json.as_object_mut().unwrap();

        for (key, value) in env_values {
            self.set_nested_value(config_obj, &key, &value);
        }

        Ok(config_json)
    }

    fn set_nested_value(
        &self,
        obj: &mut serde_json::Map<String, serde_json::Value>,
        key: &str,
        value: &str,
    ) {
        let parts: Vec<&str> = key.split('.').collect();

        if parts.len() == 1 {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(value) {
                obj.insert(parts[0].to_string(), parsed);
            } else {
                obj.insert(
                    parts[0].to_string(),
                    serde_json::Value::String(value.to_string()),
                );
            }
            return;
        }

        let current = parts[0];
        let remaining = &parts[1..].join(".");

        if !obj.contains_key(current) {
            obj.insert(
                current.to_string(),
                serde_json::Value::Object(serde_json::Map::new()),
            );
        }

        if let Some(nested) = obj.get_mut(current).and_then(|v| v.as_object_mut()) {
            self.set_nested_value(nested, remaining, value);
        }
    }

    #[allow(dead_code)] // Public API — not yet called from production code
    pub fn get_config_value(&self, plugin_name: &str, config_key: &str) -> Option<String> {
        self.config.get_env_value(plugin_name, config_key)
    }

    #[allow(dead_code)] // Public API — not yet called from production code
    pub fn list_env_keys_for_plugin(&self, plugin_name: &str) -> Vec<String> {
        let plugin_prefix = format!(
            "{}{}{}",
            self.config.prefix, plugin_name, self.config.separator
        );
        let mut keys = Vec::new();

        for (key, _) in std::env::vars() {
            if key.starts_with(&plugin_prefix) {
                keys.push(key);
            }
        }

        for (_, env_var) in &self.config.mappings {
            keys.push(env_var.clone());
        }

        keys.sort();
        keys.dedup();
        keys
    }
}

#[derive(Debug, Clone)]
pub struct EnvVarReference {
    pub key: String,
    pub default: Option<String>,
    pub required: bool,
}

#[allow(dead_code)] // Public API — not yet called from production code
impl EnvVarReference {
    pub fn new(key: String) -> Self {
        Self {
            key,
            default: None,
            required: false,
        }
    }

    pub fn with_default(mut self, default: String) -> Self {
        self.default = Some(default);
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

pub fn parse_env_refs(config_str: &str) -> Vec<EnvVarReference> {
    let mut refs = Vec::new();

    let re = regex::Regex::new(r"\$\{env:([^}:]+)(?::([^}]*))?\}").unwrap();

    for caps in re.captures_iter(config_str) {
        let key = caps.get(1).unwrap().as_str().to_string();
        let default = caps.get(2).map(|m| m.as_str().to_string());
        let required = default.is_none();

        refs.push(EnvVarReference {
            key,
            default,
            required,
        });
    }

    refs
}

pub fn resolve_env_refs(config_str: &str) -> Result<String> {
    let refs = parse_env_refs(config_str);
    let mut result = config_str.to_string();

    for env_ref in refs {
        let value = std::env::var(&env_ref.key).or_else(|_| {
            env_ref
                .default
                .clone()
                .ok_or_else(|| anyhow!("Required environment variable '{}' not found", env_ref.key))
        })?;

        let pattern = format!("\\$\\{{env:{}(?::[^}}]*)?\\}}", regex::escape(&env_ref.key));
        let re = regex::Regex::new(&pattern).unwrap();
        result = re.replace_all(&result, &value).to_string();
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_var_config() {
        let config = EnvVarConfig::new("TEST_".to_string());
        assert_eq!(config.prefix, "TEST_");

        let mut config = config;
        config.add_mapping("api_key".to_string(), "MY_API_KEY".to_string());
        assert_eq!(config.mappings.len(), 1);
    }

    #[test]
    fn test_env_var_integrator() {
        std::env::set_var("TEST_MYPLUGIN_APIKEY", "secret123");
        std::env::set_var("TEST_MYPLUGIN_HOST", "localhost");

        let integrator = EnvVarIntegrator::with_prefix("TEST_".to_string());

        let config_json = serde_json::json!({
            "APIKEY": "from_file",
            "HOST": "file_host"
        });

        let merged = integrator
            .merge_into_config("MYPLUGIN", config_json)
            .unwrap();

        assert_eq!(
            merged.get("APIKEY").and_then(|v| v.as_str()),
            Some("secret123")
        );
        assert_eq!(
            merged.get("HOST").and_then(|v| v.as_str()),
            Some("localhost")
        );

        std::env::remove_var("TEST_MYPLUGIN_APIKEY");
        std::env::remove_var("TEST_MYPLUGIN_HOST");
    }

    #[test]
    fn test_parse_env_refs() {
        let config = r#"{
            "url": "${env:API_URL:http://localhost}",
            "token": "${env:API_TOKEN}",
            "nested": {
                "value": "${env:NESTED_VAR:default}"
            }
        }"#;

        let refs = parse_env_refs(config);
        assert_eq!(refs.len(), 3);

        assert_eq!(refs[0].key, "API_URL");
        assert_eq!(refs[0].default, Some("http://localhost".to_string()));
        assert!(!refs[0].required);

        assert_eq!(refs[1].key, "API_TOKEN");
        assert_eq!(refs[1].default, None);
        assert!(refs[1].required);
    }

    #[test]
    fn test_resolve_env_refs() {
        std::env::set_var("TEST_API_URL", "https://api.example.com");

        let config = r#"{
            "url": "${env:TEST_API_URL}",
            "token": "${env:TEST_API_TOKEN:default_token}"
        }"#;

        let resolved = resolve_env_refs(config).unwrap();

        assert!(resolved.contains("https://api.example.com"));
        assert!(resolved.contains("default_token"));

        std::env::remove_var("TEST_API_URL");
    }

    #[test]
    fn test_nested_values() {
        let integrator = EnvVarIntegrator::with_prefix("TEST_".to_string());

        let mut config_json = serde_json::json!({});
        let config_obj = config_json.as_object_mut().unwrap();

        integrator.set_nested_value(config_obj, "database.host", "localhost");
        integrator.set_nested_value(config_obj, "database.port", "5432");
        integrator.set_nested_value(config_obj, "simple_key", "value");

        assert_eq!(
            config_json.get("database").unwrap().get("host").unwrap(),
            "localhost"
        );
        assert_eq!(
            config_json.get("database").unwrap().get("port").unwrap(),
            &serde_json::json!(5432)
        );
        assert_eq!(config_json.get("simple_key").unwrap(), "value");
    }
}
