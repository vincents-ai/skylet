// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

#[allow(unused_imports)]
use anyhow::{Context, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigReloadEvent {
    pub plugin_name: String,
    pub config_path: PathBuf,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub reload_status: ReloadStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReloadStatus {
    Started,
    Success,
    Failed(String),
    Skipped,
}

#[derive(Debug, Clone)]
pub struct ReloadConfig {
    pub enabled: bool,
    #[allow(dead_code)] // Config field — future debounce support
    pub debounce_duration: Duration,
    pub validate_after_reload: bool,
    #[allow(dead_code)] // Config field — future backup/retry support
    pub backup_on_failure: bool,
    #[allow(dead_code)] // Config field — future backup/retry support
    pub max_retries: usize,
}

impl Default for ReloadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            debounce_duration: Duration::from_millis(500),
            validate_after_reload: true,
            backup_on_failure: true,
            max_retries: 3,
        }
    }
}

#[allow(dead_code)] // Public API — not yet called from production code
pub type ReloadCallback = Arc<dyn Fn(ConfigReloadEvent) + Send + Sync>;

#[allow(dead_code)] // Public API — not yet called from production code
pub struct ConfigHotReload {
    config_dir: PathBuf,
    reload_config: ReloadConfig,
    configs: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    #[allow(dead_code)] // Stored for future per-plugin watcher support
    watchers: Arc<RwLock<HashMap<String, RecommendedWatcher>>>,
    callbacks: Arc<RwLock<Vec<ReloadCallback>>>,
    event_history: Arc<RwLock<Vec<ConfigReloadEvent>>>,
    _watcher: RecommendedWatcher,
}

#[allow(dead_code)] // Public API — not yet called from production code
impl ConfigHotReload {
    pub fn new(config_dir: PathBuf) -> Result<Self> {
        let reload_config = ReloadConfig::default();
        Self::with_config(config_dir, reload_config)
    }

