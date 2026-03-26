// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Configuration Schema Types - RFC-0006
//!
//! Defines the schema for plugin configuration including field types,
//! validation rules, default values, and secret references.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A configuration field type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigFieldType {
    /// String value
    String,
    /// Integer value (i64)
    Integer,
    /// Floating point value (f64)
    Float,
    /// Boolean value
    Boolean,
    /// Array of values
    Array(Box<ConfigFieldType>),
    /// Object/map of key-value pairs
    Object,
    /// Secret reference (vault://...)
    Secret,
    /// Enum with predefined values
    Enum { variants: Vec<String> },
    /// Path to a file or directory
    Path { must_exist: bool, is_dir: bool },
    /// URL value with optional scheme restriction
    Url { schemes: Vec<String> },
    /// Duration in human-readable format (e.g., "5s", "1h30m")
    Duration,
    /// Port number (1-65535)
    Port,
    /// Email address
    Email,
    /// Hostname or IP address
    Host,
}

/// Validation rule for a configuration field
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationRule {
    /// Minimum value (for numbers)
    Min { value: f64 },
    /// Maximum value (for numbers)
    Max { value: f64 },
    /// Minimum length (for strings)
    MinLength { value: usize },
    /// Maximum length (for strings)
    MaxLength { value: usize },
    /// Regular expression pattern (for strings)
    Pattern { regex: String },
    /// Value must be in this list
    OneOf { values: Vec<serde_json::Value> },
    /// Value must NOT be in this list
    NotOneOf { values: Vec<serde_json::Value> },
    /// Custom validation function name
    Custom {
        name: String,
        params: HashMap<String, serde_json::Value>,
    },
}

/// A configuration field definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigField {
    /// Field name/identifier
    pub name: String,
    /// Human-readable label for UI
    pub label: Option<String>,
    /// Human-readable description
    pub description: Option<String>,
    /// Field type
    pub field_type: ConfigFieldType,
    /// Default value (if any)
    pub default: Option<serde_json::Value>,
    /// Whether this field is required
    pub required: bool,
    /// Whether this field is sensitive (should be hidden in UI)
    pub sensitive: bool,
    /// Validation rules
    pub validation: Vec<ValidationRule>,
    /// UI hints for rendering
    pub ui_hints: Option<UIHints>,
    /// Secret reference (if this field references a secret)
    pub secret_ref: Option<SecretReference>,
    /// Deprecation message (if deprecated)
    pub deprecated: Option<String>,
    /// Environment variable to read from (if any)
    pub env_var: Option<String>,
    /// Whether to reload plugin when this field changes
    pub reload_on_change: bool,
}

/// UI rendering hints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIHints {
    /// Widget type override
    pub widget: Option<WidgetType>,
    /// Placeholder text for input fields
    pub placeholder: Option<String>,
    /// Help text/tooltip
    pub help_text: Option<String>,
    /// Group this field belongs to
    pub group: Option<String>,
    /// Order within the group (lower = earlier)
    pub order: Option<i32>,
    /// Whether to show this field in advanced settings
    pub advanced: bool,
    /// Custom CSS class
    pub css_class: Option<String>,
    /// Whether to auto-focus this field
    pub autofocus: bool,
}

/// Widget type for UI rendering
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetType {
    /// Standard text input
    TextInput,
    /// Multiline text area
    TextArea,
    /// Password input (masked)
    Password,
    /// Number input with spinner
    NumberInput,
    /// Slider for numeric values (min/max/step as strings to avoid f64 Eq issues)
    Slider {
        min: String,
        max: String,
        step: String,
    },
    /// Checkbox for boolean values
    Checkbox,
    /// Toggle switch for boolean values
    Toggle,
    /// Dropdown select
    Select,
    /// Radio button group
    RadioGroup,
    /// Multi-select
    MultiSelect,
    /// Color picker
    ColorPicker,
    /// Date picker
    DatePicker,
    /// Time picker
    TimePicker,
    /// File picker
    FilePicker { extensions: Vec<String> },
    /// Directory picker
    DirectoryPicker,
    /// Code editor
    CodeEditor { language: String },
    /// Slider with marks
    MarkedSlider { marks: Vec<(f64, String)> },
}

/// Secret reference for secure value resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretReference {
    /// Secret URI (vault://path/to/secret or env://VAR_NAME)
    pub uri: String,
    /// Key within the secret (for multi-value secrets)
    pub key: Option<String>,
    /// Version of the secret (optional)
    pub version: Option<String>,
    /// Whether to cache the resolved value
    pub cache: bool,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: Option<u64>,
}

