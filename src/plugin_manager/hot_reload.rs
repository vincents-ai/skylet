// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Plugin Hot-Reload Service - RFC-0007
//!
//! This module implements hot-reload functionality for plugins:
//! - File system watching for plugin changes
//! - State serialization before reload
//! - State deserialization after reload
//! - Graceful rollback on failure
//!
//! Integration with:
//! - RFC-0002: PluginLifecycleManager for lifecycle operations
//! - RFC-0004: ABI hot-reload hooks (plugin_prepare_hot_reload, plugin_init_from_state)

use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

use super::lifecycle::{PluginLifecycleManager, PluginStatus};

/// Configuration for the hot-reload service
#[allow(dead_code)] // RFC-0007 hot-reload — not yet wired up
#[derive(Debug, Clone)]
pub struct HotReloadConfig {
    /// Debounce interval for file changes (ms)
    pub debounce_ms: u64,
    /// Maximum time to wait for state serialization (ms)
    pub serialization_timeout_ms: u64,
    /// Maximum time to wait for plugin reload (ms)
    pub reload_timeout_ms: u64,
    /// Whether to auto-reload on file changes
    pub auto_reload: bool,
    /// File patterns to watch (glob patterns)
    pub watch_patterns: Vec<String>,
    /// Directories to exclude from watching
    pub exclude_dirs: Vec<String>,
    /// Number of retry attempts on reload failure
    pub max_retries: u32,
    /// Delay between retry attempts (ms)
    pub retry_delay_ms: u64,
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 500,
            serialization_timeout_ms: 5000,
            reload_timeout_ms: 30000,
            auto_reload: true,
            watch_patterns: vec![
                "*.so".to_string(),
                "*.dylib".to_string(),
                "*.dll".to_string(),
                "config.toml".to_string(),
                "manifest.json".to_string(),
            ],
            exclude_dirs: vec![
                ".git".to_string(),
                "target".to_string(),
                "build".to_string(),
            ],
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

/// State snapshot for hot-reload
#[allow(dead_code)] // RFC-0007 hot-reload — not yet wired up
#[derive(Debug, Clone)]
pub struct PluginStateSnapshot {
    /// Plugin ID
    pub plugin_id: String,
    /// Serialized state bytes
    pub state_data: Vec<u8>,
    /// Timestamp of snapshot
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Version of the plugin that created this snapshot
    pub plugin_version: String,
    /// Checksum for integrity verification
    pub checksum: String,
}

/// Result of a hot-reload operation
#[allow(dead_code)] // RFC-0007 hot-reload — not yet wired up
#[derive(Debug, Clone)]
pub struct HotReloadResult {
    /// Plugin ID
    pub plugin_id: String,
    /// Whether reload was successful
    pub success: bool,
    /// Previous version
    pub old_version: Option<String>,
    /// New version
    pub new_version: Option<String>,
    /// Whether state was preserved
    pub state_preserved: bool,
    /// Time taken for reload (ms)
    pub duration_ms: u64,
    /// Error message if failed
    pub error: Option<String>,
    /// Whether rollback was performed
    pub rolled_back: bool,
}

/// Event emitted by the hot-reload service
#[allow(dead_code)] // RFC-0007 hot-reload — not yet wired up
#[derive(Debug, Clone)]
pub enum HotReloadEvent {
    /// File change detected
    FileChanged { plugin_id: String, path: PathBuf },
    /// Hot-reload started
    ReloadStarted { plugin_id: String },
    /// State serialized successfully
    StateSerialized {
        plugin_id: String,
        size_bytes: usize,
    },
    /// Hot-reload completed
    ReloadCompleted {
        plugin_id: String,
        result: HotReloadResult,
    },
    /// Hot-reload failed
    ReloadFailed { plugin_id: String, error: String },
    /// Rollback performed
    RollbackPerformed { plugin_id: String, reason: String },
}

/// Pending file change for debouncing
#[derive(Debug, Clone)]
#[allow(dead_code)] // RFC-0007 hot-reload — not yet wired up
struct PendingChange {
    plugin_id: String,
    #[allow(dead_code)] // Stored for diagnostics; not yet used in reload logic
    path: PathBuf,
    last_seen: Instant,
}

/// Main hot-reload service
#[allow(dead_code)] // RFC-0007 hot-reload — not yet wired up
pub struct HotReloadService {
    /// Configuration
    config: HotReloadConfig,
    /// Reference to the lifecycle manager
    lifecycle_manager: Arc<PluginLifecycleManager>,
    /// State snapshots for rollback
    snapshots: Arc<RwLock<HashMap<String, PluginStateSnapshot>>>,
    /// Pending file changes (for debouncing)
    pending_changes: Arc<RwLock<HashMap<String, PendingChange>>>,
    /// Event broadcaster
    event_sender: broadcast::Sender<HotReloadEvent>,
    /// Plugin binary paths being watched
    watched_paths: Arc<RwLock<HashMap<PathBuf, String>>>, // path -> plugin_id
    /// Service running flag
    running: Arc<RwLock<bool>>,
}

#[allow(dead_code)] // RFC-0007 hot-reload — not yet wired up
impl HotReloadService {
    /// Create a new hot-reload service
    pub fn new(config: HotReloadConfig, lifecycle_manager: Arc<PluginLifecycleManager>) -> Self {
        let (event_sender, _) = broadcast::channel(256);

        Self {
            config,
            lifecycle_manager,
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            pending_changes: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            watched_paths: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Subscribe to hot-reload events
    pub fn subscribe(&self) -> broadcast::Receiver<HotReloadEvent> {
        self.event_sender.subscribe()
    }

    /// Register a plugin for hot-reload watching
    pub async fn watch_plugin(&self, plugin_id: &str, plugin_path: &Path) -> Result<()> {
        let mut watched = self.watched_paths.write().await;
        watched.insert(plugin_path.to_path_buf(), plugin_id.to_string());

        info!(
            "Registered plugin '{}' for hot-reload watching: {:?}",
            plugin_id, plugin_path
        );
        Ok(())
    }

    /// Unregister a plugin from hot-reload watching
    pub async fn unwatch_plugin(&self, plugin_path: &Path) -> Result<()> {
        let mut watched = self.watched_paths.write().await;
        if let Some(plugin_id) = watched.remove(plugin_path) {
            info!(
                "Unregistered plugin '{}' from hot-reload watching",
                plugin_id
            );
        }
        Ok(())
    }

    /// Start the hot-reload service
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            warn!("Hot-reload service already running");
            return Ok(());
        }
        *running = true;
        drop(running);

        info!("Hot-reload service started");
        Ok(())
    }

