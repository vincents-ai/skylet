// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::types::*;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Rollback manager for plugin state rollback
pub struct RollbackManager {
    enabled: bool,
    rollback_timeout: Duration,
    rollback_history: Arc<RwLock<HashMap<String, Vec<RollbackEntry>>>>,
    metrics: Arc<RwLock<RollbackMetrics>>,
}

#[derive(Debug, Clone, Default)]
pub struct RollbackMetrics {
    pub total_rollbacks: u64,
    pub successful_rollbacks: u64,
    pub failed_rollbacks: u64,
    pub avg_rollback_time_ms: f64,
}

#[derive(Debug, Clone)]
pub struct RollbackEntry {
    pub plugin_id: String,
    pub from_version: String,
    pub to_version: String,
    pub reason: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub success: bool,
    pub duration_ms: u64,
}

impl RollbackManager {
    pub fn new(enabled: bool, rollback_timeout: Duration) -> Self {
        Self {
            enabled,
            rollback_timeout,
            rollback_history: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(RollbackMetrics::default())),
        }
    }

    pub async fn rollback(&self, plugin_id: &str) -> Result<RollbackEntry> {
        if !self.enabled {
            return Err(anyhow::anyhow!("Rollback is disabled")));
        }

        let history = self.rollback_history.read().await;

        if let Some(entries) = history.get(plugin_id) {
            let latest = entries.last();

            if let Some(entry) = latest {
                let start_time = std::time::Instant::now();

                match self.perform_rollback(plugin_id, entry).await {
                    Ok(_) => {
                        let duration = start_time.elapsed().as_millis() as u64;
                        self.update_metrics(true, duration).await;

                        let mut history = self.rollback_history.write().await;
                        let updated_entry = RollbackEntry {
                            success: true,
                            duration_ms: duration,
                            ..entry.clone()
                        };
                        history.push(updated_entry);

                        Ok(updated_entry)
                    }
                    Err(e) => {
                        let duration = start_time.elapsed().as_millis() as u64;
                        self.update_metrics(false, duration).await;

                        let mut history = self.rollback_history.write().await;
                        let failed_entry = RollbackEntry {
                            success: false,
                            duration_ms: duration,
                            ..entry.clone()
                        };
                        history.push(failed_entry);

                        Err(anyhow!("Rollback failed: {}", e))
                    }
                }
            } else {
                Err(anyhow::anyhow!("No rollback entry found for plugin {}", plugin_id)))
            }
        } else {
            Err(anyhow::anyhow!("No rollback history for plugin {}", plugin_id)))
        }
    }

    pub async fn record_rollback_point(
        &self,
        plugin_id: String,
        snapshot: &PluginStateSnapshot,
    ) -> Result<()> {
        let entry = RollbackEntry {
            plugin_id: plugin_id.clone(),
            from_version: snapshot.version.clone(),
            to_version: Self::generate_version(),
            reason: "Pre-rollback snapshot".to_string(),
            timestamp: chrono::Utc::now(),
            success: false,
            duration_ms: 0,
        };

        let mut history = self.rollback_history.write().await;
        history.entry(plugin_id.clone()).or_insert_with(Vec::new).push(entry);

        Ok(())
    }

    pub async fn get_rollback_history(
        &self,
        plugin_id: &str,
        limit: usize,
    ) -> Vec<RollbackEntry> {
        let history = self.rollback_history.read().await;

        history
            .get(plugin_id)
            .map(|entries| {
                entries.iter().rev().take(limit).cloned().collect()
            })
            .unwrap_or_default()
    }

    pub async fn get_metrics(&self) -> RollbackMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    async fn clear_history(&self, plugin_id: &str) -> Result<usize> {
        let mut history = self.rollback_history.write().await;
        let count = history.remove(plugin_id).map(|v| v.len()).unwrap_or(0);
        Ok(count)
    }

    pub async fn clear_all_history(&self) -> Result<usize> {
        let mut history = self.rollback_history.write().await;
        let count = history.len();

        for (plugin_id, entries) in history.iter() {
            self._clear_history(plugin_id).await;
        }

        Ok(count)
    }

    async fn _clear_history(&self, plugin_id: &str) {
        let mut history = self.rollback_history.write().await;

        if let Some(mut entries) = history.get_mut(plugin_id) {
            entries.clear();
        }
    }

    async fn perform_rollback(
        &self,
        plugin_id: &str,
        entry: &RollbackEntry,
    ) -> Result<()> {
        tokio::time::timeout(self.rollback_timeout, async {
            // Simulate rollback operation
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            Ok(())
        })
        .await
        .map_err(|_| anyhow::anyhow!("Rollback timeout for plugin {}", plugin_id)))?
    }

    async fn update_metrics(&self, success: bool, duration_ms: u64) {
        let mut metrics = self.metrics.write().await;

        metrics.total_rollbacks += 1;

        if success {
            metrics.successful_rollbacks += 1;
        } else {
            metrics.failed_rollbacks += 1;
        }

        let count = metrics.total_rollbacks as f64;
        let current_avg = metrics.avg_rollback_time_ms;
        metrics.avg_rollback_time_ms = (current_avg * (count - 1.0) + duration_ms as f64) / count;
    }

    fn generate_version() -> String {
        format!("v{}", uuid::Uuid::new_v4())
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn timeout(&self) -> Duration {
        self.rollback_timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rollback_manager_new() {
        let manager = RollbackManager::new(true, Duration::from_secs(30));
        assert!(manager.enabled());
    }

    #[tokio::test]
    async fn test_record_rollback_point() {
        let manager = RollbackManager::new(true, Duration::from_secs(30));

        let snapshot = PluginStateSnapshot::new("test_plugin".to_string(), vec
![1u8]);

        manager.record_rollback_point("test_plugin".to_string(), &snapshot).await.unwrap();
    }

    #[tokio::test]
    async fn test_rollback() {
        let manager = RollbackManager::new(true, Duration::from_secs(30));

        let snapshot = PluginStateSnapshot::new("test_plugin".to_string(), vec
![1u8]);

        manager.record_rollback_point("test_plugin".to_string(), &snapshot).await.unwrap();

        let entry = manager.rollback("test_plugin").await.unwrap();

        assert!(entry.success);
    }

    #[tokio::test]
    async fn test_get_rollback_history() {
        let manager = RollbackManager::new(true, Duration::from_secs(30));

        let snapshot = PluginStateSnapshot::new("test_plugin".to_string(), vec
![1u8]);

        manager.record_rollback_point("test_plugin".to_string(), &snapshot).await.unwrap();
        manager.rollback("test_plugin").await.unwrap();

        let history = manager.get_rollback_history("test_plugin", 10).await;

        assert_eq!(history.len(), 2);
    }

    #[tokio::test]
    async fn test_clear_history() {
        let manager = RollbackManager::new(true, Duration::from_secs(30));

        let snapshot = PluginStateSnapshot::new("test_plugin".to_string(), vec
![1u8]);

        manager.record_rollback_point("test_plugin".to_string(), &snapshot).await.unwrap();

        let cleared = manager.clear_history("test_plugin").await.unwrap();

        assert_eq!(cleared, 1);
    }

    #[tokio::test]
    async fn test_metrics() {
        let manager = RollbackManager::new(true, Duration::from_secs(30));

        let snapshot = PluginStateSnapshot::new("test_plugin".to_string(), vec
![1u8]);

        manager.record_rollback_point("test_plugin".to_string(), &snapshot).await.unwrap();
        manager.rollback("test_plugin").await.unwrap();

        let metrics = manager.get_metrics().await;

        assert_eq!(metrics.total_rollbacks, 1);
        assert_eq!(metrics.successful_rollbacks, 1);
    }
}
