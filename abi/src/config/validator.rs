// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Configuration Validator - RFC-0006
//!
//! Validates configuration values against schema definitions.

use std::collections::HashMap;

use super::schema::{
    ConfigField, ConfigFieldType, ConfigSchema, GlobalValidationRule, ValidationRule,
};

/// Type alias for custom validation functions
type CustomValidatorFn =
    Box<dyn Fn(&serde_json::Value) -> Result<(), ValidationError> + Send + Sync>;

/// Configuration validator
pub struct ConfigValidator {
    /// Custom validation functions
    custom_validators: HashMap<String, CustomValidatorFn>,
}

impl ConfigValidator {
    /// Create a new configuration validator
    pub fn new() -> Self {
        Self {
            custom_validators: HashMap::new(),
        }
    }

    /// Register a custom validation function
    pub fn register_custom_validator<F>(&mut self, name: &str, validator: F)
    where
        F: Fn(&serde_json::Value) -> Result<(), ValidationError> + Send + Sync + 'static,
    {
        self.custom_validators
            .insert(name.to_string(), Box::new(validator));
    }

    /// Validate a configuration value against a schema
    pub fn validate(
        &self,
        schema: &ConfigSchema,
        values: &HashMap<String, serde_json::Value>,
    ) -> Result<Vec<ValidationWarning>, ValidationError> {
        let mut warnings = Vec::new();

        // Validate each field
        for field in schema.all_fields() {
            let value = values.get(&field.name);

            // Check required fields
            if field.required
                && (value.is_none() || value.map(is_null_or_empty).unwrap_or(false))
                && field.default.is_none()
            {
                return Err(ValidationError::RequiredFieldMissing {
                    field: field.name.clone(),
                });
            }

            // If value is present, validate it
            if let Some(v) = value {
                if !is_null_or_empty(v) {
                    self.validate_field(field, v)?;
                }
            }

            // Check for deprecated fields
            if let Some(deprecation_message) = value.and(field.deprecated.as_ref()) {
                warnings.push(ValidationWarning::DeprecatedField {
                    field: field.name.clone(),
                    message: deprecation_message.clone(),
                });
            }
        }

        // Validate global rules
        for rule in &schema.global_validation {
            self.validate_global_rule(rule, values)?;
        }

        Ok(warnings)
    }

    /// Validate a single field
    pub fn validate_field(
        &self,
        field: &ConfigField,
        value: &serde_json::Value,
    ) -> Result<(), ValidationError> {
        // Validate type
        self.validate_type(&field.field_type, value, &field.name)?;

        // Validate rules
        for rule in &field.validation {
            self.validate_rule(rule, value, &field.name)?;
        }

        Ok(())
    }

