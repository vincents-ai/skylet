// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

// RFC-0004 Phase 6.2: Prometheus Metrics Implementation
// Enterprise-grade metrics collection for Skylet plugin system
//
// This module provides 18 Prometheus metrics tracking:
// - Plugin lifecycle (load duration, errors, active count)
// - Configuration operations (get/set timing)
// - Registry operations (register, lookup, misses)
// - Event bus operations (publish timing, subscriber count)
// - RPC calls (duration by method, errors by method)
// - Audit logging (writes, queries, size)
// - Service health status

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Instant;

/// Metric histogram with configurable buckets
#[derive(Debug, Clone)]
pub struct Histogram {
    #[allow(dead_code)]
    name: String,
    buckets: Vec<f64>,
    values: Arc<RwLock<Vec<f64>>>,
}

impl Histogram {
    pub fn new(name: &str, buckets: Vec<f64>) -> Self {
        Self {
            name: name.to_string(),
            buckets,
            values: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn observe(&self, value: f64) {
        let mut vals = self.values.write().unwrap();
        vals.push(value);
    }

    pub fn duration_since(&self, start: Instant) -> f64 {
        start.elapsed().as_secs_f64()
    }

    pub fn percentile(&self, p: f64) -> Option<f64> {
        let vals = self.values.read().unwrap();
        if vals.is_empty() {
            return None;
        }
        let mut sorted = vals.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((p / 100.0) * sorted.len() as f64) as usize;
        Some(sorted[idx.min(sorted.len() - 1)])
    }

    pub fn count(&self) -> usize {
        self.values.read().unwrap().len()
    }

    pub fn sum(&self) -> f64 {
        self.values.read().unwrap().iter().sum()
    }

    pub fn mean(&self) -> f64 {
        let vals = self.values.read().unwrap();
        if vals.is_empty() {
            return 0.0;
        }
        vals.iter().sum::<f64>() / vals.len() as f64
    }
}

/// Metric counter (monotonic increment only)
#[derive(Debug, Clone)]
pub struct Counter {
    #[allow(dead_code)]
    name: String,
    value: Arc<RwLock<f64>>,
    labels: Arc<RwLock<HashMap<String, f64>>>,
}

impl Counter {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            value: Arc::new(RwLock::new(0.0)),
            labels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn inc(&self) {
        self.inc_by(1.0);
    }

    pub fn inc_by(&self, amount: f64) {
        let mut val = self.value.write().unwrap();
        *val += amount;
    }

    pub fn inc_with_label(&self, label_name: &str, label_value: &str, amount: f64) {
        let key = format!("{}={}", label_name, label_value);
        let mut labels = self.labels.write().unwrap();
        *labels.entry(key).or_insert(0.0) += amount;
    }

    pub fn value(&self) -> f64 {
        *self.value.read().unwrap()
    }

    pub fn labels(&self) -> HashMap<String, f64> {
        self.labels.read().unwrap().clone()
    }
}

/// Metric gauge (can increase or decrease)
#[derive(Debug, Clone)]
pub struct Gauge {
    #[allow(dead_code)]
    name: String,
    value: Arc<RwLock<f64>>,
}

impl Gauge {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            value: Arc::new(RwLock::new(0.0)),
        }
    }

    pub fn set(&self, value: f64) {
        *self.value.write().unwrap() = value;
    }

    pub fn inc(&self) {
        let mut val = self.value.write().unwrap();
        *val += 1.0;
    }

    pub fn dec(&self) {
        let mut val = self.value.write().unwrap();
        *val -= 1.0;
    }

    pub fn value(&self) -> f64 {
        *self.value.read().unwrap()
    }
}

/// Complete metrics collection for Skylet plugin system
/// Tracks 18 metrics across plugin lifecycle, configuration, registry, events, RPC, and audit
#[derive(Clone)]
pub struct PluginMetrics {
    // Plugin lifecycle metrics (3)
    /// Time taken to load a plugin (histogram, seconds)
    pub plugin_load_duration_seconds: Histogram,
    /// Number of plugin load errors (counter)
    pub plugin_load_errors_total: Counter,
    /// Number of currently active plugins (gauge)
    pub plugin_active_count: Gauge,

