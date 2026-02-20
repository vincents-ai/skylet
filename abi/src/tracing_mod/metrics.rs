// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Performance Metrics Collection for RFC-0017
//!
//! This module provides metrics collection and export capabilities
//! for monitoring plugin performance.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Type of metric
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    /// Counter - cumulative value that only increases
    Counter,

    /// Gauge - point-in-time value that can go up or down
    Gauge,

    /// Histogram - distribution of values
    Histogram,
}

/// Performance metrics for a plugin or operation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Request count
    pub request_count: u64,

    /// Error count
    pub error_count: u64,

    /// Total duration (milliseconds)
    pub total_duration_ms: f64,

    /// Min duration (milliseconds)
    pub min_duration_ms: Option<f64>,

    /// Max duration (milliseconds)
    pub max_duration_ms: Option<f64>,

    /// Average duration (milliseconds)
    pub avg_duration_ms: f64,

    /// P50 latency (milliseconds)
    pub p50_latency_ms: Option<f64>,

    /// P95 latency (milliseconds)
    pub p95_latency_ms: Option<f64>,

    /// P99 latency (milliseconds)
    pub p99_latency_ms: Option<f64>,

    /// Custom metrics
    pub custom_metrics: HashMap<String, f64>,
}

impl PerformanceMetrics {
    /// Create empty metrics
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a request
    pub fn record_request(&mut self, duration: Duration, success: bool) {
        let duration_ms = duration.as_millis() as f64;

        self.request_count += 1;
        if !success {
            self.error_count += 1;
        }

        self.total_duration_ms += duration_ms;

        // Update min/max
        self.min_duration_ms = Some(match self.min_duration_ms {
            Some(min) => min.min(duration_ms),
            None => duration_ms,
        });

        self.max_duration_ms = Some(match self.max_duration_ms {
            Some(max) => max.max(duration_ms),
            None => duration_ms,
        });

        // Update average
        self.avg_duration_ms = self.total_duration_ms / self.request_count as f64;
    }

    /// Calculate error rate
    pub fn error_rate(&self) -> f64 {
        if self.request_count == 0 {
            0.0
        } else {
            self.error_count as f64 / self.request_count as f64
        }
    }

    /// Get throughput (requests per second)
    pub fn throughput(&self, window_seconds: f64) -> f64 {
        if window_seconds == 0.0 {
            0.0
        } else {
            self.request_count as f64 / window_seconds
        }
    }

    /// Set a custom metric
    pub fn set_custom(&mut self, name: impl Into<String>, value: f64) {
        self.custom_metrics.insert(name.into(), value);
    }

    /// Get a custom metric
    pub fn get_custom(&self, name: &str) -> Option<f64> {
        self.custom_metrics.get(name).copied()
    }
}

/// Latency bucket for histogram tracking
#[derive(Debug, Clone)]
struct LatencyBucket {
    threshold_ms: f64,
    count: u64,
}

/// Metric collector for aggregating measurements
#[derive(Debug)]
pub struct MetricCollector {
    /// Metrics by name
    metrics: Arc<RwLock<HashMap<String, PerformanceMetrics>>>,

    /// Latency histograms by name
    histograms: Arc<RwLock<HashMap<String, Vec<LatencyBucket>>>>,

    /// Active timers
    timers: Arc<RwLock<HashMap<String, Instant>>>,
}

impl Default for MetricCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricCollector {
    /// Create a new metric collector
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
            timers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start a timer
    pub fn start_timer(&self, name: impl Into<String>) -> String {
        let name = name.into();
        if let Ok(mut timers) = self.timers.write() {
            timers.insert(name.clone(), Instant::now());
        }
        name
    }

    /// Stop a timer and record the duration
    pub fn stop_timer(&self, name: &str, success: bool) -> Option<Duration> {
        let duration = if let Ok(mut timers) = self.timers.write() {
            timers.remove(name).map(|start| start.elapsed())
        } else {
            None
        }?;

        // Record in metrics
        if let Ok(mut metrics) = self.metrics.write() {
            let entry = metrics.entry(name.to_string()).or_default();
            entry.record_request(duration, success);
        }

        // Record in histogram
        self.record_latency(name, duration);

        Some(duration)
    }

    /// Record a latency measurement
    fn record_latency(&self, name: &str, duration: Duration) {
        let duration_ms = duration.as_millis() as f64;

        if let Ok(mut histograms) = self.histograms.write() {
            let histogram = histograms.entry(name.to_string()).or_insert_with(|| {
                vec![
                    LatencyBucket {
                        threshold_ms: 1.0,
                        count: 0,
                    },
                    LatencyBucket {
                        threshold_ms: 5.0,
                        count: 0,
                    },
                    LatencyBucket {
                        threshold_ms: 10.0,
                        count: 0,
                    },
                    LatencyBucket {
                        threshold_ms: 25.0,
                        count: 0,
                    },
                    LatencyBucket {
                        threshold_ms: 50.0,
                        count: 0,
                    },
                    LatencyBucket {
                        threshold_ms: 100.0,
                        count: 0,
                    },
                    LatencyBucket {
                        threshold_ms: 250.0,
                        count: 0,
                    },
                    LatencyBucket {
                        threshold_ms: 500.0,
                        count: 0,
                    },
                    LatencyBucket {
                        threshold_ms: 1000.0,
                        count: 0,
                    },
                    LatencyBucket {
                        threshold_ms: f64::MAX,
                        count: 0,
                    },
                ]
            });

            for bucket in histogram.iter_mut() {
                if duration_ms <= bucket.threshold_ms {
                    bucket.count += 1;
                    break;
                }
            }
        }
    }

