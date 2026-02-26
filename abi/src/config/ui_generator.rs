// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! UI Component Generator - RFC-0006
//!
//! Generates UI component definitions from configuration schemas
//! for use in admin interfaces, CLIs, and configuration editors.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::schema::{
    ConfigField, ConfigFieldType, ConfigSchema, ConfigSection, ValidationRule, WidgetType,
};

/// Generated UI component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIComponent {
    /// Component type
    pub component_type: UIComponentType,
    /// Field/section name
    pub name: String,
    /// Human-readable label
    pub label: String,
    /// Description/help text
    pub description: Option<String>,
    /// Whether this field is required
    pub required: bool,
    /// Whether this field is sensitive (password, secret)
    pub sensitive: bool,
    /// Default value
    pub default: Option<serde_json::Value>,
    /// Validation constraints for UI
    pub constraints: Option<UIConstraints>,
    /// Child components (for sections/objects)
    pub children: Option<Vec<UIComponent>>,
    /// Component-specific options
    pub options: Option<HashMap<String, serde_json::Value>>,
    /// Order in parent
    pub order: i32,
    /// Whether this is an advanced field
    pub advanced: bool,
    /// Icon name
    pub icon: Option<String>,
}

/// UI component type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UIComponentType {
    /// Section/group container
    Section,
    /// Text input field
    TextInput,
    /// Multiline text area
    TextArea,
    /// Password input
    PasswordInput,
    /// Number input
    NumberInput,
    /// Checkbox
    Checkbox,
    /// Toggle switch
    Toggle,
    /// Select dropdown
    Select,
    /// Multi-select
    MultiSelect,
    /// Radio button group
    RadioGroup,
    /// Slider
    Slider,
    /// Color picker
    ColorPicker,
    /// Date picker
    DatePicker,
    /// Time picker
    TimePicker,
    /// File picker
    FilePicker,
    /// Directory picker
    DirectoryPicker,
    /// Code editor
    CodeEditor,
    /// Secret reference input
    SecretReference,
    /// Custom component
    Custom,
}

/// UI validation constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIConstraints {
    /// Minimum value (for numbers)
    pub min: Option<f64>,
    /// Maximum value (for numbers)
    pub max: Option<f64>,
    /// Minimum length (for strings)
    pub min_length: Option<usize>,
    /// Maximum length (for strings)
    pub max_length: Option<usize>,
    /// Regular expression pattern
    pub pattern: Option<String>,
    /// Allowed values (for selects)
    pub allowed_values: Option<Vec<serde_json::Value>>,
    /// Step value (for sliders/numbers)
    pub step: Option<f64>,
    /// Placeholder text
    pub placeholder: Option<String>,
    /// File extensions (for file pickers)
    pub extensions: Option<Vec<String>>,
    /// Whether to show marks on slider
    pub slider_marks: Option<Vec<(f64, String)>>,
}

/// UI generator
pub struct UIGenerator {
    /// Whether to include advanced fields
    pub include_advanced: bool,
    /// Default placeholder text
    pub default_placeholder: String,
    /// Custom component mappings
    pub custom_mappings: HashMap<String, UIComponentType>,
}

impl UIGenerator {
    /// Create a new UI generator
    pub fn new() -> Self {
        Self {
            include_advanced: true,
            default_placeholder: "Enter value...".to_string(),
            custom_mappings: HashMap::new(),
        }
    }

    /// Generate UI components from a configuration schema
    pub fn generate(&self, schema: &ConfigSchema) -> Vec<UIComponent> {
        let mut sections: Vec<UIComponent> = schema
            .sections
            .iter()
            .map(|s| self.generate_section(s))
            .collect();

        // Sort by order
        sections.sort_by_key(|s| s.order);

        sections
    }

    /// Generate a section component
    fn generate_section(&self, section: &ConfigSection) -> UIComponent {
        let mut children: Vec<UIComponent> = section
            .fields
            .iter()
            .filter(|f| self.include_advanced || !self.is_advanced(f))
            .map(|f| self.generate_field(f))
            .collect();

        // Sort children by order
        children.sort_by_key(|c| c.order);

        UIComponent {
            component_type: UIComponentType::Section,
            name: section.name.clone(),
            label: section
                .title
                .clone()
                .unwrap_or_else(|| section.name.clone()),
            description: section.description.clone(),
            required: false,
            sensitive: false,
            default: None,
            constraints: None,
            children: Some(children),
            options: Some({
                let mut opts = HashMap::new();
                opts.insert(
                    "collapsed".to_string(),
                    serde_json::json!(section.collapsed),
                );
                opts
            }),
            order: section.order.unwrap_or(0),
            advanced: false,
            icon: section.icon.clone(),
        }
    }

