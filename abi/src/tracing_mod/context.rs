// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Trace Context Propagation for RFC-0017
//!
//! This module provides utilities for propagating trace context across
//! plugin boundaries and external services using W3C Trace Context format.

use crate::logging::TracingContext;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::span::SpanContext;
use super::TracingError;

/// W3C Trace Context header names
pub mod headers {
    pub const TRACEPARENT: &str = "traceparent";
    pub const TRACESTATE: &str = "tracestate";
}

/// W3C trace context format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct W3CTraceContext {
    /// Version (always 00 for W3C format)
    pub version: u8,

    /// Trace ID (32 hex characters)
    pub trace_id: String,

    /// Parent ID / Span ID (16 hex characters)
    pub parent_id: String,

    /// Trace flags (2 hex characters)
    pub flags: String,

    /// Trace state (vendor-specific key-value pairs)
    pub trace_state: HashMap<String, String>,
}

impl W3CTraceContext {
    /// Parse from traceparent header
    pub fn from_traceparent(header: &str) -> Result<Self, TracingError> {
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() != 4 {
            return Err(TracingError::InvalidContext(
                "Invalid traceparent format: expected 4 parts".to_string(),
            ));
        }

        let version = u8::from_str_radix(parts[0], 16)
            .map_err(|_| TracingError::InvalidContext("Invalid version".to_string()))?;

        if version != 0 {
            return Err(TracingError::InvalidContext(format!(
                "Unsupported version: {}",
                version
            )));
        }

        let trace_id = parts[1].to_string();
        let parent_id = parts[2].to_string();
        let flags = parts[3].to_string();

        // Validate lengths
        if trace_id.len() != 32 || !trace_id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(TracingError::InvalidContext(
                "Invalid trace ID: must be 32 hex characters".to_string(),
            ));
        }

        if parent_id.len() != 16 || !parent_id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(TracingError::InvalidContext(
                "Invalid parent ID: must be 16 hex characters".to_string(),
            ));
        }

        if flags.len() != 2 || !flags.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(TracingError::InvalidContext(
                "Invalid flags: must be 2 hex characters".to_string(),
            ));
        }

        Ok(Self {
            version,
            trace_id,
            parent_id,
            flags,
            trace_state: HashMap::new(),
        })
    }

    /// Convert to traceparent header
    pub fn to_traceparent(&self) -> String {
        format!(
            "{:02x}-{}-{}-{}",
            self.version, self.trace_id, self.parent_id, self.flags
        )
    }

    /// Parse tracestate header
    pub fn with_tracestate(mut self, header: &str) -> Result<Self, TracingError> {
        if header.is_empty() {
            return Ok(self);
        }

        // Format: key1=value1,key2=value2,...
        for entry in header.split(',') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }

            let parts: Vec<&str> = entry.splitn(2, '=').collect();
            if parts.len() != 2 {
                return Err(TracingError::InvalidContext(format!(
                    "Invalid tracestate entry: {}",
                    entry
                )));
            }

            self.trace_state
                .insert(parts[0].to_string(), parts[1].to_string());
        }

        Ok(self)
    }

    /// Convert to tracestate header
    pub fn to_tracestate(&self) -> String {
        self.trace_state
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Check if sampled flag is set
    pub fn is_sampled(&self) -> bool {
        self.flags == "01"
    }

    /// Set sampled flag
    pub fn set_sampled(&mut self, sampled: bool) {
        self.flags = if sampled { "01" } else { "00" }.to_string();
    }
}

impl From<SpanContext> for W3CTraceContext {
    fn from(ctx: SpanContext) -> Self {
        Self {
            version: 0,
            trace_id: ctx.trace_id.as_str().to_string(),
            parent_id: ctx.span_id.as_str().to_string(),
            flags: if ctx.sampled { "01" } else { "00" }.to_string(),
            trace_state: HashMap::new(),
        }
    }
}

impl TryFrom<W3CTraceContext> for SpanContext {
    type Error = TracingError;

    fn try_from(ctx: W3CTraceContext) -> Result<Self, Self::Error> {
        let sampled = ctx.is_sampled(); // Check sampled before moves
        Ok(SpanContext {
            trace_id: super::span::TraceId::from_string(ctx.trace_id)?,
            span_id: super::span::SpanId::from_string(ctx.parent_id)?,
            parent_span_id: None,
            sampled,
        })
    }
}

/// Extension trait for integrating with RFC-0018 TracingContext
pub trait TraceContextExt {
    /// Convert to W3C trace context
    fn to_w3c(&self) -> W3CTraceContext;

    /// Create from W3C trace context
    fn from_w3c(w3c: &W3CTraceContext) -> Self;

    /// Extract from HTTP headers
    fn from_headers(headers: &HashMap<String, String>) -> Option<Self>
    where
        Self: Sized;

