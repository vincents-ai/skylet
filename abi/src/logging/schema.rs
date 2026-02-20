// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! RFC-0018: Structured Logging Schema
//!
//! This module provides the formal JSON schema definition for structured logging
//! in the Skylet ecosystem. All log events must conform to this schema.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// RFC-0018 Log Event Schema
///
/// The canonical structure for all log events in Skylet.
/// This schema is designed for:
/// - Structured JSON logging
/// - Distributed tracing correlation
/// - Cross-plugin log aggregation
/// - Performance analysis and debugging
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogEvent {
    /// ISO 8601 timestamp with microsecond precision (e.g., "2024-02-03T14:25:30.123456Z")
    pub timestamp: String,

    /// Log level: TRACE, DEBUG, INFO, WARN, ERROR
    pub level: LogLevel,

    /// Human-readable log message
    pub message: String,

    /// Name of the plugin that generated this log event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,

    /// Distributed tracing trace ID for request correlation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,

    /// Distributed tracing span ID for nested operation tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,

    /// Correlation ID for linking related events across plugins
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,

    /// Parent span ID for span hierarchy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,

    /// Additional structured key-value data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,

    /// Error information if this is an error log
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,

    /// Source code location (file, line, function)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceLocation>,

    /// Request context for HTTP/API operations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<RequestContext>,
}

/// Log level enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "TRACE" => Ok(Self::Trace),
            "DEBUG" => Ok(Self::Debug),
            "INFO" => Ok(Self::Info),
            "WARN" | "WARNING" => Ok(Self::Warn),
            "ERROR" | "ERR" => Ok(Self::Error),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

impl From<crate::PluginLogLevel> for LogLevel {
    fn from(level: crate::PluginLogLevel) -> Self {
        match level {
            crate::PluginLogLevel::Trace => LogLevel::Trace,
            crate::PluginLogLevel::Debug => LogLevel::Debug,
            crate::PluginLogLevel::Info => LogLevel::Info,
            crate::PluginLogLevel::Warn => LogLevel::Warn,
            crate::PluginLogLevel::Error => LogLevel::Error,
        }
    }
}

impl From<LogLevel> for crate::PluginLogLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => crate::PluginLogLevel::Trace,
            LogLevel::Debug => crate::PluginLogLevel::Debug,
            LogLevel::Info => crate::PluginLogLevel::Info,
            LogLevel::Warn => crate::PluginLogLevel::Warn,
            LogLevel::Error => crate::PluginLogLevel::Error,
        }
    }
}

/// Error information for structured error logging
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorInfo {
    /// Error type/name
    pub error_type: String,

    /// Error message
    pub message: String,

    /// Stack trace (debug builds only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_trace: Option<String>,

    /// Error code if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    /// Nested error cause
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<Box<ErrorInfo>>,
}

/// Source code location for debugging
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceLocation {
    /// Source file path
    pub file: String,

    /// Line number
    pub line: u32,

    /// Function/method name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,

    /// Module path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
}

/// Request context for HTTP/API operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequestContext {
    /// HTTP method
    pub method: String,

    /// Request path
    pub path: String,

    /// Request ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,

    /// User ID if authenticated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    /// Client IP address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_ip: Option<String>,

    /// User agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

/// Generate an ISO 8601 timestamp using only std library
fn current_timestamp() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let nanos = duration.subsec_nanos();

    // Convert to datetime components
    let days = secs / 86400;
    let (year, month, day) = days_to_ymd(days);
    let remaining_secs = secs % 86400;
    let hours = remaining_secs / 3600;
    let minutes = (remaining_secs % 3600) / 60;
    let seconds = remaining_secs % 60;
    let micros = nanos / 1000;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z",
        year, month, day, hours, minutes, seconds, micros
    )
}

