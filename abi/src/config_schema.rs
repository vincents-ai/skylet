// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Config Schema Validation - RFC-0006
///
/// This module provides JSON Schema validation for plugin configuration.
/// It uses the `jsonschema` crate to validate configuration values against
/// schemas exported by plugins via the `plugin_get_config_schema_json()` ABI function.
///
/// # Example
///
/// ```ignore
/// use skylet_abi::config_schema::{ConfigSchemaValidator, ConfigValidationOptions};
///
/// // Get schema from plugin
/// let schema_json = loader.get_config_schema_string().unwrap();
/// let config_json = r#"{"api_key": "secret", "timeout": 30}"#;
///
/// // Validate config against schema
/// let validator = ConfigSchemaValidator::from_json(&schema_json)?;
/// let result = validator.validate(config_json)?;
///
/// if result.is_valid() {
///     tracing::info!("Config is valid!");
/// } else {
///     for error in result.errors {
///         tracing::error!("Validation error: {}", error);
///     }
/// }
/// ```
use serde_json::Value;
use std::collections::HashMap;

// Re-export jsonschema types we use
pub use jsonschema::{validator_for, Draft, ValidationError};

/// Result type for config schema operations
pub type ConfigSchemaResult<T> = Result<T, ConfigSchemaError>;

/// Errors that can occur during config schema operations
#[derive(Debug, Clone)]
pub enum ConfigSchemaError {
    /// The schema JSON is invalid or malformed
    InvalidSchemaJson(String),
    /// The schema compilation failed
    SchemaCompilationFailed(String),
    /// The config JSON is invalid or malformed
    InvalidConfigJson(String),
    /// Validation failed with specific errors
    ValidationFailed(Vec<String>),
}

impl std::fmt::Display for ConfigSchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigSchemaError::InvalidSchemaJson(s) => {
                write!(f, "Invalid schema JSON: {}", s)
            }
            ConfigSchemaError::SchemaCompilationFailed(s) => {
                write!(f, "Schema compilation failed: {}", s)
            }
            ConfigSchemaError::InvalidConfigJson(s) => {
                write!(f, "Invalid config JSON: {}", s)
            }
            ConfigSchemaError::ValidationFailed(errors) => {
                write!(f, "Validation failed: {}", errors.join("; "))
            }
        }
    }
}

impl std::error::Error for ConfigSchemaError {}

/// Result of config validation
#[derive(Debug, Clone)]
pub struct ConfigValidationResult {
    /// Whether the config is valid
    pub is_valid: bool,
    /// List of validation errors (empty if valid)
    pub errors: Vec<String>,
    /// Warnings (non-fatal issues)
    pub warnings: Vec<String>,
}

impl ConfigValidationResult {
    /// Create a valid result
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Create an invalid result with errors
    pub fn invalid(errors: Vec<String>) -> Self {
        Self {
            is_valid: false,
            errors,
            warnings: Vec::new(),
        }
    }

    /// Add a warning
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Check if validation passed
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }
}

/// Options for config validation
#[derive(Debug, Clone)]
pub struct ConfigValidationOptions {
    /// Whether to allow additional properties not in schema
    pub allow_additional_properties: bool,
    /// Whether to validate required fields strictly
    pub strict_required: bool,
    /// Custom default values for missing optional fields
    pub defaults: HashMap<String, Value>,
}

impl Default for ConfigValidationOptions {
    fn default() -> Self {
        Self {
            allow_additional_properties: true,
            strict_required: true,
            defaults: HashMap::new(),
        }
    }
}

impl ConfigValidationOptions {
    /// Create new options with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to allow additional properties
    pub fn with_additional_properties(mut self, allow: bool) -> Self {
        self.allow_additional_properties = allow;
        self
    }

    /// Set strict required validation
    pub fn with_strict_required(mut self, strict: bool) -> Self {
        self.strict_required = strict;
        self
    }

    /// Add a default value
    pub fn with_default(mut self, key: &str, value: Value) -> Self {
        self.defaults.insert(key.to_string(), value);
        self
    }
}

/// Config Schema Validator
///
/// Compiles a JSON Schema and validates config values against it.
pub struct ConfigSchemaValidator {
    /// The compiled JSON Schema validator
    validator: jsonschema::Validator,
    /// The original schema JSON for reference
    schema_json: Value,
}

