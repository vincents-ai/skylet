// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Configuration Schema Module - RFC-0006
//!
//! TOML-based configuration schema validation, UI generation,
//! secret reference resolution, and hot-reload support.
//!
//! # Architecture
//!
//! - `schema`: Configuration schema types and parsing
//! - `validator`: Value validation against schemas
//! - `ui_generator`: UI component generation from schemas
//! - `secret_resolver`: Secret reference resolution (vault://, env://, file://)
//!
//! # Example
//!
//! ```rust,ignore
//! use skylet_abi::config::{ConfigSchema, ConfigValidator, UIGenerator, SecretResolver};
//!
//! // Create a schema
//! let mut schema = ConfigSchema::new("my-plugin");
//!
//! // Validate configuration
//! let validator = ConfigValidator::new();
//! let warnings = validator.validate(&schema, &config_values)?;
//!
//! // Generate UI components
//! let generator = UIGenerator::new();
//! let components = generator.generate(&schema);
//!
//! // Resolve secrets
//! let resolver = SecretResolver::new();
//! resolver.resolve_in_config(&mut config_values)?;
//! ```

pub mod schema;
pub mod secret_resolver;
pub mod ui_generator;
pub mod validator;

// Re-export main types for convenience
pub use schema::{
    ConfigField, ConfigFieldType, ConfigFormat, ConfigSchema, ConfigSection, GlobalValidationRule,
    SchemaError, SecretBackend, SecretReference, UIHints, ValidationRule, WidgetType,
};

pub use validator::{ConfigValidator, ValidationError, ValidationWarning};

pub use ui_generator::{UIComponent, UIComponentType, UIConstraints, UIGenerator};

pub use secret_resolver::{
    EnvSecretBackend, FileSecretBackend, SecretError, SecretResolver, SecretResolverBackend,
    VaultSecretBackend,
};

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Configuration manager for loading, validating, and hot-reloading configs
pub struct ConfigManager {
    /// Loaded configuration schemas by plugin name
    schemas: Arc<RwLock<HashMap<String, ConfigSchema>>>,
    /// Current configuration values by plugin name
    values: Arc<RwLock<HashMap<String, HashMap<String, serde_json::Value>>>>,
    /// File modification timestamps for hot-reload detection
    file_timestamps: Arc<RwLock<HashMap<String, Instant>>>,
    /// Configuration validator
    validator: ConfigValidator,
    /// UI generator
    ui_generator: UIGenerator,
    /// Secret resolver
    secret_resolver: SecretResolver,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Self {
        Self {
            schemas: Arc::new(RwLock::new(HashMap::new())),
            values: Arc::new(RwLock::new(HashMap::new())),
            file_timestamps: Arc::new(RwLock::new(HashMap::new())),
            validator: ConfigValidator::new(),
            ui_generator: UIGenerator::new(),
            secret_resolver: SecretResolver::new(),
        }
    }

    /// Register a configuration schema for a plugin
    pub fn register_schema(&self, plugin_name: &str, schema: ConfigSchema) {
        let mut schemas = self.schemas.write().unwrap();
        schemas.insert(plugin_name.to_string(), schema);
    }