impl SecretReference {
    /// Create a new secret reference from a URI
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            key: None,
            version: None,
            cache: true,
            cache_ttl_seconds: Some(300), // 5 minutes default
        }
    }

    /// Parse a secret URI string
    pub fn parse(uri: &str) -> Option<Self> {
        if uri.starts_with("vault://") || uri.starts_with("env://") || uri.starts_with("file://") {
            Some(Self::new(uri))
        } else {
            None
        }
    }

    /// Get the secret backend type
    pub fn backend(&self) -> SecretBackend {
        if self.uri.starts_with("vault://") {
            SecretBackend::Vault
        } else if self.uri.starts_with("env://") {
            SecretBackend::Environment
        } else if self.uri.starts_with("file://") {
            SecretBackend::File
        } else {
            SecretBackend::Unknown
        }
    }

    /// Get the path portion of the URI
    pub fn path(&self) -> &str {
        let separator = "://";
        if let Some(pos) = self.uri.find(separator) {
            &self.uri[pos + separator.len()..]
        } else {
            &self.uri
        }
    }
}

/// Secret backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretBackend {
    /// HashiCorp Vault
    Vault,
    /// Environment variable
    Environment,
    /// File-based secret
    File,
    /// Unknown backend
    Unknown,
}

/// A configuration section/group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSection {
    /// Section name/identifier
    pub name: String,
    /// Human-readable title
    pub title: Option<String>,
    /// Section description
    pub description: Option<String>,
    /// Fields in this section
    pub fields: Vec<ConfigField>,
    /// Order of this section (lower = earlier)
    pub order: Option<i32>,
    /// Whether this section is collapsed by default
    pub collapsed: bool,
    /// Icon for UI
    pub icon: Option<String>,
}

/// Complete configuration schema for a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSchema {
    /// Schema version
    pub version: String,
    /// Plugin name this schema applies to
    pub plugin_name: String,
    /// Schema description
    pub description: Option<String>,
    /// Configuration sections
    pub sections: Vec<ConfigSection>,
    /// Global validation rules that apply across fields
    pub global_validation: Vec<GlobalValidationRule>,
    /// Configuration file format
    pub format: ConfigFormat,
}

/// Configuration file format
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigFormat {
    /// TOML format
    Toml,
    /// JSON format
    Json,
    /// YAML format
    Yaml,
}

/// Global validation rule that spans multiple fields
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalValidationRule {
    /// Fields must have the same value
    FieldsEqual { field1: String, field2: String },
    /// At least one of the fields must be set
    AtLeastOneRequired { fields: Vec<String> },
    /// Exactly one of the fields must be set
    ExactlyOneRequired { fields: Vec<String> },
    /// Conditional requirement
    ConditionalRequired {
        when_field: String,
        when_value: serde_json::Value,
        then_required: Vec<String>,
    },
    /// Custom validation function
    Custom {
        name: String,
        fields: Vec<String>,
        params: HashMap<String, serde_json::Value>,
    },
}

impl ConfigSchema {
    /// Create a new configuration schema for a plugin
    pub fn new(plugin_name: impl Into<String>) -> Self {
        Self {
            version: "1.0.0".to_string(),
            plugin_name: plugin_name.into(),
            description: None,
            sections: Vec::new(),
            global_validation: Vec::new(),
            format: ConfigFormat::Toml,
        }
    }

    /// Add a section to the schema
    pub fn add_section(&mut self, section: ConfigSection) {
        self.sections.push(section);
    }

    /// Get a field by name
    pub fn get_field(&self, name: &str) -> Option<&ConfigField> {
        for section in &self.sections {
            for field in &section.fields {
                if field.name == name {
                    return Some(field);
                }
            }
        }
        None
    }

    /// Get a mutable field by name
    pub fn get_field_mut(&mut self, name: &str) -> Option<&mut ConfigField> {
        for section in &mut self.sections {
            for field in &mut section.fields {
                if field.name == name {
                    return Some(field);
                }
            }
        }
        None
    }

    /// Get all fields across all sections
    pub fn all_fields(&self) -> Vec<&ConfigField> {
        self.sections.iter().flat_map(|s| s.fields.iter()).collect()
    }