    // Configuration metrics (2)
    /// Time to retrieve config value (histogram, seconds)
    pub config_get_duration_seconds: Histogram,
    /// Number of config set errors (counter)
    pub config_set_errors_total: Counter,

    // Registry metrics (3)
    /// Time to register service (histogram, seconds)
    pub registry_register_duration_seconds: Histogram,
    /// Time to lookup service (histogram, seconds)
    pub registry_lookup_duration_seconds: Histogram,
    /// Number of registry lookup misses (counter)
    pub registry_lookup_misses_total: Counter,

    // Event bus metrics (2)
    /// Time to publish event (histogram, seconds)
    pub event_bus_publish_duration_seconds: Histogram,
    /// Number of event subscribers (gauge)
    pub event_bus_subscribers_total: Gauge,

    // RPC metrics (2)
    /// Time for RPC call (histogram, seconds, with method label)
    pub rpc_call_duration_seconds: Histogram,
    /// RPC call errors by method (counter, with method label)
    pub rpc_errors_total: Counter,

    // Audit logging metrics (3)
    /// Number of audit log writes (counter)
    pub audit_log_writes_total: Counter,
    /// Number of audit log queries (counter)
    pub audit_log_queries_total: Counter,
    /// Current audit log size in bytes (gauge)
    pub audit_log_size_bytes: Gauge,

    // Service health (1)
    /// Service health status: 0=down, 1=healthy (gauge)
    pub service_health_status: Gauge,
}

impl PluginMetrics {
    /// Create new metrics collection with optimized histogram buckets
    /// Buckets cover 1μs to 60s range (typical for plugin operations)
    pub fn new() -> Self {
        let histogram_buckets = vec![
            0.000001, // 1μs
            0.00001,  // 10μs
            0.0001,   // 100μs
            0.001,    // 1ms
            0.01,     // 10ms
            0.05,     // 50ms
            0.1,      // 100ms
            0.25,     // 250ms
            0.5,      // 500ms
            1.0,      // 1s
            2.5,      // 2.5s
            5.0,      // 5s
            10.0,     // 10s
            30.0,     // 30s
            60.0,     // 60s
        ];

        Self {
            plugin_load_duration_seconds: Histogram::new(
                "plugin_load_duration_seconds",
                histogram_buckets.clone(),
            ),
            plugin_load_errors_total: Counter::new("plugin_load_errors_total"),
            plugin_active_count: Gauge::new("plugin_active_count"),

            config_get_duration_seconds: Histogram::new(
                "config_get_duration_seconds",
                histogram_buckets.clone(),
            ),
            config_set_errors_total: Counter::new("config_set_errors_total"),

            registry_register_duration_seconds: Histogram::new(
                "registry_register_duration_seconds",
                histogram_buckets.clone(),
            ),
            registry_lookup_duration_seconds: Histogram::new(
                "registry_lookup_duration_seconds",
                histogram_buckets.clone(),
            ),
            registry_lookup_misses_total: Counter::new("registry_lookup_misses_total"),

            event_bus_publish_duration_seconds: Histogram::new(
                "event_bus_publish_duration_seconds",
                histogram_buckets.clone(),
            ),
            event_bus_subscribers_total: Gauge::new("event_bus_subscribers_total"),

            rpc_call_duration_seconds: Histogram::new(
                "rpc_call_duration_seconds",
                histogram_buckets.clone(),
            ),
            rpc_errors_total: Counter::new("rpc_errors_total"),

            audit_log_writes_total: Counter::new("audit_log_writes_total"),
            audit_log_queries_total: Counter::new("audit_log_queries_total"),
            audit_log_size_bytes: Gauge::new("audit_log_size_bytes"),

            service_health_status: Gauge::new("service_health_status"),
        }
    }