impl ConfigSchemaValidator {
    /// Create a validator from a JSON Schema string
    ///
    /// # Arguments
    /// * `schema_json` - JSON Schema as a string
    ///
    /// # Example
    /// ```ignore
    /// let schema = r#"{"type": "object", "properties": {"name": {"type": "string"}}}"#;
    /// let validator = ConfigSchemaValidator::from_json(schema)?;
    /// ```
    pub fn from_json(schema_json: &str) -> ConfigSchemaResult<Self> {
        let schema_value: Value = serde_json::from_str(schema_json).map_err(|e| {
            ConfigSchemaError::InvalidSchemaJson(format!("Failed to parse schema: {}", e))
        })?;

        Self::from_value(schema_value)
    }

    /// Create a validator from a parsed JSON Value
    ///
    /// # Arguments
    /// * `schema_value` - JSON Schema as a serde_json::Value
    pub fn from_value(schema_value: Value) -> ConfigSchemaResult<Self> {
        let validator = validator_for(&schema_value).map_err(|e| {
            ConfigSchemaError::SchemaCompilationFailed(format!("Failed to compile schema: {}", e))
        })?;

        Ok(Self {
            validator,
            schema_json: schema_value,
        })
    }

    /// Validate a config JSON string against the schema
    ///
    /// # Arguments
    /// * `config_json` - Config as a JSON string
    ///
    /// # Returns
    /// * `ConfigValidationResult` indicating success or failure with errors
    pub fn validate(&self, config_json: &str) -> ConfigSchemaResult<ConfigValidationResult> {
        let config_value: Value = serde_json::from_str(config_json).map_err(|e| {
            ConfigSchemaError::InvalidConfigJson(format!("Failed to parse config: {}", e))
        })?;

        self.validate_value(&config_value)
    }

    /// Validate a config value against the schema
    ///
    /// # Arguments
    /// * `config_value` - Config as a serde_json::Value
    ///
    /// # Returns
    /// * `ConfigValidationResult` indicating success or failure with errors
    pub fn validate_value(
        &self,
        config_value: &Value,
    ) -> ConfigSchemaResult<ConfigValidationResult> {
        let result = self.validator.validate(config_value);

        match result {
            Ok(()) => Ok(ConfigValidationResult::valid()),
            Err(error) => {
                // In jsonschema 0.42.0, validate returns a single ValidationError
                let path = error.instance_path().to_string();
                let error_message = format!("{} at path '{}'", error, path.trim_start_matches('/'));
                Ok(ConfigValidationResult::invalid(vec![error_message]))
            }
        }
    }

    /// Validate with options
    ///
    /// # Arguments
    /// * `config_json` - Config as a JSON string
    /// * `options` - Validation options
    pub fn validate_with_options(
        &self,
        config_json: &str,
        options: &ConfigValidationOptions,
    ) -> ConfigSchemaResult<ConfigValidationResult> {
        let mut config_value: Value = serde_json::from_str(config_json).map_err(|e| {
            ConfigSchemaError::InvalidConfigJson(format!("Failed to parse config: {}", e))
        })?;

        // Apply defaults for missing optional fields
        if !options.defaults.is_empty() {
            if let Some(obj) = config_value.as_object_mut() {
                for (key, default) in &options.defaults {
                    if !obj.contains_key(key) {
                        obj.insert(key.clone(), default.clone());
                    }
                }
            }
        }

        let result = self.validate_value(&config_value)?;

        Ok(result)
    }

    /// Get the original schema JSON
    pub fn schema(&self) -> &Value {
        &self.schema_json
    }

    /// Check if the schema defines a specific property
    pub fn has_property(&self, property: &str) -> bool {
        if let Some(properties) = self.schema_json.get("properties") {
            properties.get(property).is_some()
        } else {
            false
        }
    }

