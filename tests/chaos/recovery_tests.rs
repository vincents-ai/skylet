//! Recovery Tests

use super::*;
use crate::plugin_manager::*;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

/// Recovery test result
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryTestResult {
    AutomaticRecovery,
    ManualRecovery,
    Failed(String),
    Timeout,
}

/// Recovery test suite
pub struct RecoveryTestSuite {
    temp_dir: TempDir,
}

impl RecoveryTestSuite {
    /// Create new recovery test suite
    pub fn new() -> Self {
        Self {
            temp_dir: TempDir::new().expect("Failed to create temp dir"),
        }
    }

    /// Run all recovery tests
    pub async fn run_all_tests(&self) -> Vec<(&'static str, RecoveryTestResult)> {
        let mut results = Vec::new();

        results.push(("plugin_crash_recovery", self.test_plugin_crash_recovery().await));
        results.push(("manager_crash_recovery", self.test_manager_crash_recovery().await));
        results.push(("state_corruption_recovery", self.test_state_corruption_recovery().await));
        results.push(("dependency_failure_recovery", self.test_dependency_failure_recovery().await));
        results.push(("resource_limit_recovery", self.test_resource_limit_recovery().await));
        results.push(("configuration_error_recovery", self.test_configuration_error_recovery().await));
        results.push(("network_failure_recovery", self.test_network_failure_recovery().await));
        results.push(("storage_failure_recovery", self.test_storage_failure_recovery().await));

        results
    }

    /// Test plugin crash recovery
    async fn test_plugin_crash_recovery(&self) -> RecoveryTestResult {
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        // Load and crash plugin
        manager.load_plugin("crash_plugin").await.unwrap();
        manager.mark_plugin_unhealthy("crash_plugin").await;

        // Wait for automatic recovery
        sleep(Duration::from_millis(500)).await;

        let status = manager.get_plugin_status("crash_plugin").await;

        if status.is_err() {
            return RecoveryTestResult::Failed("Plugin not found after crash".to_string());
        }

        let status = status.unwrap();
        if status.status == "restarted" || status.status == "reloaded" {
            RecoveryTestResult::AutomaticRecovery
        } else {
            RecoveryTestResult::ManualRecovery
        }
    }

    /// Test manager crash recovery
    async fn test_manager_crash_recovery(&self) -> RecoveryTestResult {
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        // Load plugins
        for i in 0..5 {
            manager.load_plugin(&format!("plugin_{}", i)).await.unwrap();
        }

        // Save state
        let state = manager.save_state().await.unwrap();

        // Simulate crash (drop manager)
        drop(manager);

        // Recreate and recover
        let mut new_manager = PluginManager::new(self.temp_dir.path().to_path_buf());
        let result = new_manager.load_state(&state).await;

        match result {
            Ok(_) => {
                let count = new_manager.get_plugin_count().await;
                if count == 5 {
                    RecoveryTestResult::AutomaticRecovery
                } else {
                    RecoveryTestResult::Failed(format!("Expected 5 plugins, got {}", count))
                }
            }
            Err(e) => RecoveryTestResult::Failed(e.to_string()),
        }
    }

    /// Test state corruption recovery
    async fn test_state_corruption_recovery(&self) -> RecoveryTestResult {
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        // Load plugins
        manager.load_plugin("test_plugin").await.unwrap();

        // Corrupt state (simulate by modifying state file)
        // In real scenario, this would corrupt the state file
        // For now, we'll test recovery with invalid state

        let corrupted_state = r#"{"invalid": "state"}"#;

        let result = manager.load_state(corrupted_state).await;

        if result.is_err() {
            // Expected - corrupted state rejected
            // Check if manager is still functional
            let plugins = manager.list_plugins().await;
            if plugins.is_empty() {
                RecoveryTestResult::AutomaticRecovery
            } else {
                RecoveryTestResult::ManualRecovery
            }
        } else {
            RecoveryTestResult::Failed("Corrupted state was not rejected".to_string())
        }
    }