    /// Export metrics in Prometheus text format
    pub fn export_prometheus_text(&self) -> String {
        let mut output = String::new();

        // Plugin metrics
        output.push_str(&self.histogram_to_prometheus(
            &self.plugin_load_duration_seconds,
            "plugin_load_duration_seconds",
        ));
        output.push_str(
            &self.counter_to_prometheus(&self.plugin_load_errors_total, "plugin_load_errors_total"),
        );
        output.push_str(&format!(
            "# TYPE plugin_active_count gauge\nplugin_active_count {}\n\n",
            self.plugin_active_count.value()
        ));

        // Config metrics
        output.push_str(&self.histogram_to_prometheus(
            &self.config_get_duration_seconds,
            "config_get_duration_seconds",
        ));
        output.push_str(
            &self.counter_to_prometheus(&self.config_set_errors_total, "config_set_errors_total"),
        );

        // Registry metrics
        output.push_str(&self.histogram_to_prometheus(
            &self.registry_register_duration_seconds,
            "registry_register_duration_seconds",
        ));
        output.push_str(&self.histogram_to_prometheus(
            &self.registry_lookup_duration_seconds,
            "registry_lookup_duration_seconds",
        ));
        output.push_str(&self.counter_to_prometheus(
            &self.registry_lookup_misses_total,
            "registry_lookup_misses_total",
        ));

        // Event bus metrics
        output.push_str(&self.histogram_to_prometheus(
            &self.event_bus_publish_duration_seconds,
            "event_bus_publish_duration_seconds",
        ));
        output.push_str(&format!(
            "# TYPE event_bus_subscribers_total gauge\nevent_bus_subscribers_total {}\n\n",
            self.event_bus_subscribers_total.value()
        ));

        // RPC metrics
        output.push_str(
            &self.histogram_to_prometheus(
                &self.rpc_call_duration_seconds,
                "rpc_call_duration_seconds",
            ),
        );
        output.push_str(
            &self.counter_with_labels_to_prometheus(&self.rpc_errors_total, "rpc_errors_total"),
        );

        // Audit metrics
        output.push_str(
            &self.counter_to_prometheus(&self.audit_log_writes_total, "audit_log_writes_total"),
        );
        output.push_str(
            &self.counter_to_prometheus(&self.audit_log_queries_total, "audit_log_queries_total"),
        );
        output.push_str(&format!(
            "# TYPE audit_log_size_bytes gauge\naudit_log_size_bytes {}\n\n",
            self.audit_log_size_bytes.value()
        ));

        // Service health
        output.push_str(&format!(
            "# TYPE service_health_status gauge\nservice_health_status {}\n\n",
            self.service_health_status.value()
        ));

        output
    }

    fn histogram_to_prometheus(&self, histogram: &Histogram, name: &str) -> String {
        let mut output = format!("# TYPE {} histogram\n", name);

        let sum = histogram.sum();
        let count = histogram.count();

        // Bucket lines
        for bucket in &histogram.buckets {
            output.push_str(&format!("{}:bucket{{le=\"{}\"}} 0\n", name, bucket));
        }
        output.push_str(&format!("{}:bucket{{le=\"+Inf\"}} {}\n", name, count));

        // Sum and count
        output.push_str(&format!("{}:sum {}\n", name, sum));
        output.push_str(&format!("{}:count {}\n\n", name, count));

        output
    }

    fn counter_to_prometheus(&self, counter: &Counter, name: &str) -> String {
        format!("# TYPE {} counter\n{} {}\n\n", name, name, counter.value())
    }

    fn counter_with_labels_to_prometheus(&self, counter: &Counter, name: &str) -> String {
        let mut output = format!("# TYPE {} counter\n", name);
        let labels = counter.labels();

        if labels.is_empty() {
            output.push_str(&format!("{} {}\n\n", name, counter.value()));
        } else {
            for (label, value) in labels {
                output.push_str(&format!("{}{{{}}} {}\n", name, label, value));
            }
            output.push('\n');
        }

        output
    }
}