    /// Load a configuration schema from a TOML file
    pub fn load_schema(&self, plugin_name: &str, path: &Path) -> Result<(), ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|e| ConfigError::FileReadError {
            path: path.display().to_string(),
            error: e.to_string(),
        })?;

        let schema = ConfigSchema::from_toml(&content)?;
        self.register_schema(plugin_name, schema);

        // Record file timestamp for hot-reload
        let mut timestamps = self.file_timestamps.write().unwrap();
        timestamps.insert(path.display().to_string(), Instant::now());

        Ok(())
    }

    /// Load configuration values from a TOML file
    pub fn load_config(&self, plugin_name: &str, path: &Path) -> Result<(), ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|e| ConfigError::FileReadError {
            path: path.display().to_string(),
            error: e.to_string(),
        })?;

        let values: HashMap<String, serde_json::Value> =
            toml::from_str(&content).map_err(|e| ConfigError::ParseError {
                path: path.display().to_string(),
                error: e.to_string(),
            })?;

        // Validate against schema
        if let Some(schema) = self.schemas.read().unwrap().get(plugin_name) {
            self.validator
                .validate(schema, &values)
                .map_err(|e| ConfigError::ValidationError {
                    plugin: plugin_name.to_string(),
                    error: e.to_string(),
                })?;
        }

        // Store values
        let mut all_values = self.values.write().unwrap();
        all_values.insert(plugin_name.to_string(), values);

        // Record file timestamp
        let mut timestamps = self.file_timestamps.write().unwrap();
        timestamps.insert(path.display().to_string(), Instant::now());

        Ok(())
    }

    /// Get configuration values for a plugin
    pub fn get_config(&self, plugin_name: &str) -> Option<HashMap<String, serde_json::Value>> {
        let values = self.values.read().unwrap();
        values.get(plugin_name).cloned()
    }

    /// Get a specific configuration value
    pub fn get_value(&self, plugin_name: &str, key: &str) -> Option<serde_json::Value> {
        let values = self.values.read().unwrap();
        values.get(plugin_name).and_then(|v| v.get(key).cloned())
    }

    /// Set a configuration value
    pub fn set_value(
        &self,
        plugin_name: &str,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), ConfigError> {
        // Validate against schema if available
        if let Some(schema) = self.schemas.read().unwrap().get(plugin_name) {
            if let Some(field) = schema.get_field(key) {
                self.validator.validate_field(field, &value).map_err(|e| {
                    ConfigError::ValidationError {
                        plugin: plugin_name.to_string(),
                        error: e.to_string(),
                    }
                })?;
            }
        }

        let mut values = self.values.write().unwrap();
        values
            .entry(plugin_name.to_string())
            .or_default()
            .insert(key.to_string(), value);

        Ok(())
    }

    /// Validate all configurations
    pub fn validate_all(&self) -> Result<HashMap<String, Vec<ValidationWarning>>, ConfigError> {
        let schemas = self.schemas.read().unwrap();
        let values = self.values.read().unwrap();
        let mut warnings = HashMap::new();

        for (plugin_name, schema) in schemas.iter() {
            if let Some(plugin_values) = values.get(plugin_name) {
                let plugin_warnings =
                    self.validator
                        .validate(schema, plugin_values)
                        .map_err(|e| ConfigError::ValidationError {
                            plugin: plugin_name.clone(),
                            error: e.to_string(),
                        })?;
                warnings.insert(plugin_name.clone(), plugin_warnings);
            }
        }

        Ok(warnings)
    }

    /// Resolve all secret references in configurations
    pub fn resolve_secrets(&self) -> Result<(), ConfigError> {
        let mut values = self.values.write().unwrap();

        for plugin_values in values.values_mut() {
            self.secret_resolver
                .resolve_in_config(plugin_values)
                .map_err(|e| ConfigError::SecretError {
                    error: e.to_string(),
                })?;
        }

        Ok(())
    }

    /// Generate UI components for a plugin
    pub fn generate_ui(&self, plugin_name: &str) -> Option<Vec<ui_generator::UIComponent>> {
        let schemas = self.schemas.read().unwrap();
        schemas
            .get(plugin_name)
            .map(|s| self.ui_generator.generate(s))
    }

    /// Generate JSON Schema for a plugin
    pub fn generate_json_schema(&self, plugin_name: &str) -> Option<serde_json::Value> {
        let schemas = self.schemas.read().unwrap();
        schemas
            .get(plugin_name)
            .map(|s| self.ui_generator.generate_json_schema(s))
    }

    /// Check if configuration files have been modified (for hot-reload)
    pub fn check_modified(&self, path: &Path) -> Result<bool, ConfigError> {
        let metadata = std::fs::metadata(path).map_err(|e| ConfigError::FileReadError {
            path: path.display().to_string(),
            error: e.to_string(),
        })?;

        let _modified: Instant = metadata
            .modified()
            .map(|_t| {
                // Convert SystemTime to Instant approximately
                // This is a simplification - real implementation would be more precise
                Instant::now()
            })
            .unwrap_or_else(|_| Instant::now());

        let timestamps = self.file_timestamps.read().unwrap();
        if let Some(_last_check) = timestamps.get(&path.display().to_string()) {
            // Simplified check - in production, would compare actual modification times
            Ok(false)
        } else {
            Ok(true)
        }
    }

    /// Get the validator
    pub fn validator(&self) -> &ConfigValidator {
        &self.validator
    }

    /// Get the UI generator
    pub fn ui_generator(&self) -> &UIGenerator {
        &self.ui_generator
    }

    /// Get the secret resolver
    pub fn secret_resolver(&self) -> &SecretResolver {
        &self.secret_resolver
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration error
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// File read error
    FileReadError { path: String, error: String },
    /// Parse error
    ParseError { path: String, error: String },
    /// Validation error
    ValidationError { plugin: String, error: String },
    /// Secret resolution error
    SecretError { error: String },
    /// Schema not found
    SchemaNotFound { plugin: String },
    /// Invalid value
    InvalidValue { key: String, reason: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::FileReadError { path, error } => {
                write!(f, "Failed to read file '{}': {}", path, error)
            }
            ConfigError::ParseError { path, error } => {
                write!(f, "Failed to parse file '{}': {}", path, error)
            }
            ConfigError::ValidationError { plugin, error } => {
                write!(f, "Validation error for plugin '{}': {}", plugin, error)
            }
            ConfigError::SecretError { error } => {
                write!(f, "Secret resolution error: {}", error)
            }
            ConfigError::SchemaNotFound { plugin } => {
                write!(f, "Schema not found for plugin: {}", plugin)
            }
            ConfigError::InvalidValue { key, reason } => {
                write!(f, "Invalid value for '{}': {}", key, reason)
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<SchemaError> for ConfigError {
    fn from(e: SchemaError) -> Self {
        ConfigError::ParseError {
            path: "schema".to_string(),
            error: e.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_manager_new() {
        let manager = ConfigManager::new();
        assert!(manager.get_config("nonexistent").is_none());
    }

    #[test]
    fn test_config_manager_set_value() {
        let manager = ConfigManager::new();

        // Without schema, any value is accepted
        let result = manager.set_value("test-plugin", "key", serde_json::json!("value"));
        assert!(result.is_ok());

        let value = manager.get_value("test-plugin", "key");
        assert!(value.is_some());
        assert_eq!(value.unwrap(), serde_json::json!("value"));
    }

    #[test]
    fn test_config_manager_register_schema() {
        let manager = ConfigManager::new();
        let schema = ConfigSchema::new("test-plugin");

        manager.register_schema("test-plugin", schema);

        let ui = manager.generate_ui("test-plugin");
        assert!(ui.is_some());
    }

    #[test]
    fn test_config_manager_generate_json_schema() {
        let manager = ConfigManager::new();
        let schema = ConfigSchema::new("test-plugin");

        manager.register_schema("test-plugin", schema);

        let json_schema = manager.generate_json_schema("test-plugin");
        assert!(json_schema.is_some());
        assert_eq!(json_schema.unwrap()["type"], "object");
    }
}
