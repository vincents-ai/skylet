// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

pub mod rollback;
pub mod state_manager;
pub mod types;

use std::time::Duration;

pub mod rollback;
pub mod state_manager;
pub mod types;

use crate::plugin_manager::dependency_resolver::DependencyResolver;

/// Enhanced hot-reload configuration
#[derive(Debug, Clone)]
pub struct EnhancedHotReloadConfig {
    pub enabled: bool,
    pub debounce_duration: Duration,
    pub state_preservation_enabled: bool,
    pub rollback_enabled: bool,
    pub rollback_timeout: Duration,
    pub monitoring_enabled: bool,
    pub alert_on_failure: bool,
    pub max_reload_attempts: u32,
    pub reload_batch_size: usize,
}

impl Default for EnhancedHotReloadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            debounce_duration: Duration::from_millis(500),
            state_preservation_enabled: true,
            rollback_enabled: true,
            rollback_timeout: Duration::from_secs(30),
            monitoring_enabled: true,
            alert_on_failure: true,
            max_reload_attempts: 3,
            reload_batch_size: 5,
        }
    }
}

/// Main enhanced hot-reload manager
pub struct EnhancedHotReloadManager {
    config: EnhancedHotReloadConfig,
    dependency_resolver: Arc<DependencyResolver>,
    state_manager: Arc<state_manager::StateManager>,
    rollback_manager: Arc<rollback::RollbackManager>,
    reload_queue: Arc<tokio::sync::RwLock<Vec<ReloadRequest>>>,
    monitoring: Arc<tokio::sync::RwLock<ReloadMonitoring>>,
    alerts: Arc<tokio::sync::RwLock<Vec<ReloadAlert>>>>,
    active_reloads: Arc<tokio::sync::RwLock<HashMap<String, ReloadState>>>>,
}

/// Reload request with dependencies
#[derive(Debug, Clone)]
pub struct ReloadRequest {
    pub plugin_id: String,
    pub plugin_path: std::path::PathBuf,
    pub reason: ReloadReason,
    pub requested_by: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub dependencies: Vec<String>,
}

