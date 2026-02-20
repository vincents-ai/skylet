// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Span Management for RFC-0017
//!
//! This module provides span creation, management, and lifecycle handling
//! for distributed tracing in Skylet.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use super::TracingError;

/// Unique identifier for a trace (32 hex characters)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(String);

impl TraceId {
    /// Generate a new random trace ID
    pub fn new() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let part1: u64 = rng.gen();
        let part2: u64 = rng.gen();
        Self(format!("{:016x}{:016x}", part1, part2))
    }

    /// Create a trace ID from a string
    pub fn from_string(s: impl Into<String>) -> Result<Self, TracingError> {
        let s = s.into();
        if s.len() != 32 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(TracingError::InvalidContext(
                "Trace ID must be 32 hex characters".to_string(),
            ));
        }
        Ok(Self(s))
    }

    /// Get the trace ID as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique identifier for a span (16 hex characters)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpanId(String);

impl SpanId {
    /// Generate a new random span ID
    pub fn new() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let value: u64 = rng.gen();
        Self(format!("{:016x}", value))
    }

    /// Create a span ID from a string
    pub fn from_string(s: impl Into<String>) -> Result<Self, TracingError> {
        let s = s.into();
        if s.len() != 16 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(TracingError::InvalidContext(
                "Span ID must be 16 hex characters".to_string(),
            ));
        }
        Ok(Self(s))
    }

    /// Get the span ID as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SpanId {
    fn default() -> Self {
        Self::new()
    }
}

/// Span context containing trace and span identifiers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanContext {
    /// Trace ID (shared across all spans in a trace)
    pub trace_id: TraceId,

    /// Span ID (unique to this span)
    pub span_id: SpanId,

    /// Parent span ID (None for root spans)
    pub parent_span_id: Option<SpanId>,

    /// Whether this span is sampled
    pub sampled: bool,
}

impl SpanContext {
    /// Create a new span context for a root span
    pub fn root() -> Self {
        Self {
            trace_id: TraceId::new(),
            span_id: SpanId::new(),
            parent_span_id: None,
            sampled: true,
        }
    }

    /// Create a child context from a parent
    pub fn child(parent: &SpanContext) -> Self {
        Self {
            trace_id: parent.trace_id.clone(),
            span_id: SpanId::new(),
            parent_span_id: Some(parent.span_id.clone()),
            sampled: parent.sampled,
        }
    }

    /// Convert to W3C traceparent format
    pub fn to_traceparent(&self) -> String {
        let flags = if self.sampled { "01" } else { "00" };
        format!(
            "00-{}-{}-{}",
            self.trace_id.as_str(),
            self.span_id.as_str(),
            flags
        )
    }

    /// Parse from W3C traceparent format
    pub fn from_traceparent(traceparent: &str) -> Result<Self, TracingError> {
        let parts: Vec<&str> = traceparent.split('-').collect();
        if parts.len() != 4 {
            return Err(TracingError::InvalidContext(
                "Invalid traceparent format".to_string(),
            ));
        }

        let trace_id = TraceId::from_string(parts[1])?;
        let span_id = SpanId::from_string(parts[2])?;
        let sampled = parts[3] == "01";

        Ok(Self {
            trace_id,
            span_id,
            parent_span_id: None,
            sampled,
        })
    }
}

/// A span event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    /// Event name
    pub name: String,

    /// Event timestamp (Unix nanoseconds)
    pub timestamp: u64,

    /// Event attributes
    pub attributes: HashMap<String, String>,
}

/// Span status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    /// Span is still being recorded
    Unset,

    /// Span completed successfully
    Ok,

    /// Span encountered an error
    Error,
}

/// A tracing span
#[derive(Debug, Clone)]
pub struct Span {
    /// Span context
    context: SpanContext,

    /// Span name
    name: String,

    /// Start time
    start_time: Instant,

    /// Span attributes
    attributes: Arc<RwLock<HashMap<String, String>>>,

    /// Span events
    events: Arc<RwLock<Vec<SpanEvent>>>,

    /// Span status
    status: Arc<RwLock<SpanStatus>>,

    /// Whether the span has ended
    ended: Arc<RwLock<bool>>,
}

impl Span {
    /// Create a new span
    fn new(name: impl Into<String>, context: SpanContext) -> Self {
        Self {
            context,
            name: name.into(),
            start_time: Instant::now(),
            attributes: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
            status: Arc::new(RwLock::new(SpanStatus::Unset)),
            ended: Arc::new(RwLock::new(false)),
        }
    }

    /// Get the span context
    pub fn context(&self) -> &SpanContext {
        &self.context
    }

    /// Get the span name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get elapsed time since span start
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Set an attribute on the span
    pub fn set_attribute(&self, key: impl Into<String>, value: impl Into<String>) {
        if let Ok(mut attrs) = self.attributes.write() {
            attrs.insert(key.into(), value.into());
        }
    }

    /// Add an event to the span
    pub fn add_event(&self, name: impl Into<String>) {
        self.add_event_with_attributes(name, HashMap::new());
    }

