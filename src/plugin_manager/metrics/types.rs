// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metric types supported by the metrics system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

/// Metric data types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
    Summary {
        count: u64,
        sum: f64,
        min: f64,
        max: f64,
        quantiles: Vec<(f64, f64)>,
    },
}

impl MetricValue {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            MetricValue::Counter(v) => Some(*v as f64),
            MetricValue::Gauge(v) => Some(*v),
            MetricValue::Histogram(v) => v.first().copied(),
            MetricValue::Summary { sum, count, .. } => {
                if *count > 0 {
                    Some(sum / *count as f64)
                } else {
                    None
                }
            }
        }
    }
}

/// A single metric data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub value: MetricValue,
    pub timestamp: DateTime<Utc>,
    pub labels: HashMap<String, String>,
    pub metric_type: MetricType,
}

impl Metric {
    pub fn counter(name: String, value: u64) -> Self {
        Self {
            name,
            value: MetricValue::Counter(value),
            timestamp: Utc::now(),
            labels: HashMap::new(),
            metric_type: MetricType::Counter,
        }
    }

    pub fn gauge(name: String, value: f64) -> Self {
        Self {
            name,
            value: MetricValue::Gauge(value),
            timestamp: Utc::now(),
            labels: HashMap::new(),
            metric_type: MetricType::Gauge,
        }
    }

    pub fn histogram(name: String, values: Vec<f64>) -> Self {
        Self {
            name,
            value: MetricValue::Histogram(values),
            timestamp: Utc::now(),
            labels: HashMap::new(),
            metric_type: MetricType::Histogram,
        }
    }

    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }

    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels.extend(labels);
        self
    }
}

/// Performance metrics for a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub total_latency_ms: f64,
    pub min_latency_ms: f64,
    pub max_latency_ms: f64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub throughput_rps: f64,
    pub error_rate: f64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            total_calls: 0,
            successful_calls: 0,
            failed_calls: 0,
            total_latency_ms: 0.0,
            min_latency_ms: f64::MAX,
            max_latency_ms: 0.0,
            avg_latency_ms: 0.0,
            p50_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            throughput_rps: 0.0,
            error_rate: 0.0,
        }
    }
}

impl PerformanceMetrics {
    pub fn record_call(&mut self, latency_ms: f64, success: bool) {
        self.total_calls += 1;
        self.total_latency_ms += latency_ms;
        self.min_latency_ms = self.min_latency_ms.min(latency_ms);
        self.max_latency_ms = self.max_latency_ms.max(latency_ms);

        if success {
            self.successful_calls += 1;
        } else {
            self.failed_calls += 1;
        }

        self.avg_latency_ms = self.total_latency_ms / self.total_calls as f64;
        self.error_rate = (self.failed_calls as f64) / self.total_calls as f64;
    }
}

/// Resource usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetrics {
    pub memory_mb: f64,
    pub cpu_percent: f64,
    pub thread_count: usize,
    pub file_handles: usize,
    pub timestamp: DateTime<Utc>,
}

impl Default for ResourceMetrics {
    fn default() -> Self {
        Self {
            memory_mb: 0.0,
            cpu_percent: 0.0,
            thread_count: 0,
            file_handles: 0,
            timestamp: Utc::now(),
        }
    }
}

/// Comprehensive metrics for a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetrics {
    pub plugin_name: String,
    pub performance_metrics: PerformanceMetrics,
    pub resource_metrics: Vec<ResourceMetrics>,
    pub custom_metrics: HashMap<String, Metric>,
    pub start_time: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
}

impl PluginMetrics {
    pub fn new(plugin_name: String) -> Self {
        let now = Utc::now();
        Self {
            plugin_name,
            performance_metrics: PerformanceMetrics::default(),
            resource_metrics: Vec::new(),
            custom_metrics: HashMap::new(),
            start_time: now,
            last_update: now,
        }
    }

    pub fn update_resource_metrics(&mut self, metrics: ResourceMetrics) {
        self.resource_metrics.push(metrics);
        self.last_update = Utc::now();

        if self.resource_metrics.len() > 1000 {
            self.resource_metrics.truncate(1000);
        }
    }

    pub fn record_custom_metric(&mut self, metric: Metric) {
        self.custom_metrics.insert(metric.name.clone(), metric);
        self.last_update = Utc::now();
    }

    pub fn uptime(&self) -> chrono::Duration {
        Utc::now() - self.start_time
    }

    pub fn health_score(&self) -> f64 {
        if self.performance_metrics.total_calls == 0 {
            return 1.0;
        }

        let error_penalty = 1.0 - self.performance_metrics.error_rate;
        let latency_score = if self.performance_metrics.avg_latency_ms < 100.0 {
            1.0
        } else if self.performance_metrics.avg_latency_ms < 1000.0 {
            0.7
        } else {
            0.4
        };

        error_penalty * latency_score
    }
}

/// Query for filtering metrics
#[derive(Debug, Clone, Default)]
pub struct MetricQuery {
    pub plugin_name: Option<String>,
    pub metric_name: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub labels: HashMap<String, String>,
    pub limit: Option<usize>,
}

impl MetricQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_plugin(mut self, plugin: String) -> Self {
        self.plugin_name = Some(plugin);
        self
    }

    pub fn with_metric(mut self, metric: String) -> Self {
        self.metric_name = Some(metric);
        self
    }

    pub fn with_time_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.start_time = Some(start);
        self.end_time = Some(end);
        self
    }

    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Metrics error types
#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Exporter error: {0}")]
    Exporter(String),
    #[error("Collector error: {0}")]
    Collector(String),
    #[error("Invalid metric: {0}")]
    InvalidMetric(String),
    #[error("Query error: {0}")]
    Query(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_counter() {
        let metric = Metric::counter("test_counter".to_string(), 42);
        assert_eq!(metric.name, "test_counter");
        assert_eq!(metric.metric_type, MetricType::Counter);
    }

    #[test]
    fn test_metric_gauge() {
        let metric = Metric::gauge("test_gauge".to_string(), 3.14);
        assert_eq!(metric.name, "test_gauge");
        assert_eq!(metric.metric_type, MetricType::Gauge);
    }

    #[test]
    fn test_metric_with_labels() {
        let metric = Metric::counter("test".to_string(), 1)
            .with_label("env".to_string(), "prod".to_string())
            .with_label("version".to_string(), "1.0".to_string());

        assert_eq!(metric.labels.len(), 2);
        assert_eq!(metric.labels.get("env"), Some(&"prod".to_string()));
    }

    #[test]
    fn test_performance_metrics_record() {
        let mut metrics = PerformanceMetrics::default();
        metrics.record_call(100.0, true);
        metrics.record_call(200.0, true);
        metrics.record_call(50.0, false);

        assert_eq!(metrics.total_calls, 3);
        assert_eq!(metrics.successful_calls, 2);
        assert_eq!(metrics.failed_calls, 1);
        assert_eq!(metrics.avg_latency_ms, 350.0 / 3.0);
        assert_eq!(metrics.min_latency_ms, 50.0);
        assert_eq!(metrics.max_latency_ms, 200.0);
    }

    #[test]
    fn test_plugin_metrics_health_score() {
        let mut metrics = PluginMetrics::new("test".to_string());

        let score = metrics.health_score();
        assert_eq!(score, 1.0);

        metrics.performance_metrics.record_call(50.0, true);
        let score = metrics.health_score();
        assert_eq!(score, 1.0);

        metrics.performance_metrics.record_call(50.0, false);
        let score = metrics.health_score();
        assert!(score < 1.0 && score > 0.0);
    }
}
