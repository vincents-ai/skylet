// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Correlation ID Propagation Utilities for RFC-0018
//!
//! This module provides utilities for propagating correlation IDs and tracing
//! context across plugin boundaries in the Skylet ecosystem.

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use super::schema::LogEvent;

thread_local! {
    static CURRENT_CONTEXT: RefCell<Option<TracingContext>> = const { RefCell::new(None) };
}

/// Counter for generating short span IDs
static SPAN_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Counter for generating trace IDs
static TRACE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Tracing context for correlation ID propagation across plugin boundaries
#[derive(Debug, Clone, PartialEq)]
pub struct TracingContext {
    /// Unique trace ID (32 hex characters)
    pub trace_id: String,

    /// Current span ID (16 hex characters)
    pub span_id: String,

    /// Parent span ID (16 hex characters, optional)
    pub parent_span_id: Option<String>,

    /// Correlation ID for business logic correlation
    pub correlation_id: Option<String>,

    /// Plugin ID that originated this context
    pub source_plugin: Option<String>,
}

impl Default for TracingContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TracingContext {
    /// Create a new tracing context with a fresh trace ID and span ID
    pub fn new() -> Self {
        Self {
            trace_id: generate_trace_id(),
            span_id: generate_span_id(),
            parent_span_id: None,
            correlation_id: None,
            source_plugin: None,
        }
    }

    /// Create a child context (same trace, new span, parent = current span)
    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: generate_span_id(),
            parent_span_id: Some(self.span_id.clone()),
            correlation_id: self.correlation_id.clone(),
            source_plugin: self.source_plugin.clone(),
        }
    }

    /// Create a context with a specific correlation ID
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Create a context with a specific source plugin
    pub fn with_source_plugin(mut self, plugin_id: impl Into<String>) -> Self {
        self.source_plugin = Some(plugin_id.into());
        self
    }

    /// Set this context as the current thread-local context
    pub fn set_current(&self) {
        CURRENT_CONTEXT.with(|ctx| {
            *ctx.borrow_mut() = Some(self.clone());
        });
    }

    /// Get the current thread-local context (if any)
    pub fn current() -> Option<Self> {
        CURRENT_CONTEXT.with(|ctx| ctx.borrow().clone())
    }

    /// Clear the current thread-local context
    pub fn clear_current() {
        CURRENT_CONTEXT.with(|ctx| {
            *ctx.borrow_mut() = None;
        });
    }

    /// Apply this context to a log event
    pub fn apply_to_event(&self, event: &mut LogEvent) {
        event.trace_id = Some(self.trace_id.clone());
        event.span_id = Some(self.span_id.clone());
        event.parent_span_id = self.parent_span_id.clone();
        event.correlation_id = self.correlation_id.clone();
        if let Some(ref plugin) = self.source_plugin {
            event.plugin_id = Some(plugin.clone());
        }
    }

    /// Convert to JSON for cross-plugin transport
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&TracingContextJson {
            trace_id: &self.trace_id,
            span_id: &self.span_id,
            parent_span_id: self.parent_span_id.as_deref(),
            correlation_id: self.correlation_id.as_deref(),
            source_plugin: self.source_plugin.as_deref(),
        })
    }

    /// Parse from JSON for cross-plugin transport
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let ctx: TracingContextJson = serde_json::from_str(json)?;
        Ok(Self {
            trace_id: ctx.trace_id.to_string(),
            span_id: ctx.span_id.to_string(),
            parent_span_id: ctx.parent_span_id.map(|s| s.to_string()),
            correlation_id: ctx.correlation_id.map(|s| s.to_string()),
            source_plugin: ctx.source_plugin.map(|s| s.to_string()),
        })
    }
}

/// Serializable tracing context for JSON transport
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TracingContextJson<'a> {
    trace_id: &'a str,
    span_id: &'a str,
    parent_span_id: Option<&'a str>,
    correlation_id: Option<&'a str>,
    source_plugin: Option<&'a str>,
}

/// Generate a new trace ID (32 hex characters) using rand
fn generate_trace_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let counter = TRACE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let random_part: u64 = rng.gen();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    // Combine counter, random, and timestamp for uniqueness
    let part1 = counter.wrapping_add(random_part);
    let part2 = timestamp.wrapping_add(random_part.wrapping_mul(3));

    format!("{:016x}{:016x}", part1, part2)
}

/// Generate a new span ID (16 hex characters)
fn generate_span_id() -> String {
    let counter = SPAN_COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    let combined = counter.wrapping_add(timestamp);
    format!("{:016x}", combined.wrapping_mul(0x5851F42D4C957F2D))
}

/// Scope guard for automatic context cleanup
pub struct TracingScope {
    previous_context: Option<TracingContext>,
}

impl TracingScope {
    /// Create a new scope with the given context
    pub fn new(context: TracingContext) -> Self {
        let previous_context = TracingContext::current();
        context.set_current();
        Self { previous_context }
    }
}

