// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Unified MCP tool schema types for the Skylet plugin ABI.
//!
//! All plugins that expose MCP tools should use these types for tool schema
//! definitions. The MCP gateway plugins aggregate these schemas to serve
//! `tools/list` responses and dispatch `tools/call` requests.
//!
//! # Usage
//!
//! Plugins implement `plugin_get_tools() -> *const c_char` returning a
//! JSON-serialized `Vec<ToolSchema>`. The gateway discovers tools by calling
//! this function on each loaded plugin.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MCP tool schema definition following the Model Context Protocol spec.
///
/// Each tool has a unique name, a human-readable description, and an input
/// schema that describes the parameters the tool accepts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolSchema {
    /// Unique tool name (e.g. "embedding_embed", "vector_search").
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema describing the tool's input parameters.
    pub input_schema: InputSchema,
}

/// JSON Schema-style input definition for a tool's parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputSchema {
    /// Always "object" for MCP tool inputs.
    #[serde(rename = "type")]
    pub schema_type: String,
    /// Map of parameter name to property schema.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, PropertySchema>,
    /// List of required parameter names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,
}

/// Property schema describing a single tool parameter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PropertySchema {
    /// JSON Schema type: "string", "number", "integer", "boolean", "array", "object".
    #[serde(rename = "type")]
    pub schema_type: String,
    /// Human-readable description of the parameter.
    pub description: String,
    /// For array types, the schema of each item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<PropertySchema>>,
    /// For string types with enumerated values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#enum: Option<Vec<String>>,
    /// Default value for the parameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    /// Nested properties for object types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, PropertySchema>>,
}

// ---------------------------------------------------------------------------
// Builder helpers
// ---------------------------------------------------------------------------

impl ToolSchema {
    /// Create a new tool schema.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: InputSchema {
                schema_type: "object".to_string(),
                properties: HashMap::new(),
                required: Vec::new(),
            },
        }
    }

    /// Add a parameter to the tool schema.
    pub fn param(mut self, name: impl Into<String>, schema: PropertySchema) -> Self {
        self.input_schema.properties.insert(name.into(), schema);
        self
    }

    /// Add a required parameter to the tool schema.
    pub fn required_param(mut self, name: impl Into<String>, schema: PropertySchema) -> Self {
        let name = name.into();
        self.input_schema.properties.insert(name.clone(), schema);
        self.input_schema.required.push(name);
        self
    }
}

impl PropertySchema {
    /// Create a string property.
    pub fn string(description: impl Into<String>) -> Self {
        Self {
            schema_type: "string".to_string(),
            description: description.into(),
            items: None,
            r#enum: None,
            default: None,
            properties: None,
        }
    }

    /// Create a string property with enumerated allowed values.
    pub fn string_enum(description: impl Into<String>, values: &[&str]) -> Self {
        Self {
            schema_type: "string".to_string(),
            description: description.into(),
            items: None,
            r#enum: Some(values.iter().map(|s| (*s).to_string()).collect()),
            default: None,
            properties: None,
        }
    }

    /// Create a number property.
    pub fn number(description: impl Into<String>) -> Self {
        Self {
            schema_type: "number".to_string(),
            description: description.into(),
            items: None,
            r#enum: None,
            default: None,
            properties: None,
        }
    }

    /// Create an integer property.
    pub fn integer(description: impl Into<String>) -> Self {
        Self {
            schema_type: "integer".to_string(),
            description: description.into(),
            items: None,
            r#enum: None,
            default: None,
            properties: None,
        }
    }

    /// Create a boolean property.
    pub fn boolean(description: impl Into<String>) -> Self {
        Self {
            schema_type: "boolean".to_string(),
            description: description.into(),
            items: None,
            r#enum: None,
            default: None,
            properties: None,
        }
    }