    pub fn with_config(config_dir: PathBuf, reload_config: ReloadConfig) -> Result<Self> {
        let configs = Arc::new(RwLock::new(HashMap::new()));
        let watchers = Arc::new(RwLock::new(HashMap::new()));
        let callbacks = Arc::new(RwLock::new(Vec::new()));
        let event_history = Arc::new(RwLock::new(Vec::new()));

        let configs_clone = configs.clone();
        let callbacks_clone = callbacks.clone();
        let event_history_clone = event_history.clone();
        let reload_config_clone = reload_config.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
            if let Ok(event) = res {
                Self::handle_watch_event(
                    event,
                    configs_clone.clone(),
                    callbacks_clone.clone(),
                    event_history_clone.clone(),
                    reload_config_clone.clone(),
                );
            }
        })?;

        watcher.watch(&config_dir, RecursiveMode::NonRecursive)?;

        Ok(Self {
            config_dir,
            reload_config,
            configs,
            watchers,
            callbacks,
            event_history,
            _watcher: watcher,
        })
    }

    fn handle_watch_event(
        event: notify::Event,
        configs: Arc<RwLock<HashMap<String, serde_json::Value>>>,
        callbacks: Arc<RwLock<Vec<ReloadCallback>>>,
        event_history: Arc<RwLock<Vec<ConfigReloadEvent>>>,
        reload_config: ReloadConfig,
    ) {
        if !reload_config.enabled {
            return;
        }

        for path in event.paths {
            if let Some(file_name) = path.file_stem() {
                let plugin_name = file_name.to_string_lossy().to_string();
                let configs_clone = configs.clone();
                let callbacks_clone = callbacks.clone();
                let event_history_clone = event_history.clone();
                let reload_config_clone = reload_config.clone();

                tokio::spawn(async move {
                    let reload_event = ConfigReloadEvent {
                        plugin_name: plugin_name.clone(),
                        config_path: path.clone(),
                        timestamp: chrono::Utc::now(),
                        reload_status: ReloadStatus::Started,
                    };

                    Self::log_event(&event_history_clone, reload_event.clone()).await;
                    Self::notify_callbacks(&callbacks_clone, reload_event.clone()).await;

                    if reload_config_clone.validate_after_reload {
                        if let Err(e) = Self::validate_and_reload(
                            &configs_clone,
                            &plugin_name,
                            &path,
                            &event_history_clone,
                            &callbacks_clone,
                            &reload_config_clone,
                        )
                        .await
                        {
                            tracing::error!("Config reload failed for {}: {}", plugin_name, e);
                        }
                    }
                });
            }
        }
    }

    async fn validate_and_reload(
        configs: &Arc<RwLock<HashMap<String, serde_json::Value>>>,
        plugin_name: &str,
        config_path: &PathBuf,
        event_history: &Arc<RwLock<Vec<ConfigReloadEvent>>>,
        callbacks: &Arc<RwLock<Vec<ReloadCallback>>>,
        reload_config: &ReloadConfig,
    ) -> Result<()> {
        use anyhow::Context;
        sleep(reload_config.debounce_duration).await;

        let config_content = tokio::fs::read_to_string(config_path)
            .await
            .context("Failed to read config file")?;

        let new_config: serde_json::Value = if config_path
            .extension()
            .map(|e| e == "toml")
            .unwrap_or(false)
        {
            let toml_value: toml::Value = toml::from_str(&config_content)?;
            Self::toml_to_json(toml_value)
        } else {
            serde_json::from_str(&config_content)?
        };

        if let Err(e) = serde_json::to_string(&new_config) {
            let failed_event = ConfigReloadEvent {
                plugin_name: plugin_name.to_string(),
                config_path: config_path.clone(),
                timestamp: chrono::Utc::now(),
                reload_status: ReloadStatus::Failed(e.to_string()),
            };
            Self::log_event(event_history, failed_event.clone()).await;
            Self::notify_callbacks(callbacks, failed_event).await;
            return Err(e.into());
        }

        {
            let mut configs = configs.write().await;
            configs.insert(plugin_name.to_string(), new_config);
        }

        let success_event = ConfigReloadEvent {
            plugin_name: plugin_name.to_string(),
            config_path: config_path.clone(),
            timestamp: chrono::Utc::now(),
            reload_status: ReloadStatus::Success,
        };
        Self::log_event(event_history, success_event.clone()).await;
        Self::notify_callbacks(callbacks, success_event).await;

        Ok(())
    }

    async fn log_event(
        event_history: &Arc<RwLock<Vec<ConfigReloadEvent>>>,
        event: ConfigReloadEvent,
    ) {
        let mut history = event_history.write().await;
        history.push(event);

        if history.len() > 1000 {
            history.truncate(1000);
        }
    }

    async fn notify_callbacks(
        callbacks: &Arc<RwLock<Vec<ReloadCallback>>>,
        event: ConfigReloadEvent,
    ) {
        let callbacks = callbacks.read().await;
        for callback in callbacks.iter() {
            callback(event.clone());
        }
    }

    fn toml_to_json(toml: toml::Value) -> serde_json::Value {
        match toml {
            toml::Value::String(s) => serde_json::Value::String(s),
            toml::Value::Integer(i) => serde_json::Value::Number(i.into()),
            toml::Value::Float(f) => {
                if let Some(n) = serde_json::Number::from_f64(f) {
                    serde_json::Value::Number(n)
                } else {
                    serde_json::Value::Null
                }
            }
            toml::Value::Boolean(b) => serde_json::Value::Bool(b),
            toml::Value::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(Self::toml_to_json).collect())
            }
            toml::Value::Table(table) => serde_json::Value::Object(
                table
                    .into_iter()
                    .map(|(k, v)| (k, Self::toml_to_json(v)))
                    .collect(),
            ),
            toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
        }
    }

    pub async fn add_reload_callback(&self, callback: ReloadCallback) {
        let mut callbacks = self.callbacks.write().await;
        callbacks.push(callback);
    }

    pub async fn get_config(&self, plugin_name: &str) -> Option<serde_json::Value> {
        let configs = self.configs.read().await;
        configs.get(plugin_name).cloned()
    }

    pub async fn reload_plugin_config(&self, plugin_name: &str) -> Result<()> {
        let config_path = self.config_dir.join(format!("{}.toml", plugin_name));

        if !config_path.exists() {
            return Err(anyhow::anyhow!("Config file not found: {:?}", config_path));
        }

        let config_content = tokio::fs::read_to_string(&config_path).await?;

        let new_config: serde_json::Value = if config_path
            .extension()
            .map(|e| e == "toml")
            .unwrap_or(false)
        {
            let toml_value: toml::Value = toml::from_str(&config_content)?;
            Self::toml_to_json(toml_value)
        } else {
            serde_json::from_str(&config_content)?
        };

        {
            let mut configs = self.configs.write().await;
            configs.insert(plugin_name.to_string(), new_config);
        }

        Ok(())
    }

    pub async fn get_event_history(&self) -> Vec<ConfigReloadEvent> {
        let history = self.event_history.read().await;
        history.clone()
    }

    pub async fn clear_event_history(&self) {
        let mut history = self.event_history.write().await;
        history.clear();
    }

    pub fn config_dir(&self) -> &PathBuf {
        &self.config_dir
    }

    pub fn reload_config(&self) -> &ReloadConfig {
        &self.reload_config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reload_config_default() {
        let config = ReloadConfig::default();
        assert!(config.enabled);
        assert!(config.validate_after_reload);
        assert!(config.backup_on_failure);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_config_reload_event() {
        let event = ConfigReloadEvent {
            plugin_name: "test".to_string(),
            config_path: PathBuf::from("/test/config.toml"),
            timestamp: chrono::Utc::now(),
            reload_status: ReloadStatus::Success,
        };

        assert_eq!(event.plugin_name, "test");
        assert_eq!(event.reload_status, ReloadStatus::Success);
    }

    #[test]
    fn test_reload_status_eq() {
        assert_eq!(ReloadStatus::Success, ReloadStatus::Success);
        assert_eq!(
            ReloadStatus::Failed("test".to_string()),
            ReloadStatus::Failed("test".to_string())
        );
        assert_ne!(
            ReloadStatus::Success,
            ReloadStatus::Failed("error".to_string())
        );
    }
}
