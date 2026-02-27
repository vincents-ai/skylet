//! End-to-End Tests - Full Lifecycle Validation

use super::*;
use crate::plugin_manager::*;
use crate::plugin_manager::events::*;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

/// E2E test scenario
#[derive(Debug, Clone)]
pub enum E2EScenario {
    BasicLifecycle,
    HotReload,
    Communication,
    ErrorHandling,
    Performance,
}

/// E2E test result
#[derive(Debug, Clone, PartialEq)]
pub enum E2ETestResult {
    Passed,
    Failed(String),
    Skipped(String),
}

/// E2E test suite
pub struct E2ETestSuite {
    temp_dir: TempDir,
    scenario: E2EScenario,
}

impl E2ETestSuite {
    /// Create new E2E test suite
    pub fn new(scenario: E2EScenario) -> Self {
        Self {
            temp_dir: TempDir::new().expect("Failed to create temp dir"),
            scenario,
        }
    }

    /// Run basic lifecycle test
    pub async fn test_basic_lifecycle(&self) -> E2ETestResult {
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        // Step 1: Load plugin
        let result = manager.load_plugin("test_plugin").await;
        if result.is_err() {
            return E2ETestResult::Failed("Failed to load plugin".to_string());
        }

        // Step 2: Verify plugin is loaded
        if !manager.is_plugin_loaded("test_plugin").await {
            return E2ETestResult::Failed("Plugin not marked as loaded".to_string());
        }

        // Step 3: Execute plugin
        let result = manager.execute_plugin("test_plugin", "test_operation").await;
        if result.is_err() {
            return E2ETestResult::Failed("Failed to execute plugin".to_string());
        }

        // Step 4: Verify plugin status
        let status = manager.get_plugin_status("test_plugin").await;
        if status.is_err() {
            return E2ETestResult::Failed("Failed to get plugin status".to_string());
        }

        let status = status.unwrap();
        if status.status != "loaded" && status.status != "executed" {
            return E2ETestResult::Failed(format!("Unexpected status: {}", status.status));
        }

        // Step 5: Reload plugin
        let result = manager.reload_plugin("test_plugin").await;
        if result.is_err() {
            return E2ETestResult::Failed("Failed to reload plugin".to_string());
        }

        // Step 6: Unload plugin
        let result = manager.unload_plugin("test_plugin").await;
        if result.is_err() {
            return E2ETestResult::Failed("Failed to unload plugin".to_string());
        }

        // Step 7: Verify plugin is unloaded
        if manager.is_plugin_loaded("test_plugin").await {
            return E2ETestResult::Failed("Plugin not marked as unloaded".to_string());
        }

        E2ETestResult::Passed
    }

    /// Run hot reload test
    pub async fn test_hot_reload(&self) -> E2ETestResult {
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        // Step 1: Enable hot reload
        manager.enable_hot_reload().await;

        // Step 2: Load plugin
        manager.load_plugin("hot_reload_plugin").await.unwrap();

        // Step 3: Get initial status
        let initial_status = manager.get_plugin_status("hot_reload_plugin").await.unwrap();

        // Step 4: Modify plugin file (simulate update)
        let plugin_path = self.temp_dir.path().join("hot_reload_plugin.so");
        std::fs::write(&plugin_path, b"updated_plugin_binary").unwrap();

        // Step 5: Wait for hot reload
        sleep(Duration::from_millis(500)).await;

        // Step 6: Get reloaded status
        let reloaded_status = manager.get_plugin_status("hot_reload_plugin").await.unwrap();

        // Step 7: Verify hot reload happened
        if reloaded_status.loaded_at == initial_status.loaded_at {
            return E2ETestResult::Failed("Plugin was not hot-reloaded".to_string());
        }

        // Step 8: Verify plugin still works
        let result = manager.execute_plugin("hot_reload_plugin", "test").await;
        if result.is_err() {
            return E2ETestResult::Failed("Plugin failed after hot-reload".to_string());
        }

        E2ETestResult::Passed
    }

    /// Run communication test
    pub async fn test_communication(&self) -> E2ETestResult {
        let event_system = Arc::new(EventSystem::new(EventSystemConfig::default()));

        // Step 1: Create publisher
        let publisher = EventBus::new(
            event_system.clone(),
            "publisher_plugin".to_string(),
        );

        // Step 2: Create subscriber
        let received_messages = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let callback = {
            let received = received_messages.clone();
            Arc::new(move |event: Event| async move {
                let mut messages = received.lock().unwrap();
                messages.push(event);
                Ok(())
            })
        };

        let subscriber = EventBus::new(
            event_system.clone(),
            "subscriber_plugin".to_string(),
        );

        subscriber
            .subscribe(vec!["test.event".to_string()], callback)
            .await
            .unwrap();

        // Step 3: Publish messages
        for i in 0..3 {
            publisher
                .publish(
                    "test.event".to_string(),
                    serde_json::json!({"index": i}),
                )
                .await
                .unwrap();
        }

        // Step 4: Wait for messages to be received
        sleep(Duration::from_millis(200)).await;

        // Step 5: Verify all messages received
        let messages = received_messages.lock().unwrap();
        if messages.len() != 3 {
            return E2ETestResult::Failed(format!("Expected 3 messages, got {}", messages.len()));
        }

        // Step 6: Verify message content
        for (i, msg) in messages.iter().enumerate() {
            if msg.source != "publisher_plugin" {
                return E2ETestResult::Failed(format!("Message {} has wrong source", i));
            }

            if msg.event_type != "test.event" {
                return E2ETestResult::Failed(format!("Message {} has wrong type", i));
            }
        }

        E2ETestResult::Passed
    }

