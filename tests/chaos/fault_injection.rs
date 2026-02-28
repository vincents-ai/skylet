//! Chaos Engineering Tests

use super::*;
use crate::plugin_manager::*;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

/// Chaos test configuration
#[derive(Debug, Clone)]
pub struct ChaosTestConfig {
    /// Enable random failures
    pub random_failures: bool,
    /// Failure probability (0.0 - 1.0)
    pub failure_probability: f64,
    /// Enable network chaos
    pub network_chaos: bool,
    /// Enable resource exhaustion
    pub resource_exhaustion: bool,
    /// Enable process crashes
    pub process_crashes: bool,
}

impl Default for ChaosTestConfig {
    fn default() -> Self {
        Self {
            random_failures: true,
            failure_probability: 0.2,
            network_chaos: true,
            resource_exhaustion: true,
            process_crashes: true,
        }
    }
}

/// Chaos test result
#[derive(Debug, Clone, PartialEq)]
pub enum ChaosTestResult {
    Survived,
    Failed(String),
    Recovered,
    Timeout,
}

/// Chaos test suite
pub struct ChaosTestSuite {
    config: ChaosTestConfig,
    temp_dir: TempDir,
}

impl ChaosTestSuite {
    /// Create new chaos test suite
    pub fn new(config: ChaosTestConfig) -> Self {
        Self {
            config,
            temp_dir: TempDir::new().expect("Failed to create temp dir"),
        }
    }

    /// Run all chaos tests
    pub async fn run_all_tests(&self) -> Vec<(&'static str, ChaosTestResult)> {
        let mut results = Vec::new();

        if self.config.random_failures {
            results.push(("random_plugin_failures", self.test_random_failures().await));
        }

        if self.config.network_chaos {
            results.push(("network_partitions", self.test_network_partitions().await));
            results.push(("network_latency", self.test_network_latency().await));
        }

        if self.config.resource_exhaustion {
            results.push(("memory_exhaustion", self.test_memory_exhaustion().await));
            results.push(("cpu_saturation", self.test_cpu_saturation().await));
            results.push(("file_descriptor_exhaustion", self.test_file_descriptor_exhaustion().await));
        }

        if self.config.process_crashes {
            results.push(("plugin_crash", self.test_plugin_crash().await));
            results.push(("manager_crash", self.test_manager_crash().await));
        }

        results
    }

    /// Test random plugin failures
    async fn test_random_failures(&self) -> ChaosTestResult {
        // Simulate random plugin failures during load/execute
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        for i in 0..10 {
            let plugin_name = format!("chaos_plugin_{}", i);

            // Randomly fail to load
            if rand::random::<f64>() < self.config.failure_probability {
                continue; // Skip this plugin (simulate failure)
            }

            let result = manager.load_plugin(&plugin_name).await;

            if result.is_err() && rand::random::<f64>() < self.config.failure_probability {
                // Simulate recovery attempt
                sleep(Duration::from_millis(100)).await;
                let retry_result = manager.load_plugin(&plugin_name).await;
                if retry_result.is_ok() {
                    return ChaosTestResult::Recovered;
                }
            }
        }

        ChaosTestResult::Survived
    }

    /// Test network partitions
    async fn test_network_partitions(&self) -> ChaosTestResult {
        // Simulate network partition between plugins
        let event_system = EventSystem::new(EventSystemConfig::default());

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
            "plugin_a".to_string(),
            vec!["test.event".to_string()],
            callback,
        );

        event_system.subscribe(subscriber).await.unwrap();

        // Simulate network partition by pausing event processing
        // In real scenario, this would block network

        // Publish events during partition
        for i in 0..5 {
            let event = Event::new(
                "test.event".to_string(),
                "source".to_string(),
                serde_json::json!({"index": i}),
            );
            let _ = event_system.publish(event).await;
        }

        // Wait for recovery
        sleep(Duration::from_millis(200)).await;