    /// Get all required fields
    pub fn required_fields(&self) -> Vec<&ConfigField> {
        self.all_fields()
            .into_iter()
            .filter(|f| f.required)
            .collect()
    }

    /// Get all fields with secret references
    pub fn secret_fields(&self) -> Vec<&ConfigField> {
        self.all_fields()
            .into_iter()
            .filter(|f| f.secret_ref.is_some())
            .collect()
    }

    /// Parse from TOML string
    pub fn from_toml(toml_str: &str) -> Result<Self, SchemaError> {
        toml::from_str(toml_str).map_err(|e| SchemaError::ParseError(e.to_string()))
    }

    /// Serialize to TOML string
    pub fn to_toml(&self) -> Result<String, SchemaError> {
        toml::to_string_pretty(self).map_err(|e| SchemaError::SerializeError(e.to_string()))
    }

    /// Parse from JSON string
    pub fn from_json(json_str: &str) -> Result<Self, SchemaError> {
        serde_json::from_str(json_str).map_err(|e| SchemaError::ParseError(e.to_string()))
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, SchemaError> {
        serde_json::to_string_pretty(self).map_err(|e| SchemaError::SerializeError(e.to_string()))
    }
}

/// Schema parsing/serialization errors
#[derive(Debug, Clone)]
pub enum SchemaError {
    /// Failed to parse schema
    ParseError(String),
    /// Failed to serialize schema
    SerializeError(String),
    /// Invalid field reference
    InvalidFieldReference(String),
    /// Duplicate field name
    DuplicateField(String),
    /// Invalid validation rule
    InvalidValidation(String),
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaError::ParseError(msg) => write!(f, "Schema parse error: {}", msg),
            SchemaError::SerializeError(msg) => write!(f, "Schema serialize error: {}", msg),
            SchemaError::InvalidFieldReference(field) => {
                write!(f, "Invalid field reference: {}", field)
            }
            SchemaError::DuplicateField(field) => write!(f, "Duplicate field: {}", field),
            SchemaError::InvalidValidation(msg) => write!(f, "Invalid validation: {}", msg),
        }
    }
}

impl std::error::Error for SchemaError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_reference_parse() {
        let vault_ref = SecretReference::parse("vault://secret/my-api-key");
        assert!(vault_ref.is_some());
        assert_eq!(vault_ref.unwrap().backend(), SecretBackend::Vault);

        let env_ref = SecretReference::parse("env://MY_API_KEY");
        assert!(env_ref.is_some());
        assert_eq!(env_ref.unwrap().backend(), SecretBackend::Environment);

        let invalid_ref = SecretReference::parse("not-a-secret-uri");
        assert!(invalid_ref.is_none());
    }

    #[test]
    fn test_config_schema_new() {
        let schema = ConfigSchema::new("test-plugin");
        assert_eq!(schema.plugin_name, "test-plugin");
        assert_eq!(schema.version, "1.0.0");
        assert!(schema.sections.is_empty());
    }

    #[test]
    fn test_config_field_defaults() {
        let field = ConfigField {
            name: "test_field".to_string(),
            label: None,
            description: None,
            field_type: ConfigFieldType::String,
            default: Some(serde_json::json!("default_value")),
            required: true,
            sensitive: false,
            validation: vec![],
            ui_hints: None,
            secret_ref: None,
            deprecated: None,
            env_var: None,
            reload_on_change: false,
        };

        assert!(field.required);
        assert!(!field.sensitive);
        assert_eq!(field.default, Some(serde_json::json!("default_value")));
    }

    #[test]
    fn test_config_schema_add_section() {
        let mut schema = ConfigSchema::new("test-plugin");
        let section = ConfigSection {
            name: "general".to_string(),
            title: Some("General Settings".to_string()),
            description: None,
            fields: vec![ConfigField {
                name: "enabled".to_string(),
                label: Some("Enabled".to_string()),
                description: Some("Enable the plugin".to_string()),
                field_type: ConfigFieldType::Boolean,
                default: Some(serde_json::json!(true)),
                required: false,
                sensitive: false,
                validation: vec![],
                ui_hints: None,
                secret_ref: None,
                deprecated: None,
                env_var: None,
                reload_on_change: true,
            }],
            order: Some(0),
            collapsed: false,
            icon: None,
        };

        schema.add_section(section);
        assert_eq!(schema.sections.len(), 1);
        assert!(schema.get_field("enabled").is_some());
    }
}