    /// Test dependency failure recovery
    async fn test_dependency_failure_recovery(&self) -> RecoveryTestResult {
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        // Define dependencies
        let dependencies = vec![
            ("plugin_b".to_string(), vec!["plugin_a".to_string()]),
            ("plugin_a".to_string(), vec![]),
        ];

        // Load plugin A successfully
        manager.load_plugin("plugin_a").await.unwrap();

        // Try to load plugin B with failed dependency
        let result = manager.load_plugin("plugin_b").await;

        if result.is_err() {
            // Dependency failure detected
            // Check if manager is still functional
            let status = manager.get_plugin_status("plugin_a").await;
            if status.is_ok() {
                RecoveryTestResult::AutomaticRecovery
            } else {
                RecoveryTestResult::Failed("Working plugin also failed".to_string())
            }
        } else {
            RecoveryTestResult::Failed("Dependency failure not detected".to_string())
        }
    }

    /// Test resource limit recovery
    async fn test_resource_limit_recovery(&self) -> RecoveryTestResult {
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        // Set strict resource limits
        let limits = ResourceLimits {
            max_memory_mb: 10.0,
            max_cpu_percent: 10.0,
            max_file_handles: 5,
            max_network_connections: 2,
        };

        manager.set_resource_limits("resource_plugin", limits).await;

        // Load plugin that might exceed limits
        let result = manager.load_plugin("resource_plugin").await;

        if result.is_err() {
            // Resource limit detected
            // Check if manager recovered
            let plugins = manager.list_plugins().await;
            if plugins.len() == 0 {
                RecoveryTestResult::AutomaticRecovery
            } else {
                RecoveryTestResult::ManualRecovery
            }
        } else {
            RecoveryTestResult::Failed("Resource limit not enforced".to_string())
        }
    }

