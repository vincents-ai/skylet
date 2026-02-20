// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
    pub code: String,
}

#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn invalid(errors: Vec<ValidationError>) -> Self {
        Self {
            is_valid: false,
            errors,
            warnings: Vec::new(),
        }
    }

    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }

    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings.extend(warnings);
        self
    }

    pub fn add_error(&mut self, path: String, message: String, code: String) {
        self.is_valid = false;
        self.errors.push(ValidationError {
            path,
            message,
            code,
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaType {
    String,
    Number,
    Integer,
    Boolean,
    Array,
    Object,
    Null,
    Any,
}

impl SchemaType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "string" => Some(SchemaType::String),
            "number" | "float" => Some(SchemaType::Number),
            "integer" | "int" => Some(SchemaType::Integer),
            "boolean" | "bool" => Some(SchemaType::Boolean),
            "array" | "list" => Some(SchemaType::Array),
            "object" | "map" | "struct" => Some(SchemaType::Object),
            "null" => Some(SchemaType::Null),
            "any" => Some(SchemaType::Any),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SchemaProperty {
    pub schema_type: SchemaType,
    pub required: bool,
    #[allow(dead_code)] // Public API — not yet called from production code
    pub default: Option<Value>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub enum_values: Option<Vec<String>>,
    pub pattern: Option<String>,
    #[allow(dead_code)] // Public API — not yet called from production code
    pub description: Option<String>,
}

impl Default for SchemaProperty {
    fn default() -> Self {
        Self {
            schema_type: SchemaType::String,
            required: false,
            default: None,
            min_length: None,
            max_length: None,
            minimum: None,
            maximum: None,
            enum_values: None,
            pattern: None,
            description: None,
        }
    }
}

#[allow(dead_code)] // Public API — not yet called from production code
pub type CustomValidator = fn(value: &Value) -> Result<()>;

#[derive(Debug, Clone)]
pub struct SchemaValidator {
    pub properties: HashMap<String, SchemaProperty>,
    pub required: Vec<String>,
    #[allow(dead_code)] // Public API — not yet called from production code
    pub custom_validators: HashMap<String, String>,
    pub allow_additional: bool,
}

impl SchemaValidator {
    pub fn new() -> Self {
        Self {
            properties: HashMap::new(),
            required: Vec::new(),
            custom_validators: HashMap::new(),
            allow_additional: true,
        }
    }

    pub fn from_json(json_schema: &str) -> Result<Self> {
        let schema: Value = serde_json::from_str(json_schema)?;

        let mut validator = Self::new();

        if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
            for (name, prop_schema) in props {
                let prop = Self::parse_property(prop_schema)?;
                validator.properties.insert(name.clone(), prop.clone());

                if prop.required {
                    validator.required.push(name.clone());
                }
            }
        }

        if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
            validator.required = required
                .iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect();
        }

        validator.allow_additional = schema
            .get("additionalProperties")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        Ok(validator)
    }

    fn parse_property(prop_schema: &Value) -> Result<SchemaProperty> {
        let schema_type = prop_schema
            .get("type")
            .and_then(|v| v.as_str())
            .and_then(SchemaType::from_str)
            .unwrap_or(SchemaType::String);

        let required = prop_schema
            .get("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let default = prop_schema.get("default").cloned();

        let min_length = prop_schema
            .get("minLength")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        let max_length = prop_schema
            .get("maxLength")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        let minimum = prop_schema.get("minimum").and_then(|v| v.as_f64());
        let maximum = prop_schema.get("maximum").and_then(|v| v.as_f64());

        let enum_values = prop_schema
            .get("enum")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            });

        let pattern = prop_schema
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(String::from);

        let description = prop_schema
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        Ok(SchemaProperty {
            schema_type,
            required,
            default,
            min_length,
            max_length,
            minimum,
            maximum,
            enum_values,
            pattern,
            description,
        })
    }

    pub fn validate(&self, config_json: &str) -> Result<ValidationResult> {
        let config: Value = serde_json::from_str(config_json)?;

        let mut result = ValidationResult::valid();
        let config_obj = config
            .as_object()
            .ok_or_else(|| anyhow!("Config must be an object"))?;

        for required_field in &self.required {
            if !config_obj.contains_key(required_field) {
                result.add_error(
                    required_field.clone(),
                    format!("Required field '{}' is missing", required_field),
                    "MISSING_REQUIRED".to_string(),
                );
            }
        }

        for (key, value) in config_obj {
            if let Some(prop) = self.properties.get(key) {
                self.validate_value(key, value, prop, &mut result);
            } else if !self.allow_additional {
                result.add_error(
                    key.clone(),
                    format!("Unexpected additional property '{}'", key),
                    "ADDITIONAL_PROPERTY".to_string(),
                );
            }
        }

        result.is_valid = result.errors.is_empty();
        Ok(result)
    }

    fn validate_value(
        &self,
        key: &str,
        value: &Value,
        prop: &SchemaProperty,
        result: &mut ValidationResult,
    ) {
        if !self.check_type(value, &prop.schema_type) {
            result.add_error(
                key.to_string(),
                format!(
                    "Expected type '{:?}' but got '{:?}'",
                    prop.schema_type, value
                ),
                "TYPE_MISMATCH".to_string(),
            );
            return;
        }

        match &prop.schema_type {
            SchemaType::String => {
                if let Some(s) = value.as_str() {
                    if let Some(min_len) = prop.min_length {
                        if s.len() < min_len {
                            result.add_error(
                                key.to_string(),
                                format!(
                                    "String length {} is less than minimum {}",
                                    s.len(),
                                    min_len
                                ),
                                "MIN_LENGTH".to_string(),
                            );
                        }
                    }

                    if let Some(max_len) = prop.max_length {
                        if s.len() > max_len {
                            result.add_error(
                                key.to_string(),
                                format!("String length {} exceeds maximum {}", s.len(), max_len),
                                "MAX_LENGTH".to_string(),
                            );
                        }
                    }

                    if let Some(ref pattern) = prop.pattern {
                        if let Ok(re) = regex::Regex::new(pattern) {
                            if !re.is_match(s) {
                                result.add_error(
                                    key.to_string(),
                                    format!("String does not match pattern: {}", pattern),
                                    "PATTERN_MISMATCH".to_string(),
                                );
                            }
                        }
                    }

                    if let Some(ref enum_values) = prop.enum_values {
                        if !enum_values.contains(&s.to_string()) {
                            result.add_error(
                                key.to_string(),
                                format!(
                                    "Value '{}' is not one of the allowed values: {:?}",
                                    s, enum_values
                                ),
                                "INVALID_ENUM".to_string(),
                            );
                        }
                    }
                }
            }
            SchemaType::Number | SchemaType::Integer => {
                if let Some(n) = value.as_f64() {
                    if let Some(min) = prop.minimum {
                        if n < min {
                            result.add_error(
                                key.to_string(),
                                format!("Value {} is less than minimum {}", n, min),
                                "MINIMUM".to_string(),
                            );
                        }
                    }

                    if let Some(max) = prop.maximum {
                        if n > max {
                            result.add_error(
                                key.to_string(),
                                format!("Value {} exceeds maximum {}", n, max),
                                "MAXIMUM".to_string(),
                            );
                        }
                    }

                    if prop.schema_type == SchemaType::Integer {
                        if !value.is_i64() && !value.is_u64() {
                            result.add_error(
                                key.to_string(),
                                format!("Value {} is not an integer", n),
                                "NOT_INTEGER".to_string(),
                            );
                        }
                    }
                }
            }
            SchemaType::Array => {
                if let Some(arr) = value.as_array() {
                    if let Some(min_len) = prop.min_length {
                        if arr.len() < min_len {
                            result.add_error(
                                key.to_string(),
                                format!(
                                    "Array length {} is less than minimum {}",
                                    arr.len(),
                                    min_len
                                ),
                                "MIN_LENGTH".to_string(),
                            );
                        }
                    }

                    if let Some(max_len) = prop.max_length {
                        if arr.len() > max_len {
                            result.add_error(
                                key.to_string(),
                                format!("Array length {} exceeds maximum {}", arr.len(), max_len),
                                "MAX_LENGTH".to_string(),
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn check_type(&self, value: &Value, schema_type: &SchemaType) -> bool {
        match schema_type {
            SchemaType::String => value.is_string(),
            SchemaType::Number => value.is_number(),
            SchemaType::Integer => value.is_i64() || value.is_u64(),
            SchemaType::Boolean => value.is_boolean(),
            SchemaType::Array => value.is_array(),
            SchemaType::Object => value.is_object(),
            SchemaType::Null => value.is_null(),
            SchemaType::Any => true,
        }
    }

    #[allow(dead_code)] // Public API — not yet called from production code
    pub fn add_property(&mut self, name: String, property: SchemaProperty) {
        self.properties.insert(name, property);
    }

    #[allow(dead_code)] // Public API — not yet called from production code
    pub fn set_required(&mut self, required: Vec<String>) {
        self.required = required;
    }
}

impl Default for SchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_from_str() {
        assert_eq!(SchemaType::from_str("string"), Some(SchemaType::String));
        assert_eq!(SchemaType::from_str("number"), Some(SchemaType::Number));
        assert_eq!(SchemaType::from_str("integer"), Some(SchemaType::Integer));
        assert_eq!(SchemaType::from_str("boolean"), Some(SchemaType::Boolean));
        assert_eq!(SchemaType::from_str("array"), Some(SchemaType::Array));
        assert_eq!(SchemaType::from_str("object"), Some(SchemaType::Object));
        assert_eq!(SchemaType::from_str("null"), Some(SchemaType::Null));
        assert_eq!(SchemaType::from_str("any"), Some(SchemaType::Any));
    }

    #[test]
    fn test_validation_result() {
        let result = ValidationResult::valid();
        assert!(result.is_valid);
        assert!(result.errors.is_empty());

        let errors = vec![ValidationError {
            path: "test".to_string(),
            message: "error".to_string(),
            code: "TEST".to_string(),
        }];
        let result = ValidationResult::invalid(errors);
        assert!(!result.is_valid);
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_schema_validator() {
        let mut validator = SchemaValidator::new();

        let mut prop = SchemaProperty {
            schema_type: SchemaType::String,
            required: true,
            ..Default::default()
        };
        prop.min_length = Some(1);
        prop.max_length = Some(10);

        validator.add_property("name".to_string(), prop);

        let config = r#"{"name": "test"}"#;
        let result = validator.validate(config).unwrap();
        assert!(result.is_valid);

        let invalid_config = r#"{"name": ""}"#;
        let result = validator.validate(invalid_config).unwrap();
        assert!(!result.is_valid);
    }

    #[test]
    fn test_schema_validator_from_json() {
        let schema = r#"{
            "properties": {
                "name": {
                    "type": "string",
                    "required": true,
                    "minLength": 1,
                    "maxLength": 100
                },
                "age": {
                    "type": "integer",
                    "required": false,
                    "minimum": 0,
                    "maximum": 150
                }
            },
            "required": ["name"]
        }"#;

        let validator = SchemaValidator::from_json(schema).unwrap();
        assert_eq!(validator.properties.len(), 2);
        assert!(validator.required.contains(&"name".to_string()));
        assert!(!validator.required.contains(&"age".to_string()));
    }
}