    /// Inject into HTTP headers
    fn inject_headers(&self, headers: &mut HashMap<String, String>);
}

impl TraceContextExt for TracingContext {
    fn to_w3c(&self) -> W3CTraceContext {
        W3CTraceContext {
            version: 0,
            trace_id: self.trace_id.clone(),
            parent_id: self.span_id.clone(),
            flags: "01".to_string(), // Always sampled for now
            trace_state: HashMap::new(),
        }
    }

    fn from_w3c(w3c: &W3CTraceContext) -> Self {
        Self {
            trace_id: w3c.trace_id.clone(),
            span_id: w3c.parent_id.clone(),
            parent_span_id: None,
            correlation_id: None,
            source_plugin: None,
        }
    }

    fn from_headers(headers: &HashMap<String, String>) -> Option<Self> {
        let traceparent = headers.get(headers::TRACEPARENT)?;
        let w3c = W3CTraceContext::from_traceparent(traceparent).ok()?;

        let mut ctx = Self::from_w3c(&w3c);

        // Also extract tracestate if present
        if let Some(tracestate) = headers.get(headers::TRACESTATE) {
            if let Ok(w3c_with_state) = w3c.with_tracestate(tracestate) {
                // Store correlation ID from trace state if present
                if let Some(corr_id) = w3c_with_state.trace_state.get("skylet.correlation_id") {
                    ctx.correlation_id = Some(corr_id.clone());
                }
            }
        }

        Some(ctx)
    }

    fn inject_headers(&self, headers: &mut HashMap<String, String>) {
        let mut w3c = self.to_w3c();
        headers.insert(headers::TRACEPARENT.to_string(), w3c.to_traceparent());

        // Add correlation ID to trace state
        if let Some(ref corr_id) = self.correlation_id {
            w3c.trace_state
                .insert("skylet.correlation_id".to_string(), corr_id.clone());
        }

        let tracestate = w3c.to_tracestate();
        if !tracestate.is_empty() {
            headers.insert(headers::TRACESTATE.to_string(), tracestate);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_w3c_trace_context_parsing() {
        let header = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let ctx = W3CTraceContext::from_traceparent(header).unwrap();

        assert_eq!(ctx.version, 0);
        assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(ctx.parent_id, "00f067aa0ba902b7");
        assert_eq!(ctx.flags, "01");
        assert!(ctx.is_sampled());
    }

    #[test]
    fn test_w3c_trace_context_serialization() {
        let ctx = W3CTraceContext {
            version: 0,
            trace_id: "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
            parent_id: "00f067aa0ba902b7".to_string(),
            flags: "01".to_string(),
            trace_state: HashMap::new(),
        };

        let header = ctx.to_traceparent();
        assert_eq!(
            header,
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        );
    }

    #[test]
    fn test_trace_state_parsing() {
        let ctx = W3CTraceContext {
            version: 0,
            trace_id: "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
            parent_id: "00f067aa0ba902b7".to_string(),
            flags: "01".to_string(),
            trace_state: HashMap::new(),
        };

        let ctx = ctx
            .with_tracestate("rojo=00f067aa0ba902b7,congo=t61rcWkgMzE")
            .unwrap();

        assert_eq!(
            ctx.trace_state.get("rojo"),
            Some(&"00f067aa0ba902b7".to_string())
        );
        assert_eq!(
            ctx.trace_state.get("congo"),
            Some(&"t61rcWkgMzE".to_string())
        );
    }

    #[test]
    fn test_rfc0018_integration() {
        let rfc0018_ctx = TracingContext::new()
            .with_correlation_id("order-12345")
            .with_source_plugin("order-processor");

        // Convert to W3C
        let w3c = rfc0018_ctx.to_w3c();
        assert_eq!(w3c.version, 0);
        assert!(w3c.is_sampled());

        // Inject into headers
        let mut headers = HashMap::new();
        rfc0018_ctx.inject_headers(&mut headers);

        assert!(headers.contains_key(headers::TRACEPARENT));
        assert!(headers.contains_key(headers::TRACESTATE));

        // Extract from headers
        let extracted = TracingContext::from_headers(&headers).unwrap();
        assert_eq!(extracted.trace_id, rfc0018_ctx.trace_id);
        assert_eq!(extracted.correlation_id, Some("order-12345".to_string()));
    }

    #[test]
    fn test_invalid_traceparent() {
        // Too few parts
        assert!(W3CTraceContext::from_traceparent("00-abc-123").is_err());

        // Invalid version
        assert!(W3CTraceContext::from_traceparent("01-abc-123-01").is_err());

        // Invalid trace ID
        assert!(W3CTraceContext::from_traceparent("00-xyz-00f067aa0ba902b7-01").is_err());

        // Invalid parent ID
        assert!(
            W3CTraceContext::from_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-xyz-01")
                .is_err()
        );
    }
}