    /// Add an event with attributes
    pub fn add_event_with_attributes(
        &self,
        name: impl Into<String>,
        attributes: HashMap<String, String>,
    ) {
        if let Ok(mut events) = self.events.write() {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);

            events.push(SpanEvent {
                name: name.into(),
                timestamp,
                attributes,
            });
        }
    }

    /// Set span status
    pub fn set_status(&self, status: SpanStatus) {
        if let Ok(mut s) = self.status.write() {
            *s = status;
        }
    }

    /// End the span
    pub fn end(&self) {
        if let Ok(mut ended) = self.ended.write() {
            *ended = true;
        }
    }

    /// Check if the span has ended
    pub fn is_ended(&self) -> bool {
        self.ended.read().map(|e| *e).unwrap_or(false)
    }

    /// Get all attributes
    pub fn attributes(&self) -> HashMap<String, String> {
        self.attributes
            .read()
            .map(|attrs| attrs.clone())
            .unwrap_or_default()
    }

    /// Get all events
    pub fn events(&self) -> Vec<SpanEvent> {
        self.events
            .read()
            .map(|events| events.clone())
            .unwrap_or_default()
    }

    /// Get span status
    pub fn status(&self) -> SpanStatus {
        self.status.read().map(|s| *s).unwrap_or(SpanStatus::Unset)
    }
}

/// Builder for creating spans
pub struct SpanBuilder {
    name: String,
    parent_context: Option<SpanContext>,
    attributes: HashMap<String, String>,
}

impl SpanBuilder {
    /// Create a new span builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            parent_context: None,
            attributes: HashMap::new(),
        }
    }

    /// Set the parent context
    pub fn with_parent(mut self, context: SpanContext) -> Self {
        self.parent_context = Some(context);
        self
    }

    /// Add an attribute
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Build and start the span
    pub fn start(self, manager: &SpanManager) -> Span {
        let context = match self.parent_context {
            Some(parent) => SpanContext::child(&parent),
            None => SpanContext::root(),
        };

        let span = Span::new(self.name, context);

        // Apply initial attributes
        for (key, value) in self.attributes {
            span.set_attribute(key, value);
        }

        manager.register(span)
    }
}

/// Manager for tracking active spans
#[derive(Debug, Default)]
pub struct SpanManager {
    spans: Arc<RwLock<HashMap<SpanId, Span>>>,
}

impl SpanManager {
    /// Create a new span manager
    pub fn new() -> Self {
        Self {
            spans: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a span with the manager
    fn register(&self, span: Span) -> Span {
        let span_id = span.context().span_id.clone();
        if let Ok(mut spans) = self.spans.write() {
            spans.insert(span_id, span.clone());
        }
        span
    }

    /// Get a span by ID
    pub fn get(&self, span_id: &SpanId) -> Option<Span> {
        self.spans.read().ok()?.get(span_id).cloned()
    }

    /// Remove a span from tracking
    pub fn remove(&self, span_id: &SpanId) -> Option<Span> {
        self.spans.write().ok()?.remove(span_id)
    }

    /// Get all active spans
    pub fn active_spans(&self) -> Vec<Span> {
        self.spans
            .read()
            .map(|spans| spans.values().filter(|s| !s.is_ended()).cloned().collect())
            .unwrap_or_default()
    }

    /// Get span count
    pub fn len(&self) -> usize {
        self.spans.read().map(|s| s.len()).unwrap_or(0)
    }

    /// Check if manager is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_id_generation() {
        let id = TraceId::new();
        assert_eq!(id.as_str().len(), 32);
        assert!(id.as_str().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_span_id_generation() {
        let id = SpanId::new();
        assert_eq!(id.as_str().len(), 16);
        assert!(id.as_str().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_span_context_root() {
        let ctx = SpanContext::root();
        assert!(ctx.parent_span_id.is_none());
        assert!(ctx.sampled);
    }

    #[test]
    fn test_span_context_child() {
        let parent = SpanContext::root();
        let child = SpanContext::child(&parent);

        assert_eq!(child.trace_id, parent.trace_id);
        assert_ne!(child.span_id, parent.span_id);
        assert_eq!(child.parent_span_id, Some(parent.span_id));
    }

    #[test]
    fn test_traceparent_format() {
        let ctx = SpanContext::root();
        let traceparent = ctx.to_traceparent();

        assert!(traceparent.starts_with("00-"));
        assert_eq!(traceparent.len(), 55); // "00-" + 32 + "-" + 16 + "-" + 2

        let parsed = SpanContext::from_traceparent(&traceparent).unwrap();
        assert_eq!(parsed.trace_id, ctx.trace_id);
        assert_eq!(parsed.span_id, ctx.span_id);
    }

    #[test]
    fn test_span_attributes() {
        let manager = SpanManager::new();
        let span = SpanBuilder::new("test_span")
            .with_attribute("key1", "value1")
            .start(&manager);

        span.set_attribute("key2", "value2");

        let attrs = span.attributes();
        assert_eq!(attrs.get("key1"), Some(&"value1".to_string()));
        assert_eq!(attrs.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_span_events() {
        let ctx = SpanContext::root();
        let span = Span::new("test_span", ctx);

        span.add_event("event1");

        let events = span.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "event1");
    }

    #[test]
    fn test_span_lifecycle() {
        let manager = SpanManager::new();
        let span = SpanBuilder::new("test_span").start(&manager);

        assert!(!span.is_ended());
        span.end();
        assert!(span.is_ended());
    }

    #[test]
    fn test_span_manager() {
        let manager = SpanManager::new();
        let span = SpanBuilder::new("test1").start(&manager);
        let _span2 = SpanBuilder::new("test2").start(&manager);

        assert_eq!(manager.len(), 2);

        let active = manager.active_spans();
        assert_eq!(active.len(), 2);

        span.end();
        let active = manager.active_spans();
        assert_eq!(active.len(), 1);
    }
}