    /// Run error handling test
    pub async fn test_error_handling(&self) -> E2ETestResult {
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        // Step 1: Try to load non-existent plugin
        let result = manager.load_plugin("nonexistent_plugin").await;
        if result.is_ok() {
            return E2ETestResult::Failed("Non-existent plugin loaded successfully".to_string());
        }

        // Step 2: Try to get status of non-existent plugin
        let result = manager.get_plugin_status("nonexistent_plugin").await;
        if result.is_ok() {
            return E2ETestResult::Failed("Got status for non-existent plugin".to_string());
        }

        // Step 3: Load a plugin
        manager.load_plugin("error_test_plugin").await.unwrap();

        // Step 4: Try to execute with invalid input
        let result = manager.execute_plugin("error_test_plugin", "invalid_operation").await;
        if result.is_ok() {
            return E2ETestResult::Failed("Invalid operation succeeded".to_string());
        }

        // Step 5: Verify manager is still functional
        let result = manager.list_plugins().await;
        if result.is_err() {
            return E2ETestResult::Failed("Manager not functional after error".to_string());
        }

        // Step 6: Unload plugin
        manager.unload_plugin("error_test_plugin").await.unwrap();

        // Step 7: Try to unload non-existent plugin
        let result = manager.unload_plugin("nonexistent_plugin").await;
        if result.is_ok() {
            return E2ETestResult::Failed("Non-existent plugin unloaded successfully".to_string());
        }

        E2ETestResult::Passed
    }

    /// Run performance test
    pub async fn test_performance(&self) -> E2ETestResult {
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        // Step 1: Load multiple plugins
        let plugin_names: Vec<String> = (0..10)
            .map(|i| format!("perf_plugin_{}", i))
            .collect();

        for name in &plugin_names {
            let _ = manager.load_plugin(name).await;
        }

        // Step 2: Measure load time
        let load_start = std::time::Instant::now();
        for name in &plugin_names {
            let _ = manager.load_plugin(name).await;
        }
        let load_time = load_start.elapsed();

        // Load should be fast
        if load_time > Duration::from_secs(5) {
            return E2ETestResult::Failed(format!("Loading too slow: {:?}", load_time));
        }

        // Step 3: Execute all plugins
        for name in &plugin_names {
            let _ = manager.execute_plugin(name, "test").await;
        }

        // Step 4: Measure execution time
        let exec_start = std::time::Instant::now();
        for name in &plugin_names {
            let _ = manager.execute_plugin(name, "test").await;
        }
        let exec_time = exec_start.elapsed();

        // Execution should be fast
        if exec_time > Duration::from_secs(2) {
            return E2ETestResult::Failed(format!("Execution too slow: {:?}", exec_time));
        }

        // Step 5: Check resource usage
        for name in &plugin_names {
            let resources = manager.get_plugin_resources(name).await;

            if let Ok(resources) = resources {
                // Memory should be reasonable
                if resources.memory_usage_mb > 100.0 {
                    return E2ETestResult::Failed(format!("{} uses too much memory: {} MB", name, resources.memory_usage_mb));
                }

                // CPU should be reasonable
                if resources.cpu_usage_percent > 80.0 {
                    return E2ETestResult::Failed(format!("{} uses too much CPU: {}%", name, resources.cpu_usage_percent));
                }
            }
        }

        E2ETestResult::Passed
    }

    /// Run all E2E tests
    pub async fn run_all_tests(&self) -> Vec<(&'static str, E2ETestResult)> {
        let mut results = Vec::new();

        results.push(("basic_lifecycle", self.test_basic_lifecycle().await));
        results.push(("hot_reload", self.test_hot_reload().await));
        results.push(("communication", self.test_communication().await));
        results.push(("error_handling", self.test_error_handling().await));
        results.push(("performance", self.test_performance().await));

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_e2e_basic_lifecycle() {
        let suite = E2ETestSuite::new(E2EScenario::BasicLifecycle);
        let result = suite.test_basic_lifecycle().await;

        assert_eq!(result, E2ETestResult::Passed);
    }

    #[tokio::test]
    async fn test_e2e_hot_reload() {
        let suite = E2ETestSuite::new(E2EScenario::HotReload);
        let result = suite.test_hot_reload().await;

        assert_eq!(result, E2ETestResult::Passed);
    }

    #[tokio::test]
    async fn test_e2e_communication() {
        let suite = E2ETestSuite::new(E2EScenario::Communication);
        let result = suite.test_communication().await;

        assert_eq!(result, E2ETestResult::Passed);
    }

    #[tokio::test]
    async fn test_e2e_error_handling() {
        let suite = E2ETestSuite::new(E2EScenario::ErrorHandling);
        let result = suite.test_error_handling().await;

        assert_eq!(result, E2ETestResult::Passed);
    }

    #[tokio::test]
    async fn test_e2e_performance() {
        let suite = E2ETestSuite::new(E2EScenario::Performance);
        let result = suite.test_performance().await;

        assert_eq!(result, E2ETestResult::Passed);
    }

    #[tokio::test]
    async fn test_all_e2e_tests() {
        let suite = E2ETestSuite::new(E2EScenario::BasicLifecycle);
        let results = suite.run_all_tests().await;

        // All tests should pass
        let passed = results.iter()
            .filter(|(_, r)| matches!(r, E2ETestResult::Passed))
            .count();

        assert_eq!(passed, results.len(), "Some E2E tests failed");

        // Print results summary
        println!("\nE2E Test Results:");
        for (name, result) in &results {
            println!("  {}: {:?}", name, result);
        }
    }
}