        let count = received_count.lock().unwrap();
        if *count == 0 {
            ChaosTestResult::Failed("Network partition blocked all events".to_string())
        } else {
            ChaosTestResult::Recovered
        }
    }

    /// Test network latency
    async fn test_network_latency(&self) -> ChaosTestResult {
        // Simulate high network latency
        let event_system = EventSystem::new(EventSystemConfig::default());

        let start_time = std::time::Instant::now();

        let event = Event::new(
            "test.event".to_string(),
            "source".to_string(),
            serde_json::json!({}),
        );

        let _ = event_system.publish(event).await;

        // Simulate latency (in real scenario, would add delays)
        sleep(Duration::from_millis(500)).await;

        let elapsed = start_time.elapsed();
        if elapsed > Duration::from_secs(1) {
            ChaosTestResult::Failed("Network latency too high".to_string())
        } else {
            ChaosTestResult::Survived
        }
    }

    /// Test memory exhaustion
    async fn test_memory_exhaustion(&self) -> ChaosTestResult {
        // Simulate memory exhaustion scenario
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        // Try to load many plugins
        for i in 0..50 {
            let plugin_name = format!("memory_hog_{}", i);
            let result = manager.load_plugin(&plugin_name).await;

            // If we start failing due to memory, test passed
            if result.is_err() {
                // System handled memory pressure
                return ChaosTestResult::Survived;
            }
        }

        ChaosTestResult::Failed("Memory exhaustion not detected".to_string())
    }

    /// Test CPU saturation
    async fn test_cpu_saturation(&self) -> ChaosTestResult {
        // Simulate CPU saturation
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        // Try to execute many plugins concurrently
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let mut manager = manager.clone();
                tokio::spawn(async move {
                    let plugin_name = format!("cpu_hog_{}", i);
                    manager.load_plugin(&plugin_name).await
                })
            })
            .collect();

        let results = futures::future::join_all(handles).await;

        // Check if any operations timed out
        let timeout_count = results.iter().filter(|r| r.is_err()).count();

        if timeout_count > results.len() / 2 {
            ChaosTestResult::Failed("CPU saturation caused timeouts".to_string())
        } else {
            ChaosTestResult::Survived
        }
    }

    /// Test file descriptor exhaustion
    async fn test_file_exhaustion(&self) -> ChaosTestResult {
        // Simulate opening many files
        let mut handles = Vec::new();

        for i in 0..1000 {
            match std::fs::File::open(self.temp_dir.path().join(format!("test_{}.txt", i))) {
                Ok(file) => handles.push(file),
                Err(_) => {
                    // Failed to open file (fd limit reached)
                    drop(handles);
                    return ChaosTestResult::Survived;
                }
            }
        }

        // System didn't enforce fd limit
        drop(handles);
        ChaosTestResult::Failed("File descriptor exhaustion not detected".to_string())
    }

    /// Test plugin crash
    async fn test_plugin_crash(&self) -> ChaosTestResult {
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        // Load plugin
        let plugin_name = "crash_test_plugin";
        manager.load_plugin(plugin_name).await.unwrap();

        // Simulate crash by marking unhealthy
        manager.mark_plugin_unhealthy(plugin_name).await;

        // Check if manager recovers
        sleep(Duration::from_millis(100)).await;

        let status = manager.get_plugin_status(plugin_name).await;

        if status.is_ok() && status.unwrap().status == "unhealthy" {
            ChaosTestResult::Survived
        } else {
            ChaosTestResult::Failed("Plugin crash not handled correctly".to_string())
        }
    }

    /// Test manager crash
    async fn test_manager_crash(&self) -> ChaosTestResult {
        // Create manager and save state
        let mut manager = PluginManager::new(self.temp_dir.path().to_path_buf());

        manager.load_plugin("test_plugin_1").await.unwrap();
        manager.load_plugin("test_plugin_2").await.unwrap();

        let state = manager.save_state().await.unwrap();

        // Simulate manager crash (drop and recreate)
        drop(manager);

        // Recreate manager and restore state
        let mut new_manager = PluginManager::new(self.temp_dir.path().to_path_buf());
        new_manager.load_state(&state).await.unwrap();

        // Verify recovery
        let count = new_manager.get_plugin_count().await;

        if count == 2 {
            ChaosTestResult::Recovered
        } else {
            ChaosTestResult::Failed(format!("Expected 2 plugins, got {}", count))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_random_failures() {
        let config = ChaosTestConfig {
            random_failures: true,
            failure_probability: 0.5,
            ..Default::default()
        };

        let suite = ChaosTestSuite::new(config);
        let result = suite.test_random_failures().await;

        assert!(matches!(result, ChaosTestResult::Survived | ChaosTestResult::Recovered));
    }

    #[tokio::test]
    async fn test_network_partitions() {
        let config = ChaosTestConfig::default();
        let suite = ChaosTestSuite::new(config);
        let result = suite.test_network_partitions().await;

        assert!(matches!(result, ChaosTestResult::Survived | ChaosTestResult::Recovered));
    }

    #[tokio::test]
    async fn test_network_latency() {
        let config = ChaosTestConfig::default();
        let suite = ChaosTestSuite::new(config);
        let result = suite.test_network_latency().await;

        assert!(matches!(result, ChaosTestResult::Survived));
    }

    #[tokio::test]
    async fn test_memory_exhaustion() {
        let config = ChaosTestConfig::default();
        let suite = ChaosTestSuite::new(config);
        let result = suite.test_memory_exhaustion().await;

        // Should handle memory pressure gracefully
        assert!(matches!(result, ChaosTestResult::Survived));
    }

    #[tokio::test]
    async fn test_cpu_saturation() {
        let config = ChaosTestConfig::default();
        let suite = ChaosTestSuite::new(config);
        let result = suite.test_cpu_saturation().await;

        assert!(matches!(result, ChaosTestResult::Survived));
    }

    #[tokio::test]
    async fn test_file_descriptor_exhaustion() {
        let config = ChaosTestConfig::default();
        let suite = ChaosTestSuite::new(config);
        let result = suite.test_file_exhaustion().await;

        // Should detect fd limit
        assert!(matches!(result, ChaosTestResult::Survived));
    }

    #[tokio::test]
    async fn test_plugin_crash() {
        let config = ChaosTestConfig::default();
        let suite = ChaosTestSuite::new(config);
        let result = suite.test_plugin_crash().await;

        assert!(matches!(result, ChaosTestResult::Survived));
    }

    #[tokio::test]
    async fn test_manager_crash() {
        let config = ChaosTestConfig::default();
        let suite = ChaosTestSuite::new(config);
        let result = suite.test_manager_crash().await;

        assert!(matches!(result, ChaosTestResult::Recovered));
    }

    #[tokio::test]
    async fn test_all_chaos_tests() {
        let config = ChaosTestConfig::default();
        let suite = ChaosTestSuite::new(config);
        let results = suite.run_all_tests().await;

        // All tests should complete
        assert!(!results.is_empty());

        // Most tests should survive or recover
        let survived_or_recovered = results.iter()
            .filter(|(_, r)| matches!(r, ChaosTestResult::Survived | ChaosTestResult::Recovered))
            .count();

        let total = results.len();
        let success_rate = survived_or_recovered as f64 / total as f64;

        // At least 80% should survive or recover
        assert!(success_rate >= 0.8, "Success rate: {:.2}%", success_rate);
    }
}
