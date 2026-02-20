// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(dead_code)] // Plugin metrics integration - not yet wired into production

use super::types::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Plugin metrics integration
pub struct PluginMetricsIntegration {
    manager: Arc<super::MetricsManager>,
    plugin_timers: Arc<RwLock<HashMap<String, MetricTimer>>>,
}

impl PluginMetricsIntegration {
    pub fn new(manager: Arc<super::MetricsManager>) -> Self {
        Self {
            manager,
            plugin_timers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn record_call_start(&self, plugin_name: &str, operation: &str) {
        let timer_key = format!("{}:{}", plugin_name, operation);
        let mut timers = self.plugin_timers.write().await;
        timers.insert(timer_key.clone(), MetricTimer::new());
    }

    pub async fn record_call_end(&self, plugin_name: &str, operation: &str, success: bool) {
        let timer_key = format!("{}:{}", plugin_name, operation);
        let mut timers = self.plugin_timers.write().await;

        if let Some(timer) = timers.remove(&timer_key) {
            let elapsed_ms = timer.elapsed_ms();

            let metric_name = format!("plugin_{}_latency_ms", operation);
            let metric = Metric::gauge(metric_name, elapsed_ms)
                .with_label("plugin".to_string(), plugin_name.to_string())
                .with_label("success".to_string(), success.to_string());

            self.manager.record_metric(metric).await;

            // Record total_calls counter
            let total_metric = Metric::counter("total_calls".to_string(), 1)
                .with_label("plugin".to_string(), plugin_name.to_string());
            self.manager.record_metric(total_metric).await;

            // Record success/failure counter
            if success {
                let success_metric = Metric::counter("successful_calls".to_string(), 1)
                    .with_label("plugin".to_string(), plugin_name.to_string());
                self.manager.record_metric(success_metric).await;
            } else {
                let fail_metric = Metric::counter("failed_calls".to_string(), 1)
                    .with_label("plugin".to_string(), plugin_name.to_string());
                self.manager.record_metric(fail_metric).await;
            }

            self.update_plugin_performance(plugin_name, elapsed_ms, success)
                .await;
        }
    }

    async fn update_plugin_performance(&self, plugin_name: &str, latency_ms: f64, success: bool) {
        if let Some(mut metrics) = self.manager.get_plugin_metrics(plugin_name).await {
            metrics.performance_metrics.record_call(latency_ms, success);
            self.manager
                .update_plugin_metrics(plugin_name, metrics)
                .await;
        }
    }

    pub async fn record_custom_metric(
        &self,
        plugin_name: &str,
        _metric_name: String,
        metric: Metric,
    ) {
        let labeled_metric = metric.with_label("plugin".to_string(), plugin_name.to_string());

        self.manager.record_metric(labeled_metric.clone()).await;

        if let Some(mut metrics) = self.manager.get_plugin_metrics(plugin_name).await {
            metrics.record_custom_metric(labeled_metric);
        }
    }

    #[allow(dead_code)] // Public API — not yet called from production code
    pub async fn increment_counter(&self, plugin_name: &str, counter_name: String, value: u64) {
        let metric = Metric::counter(counter_name, value)
            .with_label("plugin".to_string(), plugin_name.to_string());

        self.manager.record_metric(metric).await;
    }

    #[allow(dead_code)] // Public API — not yet called from production code
    pub async fn set_gauge(&self, plugin_name: &str, gauge_name: String, value: f64) {
        let metric = Metric::gauge(gauge_name, value)
            .with_label("plugin".to_string(), plugin_name.to_string());

        self.manager.record_metric(metric).await;
    }

    #[allow(dead_code)] // Public API — not yet called from production code
    pub async fn record_histogram(
        &self,
        plugin_name: &str,
        histogram_name: String,
        values: Vec<f64>,
    ) {
        let metric = Metric::histogram(histogram_name, values)
            .with_label("plugin".to_string(), plugin_name.to_string());

        self.manager.record_metric(metric).await;
    }

    pub async fn get_plugin_summary(&self, plugin_name: &str) -> PluginSummary {
        let query = MetricQuery::new()
            .with_plugin(plugin_name.to_string())
            .with_limit(1000);

        let metrics = self.manager.query_metrics(query).await;

        let mut total_calls = 0u64;
        let mut successful_calls = 0u64;
        let mut failed_calls = 0u64;
        let mut total_latency = 0.0;
        let mut latencies = Vec::new();

        for metric in metrics {
            if metric.name.contains("total_calls") {
                if let MetricValue::Counter(v) = metric.value {
                    total_calls += v;
                }
            } else if metric.name.contains("successful_calls") {
                if let MetricValue::Counter(v) = metric.value {
                    successful_calls += v;
                }
            } else if metric.name.contains("failed_calls") {
                if let MetricValue::Counter(v) = metric.value {
                    failed_calls += v;
                }
            } else if metric.name.contains("latency_ms") {
                if let MetricValue::Gauge(v) = metric.value {
                    latencies.push(v);
                    total_latency += v;
                }
            }
        }

        let avg_latency = if latencies.is_empty() {
            0.0
        } else {
            total_latency / latencies.len() as f64
        };

        let (p50, p95, p99) = if !latencies.is_empty() {
            let mut sorted = latencies.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let len = sorted.len();

            (
                sorted[len * 50 / 100],
                sorted[len * 95 / 100],
                sorted[len * 99 / 100],
            )
        } else {
            (0.0, 0.0, 0.0)
        };

        let error_rate = if total_calls > 0 {
            failed_calls as f64 / total_calls as f64
        } else {
            0.0
        };

        PluginSummary {
            plugin_name: plugin_name.to_string(),
            total_calls,
            successful_calls,
            failed_calls,
            error_rate,
            avg_latency_ms: avg_latency,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            p99_latency_ms: p99,
            health_score: 1.0 - error_rate,
        }
    }
}

/// Simple metric timer
pub struct MetricTimer {
    start: std::time::Instant,
}

impl MetricTimer {
    pub fn new() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.start.elapsed().as_secs_f64() * 1000.0
    }
}

/// Summary of plugin metrics
#[derive(Debug, Clone)]
#[allow(dead_code)] // Public API — not yet called from production code
pub struct PluginSummary {
    pub plugin_name: String,
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub error_rate: f64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub health_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metric_timer() {
        let timer = MetricTimer::new();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let elapsed = timer.elapsed_ms();

        assert!(elapsed >= 100.0);
        assert!(elapsed < 200.0);
    }