/// Reload state tracking
#[derive(Debug, Clone)]
pub struct ReloadState {
    pub plugin_id: String,
    pub status: ReloadStatus,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub attempts: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadStatus {
    Queued,
    InProgress,
    Completed,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadReason {
    FileChange,
    ManualRequest,
    DependencyUpdate,
    VersionUpgrade,
    Recovery,
}

/// Reload monitoring data
#[derive(Debug, Clone)]
pub struct ReloadMonitoring {
    pub total_reloads: u64,
    pub successful_reloads: u64,
    pub failed_reloads: u64,
    pub rollback_count: u64,
    pub average_reload_time_ms: f64,
    pub last_reload_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Reload alert
#[derive(Debug, Clone)]
pub struct ReloadAlert {
    pub plugin_id: String,
    pub alert_type: AlertType,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub resolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertType {
    ReloadFailed,
    RollbackRequired,
    StateLoss,
    Timeout,
    DependencyError,
}

#[derive(Debug, Clone)]
pub struct ReloadResult {
    pub plugin_id: String,
    pub success: bool,
    pub duration_ms: f64,
    pub error: Option<String>,
    pub state_preserved: bool,
    pub rollback_performed: bool,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum HotReloadError {
    #[error("Hot-reload is disabled")]
    Disabled,
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),
    #[error("Dependency error: {0}")]
    DependencyError(String),
    #[error("State error: {0}")]
    StateError(String),
    #[error("Rollback error: {0}")]
    RollbackError(String),
    #[error("Timeout: {0}ms exceeded")]
    Timeout(u64),
}

impl EnhancedHotReloadManager {
    pub fn new(
        config: EnhancedHotReloadConfig,
        dependency_resolver: Arc<DependencyResolver>,
    ) -> Self {
        let state_manager = Arc::new(state_manager::StateManager::new(
            config.state_preservation_enabled,
            config.rollback_timeout,
        ));

        let rollback_manager = Arc::new(rollback::RollbackManager::new(
            config.rollback_enabled,
            config.rollback_timeout,
        ));

        Self {
            config,
            dependency_resolver,
            state_manager,
            rollback_manager,
            reload_queue: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            monitoring: Arc::new(tokio::sync::RwLock::new(ReloadMonitoring {
                total_reloads: 0,
                successful_reloads: 0,
                failed_reloads: 0,
                rollback_count: 0,
                average_reload_time_ms: 0.0,
                last_reload_at: None,
            })),
            alerts: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            active_reloads: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    pub async fn request_reload(
        &self,
        plugin_id: String,
        plugin_path: std::path::PathBuf,
        reason: ReloadReason,
        requested_by: String,
    ) -> Result<String, HotReloadError> {
        if !self.config.enabled {
            return Err(HotReloadError::Disabled);
        }

        let dependencies = self
            .dependency_resolver
            .get_plugin_dependencies(&plugin_id)
            .await
            .unwrap_or_default();

        let request = ReloadRequest {
            plugin_id: plugin_id.clone(),
            plugin_path: plugin_path.clone(),
            reason,
            requested_by,
            timestamp: chrono::Utc::now(),
            dependencies,
        };

        let request_id = request.plugin_id.clone();

        let mut queue = self.reload_queue.write().await;
        queue.push(request);

        Ok(request_id)
    }

    pub async fn request_batch_reload(
        &self,
        plugins: Vec<(String, std::path::PathBuf, ReloadReason)>,
        requested_by: String,
    ) -> Result<Vec<String>, HotReloadError> {
        if !self.config.enabled {
            return Err(HotReloadError::Disabled);
        }

        let mut request_ids = Vec::new();

        for (plugin_id, plugin_path, reason) in plugins {
            let request_id = self
                .request_reload(plugin_id.clone(), plugin_path.clone(), reason.clone(), requested_by.clone())
                .await?;

            request_ids.push(request_id);
        }

        Ok(request_ids)
    }

    pub async fn process_reload_queue(&self) -> Result<usize, HotReloadError> {
        let mut queue = self.reload_queue.write().await;

        if queue.is_empty() {
            return Ok(0);
        }

        let batch_size = self.config.reload_batch_size.min(queue.len());
        let batch: Vec<ReloadRequest> = queue.drain(..batch_size).collect();

        let processed = self.process_reload_batch(batch).await?;

        let mut monitoring = self.monitoring.write().await;
        monitoring.total_reloads += processed as u64;

        Ok(processed)
    }

    async fn process_reload_batch(&self, batch: Vec<ReloadRequest>) -> Result<usize> {
        let mut processed = 0;

        for request in batch {
            match self.reload_single_plugin(&request).await {
                Ok(_) => {
                    processed += 1;

                    let mut monitoring = self.monitoring.write().await;
                    monitoring.successful_reloads += 1;
                }
                Err(e) => {
                    let alert = ReloadAlert {
                        plugin_id: request.plugin_id.clone(),
                        alert_type: AlertType::ReloadFailed,
                        message: format!("Reload failed: {}", e),
                        timestamp: chrono::Utc::now(),
                        resolved: false,
                    };

                    let mut alerts = self.alerts.write().await;
                    alerts.push(alert);

                    if self.config.alert_on_failure {
                        self.send_alert(&alert).await;
                    }

                    if self.config.rollback_enabled {
                        let _ = self.rollback_manager.rollback(&request.plugin_id).await;
                    }

                    let mut monitoring = self.monitoring.write().await;
                    monitoring.failed_reloads += 1;
                    monitoring.rollback_count += 1;
                }
            }
        }

        Ok(processed)
    }

    async fn reload_single_plugin(&self, request: &ReloadRequest) -> Result<ReloadResult> {
        let start_time = std::time::Instant::now();

        let state = ReloadState {
            plugin_id: request.plugin_id.clone(),
            status: ReloadStatus::InProgress,
            started_at: chrono::Utc::now(),
            completed_at: None,
            attempts: 1,
            last_error: None,
        };

        let mut active = self.active_reloads.write().await;
        active.insert(request.plugin_id.clone(), state);

        let result = self.perform_reload(request).await?;

        let elapsed = start_time.elapsed().as_millis() as f64;

        let result = ReloadResult {
            plugin_id: request.plugin_id.clone(),
            success: result.is_ok(),
            duration_ms: elapsed,
            error: result.err().map(|e| e.to_string()),
            state_preserved: false,
            rollback_performed: false,
        };

        if result.is_ok() {
            let _ = self.state_manager.save_state(&request.plugin_id).await;
            result.state_preserved = true;
        }

        let mut active = self.active_reloads.write().await;
        if let Some(mut state) = active.get_mut(&request.plugin_id) {
            state.status = ReloadStatus::Completed;
            state.completed_at = Some(chrono::Utc::now());
        }

        let mut monitoring = self.monitoring.write().await;
        let current_avg = monitoring.average_reload_time_ms;
        let new_avg = (current_avg * (monitoring.successful_reloads as f64) + elapsed)
            / (monitoring.successful_reloads + 1) as f64;
        monitoring.average_reload_time_ms = new_avg;
        monitoring.last_reload_at = Some(chrono::Utc::now());

        result
    }

    async fn perform_reload(&self, request: &ReloadRequest) -> Result<()> {
        tokio::time::sleep(self.config.debounce_duration).await;

        if let Some(state) = self.state_manager.get_snapshot(&request.plugin_id).await {
            let _ = self.state_manager.restore_state(&request.plugin_id, &state).await;
        }

        Ok(())
    }

    async fn rollback_plugin(&self, plugin_id: &str) -> Result<()> {
        let _ = self.rollback_manager.rollback(plugin_id).await;

        let alert = ReloadAlert {
            plugin_id: plugin_id.to_string(),
            alert_type: AlertType::RollbackRequired,
            message: format!("Plugin {} rolled back to previous state", plugin_id),
            timestamp: chrono::Utc::now(),
            resolved: true,
        };

        let mut alerts = self.alerts.write().await;
        alerts.push(alert);

        Ok(())
    }

    async fn get_reload_status(&self, plugin_id: &str) -> Option<ReloadState> {
        let active = self.active_reloads.read().await;
        active.get(plugin_id).cloned()
    }

    async fn get_monitoring(&self) -> ReloadMonitoring {
        let monitoring = self.monitoring.read().await;
        monitoring.clone()
    }

    async fn get_alerts(&self, limit: usize) -> Vec<ReloadAlert> {
        let alerts = self.alerts.read().await;
        alerts.iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    async fn clear_alerts(&self) {
        let mut alerts = self.alerts.write().await;
        alerts.clear();
    }

    async fn send_alert(&self, alert: &ReloadAlert) {
        tracing::warn!(
            "Hot-reload alert [{}]: {} - {}",
            alert.alert_type,
            alert.plugin_id,
            alert.message
        );

        println!("ALERT: {:?}", alert);
    }

    pub fn config(&self) -> &EnhancedHotReloadConfig {
        &self.config
    }

    pub fn dependency_resolver(&self) -> Arc<DependencyResolver> {
        self.dependency_resolver.clone()
    }

    pub fn state_manager(&self) -> Arc<state_manager::StateManager> {
        self.state_manager.clone()
    }

    pub fn rollback_manager(&self) -> Arc<rollback::RollbackManager> {
        self.rollback_manager.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = EnhancedHotReloadConfig::default();
        assert!(config.enabled);
    }

    #[test]
    fn test_reload_request() {
        let request = ReloadRequest {
            plugin_id: "test_plugin".to_string(),
            plugin_path: std::path::PathBuf::from("/path/to/plugin.so"),
            reason: ReloadReason::ManualRequest,
            requested_by: "user".to_string(),
            timestamp: chrono::Utc::now(),
            dependencies: vec
!["dep1".to_string(), "dep2".to_string()],
        };

        assert_eq!(request.plugin_id, "test_plugin");
        assert_eq!(request.reason, ReloadReason::ManualRequest);
    }

    #[test]
    fn test_reload_status() {
        let state = ReloadState {
            plugin_id: "test".to_string(),
            status: ReloadStatus::InProgress,
            started_at: chrono::Utc::now(),
            completed_at: None,
            attempts: 1,
            last_error: None,
        };

        assert_eq!(state.status, ReloadStatus::InProgress);
        assert!(state.completed_at.is_none());
    }

    #[test]
    fn test_alert() {
        let alert = ReloadAlert {
            plugin_id: "test".to_string(),
            alert_type: AlertType::ReloadFailed,
            message: "Test error".to_string(),
            timestamp: chrono::Utc::now(),
            resolved: false,
        };

        assert_eq!(alert.alert_type, AlertType::ReloadFailed);
        assert!(!alert.resolved);
    }

    #[test]
    fn test_monitoring() {
        let monitoring = ReloadMonitoring {
            total_reloads: 100,
            successful_reloads: 80,
            failed_reloads: 15,
            rollback_count: 5,
            average_reload_time_ms: 250.5,
            last_reload_at: Some(chrono::Utc::now()),
        };

        assert_eq!(monitoring.total_reloads, 100);
        assert_eq!(monitoring.successful_reloads, 80);
        assert_eq!(monitoring.rollback_count, 5);
    }

    #[test]
    fn test_reload_result() {
        let result = ReloadResult {
            plugin_id: "test".to_string(),
            success: true,
            duration_ms: 100.0,
            error: None,
            state_preserved: true,
            rollback_performed: false,
        };

        assert!(result.success);
        assert!(result.state_preserved);
        assert!(result.duration_ms, 100.0);
    }
}