    /// Generate a field component
    fn generate_field(&self, field: &ConfigField) -> UIComponent {
        let component_type = self.determine_component_type(field);
        let constraints = self.generate_constraints(field);
        let options = self.generate_options(field);

        UIComponent {
            component_type,
            name: field.name.clone(),
            label: field.label.clone().unwrap_or_else(|| field.name.clone()),
            description: field.description.clone(),
            required: field.required,
            sensitive: field.sensitive,
            default: field.default.clone(),
            constraints,
            children: None,
            options,
            order: field.ui_hints.as_ref().and_then(|h| h.order).unwrap_or(0),
            advanced: self.is_advanced(field),
            icon: None,
        }
    }

    /// Determine the UI component type for a field
    fn determine_component_type(&self, field: &ConfigField) -> UIComponentType {
        // Check for UI hints widget override
        if let Some(hints) = &field.ui_hints {
            if let Some(widget) = &hints.widget {
                return match widget {
                    WidgetType::TextInput => UIComponentType::TextInput,
                    WidgetType::TextArea => UIComponentType::TextArea,
                    WidgetType::Password => UIComponentType::PasswordInput,
                    WidgetType::NumberInput => UIComponentType::NumberInput,
                    WidgetType::Slider { .. } => UIComponentType::Slider,
                    WidgetType::Checkbox => UIComponentType::Checkbox,
                    WidgetType::Toggle => UIComponentType::Toggle,
                    WidgetType::Select => UIComponentType::Select,
                    WidgetType::RadioGroup => UIComponentType::RadioGroup,
                    WidgetType::MultiSelect => UIComponentType::MultiSelect,
                    WidgetType::ColorPicker => UIComponentType::ColorPicker,
                    WidgetType::DatePicker => UIComponentType::DatePicker,
                    WidgetType::TimePicker => UIComponentType::TimePicker,
                    WidgetType::FilePicker { .. } => UIComponentType::FilePicker,
                    WidgetType::DirectoryPicker => UIComponentType::DirectoryPicker,
                    WidgetType::CodeEditor { .. } => UIComponentType::CodeEditor,
                    WidgetType::MarkedSlider { .. } => UIComponentType::Slider,
                };
            }
        }

        // Auto-determine based on field type
        match &field.field_type {
            ConfigFieldType::String => {
                if field.sensitive {
                    UIComponentType::PasswordInput
                } else if field.secret_ref.is_some() {
                    UIComponentType::SecretReference
                } else {
                    UIComponentType::TextInput
                }
            }
            ConfigFieldType::Integer | ConfigFieldType::Float | ConfigFieldType::Port => {
                UIComponentType::NumberInput
            }
            ConfigFieldType::Boolean => UIComponentType::Toggle,
            ConfigFieldType::Array(_) => UIComponentType::MultiSelect,
            ConfigFieldType::Object => UIComponentType::Section,
            ConfigFieldType::Secret => UIComponentType::SecretReference,
            ConfigFieldType::Enum { .. } => UIComponentType::Select,
            ConfigFieldType::Path { is_dir, .. } => {
                if *is_dir {
                    UIComponentType::DirectoryPicker
                } else {
                    UIComponentType::FilePicker
                }
            }
            ConfigFieldType::Url { .. } | ConfigFieldType::Email | ConfigFieldType::Host => {
                UIComponentType::TextInput
            }
            ConfigFieldType::Duration => UIComponentType::TextInput,
        }
    }

    /// Generate validation constraints for a field
    fn generate_constraints(&self, field: &ConfigField) -> Option<UIConstraints> {
        let mut constraints = UIConstraints {
            min: None,
            max: None,
            min_length: None,
            max_length: None,
            pattern: None,
            allowed_values: None,
            step: None,
            placeholder: None,
            extensions: None,
            slider_marks: None,
        };

        // Extract constraints from validation rules
        for rule in &field.validation {
            match rule {
                ValidationRule::Min { value } => constraints.min = Some(*value),
                ValidationRule::Max { value } => constraints.max = Some(*value),
                ValidationRule::MinLength { value } => constraints.min_length = Some(*value),
                ValidationRule::MaxLength { value } => constraints.max_length = Some(*value),
                ValidationRule::Pattern { regex } => constraints.pattern = Some(regex.clone()),
                ValidationRule::OneOf { values } => {
                    constraints.allowed_values = Some(values.clone());
                }
                _ => {}
            }
        }

        // Add type-specific constraints
        match &field.field_type {
            ConfigFieldType::Integer | ConfigFieldType::Port => {
                constraints.step = Some(1.0);
            }
            ConfigFieldType::Enum { variants } => {
                constraints.allowed_values =
                    Some(variants.iter().map(|v| serde_json::json!(v)).collect());
            }
            ConfigFieldType::Path { is_dir: false, .. } => {
                // File picker - could set extensions from validation
            }
            _ => {}
        }

        // Add placeholder from UI hints
        if let Some(hints) = &field.ui_hints {
            constraints.placeholder = hints.placeholder.clone();
        }

        // Check if we have any constraints set
        if constraints.min.is_some()
            || constraints.max.is_some()
            || constraints.min_length.is_some()
            || constraints.max_length.is_some()
            || constraints.pattern.is_some()
            || constraints.allowed_values.is_some()
            || constraints.placeholder.is_some()
        {
            Some(constraints)
        } else {
            None
        }
    }