    /// Stop the hot-reload service
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;

        // Clear pending changes
        let mut pending = self.pending_changes.write().await;
        pending.clear();

        info!("Hot-reload service stopped");
        Ok(())
    }

    /// Handle a file change event (called by file watcher)
    pub async fn on_file_changed(&self, path: &Path) -> Result<()> {
        let running = self.running.read().await;
        if !*running {
            return Ok(());
        }
        drop(running);

        // Find which plugin this change belongs to
        let plugin_id = {
            let watched = self.watched_paths.read().await;
            watched.get(path).cloned()
        };

        let plugin_id = match plugin_id {
            Some(id) => id,
            None => {
                // Check if it's in a watched directory
                let watched = self.watched_paths.read().await;
                let mut found = None;
                for (watched_path, id) in watched.iter() {
                    if path.starts_with(watched_path.parent().unwrap_or(Path::new(""))) {
                        found = Some(id.clone());
                        break;
                    }
                }
                match found {
                    Some(id) => id,
                    None => {
                        debug!("File change ignored (not watching): {:?}", path);
                        return Ok(());
                    }
                }
            }
        };

        // Emit file change event
        let _ = self.event_sender.send(HotReloadEvent::FileChanged {
            plugin_id: plugin_id.clone(),
            path: path.to_path_buf(),
        });

        if !self.config.auto_reload {
            debug!("Auto-reload disabled, queuing change for {:?}", path);
            return Ok(());
        }

        // Add to pending changes for debouncing
        self.record_pending_change(&plugin_id, path).await?;

        // Process debounced changes
        self.process_debounced_changes().await?;

        Ok(())
    }