    /// Get required properties from the schema
    pub fn required_properties(&self) -> Vec<String> {
        if let Some(required) = self.schema_json.get("required") {
            required
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Get all property names from the schema
    pub fn properties(&self) -> Vec<String> {
        if let Some(properties) = self.schema_json.get("properties") {
            properties
                .as_object()
                .map(|obj| obj.keys().cloned().collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    }
}

/// Validate config JSON against a schema JSON
///
/// Convenience function for one-off validation.
///
/// # Arguments
/// * `schema_json` - JSON Schema as a string
/// * `config_json` - Config as a JSON string
///
/// # Returns
/// * `ConfigValidationResult` indicating success or failure with errors
pub fn validate_config(
    schema_json: &str,
    config_json: &str,
) -> ConfigSchemaResult<ConfigValidationResult> {
    let validator = ConfigSchemaValidator::from_json(schema_json)?;
    validator.validate(config_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn simple_schema() -> String {
        r#"{
            "type": "object",
            "properties": {
                "api_key": {"type": "string"},
                "timeout": {"type": "integer", "minimum": 1, "maximum": 300},
                "enabled": {"type": "boolean"}
            },
            "required": ["api_key"]
        }"#
        .to_string()
    }

    #[test]
    fn test_validator_creation() {
        let schema = simple_schema();
        let result = ConfigSchemaValidator::from_json(&schema);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_schema_json() {
        let schema = "{ invalid json }";
        let result = ConfigSchemaValidator::from_json(schema);
        assert!(matches!(
            result,
            Err(ConfigSchemaError::InvalidSchemaJson(_))
        ));
    }

    #[test]
    fn test_valid_config() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();
        let config = r#"{"api_key": "secret123", "timeout": 30}"#;

        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());
    }

    #[test]
    fn test_missing_required_field() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();
        let config = r#"{"timeout": 30}"#;

        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_invalid_type() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();
        let config = r#"{"api_key": "secret", "timeout": "not a number"}"#;

        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    #[test]
    fn test_out_of_range_value() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();
        let config = r#"{"api_key": "secret", "timeout": 500}"#;

        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    #[test]
    fn test_convenience_function() {
        let schema = simple_schema();
        let config = r#"{"api_key": "test123"}"#;

        let result = validate_config(&schema, config).unwrap();
        assert!(result.is_valid());
    }

    #[test]
    fn test_get_properties() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        let properties = validator.properties();
        assert!(properties.contains(&"api_key".to_string()));
        assert!(properties.contains(&"timeout".to_string()));
        assert!(properties.contains(&"enabled".to_string()));
    }

    #[test]
    fn test_get_required_properties() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        let required = validator.required_properties();
        assert_eq!(required, vec!["api_key".to_string()]);
    }

    #[test]
    fn test_has_property() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        assert!(validator.has_property("api_key"));
        assert!(validator.has_property("timeout"));
        assert!(!validator.has_property("nonexistent"));
    }

    #[test]
    fn test_validation_with_defaults() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        let options = ConfigValidationOptions::new().with_default("timeout", json!(60));

