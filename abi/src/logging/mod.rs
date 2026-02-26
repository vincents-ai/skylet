// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! RFC-0018: Structured Logging for Skylet
//!
//! This module provides structured logging capabilities with:
//! - Formal JSON schema definition
//! - Correlation ID propagation across plugin boundaries
//! - Distributed tracing context management
//!
//! # Example
//!
//! ```rust
//! use skylet_abi::logging::{LogEvent, LogLevel, TracingContext};
//!
//! // Create a log event with tracing context
//! let ctx = TracingContext::new()
//!     .with_correlation_id("order-12345")
//!     .with_source_plugin("order-processor");
//!
//! let event = LogEvent::new(LogLevel::Info, "Order processed")
//!     .with_plugin_id("order-processor")
//!     .with_trace_id(&ctx.trace_id)
//!     .with_span_id(&ctx.span_id)
//!     .with_correlation_id(&ctx.correlation_id.unwrap());
//!
//! tracing::info!("{}", event.to_json().unwrap());
//! ```

pub mod correlation;
pub mod schema;

// Re-export main types for convenience
pub use correlation::{
    with_child_context, with_context, SharedTracingContext, SpanGuard, TracingContext, TracingScope,
};
pub use schema::{
    rfc0018_json_schema, ErrorInfo, LogEvent, LogLevel, RequestContext, SourceLocation,
    RFC0018_JSON_SCHEMA,
};