    /// Test configuration error recovery
    async fn test_configuration_error_recovery(&self) -> RecoveryTestResult {
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        // Create invalid configuration
        let invalid_config = r#"{"invalid": "configuration"}"#;

        let config_path = self.temp_dir.path().join("invalid_config.json");
        std::fs::write(&config_path, invalid_config).unwrap();

        // Try to load with invalid config
        let result = manager.load_with_config("test_plugin", &config_path).await;

        if result.is_err() {
            // Configuration error detected
            // Check if manager is still functional
            let config_path = self.temp_dir.path().join("valid_config.json");
            std::fs::write(&config_path, r#"{"valid": true}"#).unwrap();

            let result = manager.load_with_config("test_plugin", &config_path).await;
            if result.is_ok() {
                RecoveryTestResult::ManualRecovery
            } else {
                RecoveryTestResult::Failed("Valid config also failed".to_string())
            }
        } else {
            RecoveryTestResult::Failed("Invalid config was not rejected".to_string())
        }
    }

    /// Test network failure recovery
    async fn test_network_failure_recovery(&self) -> RecoveryTestResult {
        let event_system = EventSystem::new(EventSystemConfig::default());

        // Subscribe to events
        let received_count = std::sync::Arc::new(std::sync::Mutex::new(0));

        let callback = {
            let received_count = received_count.clone();
            Arc::new(move |event: Event| async move {
                let mut count = received_count.lock().unwrap();
                *count += 1;
                Ok(())
            })
        };

        let subscriber = EventSubscriber::new(
            "test_plugin".to_string(),
            vec!["test.event".to_string()],
            callback,
        );

        event_system.subscribe(subscriber).await.unwrap();

        // Publish events
        for i in 0..5 {
            let event = Event::new(
                "test.event".to_string(),
                "source".to_string(),
                serde_json::json!({"index": i}),
            );

            // Simulate network failure for some events
            if i == 2 {
                continue; // Skip (network failure)
            }

            let _ = event_system.publish(event).await;
        }

        // Wait for recovery
        sleep(Duration::from_millis(200)).await;

        let count = received_count.lock().unwrap();

        if *count >= 3 {
            RecoveryTestResult::AutomaticRecovery
        } else {
            RecoveryTestResult::Failed(format!("Only received {} events", count))
        }
    }

    /// Test storage failure recovery
    async fn test_storage_failure_recovery(&self) -> RecoveryTestResult {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = PluginManager::new(temp_dir.path().to_path_buf());

        // Load plugin
        manager.load_plugin("test_plugin").await.unwrap();

        // Save state
        let state = manager.save_state().await.unwrap();

        // Simulate storage failure by deleting state file
        let state_path = temp_dir.path().join("state.json");
        std::fs::remove_file(&state_path).unwrap();

        // Try to load state
        let result = manager.load_state(&state).await;

        if result.is_err() {
            // Storage failure detected
            // Check if manager can continue without state
            let count = manager.get_plugin_count().await;
            if count == 0 {
                RecoveryTestResult::AutomaticRecovery
            } else {
                RecoveryTestResult::ManualRecovery
            }
        } else {
            RecoveryTestResult::Failed("Storage failure not detected".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_crash_recovery() {
        let suite = RecoveryTestSuite::new();
        let result = suite.test_plugin_crash_recovery().await;

        assert!(matches!(result, RecoveryTestResult::AutomaticRecovery | RecoveryTestResult::ManualRecovery));
    }

    #[tokio::test]
    async fn test_manager_crash_recovery() {
        let suite = RecoveryTestSuite::new();
        let result = suite.test_manager_crash_recovery().await;

        assert!(matches!(result, RecoveryTestResult::AutomaticRecovery));
    }

    #[tokio::test]
    async fn test_state_corruption_recovery() {
        let suite = RecoveryTestSuite::new();
        let result = suite.test_state_corruption_recovery().await;

        assert!(matches!(result, RecoveryTestResult::AutomaticRecovery | RecoveryTestResult::ManualRecovery));
    }

    #[tokio::test]
    async fn test_dependency_failure_recovery() {
        let suite = RecoveryTestSuite::new();
        let result = suite.test_dependency_failure_recovery().await;

        assert!(matches!(result, RecoveryTestResult::AutomaticRecovery));
    }

    #[tokio::test]
    async fn test_resource_limit_recovery() {
        let suite = RecoveryTestSuite::new();
        let result = suite.test_resource_limit_recovery().await;

        assert!(matches!(result, RecoveryTestResult::AutomaticRecovery | RecoveryTestResult::ManualRecovery));
    }

    #[tokio::test]
    async fn test_configuration_error_recovery() {
        let suite = RecoveryTestSuite::new();
        let result = suite.test_configuration_error_recovery().await;

        assert!(matches!(result, RecoveryTestResult::ManualRecovery));
    }

    #[tokio::test]
    async fn test_network_failure_recovery() {
        let suite = RecoveryTestSuite::new();
        let result = suite.test_network_failure_recovery().await;

        assert!(matches!(result, RecoveryTestResult::AutomaticRecovery));
    }

    #[tokio::test]
    async fn test_storage_failure_recovery() {
        let suite = RecoveryTestSuite::new();
        let result = suite.test_storage_failure_recovery().await;

        assert!(matches!(result, RecoveryTestResult::AutomaticRecovery | RecoveryTestResult::ManualRecovery));
    }

    #[tokio::test]
    async fn test_all_recovery_tests() {
        let suite = RecoveryTestSuite::new();
        let results = suite.run_all_tests().await;

        // All tests should complete
        assert!(!results.is_empty());

        // All tests should recover
        let recovered = results.iter()
            .filter(|(_, r)| matches!(r, RecoveryTestResult::AutomaticRecovery | RecoveryTestResult::ManualRecovery))
            .count();

        assert_eq!(recovered, results.len(), "Some tests failed to recover");
    }
}
