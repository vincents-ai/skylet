// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Trace Exporters for RFC-0017
//!
//! This module provides exporters for sending trace data to various
//! backends (Jaeger, OTLP, custom).

use serde::{Deserialize, Serialize};

use super::TracingError;

/// Exporter type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExporterType {
    /// No exporter (disabled)
    None,

    /// OTLP exporter (OpenTelemetry Protocol)
    Otlp,

    /// Jaeger exporter
    Jaeger,

    /// Custom exporter (plugin-provided)
    Custom(String),
}

/// Configuration for trace exporters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExporterConfig {
    /// Exporter type
    pub exporter_type: ExporterType,

    /// Endpoint URL (for OTLP/Jaeger)
    pub endpoint: Option<String>,

    /// Service name
    pub service_name: String,

    /// Sample rate (0.0 - 1.0)
    pub sample_rate: f64,

    /// Export timeout in milliseconds
    pub export_timeout_ms: u64,

    /// Batch size for exporting
    pub batch_size: usize,

    /// Additional headers
    pub headers: std::collections::HashMap<String, String>,
}

impl Default for ExporterConfig {
    fn default() -> Self {
        Self {
            exporter_type: ExporterType::None,
            endpoint: None,
            service_name: "skylet".to_string(),
            sample_rate: 1.0,
            export_timeout_ms: 30000,
            batch_size: 512,
            headers: std::collections::HashMap::new(),
        }
    }
}

impl ExporterConfig {
    /// Create an OTLP exporter configuration
    pub fn otlp(endpoint: impl Into<String>) -> Self {
        Self {
            exporter_type: ExporterType::Otlp,
            endpoint: Some(endpoint.into()),
            ..Default::default()
        }
    }

    /// Create a Jaeger exporter configuration
    pub fn jaeger(endpoint: impl Into<String>) -> Self {
        Self {
            exporter_type: ExporterType::Jaeger,
            endpoint: Some(endpoint.into()),
            ..Default::default()
        }
    }

    /// Create a disabled exporter configuration
    pub fn disabled() -> Self {
        Self {
            exporter_type: ExporterType::None,
            ..Default::default()
        }
    }

    /// Set the service name
    pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = name.into();
        self
    }

    /// Set the sample rate
    pub fn with_sample_rate(mut self, rate: f64) -> Self {
        self.sample_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Add a header
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
}

/// Trait for trace exporters
pub trait TracingExporter: Send + Sync {
    /// Export a batch of spans
    fn export(&self, batch: &[ExportSpan]) -> Result<(), TracingError>;

    /// Flush any pending exports
    fn flush(&self) -> Result<(), TracingError>;

    /// Shutdown the exporter
    fn shutdown(&self) -> Result<(), TracingError>;
}

/// Serializable span data for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSpan {
    /// Trace ID (hex string)
    pub trace_id: String,

    /// Span ID (hex string)
    pub span_id: String,

    /// Parent span ID (hex string, optional)
    pub parent_span_id: Option<String>,

    /// Span name
    pub name: String,

    /// Start time (Unix nanoseconds)
    pub start_time: u64,

    /// End time (Unix nanoseconds)
    pub end_time: u64,

    /// Span attributes
    pub attributes: std::collections::HashMap<String, String>,

    /// Span events
    pub events: Vec<ExportEvent>,

    /// Span status
    pub status: ExportStatus,

    /// Service name
    pub service_name: String,
}

/// Serializable event for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportEvent {
    /// Event name
    pub name: String,

    /// Timestamp (Unix nanoseconds)
    pub timestamp: u64,

    /// Event attributes
    pub attributes: std::collections::HashMap<String, String>,
}

/// Span status for export
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportStatus {
    /// OK - span completed successfully
    Ok,

    /// Error - span encountered an error
    Error,
}

/// In-memory exporter for testing
#[derive(Debug, Default)]
pub struct InMemoryExporter {
    spans: std::sync::RwLock<Vec<ExportSpan>>,
}

#[allow(dead_code)]
impl InMemoryExporter {
    /// Create a new in-memory exporter
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all exported spans
    pub fn get_spans(&self) -> Vec<ExportSpan> {
        self.spans.read().map(|s| s.clone()).unwrap_or_default()
    }

    /// Clear all exported spans
    pub fn clear(&self) {
        if let Ok(mut spans) = self.spans.write() {
            spans.clear();
        }
    }
}

impl TracingExporter for InMemoryExporter {
    fn export(&self, batch: &[ExportSpan]) -> Result<(), TracingError> {
        if let Ok(mut spans) = self.spans.write() {
            spans.extend(batch.iter().cloned());
        }
        Ok(())
    }

    fn flush(&self) -> Result<(), TracingError> {
        Ok(())
    }

    fn shutdown(&self) -> Result<(), TracingError> {
        Ok(())
    }
}

/// No-op exporter (disabled tracing)
#[derive(Debug, Default)]
pub struct NoOpExporter;

impl NoOpExporter {
    /// Create a new no-op exporter
    pub fn new() -> Self {
        Self
    }
}

impl TracingExporter for NoOpExporter {
    fn export(&self, _batch: &[ExportSpan]) -> Result<(), TracingError> {
        Ok(())
    }

    fn flush(&self) -> Result<(), TracingError> {
        Ok(())
    }

    fn shutdown(&self) -> Result<(), TracingError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exporter_config() {
        let config = ExporterConfig::otlp("http://localhost:4317")
            .with_service_name("test-service")
            .with_sample_rate(0.5)
            .with_header("api-key", "secret");

        assert_eq!(config.exporter_type, ExporterType::Otlp);
        assert_eq!(config.service_name, "test-service");
        assert_eq!(config.sample_rate, 0.5);
        assert_eq!(config.headers.get("api-key"), Some(&"secret".to_string()));
    }

    #[test]
    fn test_sample_rate_clamping() {
        let config = ExporterConfig::default().with_sample_rate(2.0);
        assert_eq!(config.sample_rate, 1.0);

        let config = ExporterConfig::default().with_sample_rate(-0.5);
        assert_eq!(config.sample_rate, 0.0);
    }

    #[test]
    fn test_in_memory_exporter() {
        let exporter = InMemoryExporter::new();

        let span = ExportSpan {
            trace_id: "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
            span_id: "00f067aa0ba902b7".to_string(),
            parent_span_id: None,
            name: "test_span".to_string(),
            start_time: 0,
            end_time: 1000,
            attributes: std::collections::HashMap::new(),
            events: vec![],
            status: ExportStatus::Ok,
            service_name: "test".to_string(),
        };

        exporter.export(&[span.clone()]).unwrap();

        let spans = exporter.get_spans();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].name, "test_span");

        exporter.clear();
        assert!(exporter.get_spans().is_empty());
    }

    #[test]
    fn test_noop_exporter() {
        let exporter = NoOpExporter::new();

        let span = ExportSpan {
            trace_id: "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
            span_id: "00f067aa0ba902b7".to_string(),
            parent_span_id: None,
            name: "test_span".to_string(),
            start_time: 0,
            end_time: 1000,
            attributes: std::collections::HashMap::new(),
            events: vec![],
            status: ExportStatus::Ok,
            service_name: "test".to_string(),
        };

        // Should succeed without errors
        assert!(exporter.export(&[span]).is_ok());
        assert!(exporter.flush().is_ok());
        assert!(exporter.shutdown().is_ok());
    }
}