impl Drop for TracingScope {
    fn drop(&mut self) {
        if let Some(ctx) = self.previous_context.take() {
            ctx.set_current();
        } else {
            TracingContext::clear_current();
        }
    }
}

/// RAII wrapper for creating a child span within a scope
pub struct SpanGuard {
    _scope: TracingScope,
}

impl SpanGuard {
    /// Create a new child span from the current context
    pub fn new(name: &str) -> Self {
        let parent = TracingContext::current().unwrap_or_default();
        let child = parent.child().with_source_plugin(name);
        Self {
            _scope: TracingScope::new(child),
        }
    }

    /// Create a new child span with a specific correlation ID
    pub fn with_correlation(name: &str, correlation_id: impl Into<String>) -> Self {
        let parent = TracingContext::current().unwrap_or_default();
        let child = parent
            .child()
            .with_source_plugin(name)
            .with_correlation_id(correlation_id);
        Self {
            _scope: TracingScope::new(child),
        }
    }
}

/// Propagate tracing context to a closure
pub fn with_context<F, T>(context: &TracingContext, f: F) -> T
where
    F: FnOnce() -> T,
{
    let _scope = TracingScope::new(context.clone());
    f()
}

/// Propagate a child context to a closure
pub fn with_child_context<F, T>(f: F) -> T
where
    F: FnOnce(&TracingContext) -> T,
{
    let parent = TracingContext::current().unwrap_or_default();
    let child = parent.child();
    let _scope = TracingScope::new(child.clone());
    f(&child)
}

/// Cross-plugin context propagation via shared memory
///
/// This is used when plugins need to pass context through the service registry
/// or event bus. The context is serialized to JSON and stored in a thread-safe
/// wrapper.
#[derive(Debug)]
pub struct SharedTracingContext {
    inner: Arc<RwLock<Option<TracingContext>>>,
}

impl Clone for SharedTracingContext {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Default for SharedTracingContext {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedTracingContext {
    /// Create a new empty shared context
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from an existing context
    pub fn from_context(context: TracingContext) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Some(context))),
        }
    }

    /// Set the context
    pub fn set(&self, context: TracingContext) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = Some(context);
        }
    }

    /// Get the context
    pub fn get(&self) -> Option<TracingContext> {
        self.inner.read().ok()?.clone()
    }

    /// Clear the context
    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.write() {
            *guard = None;
        }
    }

    /// Execute a closure with the shared context set as current
    pub fn with_current<F, T>(&self, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        if let Some(ctx) = self.get() {
            let _scope = TracingScope::new(ctx);
            f()
        } else {
            f()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_id_format() {
        let id = generate_trace_id();
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_span_id_format() {
        let id = generate_span_id();
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_context_child() {
        let parent = TracingContext::new()
            .with_correlation_id("corr-123")
            .with_source_plugin("parent-plugin");

        let child = parent.child();

        // Same trace ID
        assert_eq!(child.trace_id, parent.trace_id);

        // Different span ID
        assert_ne!(child.span_id, parent.span_id);

        // Parent span ID is set
        assert_eq!(child.parent_span_id, Some(parent.span_id));

        // Correlation ID propagated
        assert_eq!(child.correlation_id, Some("corr-123".to_string()));
    }

    #[test]
    fn test_context_thread_local() {
        let ctx = TracingContext::new().with_correlation_id("test-corr");

        ctx.set_current();
        let current = TracingContext::current();
        assert!(current.is_some());
        assert_eq!(
            current.unwrap().correlation_id,
            Some("test-corr".to_string())
        );

        TracingContext::clear_current();
        assert!(TracingContext::current().is_none());
    }

    #[test]
    fn test_scope_guard() {
        let ctx1 = TracingContext::new().with_correlation_id("ctx1");
        ctx1.set_current();

        {
            let ctx2 = TracingContext::new().with_correlation_id("ctx2");
            let _scope = TracingScope::new(ctx2);

            let current = TracingContext::current().unwrap();
            assert_eq!(current.correlation_id, Some("ctx2".to_string()));
        }

        // Should restore ctx1
        let current = TracingContext::current().unwrap();
        assert_eq!(current.correlation_id, Some("ctx1".to_string()));
    }

    #[test]
    fn test_json_serialization() {
        let ctx = TracingContext::new()
            .with_correlation_id("corr-456")
            .with_source_plugin("test-plugin");

        let json = ctx.to_json().unwrap();
        let parsed = TracingContext::from_json(&json).unwrap();

        assert_eq!(ctx, parsed);
    }

    #[test]
    fn test_shared_context() {
        let shared = SharedTracingContext::new();
        assert!(shared.get().is_none());

        let ctx = TracingContext::new().with_correlation_id("shared-corr");
        shared.set(ctx.clone());

        let retrieved = shared.get().unwrap();
        assert_eq!(retrieved.correlation_id, Some("shared-corr".to_string()));

        shared.clear();
        assert!(shared.get().is_none());
    }
}