    /// Record a pending change for debouncing
    async fn record_pending_change(&self, plugin_id: &str, path: &Path) -> Result<()> {
        let mut pending = self.pending_changes.write().await;
        let key = plugin_id.to_string();

        let now = Instant::now();
        if let Some(existing) = pending.get_mut(&key) {
            existing.last_seen = now;
        } else {
            pending.insert(
                key,
                PendingChange {
                    plugin_id: plugin_id.to_string(),
                    path: path.to_path_buf(),
                    last_seen: now,
                },
            );
        }

        Ok(())
    }

    /// Process debounced file changes
    async fn process_debounced_changes(&self) -> Result<()> {
        let debounce_duration = Duration::from_millis(self.config.debounce_ms);
        let now = Instant::now();

        let to_reload = {
            let mut pending = self.pending_changes.write().await;
            let mut ready = Vec::new();

            pending.retain(|_key, change| {
                if now.duration_since(change.last_seen) >= debounce_duration {
                    ready.push(change.clone());
                    false // Remove from pending
                } else {
                    true // Keep in pending
                }
            });

            ready
        };

        // Trigger reload for each ready change
        for change in to_reload {
            debug!(
                "Processing debounced change for plugin: {}",
                change.plugin_id
            );
            if let Err(e) = self.reload_plugin(&change.plugin_id).await {
                error!("Failed to reload plugin '{}': {}", change.plugin_id, e);
            }
        }

        Ok(())
    }

    /// Perform hot-reload for a plugin
    pub async fn reload_plugin(&self, plugin_id: &str) -> Result<HotReloadResult> {
        let start = Instant::now();

        info!("Starting hot-reload for plugin: {}", plugin_id);

        // Emit reload started event
        let _ = self.event_sender.send(HotReloadEvent::ReloadStarted {
            plugin_id: plugin_id.to_string(),
        });

        // Get current plugin state
        let current_state = self
            .lifecycle_manager
            .get_state(plugin_id)
            .await
            .ok_or_else(|| anyhow!("Plugin '{}' not found", plugin_id))?;

        let old_version = current_state.version.clone();

        // Check if plugin is active
        if current_state.status != PluginStatus::Active {
            return Ok(HotReloadResult {
                plugin_id: plugin_id.to_string(),
                success: false,
                old_version: Some(old_version),
                new_version: None,
                state_preserved: false,
                duration_ms: start.elapsed().as_millis() as u64,
                error: Some("Plugin is not active".to_string()),
                rolled_back: false,
            });
        }

        // Step 1: Serialize plugin state (via ABI hook)
        let snapshot = self.serialize_plugin_state(plugin_id).await?;

        // Emit state serialized event
        let _ = self.event_sender.send(HotReloadEvent::StateSerialized {
            plugin_id: plugin_id.to_string(),
            size_bytes: snapshot.state_data.len(),
        });

        // Step 2: Deactivate plugin
        let deactivate_result = self
            .lifecycle_manager
            .deactivate(plugin_id)
            .await
            .with_context(|| "Deactivating plugin for reload")?;

        if !deactivate_result.success {
            return Ok(HotReloadResult {
                plugin_id: plugin_id.to_string(),
                success: false,
                old_version: Some(old_version),
                new_version: None,
                state_preserved: false,
                duration_ms: start.elapsed().as_millis() as u64,
                error: deactivate_result.error,
                rolled_back: false,
            });
        }

        // Step 3: Attempt to reactivate with new binary
        let activate_result = self.lifecycle_manager.activate(plugin_id).await;

        match activate_result {
            Ok(result) if result.success => {
                // Step 4: Restore state (via ABI hook)
                let state_restored = self
                    .restore_plugin_state(plugin_id, &snapshot)
                    .await
                    .unwrap_or(false);

                let new_version = self
                    .lifecycle_manager
                    .get_state(plugin_id)
                    .await
                    .map(|s| s.version);

                let reload_result = HotReloadResult {
                    plugin_id: plugin_id.to_string(),
                    success: true,
                    old_version: Some(old_version),
                    new_version,
                    state_preserved: state_restored,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error: None,
                    rolled_back: false,
                };

                // Emit completion event
                let _ = self.event_sender.send(HotReloadEvent::ReloadCompleted {
                    plugin_id: plugin_id.to_string(),
                    result: reload_result.clone(),
                });

                info!(
                    "Hot-reload completed for plugin '{}' in {}ms",
                    plugin_id, reload_result.duration_ms
                );

                Ok(reload_result)
            }
            Ok(result) => {
                // Activation failed - attempt rollback
                warn!(
                    "Activation failed after reload, attempting rollback: {:?}",
                    result.error
                );
                let rolled_back = self
                    .rollback_plugin(plugin_id, &snapshot)
                    .await
                    .unwrap_or(false);

                let reload_result = HotReloadResult {
                    plugin_id: plugin_id.to_string(),
                    success: false,
                    old_version: Some(old_version),
                    new_version: None,
                    state_preserved: false,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error: result.error,
                    rolled_back,
                };

                // Emit failure event
                let _ = self.event_sender.send(HotReloadEvent::ReloadFailed {
                    plugin_id: plugin_id.to_string(),
                    error: reload_result.error.clone().unwrap_or_default(),
                });

                Ok(reload_result)
            }
            Err(e) => {
                // Activation error - attempt rollback
                error!("Activation error after reload: {}", e);
                let rolled_back = self
                    .rollback_plugin(plugin_id, &snapshot)
                    .await
                    .unwrap_or(false);

                let reload_result = HotReloadResult {
                    plugin_id: plugin_id.to_string(),
                    success: false,
                    old_version: Some(old_version),
                    new_version: None,
                    state_preserved: false,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error: Some(e.to_string()),
                    rolled_back,
                };

                // Emit failure event
                let _ = self.event_sender.send(HotReloadEvent::ReloadFailed {
                    plugin_id: plugin_id.to_string(),
                    error: reload_result.error.clone().unwrap_or_default(),
                });

                Ok(reload_result)
            }
        }
    }

