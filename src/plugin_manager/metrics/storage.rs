// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::types::*;
use anyhow::Result;
use chrono::{DateTime, TimeDelta, Utc};
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Time-series metrics storage
pub struct MetricsStorage {
    metrics: Arc<RwLock<HashMap<String, Vec<Metric>>>>,
    retention_period: TimeDelta,
    cache: Arc<RwLock<LruCache<String, Vec<Metric>>>>,
    max_cache_size: usize,
}

impl MetricsStorage {
    pub fn new(retention_period: TimeDelta) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            retention_period,
            cache: Arc::new(RwLock::new(LruCache::new(
                NonZeroUsize::new(1000).unwrap(),
            ))),
            max_cache_size: 1000,
        }
    }

    pub async fn store(&self, metric: Metric) -> Result<()> {
        let mut metrics = self.metrics.write().await;
        let entry = metrics.entry(metric.name.clone()).or_insert_with(Vec::new);
        entry.push(metric.clone());

        self.cleanup_old_metrics(&mut metrics, &metric.name).await;
        self.update_cache(metric).await;

        Ok(())
    }

    pub async fn store_batch(&self, metrics: Vec<Metric>) -> Result<()> {
        let mut all_metrics = self.metrics.write().await;

        for metric in metrics {
            let entry = all_metrics
                .entry(metric.name.clone())
                .or_insert_with(Vec::new);
            entry.push(metric.clone());

            self.cleanup_old_metrics(&mut all_metrics, &metric.name).await;
            self.update_cache(metric).await;
        }

        Ok(())
    }

    pub async fn query(&self, query: MetricQuery) -> Vec<Metric> {
        let metrics = self.metrics.read().await;
        let mut results = Vec::new();

        for (metric_name, metric_values) in metrics.iter() {
            if let Some(ref query_name) = query.metric_name {
                if metric_name != query_name {
                    continue;
                }
            }

            for metric in metric_values {
                if self.matches_query(metric, &query).is_err() {
                    continue;
                }

                results.push(metric.clone());
            }
        }

        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        results
    }

    pub async fn get_latest(&self, metric_name: &str) -> Option<Metric> {
        let metrics = self.metrics.read().await;
        metrics
            .get(metric_name)
            .and_then(|values| values.last().cloned())
    }

    pub async fn get_range(
        &self,
        metric_name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<Metric> {
        let query = MetricQuery::new()
            .with_metric(metric_name.to_string())
            .with_time_range(start, end);
        self.query(query).await
    }

    async fn cleanup_old_metrics(&self, metrics: &mut HashMap<String, Vec<Metric>>, name: &str) {
        let cutoff = Utc::now() - self.retention_period;

        if let Some(values) = metrics.get_mut(name) {
            values.retain(|m| m.timestamp > cutoff);
        }
    }

    async fn update_cache(&self, metric: Metric) {
        let mut cache = self.cache.write().await;

        if let Some(values) = cache.get_mut(&metric.name) {
            values.push(metric);
        } else {
            cache.put(metric.name.clone(), vec![metric]);
        }

        if cache.len() > self.max_cache_size {
            cache.pop_lru();
        }
    }

    fn matches_query(&self, metric: &Metric, query: &MetricQuery) -> Result<()> {
        if let Some(ref plugin) = query.plugin_name {
            let metric_plugin = metric.labels.get("plugin").ok_or_else(|| {
                anyhow::anyhow!("Metric missing 'plugin' label")
            })?;

            if metric_plugin != plugin {
                return Err(anyhow::anyhow!("Plugin mismatch"));
            }
        }

        if let Some(ref start) = query.start_time {
            if metric.timestamp < *start {
                return Err(anyhow::anyhow!("Before start time"));
            }
        }

        if let Some(ref end) = query.end_time {
            if metric.timestamp > *end {
                return Err(anyhow::anyhow!("After end time"));
            }
        }

        for (key, value) in &query.labels {
            if metric.labels.get(key) != Some(value) {
                return Err(anyhow::anyhow!("Label mismatch"));
            }
        }

        Ok(())
    }

    pub async fn get_all_metric_names(&self) -> Vec<String> {
        let metrics = self.metrics.read().await;
        metrics.keys().cloned().collect()
    }

    pub async fn get_metric_count(&self, metric_name: &str) -> usize {
        let metrics = self.metrics.read().await;
        metrics.get(metric_name).map(|v| v.len()).unwrap_or(0)
    }

    pub async fn clear(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.clear();

        let mut cache = self.cache.write().await;
        cache.clear();
    }

    pub async fn get_stats(&self) -> StorageStats {
        let metrics = self.metrics.read().await;
        let cache = self.cache.read().await;

        let total_metrics = metrics.values().map(|v| v.len()).sum();
        let metric_names = metrics.len();

        StorageStats {
            total_metrics,
            metric_names,
            cache_size: cache.len(),
            retention_hours: self.retention_period.num_hours(),
        }
    }
}

impl Default for MetricsStorage {
    fn default() -> Self {
        Self::new(TimeDelta::hours(24))
    }
}

#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_metrics: usize,
    pub metric_names: usize,
    pub cache_size: usize,
    pub retention_hours: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_storage_store_and_query() {
        let storage = MetricsStorage::new(TimeDelta::hours(1));

        let metric = Metric::counter("test_counter".to_string(), 42)
            .with_label("plugin".to_string(), "test_plugin".to_string());

        storage.store(metric.clone()).await.unwrap();

        let query = MetricQuery::new()
            .with_metric("test_counter".to_string())
            .with_label("plugin".to_string(), "test_plugin".to_string());

        let results = storage.query(query).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "test_counter");
    }

    #[tokio::test]
    async fn test_storage_get_latest() {
        let storage = MetricsStorage::new(TimeDelta::hours(1));

        let _now = Utc::now();
        let metric1 = Metric::gauge("test_gauge".to_string(), 1.0);
        let metric2 = Metric::gauge("test_gauge".to_string(), 2.0);

        storage.store(metric1).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        storage.store(metric2).await.unwrap();

        let latest = storage.get_latest("test_gauge").await;
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().value, MetricValue::Gauge(2.0));
    }

    #[tokio::test]
    async fn test_storage_time_range() {
        let storage = MetricsStorage::new(TimeDelta::hours(1));

        let now = Utc::now();
        let metric1 = Metric::counter("test_counter".to_string(), 1);

        storage.store(metric1).await.unwrap();

        let hour_ago = now - TimeDelta::hours(1);
        let hour_ahead = now + TimeDelta::hours(1);

        let results = storage
            .get_range("test_counter", hour_ago, hour_ahead)
            .await;

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_storage_cleanup() {
        let storage = MetricsStorage::new(TimeDelta::milliseconds(100));

        let metric = Metric::counter("test_counter".to_string(), 1);
        storage.store(metric).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // Trigger cleanup by storing another metric with the same name
        let metric2 = Metric::counter("test_counter".to_string(), 2);
        storage.store(metric2).await.unwrap();

        // The old metric should have been cleaned up; only the new one remains
        let count = storage.get_metric_count("test_counter").await;
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_storage_stats() {
        let storage = MetricsStorage::new(TimeDelta::hours(1));

        let metric = Metric::counter("test_counter".to_string(), 1);
        storage.store(metric).await.unwrap();

        let stats = storage.get_stats().await;
        assert_eq!(stats.total_metrics, 1);
        assert_eq!(stats.metric_names, 1);
    }
}
