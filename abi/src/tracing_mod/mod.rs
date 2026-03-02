// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! RFC-0017: Distributed Tracing and Telemetry
//!
//! This module provides OpenTelemetry-based distributed tracing capabilities
//! for the Skylet plugin ecosystem. It enables:
//!
//! - **Distributed Tracing**: Track request flows across plugin boundaries
//! - **Context Propagation**: Automatic trace context propagation between plugins
//! - **Performance Metrics**: Collect and export performance metrics
//! - **Multiple Exporters**: Support for Jaeger, OTLP, and custom exporters
//!
//! # Architecture
//!
//! The tracing system integrates with RFC-0018 (Structured Logging) to provide
//! unified observability. The `TracingContext` from RFC-0018 is extended to
//! work with OpenTelemetry spans.
//!
//! # Example
//!
//! ```rust
//! use skylet_abi::tracing::{SpanManager, SpanBuilder};
//!
//! // Create a span manager
//! let manager = SpanManager::new();
//!
//! // Start a root span
//! let root_span = SpanBuilder::new("process_request")
//!     .with_attribute("http.method", "GET")
//!     .start(&manager);
//!
//! // Create a child span
//! let child_span = SpanBuilder::new("query_database")
//!     .with_parent(root_span.context())
//!     .start(&manager);
//!
//! // ... do work ...
//!
//! child_span.end();
//! root_span.end();
//! ```

pub mod context;
pub mod exporter;
pub mod metrics;
pub mod opentelemetry;
pub mod span;

// Re-export main types for convenience
pub use context::{TraceContextExt, W3CTraceContext};
pub use exporter::{ExporterConfig, TracingExporter};
// Note: PerformanceMetrics is already exported by lifecycle module, so we don't re-export it here
pub use metrics::{MetricCollector, MetricType};
pub use opentelemetry::{OtelTracer, SamplerConfig, TracerConfig};
pub use span::{Span, SpanBuilder, SpanContext, SpanId, SpanManager, TraceId};

/// Standard span attributes for Skylet plugins
#[allow(dead_code)] // Standard attribute keys for plugin span tagging
pub mod attributes {
    pub const PLUGIN_NAME: &str = "skylet.plugin.name";
    pub const PLUGIN_VERSION: &str = "skylet.plugin.version";
    pub const SERVICE_NAME: &str = "skylet.service.name";
    pub const SERVICE_METHOD: &str = "skylet.service.method";
    pub const EVENT_NAME: &str = "skylet.event.name";
    pub const CORRELATION_ID: &str = "skylet.correlation_id";
    pub const USER_ID: &str = "skylet.user.id";
}

/// Initialize the global tracing provider
///
/// This should be called once during Skylet startup to configure
/// the OpenTelemetry tracer provider with the specified exporter.
pub fn init_tracing(config: ExporterConfig) -> Result<(), TracingError> {
    opentelemetry::init_tracer(config)
}

/// Errors that can occur during tracing operations
#[derive(Debug, thiserror::Error)]
pub enum TracingError {
    #[error("Failed to initialize tracer: {0}")]
    InitializationFailed(String),

    #[error("Span not found: {0}")]
    SpanNotFound(String),

    #[error("Invalid trace context: {0}")]
    InvalidContext(String),

    #[error("Exporter error: {0}")]
    ExporterError(String),

    #[error("Metric collection error: {0}")]
    MetricError(String),
}