    /// Create an array property with the given item schema.
    pub fn array(description: impl Into<String>, items: PropertySchema) -> Self {
        Self {
            schema_type: "array".to_string(),
            description: description.into(),
            items: Some(Box::new(items)),
            r#enum: None,
            default: None,
            properties: None,
        }
    }

    /// Create an object property with nested properties.
    pub fn object(
        description: impl Into<String>,
        properties: HashMap<String, PropertySchema>,
    ) -> Self {
        Self {
            schema_type: "object".to_string(),
            description: description.into(),
            items: None,
            r#enum: None,
            default: None,
            properties: Some(properties),
        }
    }

    /// Set a default value on this property.
    pub fn with_default(mut self, default: serde_json::Value) -> Self {
        self.default = Some(default);
        self
    }
}

impl InputSchema {
    /// Create an empty object input schema.
    pub fn empty() -> Self {
        Self {
            schema_type: "object".to_string(),
            properties: HashMap::new(),
            required: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_schema_builder() {
        let schema = ToolSchema::new("my_tool", "Does something useful")
            .required_param("input", PropertySchema::string("The input text"))
            .param(
                "format",
                PropertySchema::string_enum("Output format", &["json", "text"]),
            );

        assert_eq!(schema.name, "my_tool");
        assert_eq!(schema.description, "Does something useful");
        assert_eq!(schema.input_schema.required, vec!["input"]);
        assert_eq!(schema.input_schema.properties.len(), 2);
        assert!(schema.input_schema.properties.contains_key("input"));
        assert!(schema.input_schema.properties.contains_key("format"));
    }

    #[test]
    fn test_property_schema_types() {
        let s = PropertySchema::string("a string");
        assert_eq!(s.schema_type, "string");

        let n = PropertySchema::number("a number");
        assert_eq!(n.schema_type, "number");

        let i = PropertySchema::integer("an integer");
        assert_eq!(i.schema_type, "integer");

        let b = PropertySchema::boolean("a bool");
        assert_eq!(b.schema_type, "boolean");

        let a = PropertySchema::array("items", PropertySchema::string("item"));
        assert_eq!(a.schema_type, "array");
        assert!(a.items.is_some());
    }

    #[test]
    fn test_string_enum() {
        let p = PropertySchema::string_enum("pick one", &["a", "b", "c"]);
        let values = p.r#enum.unwrap();
        assert_eq!(values, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_default_value() {
        let p = PropertySchema::integer("count").with_default(serde_json::json!(10));
        assert_eq!(p.default, Some(serde_json::json!(10)));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let schema = ToolSchema::new("test_tool", "A test tool")
            .required_param("query", PropertySchema::string("Search query"))
            .param(
                "limit",
                PropertySchema::integer("Max results").with_default(serde_json::json!(10)),
            );

        let json = serde_json::to_string(&schema).unwrap();
        let parsed: ToolSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(schema, parsed);
    }

    #[test]
    fn test_vec_serialization() {
        let schemas = vec![
            ToolSchema::new("tool_a", "First tool")
                .required_param("x", PropertySchema::string("param x")),
            ToolSchema::new("tool_b", "Second tool"),
        ];

        let json = serde_json::to_string(&schemas).unwrap();
        let parsed: Vec<ToolSchema> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "tool_a");
        assert_eq!(parsed[1].name, "tool_b");
    }

    #[test]
    fn test_empty_input_schema() {
        let schema = InputSchema::empty();
        assert_eq!(schema.schema_type, "object");
        assert!(schema.properties.is_empty());
        assert!(schema.required.is_empty());
    }

    #[test]
    fn test_camel_case_serialization() {
        let schema = ToolSchema::new("test", "test tool")
            .required_param("q", PropertySchema::string("query"));

        let json = serde_json::to_value(&schema).unwrap();
        // MCP spec uses camelCase: inputSchema not input_schema
        assert!(json.get("inputSchema").is_some());
        assert!(json.get("input_schema").is_none());
    }
}
