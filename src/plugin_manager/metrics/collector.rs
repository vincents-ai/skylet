// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::types::*;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Metrics collector for gathering system and plugin metrics
pub struct MetricsCollector {
    interval: Duration,
    sample_rate: f64,
    metrics: Arc<RwLock<Vec<Metric>>>,
    collected_count: Arc<RwLock<u64>>,
}

impl MetricsCollector {
    pub fn new(interval: Duration, sample_rate: f64) -> Self {
        Self {
            interval,
            sample_rate,
            metrics: Arc::new(RwLock::new(Vec::new())),
            collected_count: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn collect_all(&self) -> Result<Vec<Metric>> {
        let mut all_metrics = Vec::new();

        all_metrics.extend(self.collect_system_metrics().await?);
        all_metrics.extend(self.collect_process_metrics().await?);

        Ok(all_metrics)
    }

    pub async fn collect_system_metrics(&self) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();

        let cpu_usage = self.get_cpu_usage().await;
        let memory_mb = self.get_memory_usage().await;
        let thread_count = self.get_thread_count().await;

        metrics.push(
            Metric::gauge("system_cpu_usage_percent".to_string(), cpu_usage)
                .with_label("source".to_string(), "metrics_collector".to_string()),
        );

        metrics.push(
            Metric::gauge("system_memory_mb".to_string(), memory_mb)
                .with_label("source".to_string(), "metrics_collector".to_string()),
        );

        metrics.push(
            Metric::gauge("system_thread_count".to_string(), thread_count as f64)
                .with_label("source".to_string(), "metrics_collector".to_string()),
        );

        Ok(metrics)
    }

    pub async fn collect_process_metrics(&self) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();

        let fd_count = self.get_file_handle_count().await;

        metrics.push(
            Metric::gauge("process_file_handles".to_string(), fd_count as f64)
                .with_label("source".to_string(), "metrics_collector".to_string()),
        );

        Ok(metrics)
    }

    pub async fn collect_plugin_metrics(
        &self,
        plugin_name: &str,
        performance: &PerformanceMetrics,
        resource: &ResourceMetrics,
    ) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();

        metrics.push(
            Metric::counter("plugin_total_calls".to_string(), performance.total_calls)
                .with_label("plugin".to_string(), plugin_name.to_string()),
        );

        metrics.push(
            Metric::counter(
                "plugin_successful_calls".to_string(),
                performance.successful_calls,
            )
            .with_label("plugin".to_string(), plugin_name.to_string()),
        );

        metrics.push(
            Metric::counter("plugin_failed_calls".to_string(), performance.failed_calls)
                .with_label("plugin".to_string(), plugin_name.to_string()),
        );

        metrics.push(
            Metric::gauge(
                "plugin_avg_latency_ms".to_string(),
                performance.avg_latency_ms,
            )
            .with_label("plugin".to_string(), plugin_name.to_string()),
        );

        metrics.push(
            Metric::gauge(
                "plugin_p95_latency_ms".to_string(),
                performance.p95_latency_ms,
            )
            .with_label("plugin".to_string(), plugin_name.to_string()),
        );

        metrics.push(
            Metric::gauge(
                "plugin_p99_latency_ms".to_string(),
                performance.p99_latency_ms,
            )
            .with_label("plugin".to_string(), plugin_name.to_string()),
        );

        metrics.push(
            Metric::gauge("plugin_error_rate".to_string(), performance.error_rate)
                .with_label("plugin".to_string(), plugin_name.to_string()),
        );

        metrics.push(
            Metric::gauge("plugin_memory_mb".to_string(), resource.memory_mb)
                .with_label("plugin".to_string(), plugin_name.to_string()),
        );

        metrics.push(
            Metric::gauge("plugin_cpu_percent".to_string(), resource.cpu_percent)
                .with_label("plugin".to_string(), plugin_name.to_string()),
        );

        Ok(metrics)
    }

    async fn get_cpu_usage(&self) -> f64 {
        let metrics = self.metrics.write().await;
        let mut count = self.collected_count.write().await;
        *count += 1;
        drop(metrics);

        let sample = if self.sample_rate >= 1.0 || rand::random::<f64>() < self.sample_rate {
            self.sample_cpu_usage().await
        } else {
            0.0
        };

        sample
    }

    async fn sample_cpu_usage(&self) -> f64 {
        use sysinfo::System;

        let mut sys = System::new_all();
        sys.refresh_all();

        let cpu_usage = sys.global_cpu_info().cpu_usage();
        cpu_usage as f64
    }

    async fn get_memory_usage(&self) -> f64 {
        use sysinfo::System;

        let mut sys = System::new_all();
        sys.refresh_memory();

        sys.total_memory() as f64 / 1024.0 / 1024.0
    }

    async fn get_thread_count(&self) -> usize {
        tokio::runtime::Handle::current().metrics().num_workers()
    }

    async fn get_file_handle_count(&self) -> usize {
        use std::fs;

        if let Ok(entries) = fs::read_dir("/proc/self/fd") {
            entries.count()
        } else {
            0
        }
    }

    pub async fn get_collected_count(&self) -> u64 {
        *self.collected_count.read().await
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new(Duration::from_secs(5), 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_collector_new() {
        let collector = MetricsCollector::new(Duration::from_secs(5), 1.0);
        assert_eq!(collector.interval(), Duration::from_secs(5));
        assert_eq!(collector.sample_rate(), 1.0);
    }

    #[tokio::test]
    async fn test_collector_collect_all() {
        let collector = MetricsCollector::default();

        let metrics = collector.collect_all().await.unwrap();
        assert!(!metrics.is_empty());
    }

    #[tokio::test]
    async fn test_collector_collect_system_metrics() {
        let collector = MetricsCollector::default();

        let metrics = collector.collect_system_metrics().await.unwrap();
        assert!(!metrics.is_empty());

        for metric in &metrics {
            assert_eq!(
                metric.labels.get("source"),
                Some(&"metrics_collector".to_string())
            );
        }
    }

    #[tokio::test]
    async fn test_collector_collect_plugin_metrics() {
        let collector = MetricsCollector::default();

        let performance = PerformanceMetrics::default();
        let resource = ResourceMetrics::default();

        let metrics = collector
            .collect_plugin_metrics("test_plugin", &performance, &resource)
            .await
            .unwrap();

        assert!(!metrics.is_empty());

        for metric in &metrics {
            assert_eq!(
                metric.labels.get("plugin"),
                Some(&"test_plugin".to_string())
            );
        }
    }

    #[tokio::test]
    async fn test_collector_collected_count() {
        let collector = MetricsCollector::default();

        let initial_count = collector.get_collected_count().await;
        collector.collect_all().await.ok();

        let final_count = collector.get_collected_count().await;
        assert!(final_count > initial_count);
    }
}