    /// Serialize plugin state using ABI hook
    async fn serialize_plugin_state(&self, plugin_id: &str) -> Result<PluginStateSnapshot> {
        debug!("Serializing state for plugin: {}", plugin_id);

        // Get plugin version
        let state = self
            .lifecycle_manager
            .get_state(plugin_id)
            .await
            .ok_or_else(|| anyhow!("Plugin '{}' not found", plugin_id))?;
        let plugin_version = state.version;

        // In a real implementation, this would call plugin_prepare_hot_reload via ABI
        // For now, we create an empty state snapshot
        // The actual ABI integration would look like:
        // let plugin_state = unsafe { (prepare_fn)(context) };
        // let state_data = Vec::from_raw_parts(plugin_state.data, plugin_state.len, plugin_state.len);

        let state_data = format!(
            "{{\"plugin_id\":\"{}\",\"timestamp\":\"{}\"}}",
            plugin_id,
            chrono::Utc::now().to_rfc3339()
        )
        .into_bytes();

        let checksum = format!("{:x}", md5_hash(&state_data));

        let snapshot = PluginStateSnapshot {
            plugin_id: plugin_id.to_string(),
            state_data,
            timestamp: chrono::Utc::now(),
            plugin_version,
            checksum,
        };

        // Store snapshot for potential rollback
        let mut snapshots = self.snapshots.write().await;
        snapshots.insert(plugin_id.to_string(), snapshot.clone());

        Ok(snapshot)
    }