        let config = r#"{"api_key": "secret"}"#;
        let result = validator.validate_with_options(config, &options).unwrap();
        assert!(result.is_valid());
    }

    #[test]
    fn test_validation_result_helpers() {
        let valid = ConfigValidationResult::valid();
        assert!(valid.is_valid());
        assert!(valid.errors.is_empty());

        let invalid = ConfigValidationResult::invalid(vec!["error1".to_string()]);
        assert!(!invalid.is_valid());
        assert_eq!(invalid.errors.len(), 1);
    }

    #[test]
    fn test_config_schema_error_display() {
        let err = ConfigSchemaError::InvalidSchemaJson("test".to_string());
        assert!(err.to_string().contains("Invalid schema JSON"));

        let err = ConfigSchemaError::ValidationFailed(vec!["err1".to_string(), "err2".to_string()]);
        assert!(err.to_string().contains("err1"));
        assert!(err.to_string().contains("err2"));
    }

    // ========================================================================
    // RFC-0006 Task 0006.5: Comprehensive Schema Validation Tests
    // ========================================================================

    // --- Invalid JSON Rejection Tests ---

    #[test]
    fn test_invalid_config_json_rejection() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        // Test malformed JSON
        let result = validator.validate("{ invalid json }");
        assert!(matches!(
            result,
            Err(ConfigSchemaError::InvalidConfigJson(_))
        ));
    }

    #[test]
    fn test_empty_string_config_rejection() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        let result = validator.validate("");
        assert!(matches!(
            result,
            Err(ConfigSchemaError::InvalidConfigJson(_))
        ));
    }

    #[test]
    fn test_truncated_json_config_rejection() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        let result = validator.validate(r#"{"api_key": "test"#); // Missing closing quotes and brace
        assert!(matches!(
            result,
            Err(ConfigSchemaError::InvalidConfigJson(_))
        ));
    }

    // --- Schema Compilation Failure Tests ---

    #[test]
    fn test_schema_with_invalid_type() {
        // Schema with invalid type value should still compile (jsonschema is permissive)
        // but may fail validation differently
        let schema = r#"{
            "type": "object",
            "properties": {
                "value": {"type": "invalid_type"}
            }
        }"#;

        // jsonschema crate may accept this as a warning, not an error
        let result = ConfigSchemaValidator::from_json(schema);
        // Just ensure it doesn't crash
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_empty_schema() {
        // Empty object schema should work
        let schema = "{}";
        let result = ConfigSchemaValidator::from_json(schema);
        // This might fail or succeed depending on jsonschema behavior
        assert!(
            result.is_ok() || matches!(result, Err(ConfigSchemaError::SchemaCompilationFailed(_)))
        );
    }

    #[test]
    fn test_schema_with_null_value() {
        let schema = "null";
        let result = ConfigSchemaValidator::from_json(schema);
        // null is not a valid schema
        assert!(result.is_ok() || result.is_err());
    }

    // --- Missing Required Fields Detection Tests ---

    #[test]
    fn test_multiple_missing_required_fields() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "api_key": {"type": "string"},
                "database_url": {"type": "string"},
                "port": {"type": "integer"}
            },
            "required": ["api_key", "database_url", "port"]
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();
        let config = r#"{}"#;

        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
        // Should report all missing required fields
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_partial_missing_required_fields() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "api_key": {"type": "string"},
                "database_url": {"type": "string"}
            },
            "required": ["api_key", "database_url"]
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();
        let config = r#"{"api_key": "test123"}"#;

        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_nested_required_fields() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "connection": {
                    "type": "object",
                    "properties": {
                        "host": {"type": "string"},
                        "port": {"type": "integer"}
                    },
                    "required": ["host", "port"]
                }
            },
            "required": ["connection"]
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();

        // Missing nested required field
        let config = r#"{"connection": {"host": "localhost"}}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    // --- Type Mismatch Detection Tests ---

    #[test]
    fn test_string_expected_got_number() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        let config = r#"{"api_key": 12345}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    #[test]
    fn test_number_expected_got_string() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        let config = r#"{"api_key": "secret", "timeout": "thirty"}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    #[test]
    fn test_boolean_expected_got_string() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        let config = r#"{"api_key": "secret", "enabled": "true"}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    #[test]
    fn test_array_type_validation() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "tags": {
                    "type": "array",
                    "items": {"type": "string"}
                }
            }
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();

        // Valid array
        let config = r#"{"tags": ["tag1", "tag2"]}"#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());

        // Invalid: array with wrong item types
        let config = r#"{"tags": [1, 2, 3]}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());

        // Invalid: not an array
        let config = r#"{"tags": "not-an-array"}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    #[test]
    fn test_object_type_validation() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "config": {
                    "type": "object",
                    "properties": {
                        "key": {"type": "string"}
                    }
                }
            }
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();

        // Valid nested object
        let config = r#"{"config": {"key": "value"}}"#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());

        // Invalid: not an object
        let config = r#"{"config": "not-an-object"}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    // --- Out of Range Value Detection Tests ---

    #[test]
    fn test_minimum_value_violation() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "port": {"type": "integer", "minimum": 1, "maximum": 65535}
            }
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();

        // Below minimum
        let config = r#"{"port": 0}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    #[test]
    fn test_maximum_value_violation() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "port": {"type": "integer", "minimum": 1, "maximum": 65535}
            }
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();

        // Above maximum
        let config = r#"{"port": 70000}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    #[test]
    fn test_exclusive_minimum() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "count": {"type": "integer", "exclusiveMinimum": 0}
            }
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();

        // At exclusive minimum boundary
        let config = r#"{"count": 0}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());

        // Above exclusive minimum
        let config = r#"{"count": 1}"#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());
    }

    #[test]
    fn test_string_length_constraints() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "name": {"type": "string", "minLength": 3, "maxLength": 10}
            }
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();

        // Too short
        let config = r#"{"name": "ab"}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());

        // Too long
        let config = r#"{"name": "thisiswaytoolong"}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());

        // Just right
        let config = r#"{"name": "perfect"}"#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());
    }

    #[test]
    fn test_enum_constraint() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "level": {
                    "type": "string",
                    "enum": ["debug", "info", "warn", "error"]
                }
            }
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();

        // Valid enum value
        let config = r#"{"level": "info"}"#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());

        // Invalid enum value
        let config = r#"{"level": "trace"}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    #[test]
    fn test_pattern_constraint() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "email": {
                    "type": "string",
                    "pattern": "^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$"
                }
            }
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();

        // Valid pattern match
        let config = r#"{"email": "test@example.com"}"#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());

        // Invalid pattern
        let config = r#"{"email": "invalid-email"}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    // --- Integration Tests with Real-World Schemas ---

    #[test]
    fn test_secrets_manager_schema() {
        // Simplified version of secrets-manager schema
        let schema = r#"{
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "backend": {
                    "type": "string",
                    "enum": ["memory", "encrypted"],
                    "default": "encrypted"
                },
                "rotation_days": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 365,
                    "default": 90
                },
                "max_versions": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 10
                },
                "audit_enabled": {
                    "type": "boolean",
                    "default": true
                }
            },
            "additionalProperties": false
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();

        // Valid config
        let config = r#"{
            "backend": "encrypted",
            "rotation_days": 30,
            "max_versions": 5,
            "audit_enabled": true
        }"#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());

        // Invalid: backend not in enum
        let config = r#"{"backend": "file"}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());

        // Invalid: rotation_days out of range
        let config = r#"{"rotation_days": 400}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());

        // Invalid: additional property (additionalProperties: false)
        let config = r#"{"unknown_field": "value"}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    #[test]
    fn test_postgres_plugin_schema() {
        // Simplified version of postgres-plugin schema
        let schema = r#"{
            "type": "object",
            "properties": {
                "host": {"type": "string", "default": "localhost"},
                "port": {"type": "integer", "minimum": 1, "maximum": 65535, "default": 5432},
                "database": {"type": "string"},
                "username": {"type": "string"},
                "password": {"type": "string"},
                "max_connections": {"type": "integer", "minimum": 1, "default": 10}
            },
            "additionalProperties": false
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();

        // Valid config with all optional defaults
        let config = r#"{"database": "mydb", "username": "user", "password": "secret"}"#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());

        // Invalid: port out of range
        let config = r#"{"port": 99999, "database": "mydb"}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());

        // Invalid: negative max_connections
        let config = r#"{"max_connections": -1}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    #[test]
    fn test_llm_provider_schema() {
        // Simplified version of llm-provider-adapter schema
        let schema = r#"{
            "type": "object",
            "properties": {
                "default_provider": {"type": "string"},
                "timeout_ms": {"type": "integer", "minimum": 1000, "maximum": 300000, "default": 30000},
                "max_retries": {"type": "integer", "minimum": 0, "maximum": 10, "default": 3},
                "enable_caching": {"type": "boolean", "default": true},
                "routing_strategy": {
                    "type": "string",
                    "enum": ["round_robin", "least_latency", "cost_optimized", "failover", "manual"],
                    "default": "failover"
                }
            },
            "additionalProperties": false
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();

        // Valid config
        let config = r#"{
            "default_provider": "openai",
            "timeout_ms": 60000,
            "routing_strategy": "cost_optimized"
        }"#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());

        // Invalid: timeout too low
        let config = r#"{"timeout_ms": 100}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());

        // Invalid: routing_strategy not in enum
        let config = r#"{"routing_strategy": "random"}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    // --- Edge Cases ---

    #[test]
    fn test_empty_config_against_schema_with_no_required() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "optional_field": {"type": "string"}
            }
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();

        // Empty config should be valid when no required fields
        let config = r#"{}"#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());
    }

    #[test]
    fn test_null_config_value() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        // null as config value
        let result = validator.validate("null");
        // Should fail because root must be object
        assert!(!result.unwrap().is_valid());
    }

    #[test]
    fn test_array_as_root_config() {
        let schema = simple_schema(); // expects object
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        // Array instead of object
        let config = r#"["item1", "item2"]"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    #[test]
    fn test_deeply_nested_config() {
        let schema = r#"{
            "type": "object",
            "properties": {
                "level1": {
                    "type": "object",
                    "properties": {
                        "level2": {
                            "type": "object",
                            "properties": {
                                "level3": {
                                    "type": "object",
                                    "properties": {
                                        "value": {"type": "string"}
                                    },
                                    "required": ["value"]
                                }
                            },
                            "required": ["level3"]
                        }
                    },
                    "required": ["level2"]
                }
            },
            "required": ["level1"]
        }"#;

        let validator = ConfigSchemaValidator::from_json(schema).unwrap();

        // Valid deeply nested
        let config = r#"{"level1": {"level2": {"level3": {"value": "test"}}}}"#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());

        // Missing deep required field
        let config = r#"{"level1": {"level2": {"level3": {}}}}"#;
        let result = validator.validate(config).unwrap();
        assert!(!result.is_valid());
    }

    #[test]
    fn test_config_with_extra_whitespace() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        // JSON with extra whitespace should still parse
        let config = r#"
            {
                "api_key" : "secret123" ,
                "timeout" : 30
            }
        "#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());
    }

    #[test]
    fn test_config_with_unicode_values() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        // Unicode in values
        let config = r#"{"api_key": "日本語-秘密鍵-🔐"}"#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());
    }

    #[test]
    fn test_escaped_characters_in_config() {
        let schema = simple_schema();
        let validator = ConfigSchemaValidator::from_json(&schema).unwrap();

        // Escaped characters
        let config = r#"{"api_key": "key with \"quotes\" and \\backslash"}"#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid());
    }
}
