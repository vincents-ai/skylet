// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Advanced Metrics Collection and Monitoring Module
//!
//! Provides comprehensive plugin metrics collection with:
//! - Real-time metrics collection (latency, throughput, error rates)
//! - Resource usage monitoring (memory, CPU, file handles)
//! - Custom plugin metrics support
//! - Time-series metrics storage
//! - Prometheus/OpenTelemetry integration

pub mod collector;
pub mod export;
pub mod plugin_integration;
pub mod storage;
pub mod types;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::sync::RwLock;

use types::*;

/// Metrics configuration
#[derive(Debug, Clone)]
#[allow(dead_code)] // Phase 2 metrics infrastructure — not yet wired up
pub struct MetricsConfig {
    pub enabled: bool,
    pub collection_interval: StdDuration,
    pub retention_period: StdDuration,
    pub enable_prometheus: bool,
    pub enable_otel: bool,
    pub prometheus_port: u16,
    pub otel_endpoint: Option<String>,
    pub sample_rate: f64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            collection_interval: StdDuration::from_secs(5),
            retention_period: StdDuration::from_secs(86400),
            enable_prometheus: true,
            enable_otel: false,
            prometheus_port: 9091,
            otel_endpoint: None,
            sample_rate: 1.0,
        }
    }
}

/// Main metrics manager
#[allow(dead_code)] // Phase 2 metrics infrastructure — not yet wired up
pub struct MetricsManager {
    config: MetricsConfig,
    storage: Arc<storage::MetricsStorage>,
    collector: Arc<collector::MetricsCollector>,
    exporters: Arc<RwLock<Vec<Box<dyn export::MetricsExporter>>>>,
    plugin_metrics: Arc<RwLock<HashMap<String, PluginMetrics>>>,
}

#[allow(dead_code)] // Phase 2 metrics infrastructure — not yet wired up
impl MetricsManager {
    pub fn new(config: MetricsConfig) -> Self {
        use chrono::TimeDelta;

        let retention_delta = TimeDelta::seconds(config.retention_period.as_secs() as i64);
        let storage = Arc::new(storage::MetricsStorage::new(retention_delta));
        let collector = Arc::new(collector::MetricsCollector::new(
            config.collection_interval,
            config.sample_rate,
        ));

        Self {
            config,
            storage,
            collector,
            exporters: Arc::new(RwLock::new(Vec::new())),
            plugin_metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start(&self) -> Result<(), MetricsError> {
        if !self.config.enabled {
            return Ok(());
        }

        let collector = self.collector.clone();
        let storage = self.storage.clone();
        let interval = self.config.collection_interval;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                if let Ok(metrics) = collector.collect_all().await {
                    let _ = storage.store_batch(metrics).await;
                }
            }
        });

        Ok(())
    }

    pub async fn register_plugin(&self, plugin_name: String) {
        let mut metrics = self.plugin_metrics.write().await;
        metrics.insert(
            plugin_name.clone(),
            PluginMetrics::new(plugin_name.clone()),
        );
    }

    pub async fn unregister_plugin(&self, plugin_name: &str) {
        let mut metrics = self.plugin_metrics.write().await;
        metrics.remove(plugin_name);
    }

    pub async fn record_metric(&self, metric: Metric) {
        if self.config.sample_rate >= 1.0 || rand::random::<f64>() < self.config.sample_rate {
            self.storage.store(metric).await.ok();
        }
    }

    pub async fn get_plugin_metrics(&self, plugin_name: &str) -> Option<PluginMetrics> {
        let metrics = self.plugin_metrics.read().await;
        metrics.get(plugin_name).cloned()
    }

    pub async fn update_plugin_metrics(&self, plugin_name: &str, updated: PluginMetrics) {
        let mut metrics = self.plugin_metrics.write().await;
        metrics.insert(plugin_name.to_string(), updated);
    }

    pub async fn query_metrics(&self, query: MetricQuery) -> Vec<Metric> {
        self.storage.query(query).await
    }

    pub async fn add_exporter(&self, exporter: Box<dyn export::MetricsExporter>) {
        let mut exporters = self.exporters.write().await;
        exporters.push(exporter);
    }

    pub async fn export_metrics(&self) -> Result<Vec<String>, MetricsError> {
        let exporters = self.exporters.read().await;
        let mut results = Vec::new();

        for exporter in exporters.iter() {
            match exporter.export().await {
                Ok(data) => results.push(data),
                Err(e) => {
                    tracing::warn!("Metrics export failed: {}", e);
                }
            }
        }

        Ok(results)
    }

    pub fn storage(&self) -> Arc<storage::MetricsStorage> {
        self.storage.clone()
    }

    pub fn collector(&self) -> Arc<collector::MetricsCollector> {
        self.collector.clone()
    }

    pub fn config(&self) -> &MetricsConfig {
        &self.config
    }
}

impl Default for MetricsManager {
    fn default() -> Self {
        Self::new(MetricsConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_config_default() {
        let config = MetricsConfig::default();
        assert!(config.enabled);
        assert_eq!(config.collection_interval, StdDuration::from_secs(5));
        assert_eq!(config.retention_period, StdDuration::from_secs(86400));
        assert!(config.enable_prometheus);
        assert_eq!(config.prometheus_port, 9091);
        assert_eq!(config.sample_rate, 1.0);
    }

    #[test]
    fn test_plugin_metrics_new() {
        let metrics = PluginMetrics::new("test_plugin".to_string());
        assert_eq!(metrics.plugin_name, "test_plugin");
        assert_eq!(metrics.performance_metrics.total_calls, 0);
    }
}