    /// Restore plugin state using ABI hook
    async fn restore_plugin_state(
        &self,
        plugin_id: &str,
        snapshot: &PluginStateSnapshot,
    ) -> Result<bool> {
        debug!("Restoring state for plugin: {}", plugin_id);

        // Verify checksum
        let expected_checksum = format!("{:x}", md5_hash(&snapshot.state_data));
        if expected_checksum != snapshot.checksum {
            warn!(
                "State checksum mismatch for plugin '{}', skipping restore",
                plugin_id
            );
            return Ok(false);
        }

        // In a real implementation, this would call plugin_init_from_state via ABI
        // let result = unsafe { (init_from_state_fn)(context, plugin_state) };
        // return Ok(result == PluginResult::Success);

        // For now, just log that we would restore
        info!(
            "Would restore {} bytes of state for plugin '{}'",
            snapshot.state_data.len(),
            plugin_id
        );

        Ok(true)
    }

    /// Rollback plugin to previous state
    async fn rollback_plugin(
        &self,
        plugin_id: &str,
        snapshot: &PluginStateSnapshot,
    ) -> Result<bool> {
        info!("Attempting rollback for plugin: {}", plugin_id);

        // Emit rollback event
        let _ = self.event_sender.send(HotReloadEvent::RollbackPerformed {
            plugin_id: plugin_id.to_string(),
            reason: "Reload failed".to_string(),
        });

        // Attempt to reactivate with old state
        let activate_result = self.lifecycle_manager.activate(plugin_id).await;

        match activate_result {
            Ok(result) if result.success => {
                // Try to restore previous state
                let restored = self
                    .restore_plugin_state(plugin_id, snapshot)
                    .await
                    .unwrap_or(false);
                info!(
                    "Rollback successful for plugin '{}', state restored: {}",
                    plugin_id, restored
                );
                Ok(true)
            }
            _ => {
                error!("Rollback failed for plugin '{}'", plugin_id);
                Ok(false)
            }
        }
    }

    /// Get list of watched plugins
    pub async fn list_watched(&self) -> Vec<(String, PathBuf)> {
        let watched = self.watched_paths.read().await;
        watched
            .iter()
            .map(|(path, id)| (id.clone(), path.clone()))
            .collect()
    }

    /// Check if a plugin is being watched
    pub async fn is_watching(&self, plugin_id: &str) -> bool {
        let watched = self.watched_paths.read().await;
        watched.values().any(|id| id == plugin_id)
    }

    /// Get the current snapshot for a plugin (if any)
    pub async fn get_snapshot(&self, plugin_id: &str) -> Option<PluginStateSnapshot> {
        let snapshots = self.snapshots.read().await;
        snapshots.get(plugin_id).cloned()
    }

    /// Clear snapshot for a plugin
    pub async fn clear_snapshot(&self, plugin_id: &str) {
        let mut snapshots = self.snapshots.write().await;
        snapshots.remove(plugin_id);
    }
}