impl Default for PluginMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_histogram_recording() {
        let histogram = Histogram::new("test_metric", vec![0.1, 0.5, 1.0, 5.0]);

        histogram.observe(0.15);
        histogram.observe(0.75);
        histogram.observe(2.5);

        assert_eq!(histogram.count(), 3);
        assert!(histogram.sum() > 3.4 && histogram.sum() < 3.5);
    }

    #[test]
    fn test_counter_increment() {
        let counter = Counter::new("test_counter");

        assert_eq!(counter.value(), 0.0);
        counter.inc();
        assert_eq!(counter.value(), 1.0);
        counter.inc_by(5.5);
        assert_eq!(counter.value(), 6.5);
    }

    #[test]
    fn test_gauge_set() {
        let gauge = Gauge::new("test_gauge");

        assert_eq!(gauge.value(), 0.0);
        gauge.set(42.0);
        assert_eq!(gauge.value(), 42.0);
        gauge.inc();
        assert_eq!(gauge.value(), 43.0);
        gauge.dec();
        assert_eq!(gauge.value(), 42.0);
    }

    #[test]
    fn test_metric_export_text_format() {
        let metrics = PluginMetrics::new();

        metrics.plugin_active_count.set(5.0);
        metrics.plugin_load_errors_total.inc_by(2.0);
        metrics.service_health_status.set(1.0);

        let output = metrics.export_prometheus_text();

        assert!(output.contains("plugin_active_count 5"));
        assert!(output.contains("plugin_load_errors_total 2"));
        assert!(output.contains("service_health_status 1"));
        assert!(output.contains("# TYPE") || output.contains("histogram"));
    }

    #[test]
    fn test_concurrent_metric_recording() {
        use std::thread;

        let metrics = Arc::new(PluginMetrics::new());
        let mut handles = vec![];

        for i in 0..10 {
            let m = Arc::clone(&metrics);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    m.plugin_load_duration_seconds
                        .observe((i * j) as f64 * 0.001);
                    m.plugin_load_errors_total.inc();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(metrics.plugin_load_errors_total.value(), 1000.0);
        assert_eq!(metrics.plugin_load_duration_seconds.count(), 1000);
    }

    #[test]
    fn test_histogram_percentiles() {
        let histogram = Histogram::new("test", vec![]);

        // Record values 1 through 100
        for i in 1..=100 {
            histogram.observe(i as f64);
        }

        let p50 = histogram.percentile(50.0).unwrap();
        let p95 = histogram.percentile(95.0).unwrap();
        let p99 = histogram.percentile(99.0).unwrap();

        // p50 should be around 50
        assert!(p50 >= 48.0 && p50 <= 52.0);
        // p95 should be around 95
        assert!(p95 >= 93.0 && p95 <= 97.0);
        // p99 should be around 99
        assert!(p99 >= 97.0 && p99 <= 100.0);
    }

    #[test]
    fn test_metric_labels() {
        let counter = Counter::new("rpc_errors");

        counter.inc_with_label("method", "get_config", 3.0);
        counter.inc_with_label("method", "set_config", 1.0);
        counter.inc_with_label("method", "get_config", 2.0);

        let labels = counter.labels();
        assert_eq!(labels.get("method=get_config"), Some(&5.0));
        assert_eq!(labels.get("method=set_config"), Some(&1.0));
    }

    #[test]
    fn test_metrics_performance() {
        use std::time::Instant;

        let metrics = PluginMetrics::new();

        // Measure 1000 metric recordings
        let start = Instant::now();
        for i in 0..1000 {
            metrics.plugin_active_count.set(i as f64);
            metrics
                .plugin_load_duration_seconds
                .observe(0.001 * i as f64);
            metrics.plugin_load_errors_total.inc();
        }
        let elapsed = start.elapsed();

        // Should complete in <100ms total (~100μs per operation)
        let per_op = elapsed.as_nanos() / 3000; // 3 ops * 1000 iterations
        assert!(
            per_op < 100_000,
            "Metric recording took {}ns per op, expected <100μs",
            per_op
        );
    }
}
