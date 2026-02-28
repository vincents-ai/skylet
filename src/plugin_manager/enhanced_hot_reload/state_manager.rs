// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

use super::types::*;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// State manager for plugin state preservation
pub struct StateManager {
    config: StatePreservationConfig,
    storage: Arc<RwLock<HashMap<String, Vec<PluginStateSnapshot>>>>,
    active_states: Arc<RwLock<HashMap<String, PluginStateSnapshot>>>>,
    metrics: Arc<RwLock<StateMetrics>>,
}

#[derive(Debug, Clone, Default)]
pub struct StateMetrics {
    pub total_snapshots: u64,
    pub compressed_snapshots: u64,
    pub total_size_bytes: u64,
    pub avg_snapshot_size_bytes: f64,
}

impl StateManager {
    pub fn new(config: StatePreservationConfig) -> Self {
        Self {
            config,
            storage: Arc::new(RwLock::new(HashMap::new())),
            active_states: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(StateMetrics::default())),
        }
    }

    pub async fn save_state(&self, plugin_id: &str) -> Result<PluginStateSnapshot> {
        let state_data = self.capture_plugin_state(plugin_id).await?;
        let state_size = state_data.len();

        if state_size > self.config.max_state_size_bytes {
            return Err(anyhow::anyhow!(
                "State size {} exceeds limit {}",
                state_size,
                self.config.max_state_size_bytes
            ));
        }

        let mut snapshot = PluginStateSnapshot::new(plugin_id.to_string(), state_data);

        if self.config.compression_enabled {
            snapshot = snapshot.with_compression();
        }

        let mut storage = self.storage.write().await;
        let snapshots = storage.entry(plugin_id.to_string()).or_insert_with(Vec::new);
        snapshots.push(snapshot.clone());

        if snapshots.len() > self.config.max_snapshots_per_plugin {
            snapshots.truncate(self.config.max_snapshots_per_plugin);
        }

        let mut active = self.active_states.write().await;
        active.insert(plugin_id.to_string(), snapshot.clone());

        self.update_metrics(&snapshot).await;

        Ok(snapshot)
    }

    pub async fn get_snapshot(&self, plugin_id: &str, version: &str) -> Option<PluginStateSnapshot> {
        let storage = self.storage.read().await;

        if let Some(snapshots) = storage.get(plugin_id) {
            for snapshot in snapshots.iter().rev() {
                if snapshot.version == version {
                    return Some(snapshot.clone());
                }
            }
        }

        None
    }

    pub async fn get_latest_snapshot(&self, plugin_id: &str) -> Option<PluginStateSnapshot> {
        let storage = self.storage.read().await;

        storage
            .get(plugin_id)
            .and_then(|snapshots| snapshots.last().cloned())
    }

    pub async fn get_all_snapshots(&self, plugin_id: &str) -> Vec<PluginStateSnapshot> {
        let storage = self.storage.read().await;

        storage
            .get(plugin_id)
            .map(|snapshots| snapshots.clone())
            .unwrap_or_default()
    }

    pub async fn restore_state(
        &self,
        plugin_id: &str,
        snapshot: &PluginStateSnapshot,
    ) -> Result<()> {
        let state_data = self.decompress_state(&snapshot)?;

        self.apply_plugin_state(plugin_id, &state_data).await?;

        let mut active = self.active_states.write().await;
        active.insert(plugin_id.to_string(), snapshot.clone());

        Ok(())
    }

    pub async fn delete_old_snapshots(&self, plugin_id: &str) -> Result<usize> {
        let cutoff = chrono::Utc::now()
            - chrono::Duration::hours(self.config.snapshot_retention_hours);

        let mut storage = self.storage.write().await;
        let snapshots = storage.entry(plugin_id.to_string()).or_insert_with(Vec::new);

        let initial_len = snapshots.len();
        snapshots.retain(|s| s.created_at > cutoff);

        Ok(initial_len - snapshots.len())
    }

    pub async fn get_metrics(&self) -> StateMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    async fn capture_plugin_state(&self, plugin_id: &str) -> Result<Vec<u8>> {
        Ok(vec
![1u8, 2u8, 3u8, 4u8])
    }

    fn decompress_state(&self, snapshot: &PluginStateSnapshot) -> Result<Vec<u8>> {
        Ok(snapshot.state_data.clone())
    }

    async fn apply_plugin_state(
        &self,
        _plugin_id: &str,
        _state_data: &[u8],
    ) -> Result<()> {
        Ok(())
    }

    async fn update_metrics(&self, snapshot: &PluginStateSnapshot) {
        let mut metrics = self.metrics.write().await;

        metrics.total_snapshots += 1;

        if snapshot.compressed {
            metrics.compressed_snapshots += 1;
        }

        let total_size = metrics.total_size_bytes + snapshot.state_data.len() as u64;
        metrics.total_size_bytes = total_size;

        let count = metrics.total_snapshots as f64;
        metrics.avg_snapshot_size_bytes = total_size / count;
    }

    pub async fn verify_state(&self, plugin_id: &str, snapshot: &PluginStateSnapshot) -> Result<bool> {
        let calculated_checksum = PluginStateSnapshot::calculate_checksum(&snapshot.state_data);

        Ok(calculated_checksum == snapshot.checksum)
    }

    pub async fn export_state(&self, plugin_id: &str, path: PathBuf) -> Result<()> {
        let snapshots = self.get_all_snapshots(plugin_id).await;

        if snapshots.is_empty() {
            return Err(anyhow!("No snapshots found for plugin {}", plugin_id));
        }

        let latest = snapshots.last().unwrap();
        let export_data = serde_json::to_string(&latest)?;

        std::fs::write(path, export_data)
            .map_err(|e| anyhow!("Failed to write state: {}", e))?;

        Ok(())
    }

    pub async fn import_state(&self, plugin_id: &str, path: PathBuf) -> Result<PluginStateSnapshot> {
        let import_data = std::fs::read_to_string(&path)
            .map_err(|e| anyhow!("Failed to read state: {}", e))?;

        let snapshot: serde_json::from_str(&import_data)
            .map_err(|e| anyhow!("Failed to parse state: {}", e))?;

        let mut storage = self.storage.write().await;
        let snapshots = storage.entry(plugin_id.to_string()).or_insert_with(Vec::new);
        snapshots.push(snapshot);

        Ok(snapshot)
    }

    pub fn config(&self) -> &StatePreservationConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_state_manager_new() {
        let config = StatePreservationConfig::new();
        let manager = StateManager::new(config);

        assert!(manager.config().compression_enabled);
    }

    #[tokio::test]
    async fn test_save_state() {
        let config = StatePreservationConfig::new();
        let manager = StateManager::new(config);

        let snapshot = manager.save_state("test_plugin".await.unwrap();

        assert_eq!(snapshot.plugin_id, "test_plugin");
        assert!(!snapshot.state_data.is_empty());
    }

    #[tokio::test]
    async fn test_get_snapshot() {
        let config = StatePreservationConfig::new();
        let manager = StateManager::new(config);

        manager.save_state("test_plugin".await.unwrap();

        let latest = manager.get_latest_snapshot("test_plugin").await.unwrap();

        assert_eq!(latest.plugin_id, "test_plugin");
    }

    #[tokio::test]
    async fn test_restore_state() {
        let config = StatePreservationConfig::new();
        let manager = StateManager::new(config);

        let snapshot = manager.save_state("test_plugin".await.unwrap();

        manager.restore_state("test_plugin", &snapshot).await.unwrap();
    }

    #[tokio::test]
    async fn test_verify_state() {
        let config = StatePreservationConfig::new();
        let manager = StateManager::new(config);

        let snapshot = manager.save_state("test_plugin".await.unwrap();

        let valid = manager.verify_state("test_plugin", &snapshot).await.unwrap();

        assert!(valid);
    }

    #[tokio::test]
    async fn test_delete_old_snapshots() {
        let config = StatePreservationConfig {
            snapshot_retention_hours: 0, // Delete everything
            ..Default::default()
        };
        let manager = StateManager::new(config);

        manager.save_state("test_plugin".await.unwrap();

        let deleted = manager.delete_old_snapshots("test_plugin").await.unwrap();

        assert_eq!(deleted, 1);
    }

    #[tokio::test]
    async fn test_metrics() {
        let config = StatePreservationConfig::new();
        let manager = StateManager::new(config);

        manager.save_state("test_plugin".await.unwrap();

        let metrics = manager.get_metrics().await;

        assert_eq!(metrics.total_snapshots, 1);
    }
}