    /// Generate component-specific options
    fn generate_options(&self, field: &ConfigField) -> Option<HashMap<String, serde_json::Value>> {
        let mut options = HashMap::new();

        // Add env var option
        if let Some(env_var) = &field.env_var {
            options.insert("env_var".to_string(), serde_json::json!(env_var));
        }

        // Add reload on change option
        if field.reload_on_change {
            options.insert("reload_on_change".to_string(), serde_json::json!(true));
        }

        // Add deprecation message
        if let Some(deprecated) = &field.deprecated {
            options.insert("deprecated".to_string(), serde_json::json!(deprecated));
        }

        // Add widget-specific options from UI hints
        if let Some(hints) = &field.ui_hints {
            if let Some(help_text) = &hints.help_text {
                options.insert("help_text".to_string(), serde_json::json!(help_text));
            }
            if let Some(group) = &hints.group {
                options.insert("group".to_string(), serde_json::json!(group));
            }
            if let Some(css_class) = &hints.css_class {
                options.insert("css_class".to_string(), serde_json::json!(css_class));
            }
            if hints.autofocus {
                options.insert("autofocus".to_string(), serde_json::json!(true));
            }

            // Extract widget-specific options
            if let Some(widget) = &hints.widget {
                match widget {
                    WidgetType::Slider { min, max, step } => {
                        options.insert("slider_min".to_string(), serde_json::json!(min));
                        options.insert("slider_max".to_string(), serde_json::json!(max));
                        options.insert("slider_step".to_string(), serde_json::json!(step));
                    }
                    WidgetType::MarkedSlider { marks } => {
                        options.insert(
                            "slider_marks".to_string(),
                            serde_json::json!(marks
                                .iter()
                                .map(|(v, l)| serde_json::json!([v, l]))
                                .collect::<Vec<_>>()),
                        );
                    }
                    WidgetType::FilePicker { extensions } => {
                        options.insert("extensions".to_string(), serde_json::json!(extensions));
                    }
                    WidgetType::CodeEditor { language } => {
                        options.insert("language".to_string(), serde_json::json!(language));
                    }
                    _ => {}
                }
            }
        }

        // Add secret reference options
        if let Some(secret_ref) = &field.secret_ref {
            options.insert("secret_uri".to_string(), serde_json::json!(&secret_ref.uri));
            if let Some(key) = &secret_ref.key {
                options.insert("secret_key".to_string(), serde_json::json!(key));
            }
        }

        if options.is_empty() {
            None
        } else {
            Some(options)
        }
    }

    /// Check if a field is advanced
    fn is_advanced(&self, field: &ConfigField) -> bool {
        field.ui_hints.as_ref().map(|h| h.advanced).unwrap_or(false)
    }

    /// Generate a form schema for JSON Schema compatibility
    pub fn generate_json_schema(&self, schema: &ConfigSchema) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for section in &schema.sections {
            for field in &section.fields {
                let mut prop = serde_json::Map::new();

                prop.insert(
                    "type".to_string(),
                    serde_json::json!(self.json_schema_type(&field.field_type)),
                );

                if let Some(desc) = &field.description {
                    prop.insert("description".to_string(), serde_json::json!(desc));
                }

                if let Some(default) = &field.default {
                    prop.insert("default".to_string(), default.clone());
                }

                // Add validation constraints
                for rule in &field.validation {
                    match rule {
                        ValidationRule::Min { value } => {
                            prop.insert("minimum".to_string(), serde_json::json!(value));
                        }
                        ValidationRule::Max { value } => {
                            prop.insert("maximum".to_string(), serde_json::json!(value));
                        }
                        ValidationRule::MinLength { value } => {
                            prop.insert("minLength".to_string(), serde_json::json!(value));
                        }
                        ValidationRule::MaxLength { value } => {
                            prop.insert("maxLength".to_string(), serde_json::json!(value));
                        }
                        ValidationRule::Pattern { regex } => {
                            prop.insert("pattern".to_string(), serde_json::json!(regex));
                        }
                        ValidationRule::OneOf { values } => {
                            prop.insert("enum".to_string(), serde_json::json!(values));
                        }
                        _ => {}
                    }
                }

                properties.insert(field.name.clone(), serde_json::Value::Object(prop));

                if field.required {
                    required.push(field.name.clone());
                }
            }
        }

        serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": format!("{} Configuration", schema.plugin_name),
            "type": "object",
            "properties": properties,
            "required": required
        })
    }

    /// Get JSON Schema type for a field type
    fn json_schema_type(&self, field_type: &ConfigFieldType) -> &'static str {
        match field_type {
            ConfigFieldType::String
            | ConfigFieldType::Secret
            | ConfigFieldType::Path { .. }
            | ConfigFieldType::Url { .. }
            | ConfigFieldType::Duration
            | ConfigFieldType::Email
            | ConfigFieldType::Host => "string",
            ConfigFieldType::Integer | ConfigFieldType::Port => "integer",
            ConfigFieldType::Float => "number",
            ConfigFieldType::Boolean => "boolean",
            ConfigFieldType::Array(_) => "array",
            ConfigFieldType::Object => "object",
            ConfigFieldType::Enum { .. } => "string",
        }
    }
}

impl Default for UIGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UIHints;

    fn make_test_field() -> ConfigField {
        ConfigField {
            name: "test_field".to_string(),
            label: Some("Test Field".to_string()),
            description: Some("A test field".to_string()),
            field_type: ConfigFieldType::String,
            default: Some(serde_json::json!("default")),
            required: true,
            sensitive: false,
            validation: vec![
                ValidationRule::MinLength { value: 3 },
                ValidationRule::MaxLength { value: 100 },
            ],
            ui_hints: Some(UIHints {
                widget: None,
                placeholder: Some("Enter text...".to_string()),
                help_text: Some("Help text".to_string()),
                group: None,
                order: Some(1),
                advanced: false,
                css_class: None,
                autofocus: false,
            }),
            secret_ref: None,
            deprecated: None,
            env_var: None,
            reload_on_change: false,
        }
    }

    #[test]
    fn test_ui_generator_new() {
        let gen = UIGenerator::new();
        assert!(gen.include_advanced);
    }

    #[test]
    fn test_generate_field() {
        let gen = UIGenerator::new();
        let field = make_test_field();
        let component = gen.generate_field(&field);

        assert_eq!(component.name, "test_field");
        assert_eq!(component.label, "Test Field");
        assert!(component.required);
        assert_eq!(component.component_type, UIComponentType::TextInput);
    }

    #[test]
    fn test_determine_component_type_password() {
        let gen = UIGenerator::new();
        let mut field = make_test_field();
        field.sensitive = true;

        let component_type = gen.determine_component_type(&field);
        assert_eq!(component_type, UIComponentType::PasswordInput);
    }

    #[test]
    fn test_determine_component_type_boolean() {
        let gen = UIGenerator::new();
        let mut field = make_test_field();
        field.field_type = ConfigFieldType::Boolean;

        let component_type = gen.determine_component_type(&field);
        assert_eq!(component_type, UIComponentType::Toggle);
    }

    #[test]
    fn test_generate_constraints() {
        let gen = UIGenerator::new();
        let field = make_test_field();
        let constraints = gen.generate_constraints(&field);

        assert!(constraints.is_some());
        let c = constraints.unwrap();
        assert_eq!(c.min_length, Some(3));
        assert_eq!(c.max_length, Some(100));
        assert_eq!(c.placeholder, Some("Enter text...".to_string()));
    }

    #[test]
    fn test_generate_json_schema() {
        let gen = UIGenerator::new();
        let mut schema = ConfigSchema::new("test-plugin");
        schema.add_section(ConfigSection {
            name: "general".to_string(),
            title: None,
            description: None,
            fields: vec![make_test_field()],
            order: None,
            collapsed: false,
            icon: None,
        });

        let json_schema = gen.generate_json_schema(&schema);

        assert_eq!(json_schema["type"], "object");
        assert!(json_schema["properties"]
            .as_object()
            .unwrap()
            .contains_key("test_field"));
        assert!(json_schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("test_field")));
    }
}