    #[tokio::test]
    async fn test_plugin_metrics_integration() {
        use super::super::MetricsConfig;
        use super::super::MetricsManager;

        let manager = Arc::new(MetricsManager::new(MetricsConfig::default()));
        let integration = PluginMetricsIntegration::new(manager.clone());

        manager.register_plugin("test_plugin".to_string()).await;

        integration
            .record_call_start("test_plugin", "execute")
            .await;

        std::thread::sleep(std::time::Duration::from_millis(10));

        integration
            .record_call_end("test_plugin", "execute", true)
            .await;

        let summary = integration.get_plugin_summary("test_plugin").await;
        assert_eq!(summary.total_calls, 1);
        assert_eq!(summary.successful_calls, 1);
    }

    #[tokio::test]
    async fn test_record_custom_metric() {
        use super::super::MetricsConfig;
        use super::super::MetricsManager;

        let manager = Arc::new(MetricsManager::new(MetricsConfig::default()));
        let integration = PluginMetricsIntegration::new(manager.clone());

        let metric = Metric::counter("custom_counter".to_string(), 42);
        integration
            .record_custom_metric("test_plugin", "custom_counter".to_string(), metric)
            .await;

        let summary = integration.get_plugin_summary("test_plugin").await;
        assert_eq!(summary.total_calls, 0);
    }
}
