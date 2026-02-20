// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

// Distributed tracing support for plugin system
//
// This module provides:
// - Span creation and management
// - Event recording within spans
// - Context propagation
// - Export to Jaeger, Zipkin, Grafana

use skylet_abi::{PluginTracer, SpanHandle};
use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

static SPAN_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Represents a distributed trace span
#[derive(Debug, Clone)]
pub struct Span {
    pub id: u64,
    pub name: String,
    pub parent_id: Option<u64>,
    pub start_time: u64, // Unix timestamp in nanoseconds
    pub end_time: Option<u64>,
    pub attributes: HashMap<String, String>,
    pub events: Vec<SpanEvent>,
}

/// Event recorded within a span
#[derive(Debug, Clone)]
pub struct SpanEvent {
    pub name: String,
    pub timestamp: u64,
    pub attributes: HashMap<String, String>,
}

/// Trace context for distributed tracing
#[derive(Debug, Clone)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: u64,
    pub parent_span_id: Option<u64>,
}

/// OpenTelemetry-compatible tracer for Skylet plugins
#[derive(Clone)]
pub struct SkyletPluginTracer {
    spans: Arc<std::sync::Mutex<HashMap<u64, Span>>>,
    #[allow(dead_code)]
    context: Arc<std::sync::Mutex<Option<TraceContext>>>,
    active_span: Arc<std::sync::Mutex<Option<u64>>>,
}