    /// Validate a value's type
    fn validate_type(
        &self,
        field_type: &ConfigFieldType,
        value: &serde_json::Value,
        field_name: &str,
    ) -> Result<(), ValidationError> {
        let valid = match field_type {
            ConfigFieldType::String => value.is_string(),
            ConfigFieldType::Integer => value.is_i64() || value.is_u64() || value.is_number(),
            ConfigFieldType::Float => value.is_f64() || value.is_number(),
            ConfigFieldType::Boolean => value.is_boolean(),
            ConfigFieldType::Array(inner_type) => {
                if let Some(arr) = value.as_array() {
                    arr.iter()
                        .all(|v| self.validate_type(inner_type, v, field_name).is_ok())
                } else {
                    false
                }
            }
            ConfigFieldType::Object => value.is_object(),
            ConfigFieldType::Secret => {
                // Secret can be a string or a secret reference object
                value.is_string() || value.is_object()
            }
            ConfigFieldType::Enum { variants } => {
                if let Some(s) = value.as_str() {
                    variants.contains(&s.to_string())
                } else {
                    false
                }
            }
            ConfigFieldType::Path { must_exist, is_dir } => {
                if let Some(s) = value.as_str() {
                    let path = std::path::Path::new(s);
                    if *must_exist {
                        if *is_dir {
                            path.is_dir()
                        } else {
                            path.exists() && path.is_file()
                        }
                    } else {
                        true // Path doesn't need to exist, just be a valid string
                    }
                } else {
                    false
                }
            }
            ConfigFieldType::Url { schemes } => {
                if let Some(s) = value.as_str() {
                    if let Ok(url) = url::Url::parse(s) {
                        if schemes.is_empty() {
                            true
                        } else {
                            schemes.contains(&url.scheme().to_string())
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            ConfigFieldType::Duration => {
                if let Some(s) = value.as_str() {
                    // Parse human-readable duration like "5s", "1h30m", "2d"
                    parse_duration(s).is_ok()
                } else {
                    false
                }
            }
            ConfigFieldType::Port => {
                if let Some(n) = value.as_u64() {
                    (1..=65535).contains(&n)
                } else {
                    false
                }
            }
            ConfigFieldType::Email => {
                if let Some(s) = value.as_str() {
                    s.contains('@') && s.contains('.')
                } else {
                    false
                }
            }
            ConfigFieldType::Host => {
                if let Some(s) = value.as_str() {
                    // Basic hostname validation
                    !s.is_empty() && !s.contains(' ')
                } else {
                    false
                }
            }
        };

        if !valid {
            return Err(ValidationError::TypeMismatch {
                field: field_name.to_string(),
                expected: format!("{:?}", field_type),
                actual: value_type_name(value),
            });
        }

        Ok(())
    }

    /// Validate a single validation rule
    fn validate_rule(
        &self,
        rule: &ValidationRule,
        value: &serde_json::Value,
        field_name: &str,
    ) -> Result<(), ValidationError> {
        match rule {
            ValidationRule::Min { value: min } => {
                if let Some(n) = value.as_f64() {
                    if n < *min {
                        return Err(ValidationError::ConstraintViolation {
                            field: field_name.to_string(),
                            message: format!("Value {} is less than minimum {}", n, min),
                        });
                    }
                }
            }
            ValidationRule::Max { value: max } => {
                if let Some(n) = value.as_f64() {
                    if n > *max {
                        return Err(ValidationError::ConstraintViolation {
                            field: field_name.to_string(),
                            message: format!("Value {} is greater than maximum {}", n, max),
                        });
                    }
                }
            }
            ValidationRule::MinLength { value: min_len } => {
                if let Some(s) = value.as_str() {
                    if s.len() < *min_len {
                        return Err(ValidationError::ConstraintViolation {
                            field: field_name.to_string(),
                            message: format!(
                                "String length {} is less than minimum {}",
                                s.len(),
                                min_len
                            ),
                        });
                    }
                }
            }
            ValidationRule::MaxLength { value: max_len } => {
                if let Some(s) = value.as_str() {
                    if s.len() > *max_len {
                        return Err(ValidationError::ConstraintViolation {
                            field: field_name.to_string(),
                            message: format!(
                                "String length {} is greater than maximum {}",
                                s.len(),
                                max_len
                            ),
                        });
                    }
                }
            }
            ValidationRule::Pattern { regex } => {
                if let Some(s) = value.as_str() {
                    let re = regex_lite::Regex::new(regex).map_err(|e| {
                        ValidationError::InvalidPattern {
                            field: field_name.to_string(),
                            pattern: regex.clone(),
                            error: e.to_string(),
                        }
                    })?;
                    if !re.is_match(s) {
                        return Err(ValidationError::ConstraintViolation {
                            field: field_name.to_string(),
                            message: format!("Value '{}' does not match pattern {}", s, regex),
                        });
                    }
                }
            }
            ValidationRule::OneOf { values } => {
                if !values.contains(value) {
                    return Err(ValidationError::ConstraintViolation {
                        field: field_name.to_string(),
                        message: format!("Value {:?} is not one of {:?}", value, values),
                    });
                }
            }
            ValidationRule::NotOneOf { values } => {
                if values.contains(value) {
                    return Err(ValidationError::ConstraintViolation {
                        field: field_name.to_string(),
                        message: format!("Value {:?} is not allowed", value),
                    });
                }
            }
            ValidationRule::Custom { name, params: _ } => {
                if let Some(validator) = self.custom_validators.get(name) {
                    validator(value)?;
                } else {
                    return Err(ValidationError::UnknownValidator { name: name.clone() });
                }
            }
        }

        Ok(())
    }

    /// Validate a global validation rule
    fn validate_global_rule(
        &self,
        rule: &GlobalValidationRule,
        values: &HashMap<String, serde_json::Value>,
    ) -> Result<(), ValidationError> {
        match rule {
            GlobalValidationRule::FieldsEqual { field1, field2 } => {
                let v1 = values.get(field1);
                let v2 = values.get(field2);
                if v1 != v2 {
                    return Err(ValidationError::ConstraintViolation {
                        field: format!("{} / {}", field1, field2),
                        message: "Fields must have the same value".to_string(),
                    });
                }
            }
            GlobalValidationRule::AtLeastOneRequired { fields } => {
                let any_set = fields
                    .iter()
                    .any(|f| values.get(f).map(|v| !is_null_or_empty(v)).unwrap_or(false));
                if !any_set {
                    return Err(ValidationError::ConstraintViolation {
                        field: fields.join(", "),
                        message: "At least one of these fields is required".to_string(),
                    });
                }
            }
            GlobalValidationRule::ExactlyOneRequired { fields } => {
                let count = fields
                    .iter()
                    .filter(|f| {
                        values
                            .get(*f)
                            .map(|v| !is_null_or_empty(v))
                            .unwrap_or(false)
                    })
                    .count();
                if count != 1 {
                    return Err(ValidationError::ConstraintViolation {
                        field: fields.join(", "),
                        message: format!(
                            "Exactly one of these fields is required, but {} are set",
                            count
                        ),
                    });
                }
            }
            GlobalValidationRule::ConditionalRequired {
                when_field,
                when_value,
                then_required,
            } => {
                if values.get(when_field) == Some(when_value) {
                    for field in then_required {
                        if values.get(field).map(is_null_or_empty).unwrap_or(true) {
                            return Err(ValidationError::ConstraintViolation {
                                field: field.clone(),
                                message: format!(
                                    "Field is required when {} is {:?}",
                                    when_field, when_value
                                ),
                            });
                        }
                    }
                }
            }
            GlobalValidationRule::Custom {
                name,
                fields: _,
                params: _,
            } => {
                // Custom global validation would be implemented here
                return Err(ValidationError::UnknownValidator {
                    name: format!("global:{}", name),
                });
            }
        }

        Ok(())
    }

    /// Apply default values to missing fields
    pub fn apply_defaults(
        &self,
        schema: &ConfigSchema,
        values: &mut HashMap<String, serde_json::Value>,
    ) {
        for field in schema.all_fields() {
            if !values.contains_key(&field.name) || is_null_or_empty(&values[&field.name]) {
                if let Some(default) = &field.default {
                    values.insert(field.name.clone(), default.clone());
                }
            }
        }
    }

    /// Coerce a value to the expected type
    pub fn coerce_value(
        &self,
        field_type: &ConfigFieldType,
        value: &serde_json::Value,
    ) -> Result<serde_json::Value, ValidationError> {
        match field_type {
            ConfigFieldType::String => {
                if value.is_string() {
                    Ok(value.clone())
                } else {
                    Ok(serde_json::Value::String(value.to_string()))
                }
            }
            ConfigFieldType::Integer => {
                if let Some(n) = value.as_i64() {
                    Ok(serde_json::json!(n))
                } else if let Some(n) = value.as_f64() {
                    Ok(serde_json::json!(n as i64))
                } else if let Some(s) = value.as_str() {
                    s.parse::<i64>().map(|n| serde_json::json!(n)).map_err(|_| {
                        ValidationError::TypeMismatch {
                            field: "value".to_string(),
                            expected: "integer".to_string(),
                            actual: value_type_name(value),
                        }
                    })
                } else {
                    Err(ValidationError::TypeMismatch {
                        field: "value".to_string(),
                        expected: "integer".to_string(),
                        actual: value_type_name(value),
                    })
                }
            }
            ConfigFieldType::Float => {
                if let Some(n) = value.as_f64() {
                    Ok(serde_json::json!(n))
                } else if let Some(s) = value.as_str() {
                    s.parse::<f64>().map(|n| serde_json::json!(n)).map_err(|_| {
                        ValidationError::TypeMismatch {
                            field: "value".to_string(),
                            expected: "float".to_string(),
                            actual: value_type_name(value),
                        }
                    })
                } else {
                    Err(ValidationError::TypeMismatch {
                        field: "value".to_string(),
                        expected: "float".to_string(),
                        actual: value_type_name(value),
                    })
                }
            }
            ConfigFieldType::Boolean => {
                if let Some(b) = value.as_bool() {
                    Ok(serde_json::json!(b))
                } else if let Some(s) = value.as_str() {
                    match s.to_lowercase().as_str() {
                        "true" | "yes" | "1" | "on" => Ok(serde_json::json!(true)),
                        "false" | "no" | "0" | "off" => Ok(serde_json::json!(false)),
                        _ => Err(ValidationError::TypeMismatch {
                            field: "value".to_string(),
                            expected: "boolean".to_string(),
                            actual: value_type_name(value),
                        }),
                    }
                } else {
                    Err(ValidationError::TypeMismatch {
                        field: "value".to_string(),
                        expected: "boolean".to_string(),
                        actual: value_type_name(value),
                    })
                }
            }
            _ => Ok(value.clone()), // For complex types, no coercion
        }
    }
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a value is null or empty
fn is_null_or_empty(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => true,
        serde_json::Value::String(s) => s.is_empty(),
        serde_json::Value::Array(arr) => arr.is_empty(),
        serde_json::Value::Object(obj) => obj.is_empty(),
        _ => false,
    }
}

/// Get the type name of a JSON value
fn value_type_name(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(_) => "boolean".to_string(),
        serde_json::Value::Number(_) => "number".to_string(),
        serde_json::Value::String(_) => "string".to_string(),
        serde_json::Value::Array(_) => "array".to_string(),
        serde_json::Value::Object(_) => "object".to_string(),
    }
}

/// Parse a human-readable duration string
fn parse_duration(s: &str) -> Result<std::time::Duration, String> {
    let mut total_secs: u64 = 0;
    let mut current_num = String::new();

    for c in s.chars() {
        if c.is_ascii_digit() {
            current_num.push(c);
        } else {
            let num: u64 = current_num.parse().map_err(|_| "Invalid number")?;
            current_num.clear();

            let secs = match c {
                's' => num,
                'm' => num * 60,
                'h' => num * 3600,
                'd' => num * 86400,
                'w' => num * 604800,
                _ => return Err(format!("Unknown duration unit: {}", c)),
            };
            total_secs += secs;
        }
    }

    if !current_num.is_empty() {
        return Err("Trailing number without unit".to_string());
    }

    Ok(std::time::Duration::from_secs(total_secs))
}

/// Validation error
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// A required field is missing
    RequiredFieldMissing { field: String },
    /// Type mismatch
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    /// Constraint violation
    ConstraintViolation { field: String, message: String },
    /// Invalid regex pattern
    InvalidPattern {
        field: String,
        pattern: String,
        error: String,
    },
    /// Unknown custom validator
    UnknownValidator { name: String },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::RequiredFieldMissing { field } => {
                write!(f, "Required field '{}' is missing", field)
            }
            ValidationError::TypeMismatch {
                field,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Type mismatch for field '{}': expected {}, got {}",
                    field, expected, actual
                )
            }
            ValidationError::ConstraintViolation { field, message } => {
                write!(f, "Validation failed for field '{}': {}", field, message)
            }
            ValidationError::InvalidPattern {
                field,
                pattern,
                error,
            } => {
                write!(
                    f,
                    "Invalid regex pattern '{}' for field '{}': {}",
                    pattern, field, error
                )
            }
            ValidationError::UnknownValidator { name } => {
                write!(f, "Unknown validator: {}", name)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validation warning (non-fatal issues)
#[derive(Debug, Clone)]
pub enum ValidationWarning {
    /// Field is deprecated
    DeprecatedField { field: String, message: String },
    /// Field has a default value
    UsingDefault { field: String },
    /// Field value is unusual but valid
    UnusualValue { field: String, value: String },
}

impl std::fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationWarning::DeprecatedField { field, message } => {
                write!(f, "Field '{}' is deprecated: {}", field, message)
            }
            ValidationWarning::UsingDefault { field } => {
                write!(f, "Using default value for field '{}'", field)
            }
            ValidationWarning::UnusualValue { field, value } => {
                write!(f, "Unusual value for field '{}': {}", field, value)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_test_schema() -> ConfigSchema {
        let mut schema = ConfigSchema::new("test-plugin");
        schema.add_section(super::super::schema::ConfigSection {
            name: "general".to_string(),
            title: None,
            description: None,
            fields: vec![
                ConfigField {
                    name: "port".to_string(),
                    label: None,
                    description: None,
                    field_type: ConfigFieldType::Port,
                    default: Some(serde_json::json!(8080)),
                    required: false,
                    sensitive: false,
                    validation: vec![],
                    ui_hints: None,
                    secret_ref: None,
                    deprecated: None,
                    env_var: None,
                    reload_on_change: false,
                },
                ConfigField {
                    name: "required_field".to_string(),
                    label: None,
                    description: None,
                    field_type: ConfigFieldType::String,
                    default: None,
                    required: true,
                    sensitive: false,
                    validation: vec![
                        ValidationRule::MinLength { value: 3 },
                        ValidationRule::MaxLength { value: 10 },
                    ],
                    ui_hints: None,
                    secret_ref: None,
                    deprecated: None,
                    env_var: None,
                    reload_on_change: false,
                },
            ],
            order: None,
            collapsed: false,
            icon: None,
        });
        schema
    }

    #[test]
    fn test_validator_new() {
        let validator = ConfigValidator::new();
        assert!(validator.custom_validators.is_empty());
    }

    #[test]
    fn test_validate_missing_required() {
        let validator = ConfigValidator::new();
        let schema = make_test_schema();
        let values = HashMap::new(); // Missing required_field

        let result = validator.validate(&schema, &values);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_valid_config() {
        let validator = ConfigValidator::new();
        let schema = make_test_schema();
        let mut values = HashMap::new();
        values.insert("required_field".to_string(), serde_json::json!("hello"));

        let result = validator.validate(&schema, &values);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_string_length() {
        let validator = ConfigValidator::new();
        let schema = make_test_schema();
        let mut values = HashMap::new();
        values.insert("required_field".to_string(), serde_json::json!("hi")); // Too short

        let result = validator.validate(&schema, &values);
        assert!(result.is_err());
    }

    #[test]
    fn test_coerce_boolean() {
        let validator = ConfigValidator::new();

        let result = validator.coerce_value(&ConfigFieldType::Boolean, &serde_json::json!("true"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!(true));

        let result = validator.coerce_value(&ConfigFieldType::Boolean, &serde_json::json!("yes"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!(true));
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(
            parse_duration("5s").unwrap(),
            std::time::Duration::from_secs(5)
        );
        assert_eq!(
            parse_duration("1h").unwrap(),
            std::time::Duration::from_secs(3600)
        );
        assert_eq!(
            parse_duration("1h30m").unwrap(),
            std::time::Duration::from_secs(5400)
        );
        assert_eq!(
            parse_duration("2d").unwrap(),
            std::time::Duration::from_secs(172800)
        );
    }
}