/// Simple MD5 hash for checksum (for state integrity verification)
#[allow(dead_code)] // RFC-0007 hot-reload — not yet wired up
fn md5_hash(data: &[u8]) -> u128 {
    // Simple hash for demonstration - in production use a proper hash function
    let mut hash: u128 = 0;
    for (i, byte) in data.iter().enumerate() {
        hash = hash.wrapping_add((*byte as u128).wrapping_mul((i + 1) as u128));
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_service() -> (HotReloadService, Arc<PluginLifecycleManager>) {
        let temp_dir = tempdir().unwrap();
        let config = super::super::lifecycle::LifecycleConfig::new(temp_dir.path().join("plugins"));
        let manager = Arc::new(PluginLifecycleManager::new(config).unwrap());
        let service = HotReloadService::new(HotReloadConfig::default(), manager.clone());
        (service, manager)
    }

    #[tokio::test]
    async fn test_hot_reload_config_default() {
        let config = HotReloadConfig::default();
        assert_eq!(config.debounce_ms, 500);
        assert!(config.auto_reload);
        assert_eq!(config.max_retries, 3);
    }

    #[tokio::test]
    async fn test_hot_reload_service_creation() {
        let (service, _) = create_test_service();
        assert!(service.list_watched().await.is_empty());
    }

    #[tokio::test]
    async fn test_watch_plugin() {
        let (service, _) = create_test_service();
        let path = PathBuf::from("/tmp/test_plugin.so");

        service.watch_plugin("test-plugin", &path).await.unwrap();

        assert!(service.is_watching("test-plugin").await);
        let watched = service.list_watched().await;
        assert_eq!(watched.len(), 1);
        assert_eq!(watched[0].0, "test-plugin");
    }

    #[tokio::test]
    async fn test_unwatch_plugin() {
        let (service, _) = create_test_service();
        let path = PathBuf::from("/tmp/test_plugin.so");

        service.watch_plugin("test-plugin", &path).await.unwrap();
        assert!(service.is_watching("test-plugin").await);

        service.unwatch_plugin(&path).await.unwrap();
        assert!(!service.is_watching("test-plugin").await);
    }

    #[tokio::test]
    async fn test_start_stop_service() {
        let (service, _) = create_test_service();

        service.start().await.unwrap();
        let running = service.running.read().await;
        assert!(*running);
        drop(running);

        service.stop().await.unwrap();
        let running = service.running.read().await;
        assert!(!*running);
    }

    #[tokio::test]
    async fn test_subscribe_events() {
        let (service, _) = create_test_service();
        let mut receiver = service.subscribe();

        // Send a test event
        let _ = service.event_sender.send(HotReloadEvent::ReloadStarted {
            plugin_id: "test".to_string(),
        });

        // Receive should work
        let event = receiver.try_recv();
        assert!(event.is_ok());
    }

    #[tokio::test]
    async fn test_state_snapshot_creation() {
        let snapshot = PluginStateSnapshot {
            plugin_id: "test".to_string(),
            state_data: b"test data".to_vec(),
            timestamp: chrono::Utc::now(),
            plugin_version: "1.0.0".to_string(),
            checksum: "abc123".to_string(),
        };

        assert_eq!(snapshot.plugin_id, "test");
        assert_eq!(snapshot.state_data.len(), 9);
    }

    #[tokio::test]
    async fn test_hot_reload_result() {
        let result = HotReloadResult {
            plugin_id: "test".to_string(),
            success: true,
            old_version: Some("1.0.0".to_string()),
            new_version: Some("2.0.0".to_string()),
            state_preserved: true,
            duration_ms: 150,
            error: None,
            rolled_back: false,
        };

        assert!(result.success);
        assert!(result.state_preserved);
        assert!(!result.rolled_back);
    }

    #[tokio::test]
    async fn test_md5_hash() {
        let data1 = b"test data";
        let data2 = b"test data";
        let data3 = b"different data";

        let hash1 = md5_hash(data1);
        let hash2 = md5_hash(data2);
        let hash3 = md5_hash(data3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[tokio::test]
    async fn test_on_file_changed_not_watching() {
        let (service, _) = create_test_service();
        service.start().await.unwrap();

        // File not being watched should be ignored
        let result = service.on_file_changed(Path::new("/tmp/unknown.so")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_on_file_changed_when_stopped() {
        let (service, _) = create_test_service();
        // Service not started

        let result = service.on_file_changed(Path::new("/tmp/test.so")).await;
        assert!(result.is_ok()); // Should silently ignore
    }

    #[tokio::test]
    async fn test_get_snapshot_empty() {
        let (service, _) = create_test_service();

        let snapshot = service.get_snapshot("nonexistent").await;
        assert!(snapshot.is_none());
    }

    #[tokio::test]
    async fn test_clear_snapshot() {
        let (service, _) = create_test_service();

        // Manually insert a snapshot
        {
            let mut snapshots = service.snapshots.write().await;
            snapshots.insert(
                "test".to_string(),
                PluginStateSnapshot {
                    plugin_id: "test".to_string(),
                    state_data: vec![],
                    timestamp: chrono::Utc::now(),
                    plugin_version: "1.0.0".to_string(),
                    checksum: "test".to_string(),
                },
            );
        }

        assert!(service.get_snapshot("test").await.is_some());

        service.clear_snapshot("test").await;

        assert!(service.get_snapshot("test").await.is_none());
    }
}