/// Convert days since Unix epoch to year/month/day
fn days_to_ymd(days: u64) -> (i32, u32, u32) {
    // Unix epoch: 1970-01-01
    let mut year = 1970i32;
    let mut remaining_days = days;

    loop {
        let days_in_year = if is_leap_year(year) { 366u64 } else { 365u64 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let days_in_months: [u64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for &days_in_month in &days_in_months {
        if remaining_days < days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }

    let day = remaining_days + 1;
    (year, month, day as u32)
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

impl LogEvent {
    /// Create a new log event with the current timestamp
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            timestamp: current_timestamp(),
            level,
            message: message.into(),
            plugin_id: None,
            trace_id: None,
            span_id: None,
            correlation_id: None,
            parent_span_id: None,
            metadata: None,
            error: None,
            source: None,
            request: None,
        }
    }

    /// Create a log event with a specific timestamp
    pub fn with_timestamp(level: LogLevel, message: impl Into<String>, timestamp: String) -> Self {
        Self {
            timestamp,
            level,
            message: message.into(),
            plugin_id: None,
            trace_id: None,
            span_id: None,
            correlation_id: None,
            parent_span_id: None,
            metadata: None,
            error: None,
            source: None,
            request: None,
        }
    }

    /// Set the plugin ID
    pub fn with_plugin_id(mut self, plugin_id: impl Into<String>) -> Self {
        self.plugin_id = Some(plugin_id.into());
        self
    }

    /// Set the trace ID
    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    /// Set the span ID
    pub fn with_span_id(mut self, span_id: impl Into<String>) -> Self {
        self.span_id = Some(span_id.into());
        self
    }

    /// Set the correlation ID
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Set the parent span ID
    pub fn with_parent_span_id(mut self, parent_span_id: impl Into<String>) -> Self {
        self.parent_span_id = Some(parent_span_id.into());
        self
    }

    /// Add metadata key-value pair
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value);
        self
    }

    /// Set error information
    pub fn with_error(mut self, error: ErrorInfo) -> Self {
        self.error = Some(error);
        self
    }

    /// Set source location
    pub fn with_source(mut self, source: SourceLocation) -> Self {
        self.source = Some(source);
        self
    }

    /// Set request context
    pub fn with_request(mut self, request: RequestContext) -> Self {
        self.request = Some(request);
        self
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Convert to pretty JSON string
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// JSON Schema for RFC-0018 Log Events
///
/// This is the canonical JSON Schema definition that can be used for validation
/// in external tools, documentation generation, and schema registries.
pub fn rfc0018_json_schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://skynet.dev/schemas/rfc0018-log-event.json",
        "title": "RFC-0018 Log Event",
        "description": "Structured log event schema for Skylet distributed tracing",
        "type": "object",
        "required": ["timestamp", "level", "message"],
        "properties": {
            "timestamp": {
                "type": "string",
                "format": "date-time",
                "description": "ISO 8601 timestamp with microsecond precision",
                "examples": ["2024-02-03T14:25:30.123456Z"]
            },
            "level": {
                "type": "string",
                "enum": ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"],
                "description": "Log severity level"
            },
            "message": {
                "type": "string",
                "description": "Human-readable log message",
                "minLength": 1
            },
            "plugin_id": {
                "type": "string",
                "description": "Name of the plugin that generated this log event"
            },
            "trace_id": {
                "type": "string",
                "description": "Distributed tracing trace ID for request correlation",
                "pattern": "^[a-f0-9]{32}$"
            },
            "span_id": {
                "type": "string",
                "description": "Distributed tracing span ID for nested operation tracking",
                "pattern": "^[a-f0-9]{16}$"
            },
            "correlation_id": {
                "type": "string",
                "description": "Correlation ID for linking related events across plugins"
            },
            "parent_span_id": {
                "type": "string",
                "description": "Parent span ID for span hierarchy"
            },
            "metadata": {
                "type": "object",
                "description": "Additional structured key-value data",
                "additionalProperties": true
            },
            "error": {
                "type": "object",
                "description": "Error information if this is an error log",
                "properties": {
                    "error_type": {"type": "string"},
                    "message": {"type": "string"},
                    "stack_trace": {"type": "string"},
                    "code": {"type": "string"}
                },
                "required": ["error_type", "message"]
            },
            "source": {
                "type": "object",
                "description": "Source code location for debugging",
                "properties": {
                    "file": {"type": "string"},
                    "line": {"type": "integer", "minimum": 1},
                    "function": {"type": "string"},
                    "module": {"type": "string"}
                },
                "required": ["file", "line"]
            },
            "request": {
                "type": "object",
                "description": "Request context for HTTP and API operations",
                "properties": {
                    "method": {"type": "string"},
                    "path": {"type": "string"},
                    "request_id": {"type": "string"},
                    "user_id": {"type": "string"},
                    "client_ip": {"type": "string"},
                    "user_agent": {"type": "string"}
                },
                "required": ["method", "path"]
            }
        }
    })
}

/// JSON Schema string for RFC-0018 Log Events (for external tools)
pub const RFC0018_JSON_SCHEMA: &str = include_str!("rfc0018_schema.json");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_event_creation() {
        let event = LogEvent::new(LogLevel::Info, "Test message")
            .with_plugin_id("test-plugin")
            .with_trace_id("12345678901234567890123456789012")
            .with_correlation_id("corr-123");

        assert_eq!(event.level, LogLevel::Info);
        assert_eq!(event.message, "Test message");
        assert_eq!(event.plugin_id, Some("test-plugin".to_string()));
        assert!(event.timestamp.contains('T')); // ISO 8601 format
    }

    #[test]
    fn test_log_event_serialization() {
        let event = LogEvent::new(LogLevel::Error, "Something went wrong")
            .with_plugin_id("my-plugin")
            .with_metadata("key", serde_json::json!("value"));

        let json = event.to_json().unwrap();
        assert!(json.contains("\"level\":\"ERROR\""));
        assert!(json.contains("\"message\":\"Something went wrong\""));
        assert!(json.contains("\"plugin_id\":\"my-plugin\""));

        let parsed = LogEvent::from_json(&json).unwrap();
        assert_eq!(parsed, event);
    }

    #[test]
    fn test_log_level_parsing() {
        assert_eq!("INFO".parse::<LogLevel>().unwrap(), LogLevel::Info);
        assert_eq!("debug".parse::<LogLevel>().unwrap(), LogLevel::Debug);
        assert_eq!("WARN".parse::<LogLevel>().unwrap(), LogLevel::Warn);
        assert!("invalid".parse::<LogLevel>().is_err());
    }

    #[test]
    fn test_json_schema_valid() {
        let schema: serde_json::Value = serde_json::from_str(RFC0018_JSON_SCHEMA).unwrap();
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
    }

    #[test]
    fn test_timestamp_format() {
        let ts = current_timestamp();
        // Should match: 2024-02-03T14:25:30.123456Z
        assert!(ts.len() == 27);
        assert!(ts.ends_with('Z'));
        assert!(ts.chars().nth(4) == Some('-'));
        assert!(ts.chars().nth(7) == Some('-'));
        assert!(ts.chars().nth(10) == Some('T'));
        assert!(ts.chars().nth(13) == Some(':'));
        assert!(ts.chars().nth(16) == Some(':'));
        assert!(ts.chars().nth(19) == Some('.'));
    }
}
