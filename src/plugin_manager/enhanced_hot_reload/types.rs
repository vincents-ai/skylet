// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Plugin state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStateSnapshot {
    pub plugin_id: String,
    pub version: String,
    pub state_data: Vec<u8>,
    pub compressed: bool,
    pub checksum: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub metadata: StateMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateMetadata {
    pub plugin_version: String,
    pub config_hash: String,
    pub memory_usage_mb: f64,
    pub uptime_seconds: u64,
    pub tags: Vec<String>,
}

impl PluginStateSnapshot {
    pub fn new(plugin_id: String, state_data: Vec<u8>) -> Self {
        let checksum = Self::calculate_checksum(&state_data);

        Self {
            plugin_id,
            version: Self::generate_version(),
            state_data,
            compressed: false,
            checksum: checksum.clone(),
            created_at: chrono::Utc::now(),
            metadata: StateMetadata::default(),
        }
    }

    fn with_compression(mut self) -> Self {
        self.compressed = true;
        self
    }

    fn with_metadata(mut self, metadata: StateMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    fn calculate_checksum(data: &[u8]) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    fn generate_version() -> String {
        format!("v{}", Uuid::new_v4())
    }
}

/// State preservation configuration
#[derive(Debug, Clone, Default)]
pub struct StatePreservationConfig {
    pub compression_enabled: bool,
    pub encryption_enabled: bool,
    pub max_state_size_bytes: usize,
    pub max_snapshots_per_plugin: usize,
    pub snapshot_retention_hours: i64,
}

impl StatePreservationConfig {
    pub fn new() -> Self {
        Self {
            compression_enabled: true,
            encryption_enabled: false,
            max_state_size_bytes: 10 * 1024 * 1024, // 10MB
            max_snapshots_per_plugin: 5,
            snapshot_retention_hours: 24,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_snapshot_new() {
        let state_data = vec![1u8, 2u8, 3u8, 4u8];

        let snapshot = PluginStateSnapshot::new("test_plugin".to_string(), state_data);

        assert_eq!(snapshot.plugin_id, "test_plugin");
        assert!(!snapshot.state_data.is_empty());
        assert!(!snapshot.checksum.is_empty());
    }

    #[test]
    fn test_state_snapshot_compression() {
        let snapshot =
            PluginStateSnapshot::new("test".to_string(), vec![1u8, 2u8, 3u8]).with_compression();

        assert!(snapshot.compressed);
    }

    #[test]
    fn test_state_metadata() {
        let metadata = StateMetadata {
            plugin_version: "1.0.0".to_string(),
            config_hash: "abc123".to_string(),
            memory_usage_mb: 256.0,
            uptime_seconds: 3600,
            tags: vec!["production".to_string()],
        };

        assert_eq!(metadata.plugin_version, "1.0.0");
        assert_eq!(metadata.tags.len(), 1);
    }

    #[test]
    fn test_checksum() {
        let data = vec![1u8, 2u8, 3u8];
        let checksum = PluginStateSnapshot::calculate_checksum(&data);

        assert!(!checksum.is_empty());
        assert_eq!(checksum.len(), 64);
    }

    #[test]
    fn test_config_default() {
        let config = StatePreservationConfig::new();
        assert!(config.compression_enabled);
        assert_eq!(config.max_snapshots_per_plugin, 5);
    }
}