impl SkyletPluginTracer {
    /// Create a new plugin tracer with ABI integration
    pub fn new() -> Self {
        Self {
            spans: Arc::new(std::sync::Mutex::new(HashMap::new())),
            context: Arc::new(std::sync::Mutex::new(None)),
            active_span: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Create a new span with the given name
    pub fn create_span(&self, name: &str, parent_id: Option<u64>) -> u64 {
        let span_id = SPAN_COUNTER.fetch_add(1, Ordering::SeqCst);
        let now = current_time_nanos();

        let span = Span {
            id: span_id,
            name: name.to_string(),
            parent_id,
            start_time: now,
            end_time: None,
            attributes: HashMap::new(),
            events: Vec::new(),
        };

        let mut spans = self.spans.lock().unwrap();
        spans.insert(span_id, span);

        // Update active span
        *self.active_span.lock().unwrap() = Some(span_id);

        span_id
    }

    /// End a span
    pub fn end_span(&self, span_id: u64) {
        let mut spans = self.spans.lock().unwrap();
        if let Some(span) = spans.get_mut(&span_id) {
            span.end_time = Some(current_time_nanos());
        }

        // Restore parent as active span
        if let Some(span) = spans.get(&span_id) {
            *self.active_span.lock().unwrap() = span.parent_id;
        }
    }

    /// Add an event to the active span
    pub fn add_event(&self, name: &str, attributes: Option<HashMap<String, String>>) {
        if let Some(span_id) = *self.active_span.lock().unwrap() {
            let event = SpanEvent {
                name: name.to_string(),
                timestamp: current_time_nanos(),
                attributes: attributes.unwrap_or_default(),
            };

            let mut spans = self.spans.lock().unwrap();
            if let Some(span) = spans.get_mut(&span_id) {
                span.events.push(event);
            }
        }
    }

    /// Set an attribute on the active span
    pub fn set_attribute(&self, key: &str, value: &str) {
        if let Some(span_id) = *self.active_span.lock().unwrap() {
            let mut spans = self.spans.lock().unwrap();
            if let Some(span) = spans.get_mut(&span_id) {
                span.attributes.insert(key.to_string(), value.to_string());
            }
        }
    }

    /// Get all recorded spans
    pub fn get_spans(&self) -> Vec<Span> {
        self.spans.lock().unwrap().values().cloned().collect()
    }

    /// Export spans to Jaeger format (simplified)
    pub fn export_jaeger(&self) -> String {
        let spans = self.get_spans();
        let mut jaeger_spans = Vec::new();

        for span in spans {
            jaeger_spans.push(serde_json::json!({
                "traceID": "default-trace-id",
                "spanID": format!("{:016x}", span.id),
                "operationName": span.name,
                "startTime": span.start_time,
                "duration": span.end_time
                    .map(|et| et - span.start_time)
                    .unwrap_or(0),
                "tags": span.attributes,
                "logs": span.events.iter().map(|e| serde_json::json!({
                    "timestamp": e.timestamp,
                    "fields": [{
                        "key": "event",
                        "value": e.name
                    }]
                })).collect::<Vec<_>>(),
            }));
        }

        serde_json::to_string_pretty(&jaeger_spans).unwrap_or_default()
    }

    /// Export spans to Zipkin format (simplified)
    pub fn export_zipkin(&self) -> String {
        let spans = self.get_spans();
        let mut zipkin_spans = Vec::new();

        for span in spans {
            zipkin_spans.push(serde_json::json!({
                "traceId": "default-trace-id",
                "id": format!("{:016x}", span.id),
                "name": span.name,
                "timestamp": span.start_time / 1000,
                "duration": span.end_time
                    .map(|et| (et - span.start_time) / 1000)
                    .unwrap_or(0),
                "tags": span.attributes,
            }));
        }

        serde_json::to_string_pretty(&zipkin_spans).unwrap_or_default()
    }

    /// Export spans to DataDog format (simplified)
    pub fn export_datadog(&self) -> String {
        let spans = self.get_spans();
        let mut dd_spans = Vec::new();

        for span in spans {
            dd_spans.push(serde_json::json!({
                "trace_id": "default-trace-id",
                "span_id": span.id,
                "parent_id": span.parent_id.unwrap_or(0),
                "name": span.name,
                "start": span.start_time,
                "duration": span.end_time
                    .map(|et| et - span.start_time)
                    .unwrap_or(0),
                "tags": span.attributes,
            }));
        }

        serde_json::to_string_pretty(&dd_spans).unwrap_or_default()
    }
}

impl Default for SkyletPluginTracer {
    fn default() -> Self {
        Self::new()
    }
}

/// Lightweight tracing initialization for tests
pub fn init_tracing_for_tests() -> anyhow::Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    Ok(())
}

/// Create a minimal PluginTracer shim usable by plugins via the ABI
pub fn create_plugin_tracer() -> Box<PluginTracer> {
    Box::new(PluginTracer {
        start_span: start_span_ffi,
        end_span: end_span_ffi,
        add_event: add_event_ffi,
        set_attribute: set_attribute_ffi,
    })
}

extern "C" fn start_span_ffi(
    _context: *const (),
    name_ptr: *const c_char,
    _name_len: usize,
) -> SpanHandle {
    if !name_ptr.is_null() {
        let _ = unsafe { CStr::from_ptr(name_ptr).to_string_lossy() };
    }
    SPAN_COUNTER.fetch_add(1, Ordering::SeqCst)
}

extern "C" fn end_span_ffi(_context: *const (), _span_handle: SpanHandle) {
    // No-op shim for ABI compatibility
}

extern "C" fn add_event_ffi(_context: *const (), _name_ptr: *const c_char, _name_len: usize) {
    // No-op shim for ABI compatibility
}

extern "C" fn set_attribute_ffi(
    _context: *const (),
    _key_ptr: *const c_char,
    _key_len: usize,
    _value_ptr: *const c_char,
    _value_len: usize,
) {
    // No-op shim for ABI compatibility
}

/// Get current time in nanoseconds since UNIX_EPOCH
fn current_time_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_creation() {
        let tracer = SkyletPluginTracer::new();
        let span_id = tracer.create_span("test_span", None);

        assert!(span_id > 0);
        let spans = tracer.get_spans();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].name, "test_span");
    }

    #[test]
    fn test_span_attributes() {
        let tracer = SkyletPluginTracer::new();
        let _span_id = tracer.create_span("test", None);

        tracer.set_attribute("plugin_id", "my-plugin");
        tracer.set_attribute("method", "execute");

        let spans = tracer.get_spans();
        assert_eq!(spans[0].attributes.len(), 2);
        assert_eq!(
            spans[0].attributes.get("plugin_id"),
            Some(&"my-plugin".to_string())
        );
    }

    #[test]
    fn test_span_nesting() {
        let tracer = SkyletPluginTracer::new();
        let parent_id = tracer.create_span("parent", None);
        let child_id = tracer.create_span("child", Some(parent_id));

        let spans = tracer.get_spans();
        assert_eq!(spans.len(), 2);

        let child = spans.iter().find(|s| s.id == child_id).unwrap();
        assert_eq!(child.parent_id, Some(parent_id));
    }

    #[test]
    fn test_event_recording() {
        let tracer = SkyletPluginTracer::new();
        tracer.create_span("test", None);

        tracer.add_event("event1", None);
        let mut attrs = HashMap::new();
        attrs.insert("status".to_string(), "success".to_string());
        tracer.add_event("event2", Some(attrs));

        let spans = tracer.get_spans();
        assert_eq!(spans[0].events.len(), 2);
        assert_eq!(spans[0].events[0].name, "event1");
        assert_eq!(spans[0].events[1].name, "event2");
    }

    #[test]
    fn test_context_propagation() {
        let tracer = SkyletPluginTracer::new();
        let span1 = tracer.create_span("span1", None);
        tracer.set_attribute("request_id", "req123");

        let span2 = tracer.create_span("span2", Some(span1));

        let spans = tracer.get_spans();
        let s2 = spans.iter().find(|s| s.id == span2).unwrap();
        assert_eq!(s2.parent_id, Some(span1));
    }

    #[test]
    fn test_jaeger_export() {
        let tracer = SkyletPluginTracer::new();
        let span_id = tracer.create_span("jaeger_test", None);
        tracer.set_attribute("service", "test-service");
        tracer.end_span(span_id);

        let output = tracer.export_jaeger();
        assert!(output.contains("jaeger_test"));
        assert!(output.contains("test-service"));
    }

    #[test]
    fn test_zipkin_export() {
        let tracer = SkyletPluginTracer::new();
        let span_id = tracer.create_span("zipkin_test", None);
        tracer.end_span(span_id);

        let output = tracer.export_zipkin();
        assert!(output.contains("zipkin_test"));
        assert!(output.contains("traceId"));
    }

    #[test]
    fn test_datadog_export() {
        let tracer = SkyletPluginTracer::new();
        let span_id = tracer.create_span("dd_test", None);
        tracer.end_span(span_id);

        let output = tracer.export_datadog();
        assert!(output.contains("dd_test"));
        assert!(output.contains("trace_id"));
    }

    #[test]
    fn test_span_performance() {
        use std::time::Instant;

        let tracer = SkyletPluginTracer::new();
        let start = Instant::now();

        for i in 0..1000 {
            let span_id = tracer.create_span(&format!("span_{}", i), None);
            tracer.set_attribute("index", &i.to_string());
            tracer.end_span(span_id);
        }

        let elapsed = start.elapsed();
        let per_op_ns = elapsed.as_nanos() / 3000; // 3 ops per iteration

        // Should be <50μs per span operation
        assert!(
            per_op_ns < 50_000,
            "Span ops took {}ns, expected <50μs",
            per_op_ns
        );
    }

    #[test]
    fn test_concurrent_spans() {
        use std::thread;

        let tracer = Arc::new(SkyletPluginTracer::new());
        let mut handles = vec![];

        for i in 0..10 {
            let t = Arc::clone(&tracer);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let span_id = t.create_span(&format!("span_{}_{}", i, j), None);
                    t.set_attribute("thread", &i.to_string());
                    t.end_span(span_id);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let spans = tracer.get_spans();
        assert_eq!(spans.len(), 1000);
    }
}