    /// Calculate percentile latency
    pub fn percentile_latency(&self, name: &str, percentile: f64) -> Option<f64> {
        let histograms = self.histograms.read().ok()?;
        let histogram = histograms.get(name)?;

        let total: u64 = histogram.iter().map(|b| b.count).sum();
        if total == 0 {
            return None;
        }

        let target = (total as f64 * percentile / 100.0) as u64;
        let mut cumulative = 0u64;

        for bucket in histogram.iter() {
            cumulative += bucket.count;
            if cumulative >= target {
                return Some(bucket.threshold_ms);
            }
        }

        None
    }

    /// Get metrics for a name
    pub fn get_metrics(&self, name: &str) -> Option<PerformanceMetrics> {
        self.metrics.read().ok()?.get(name).cloned()
    }

    /// Get all metrics
    pub fn all_metrics(&self) -> HashMap<String, PerformanceMetrics> {
        self.metrics.read().map(|m| m.clone()).unwrap_or_default()
    }

    /// Update metrics with percentiles
    pub fn update_percentiles(&self, name: &str) {
        let p50 = self.percentile_latency(name, 50.0);
        let p95 = self.percentile_latency(name, 95.0);
        let p99 = self.percentile_latency(name, 99.0);

        if let Ok(mut metrics) = self.metrics.write() {
            if let Some(entry) = metrics.get_mut(name) {
                entry.p50_latency_ms = p50;
                entry.p95_latency_ms = p95;
                entry.p99_latency_ms = p99;
            }
        }
    }

    /// Export metrics as JSON
    pub fn export_json(&self) -> Result<String, serde_json::Error> {
        let metrics = self.all_metrics();
        serde_json::to_string_pretty(&metrics)
    }

    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> String {
        let metrics = self.all_metrics();
        let mut output = String::new();

        for (name, m) in metrics.iter() {
            output.push_str(&format!(
                "# HELP {}_requests_total Total number of requests\n",
                name
            ));
            output.push_str(&format!("# TYPE {}_requests_total counter\n", name));
            output.push_str(&format!("{}_requests_total {}\n", name, m.request_count));

            output.push_str(&format!(
                "# HELP {}_errors_total Total number of errors\n",
                name
            ));
            output.push_str(&format!("# TYPE {}_errors_total counter\n", name));
            output.push_str(&format!("{}_errors_total {}\n", name, m.error_count));

            output.push_str(&format!(
                "# HELP {}_duration_ms Average request duration in ms\n",
                name
            ));
            output.push_str(&format!("# TYPE {}_duration_ms gauge\n", name));
            output.push_str(&format!("{}_duration_ms {}\n", name, m.avg_duration_ms));

            if let Some(p99) = m.p99_latency_ms {
                output.push_str(&format!("# HELP {}_latency_p99 P99 latency in ms\n", name));
                output.push_str(&format!("# TYPE {}_latency_p99 gauge\n", name));
                output.push_str(&format!("{}_latency_p99 {}\n", name, p99));
            }

            output.push('\n');
        }

        output
    }

    /// Reset all metrics
    pub fn reset(&self) {
        if let Ok(mut metrics) = self.metrics.write() {
            metrics.clear();
        }
        if let Ok(mut histograms) = self.histograms.write() {
            histograms.clear();
        }
        if let Ok(mut timers) = self.timers.write() {
            timers.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_metrics() {
        let mut metrics = PerformanceMetrics::new();

        metrics.record_request(Duration::from_millis(100), true);
        metrics.record_request(Duration::from_millis(200), true);
        metrics.record_request(Duration::from_millis(150), false);

        assert_eq!(metrics.request_count, 3);
        assert_eq!(metrics.error_count, 1);
        assert_eq!(metrics.min_duration_ms, Some(100.0));
        assert_eq!(metrics.max_duration_ms, Some(200.0));
        assert!(metrics.avg_duration_ms > 0.0);
        assert!(metrics.error_rate() > 0.0);
    }

    #[test]
    fn test_metric_collector_timer() {
        let collector = MetricCollector::new();

        let timer = collector.start_timer("operation1");
        std::thread::sleep(Duration::from_millis(10));
        let duration = collector.stop_timer(&timer, true);

        assert!(duration.is_some());
        assert!(duration.unwrap().as_millis() >= 10);

        let metrics = collector.get_metrics("operation1").unwrap();
        assert_eq!(metrics.request_count, 1);
        assert_eq!(metrics.error_count, 0);
    }

    #[test]
    fn test_metric_collector_percentiles() {
        let collector = MetricCollector::new();

        // Record multiple measurements
        for i in 0..100 {
            let timer = collector.start_timer("operation");
            std::thread::sleep(Duration::from_millis(i % 10));
            collector.stop_timer(&timer, true);
        }

        collector.update_percentiles("operation");

        let metrics = collector.get_metrics("operation").unwrap();
        assert_eq!(metrics.request_count, 100);
        assert!(metrics.p50_latency_ms.is_some());
        assert!(metrics.p95_latency_ms.is_some());
        assert!(metrics.p99_latency_ms.is_some());
    }

    #[test]
    fn test_prometheus_export() {
        let collector = MetricCollector::new();

        let timer = collector.start_timer("api_request");
        std::thread::sleep(Duration::from_millis(5));
        collector.stop_timer(&timer, true);

        let export = collector.export_prometheus();
        assert!(export.contains("api_request_requests_total"));
        assert!(export.contains("api_request_errors_total"));
        assert!(export.contains("api_request_duration_ms"));
    }

    #[test]
    fn test_custom_metrics() {
        let mut metrics = PerformanceMetrics::new();

        metrics.set_custom("cache_hits", 100.0);
        metrics.set_custom("cache_misses", 20.0);

        assert_eq!(metrics.get_custom("cache_hits"), Some(100.0));
        assert_eq!(metrics.get_custom("cache_misses"), Some(20.0));
    }
}
