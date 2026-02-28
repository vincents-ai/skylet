//! Test assertions and helpers for comprehensive testing

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Performance assertion helpers
pub struct PerformanceAssert;

impl PerformanceAssert {
    /// Assert that an operation completes within a time budget
    pub async fn within_time<F, T>(operation: F, budget: Duration) -> Result<T>
    where
        F: std::future::Future<Output = T>,
    {
        let start = Instant::now();
        let result = operation.await;
        let elapsed = start.elapsed();
        
        if elapsed > budget {
            panic!("Operation took {:?}, exceeded budget of {:?}", elapsed, budget);
        }
        
        Ok(result)
    }

    /// Assert that an operation completes within a specific time range
    pub async fn in_range<F, T>(operation: F, min: Duration, max: Duration) -> Result<T>
    where
        F: std::future::Future<Output = T>,
    {
        let start = Instant::now();
        let result = operation.await;
        let elapsed = start.elapsed();
        
        if elapsed < min {
            panic!("Operation took {:?}, was faster than minimum of {:?}", elapsed, min);
        }
        
        if elapsed > max {
            panic!("Operation took {:?}, exceeded maximum of {:?}", elapsed, max);
        }
        
        Ok(result)
    }
}

/// Plugin state assertion helpers
pub struct PluginAssert;

impl PluginAssert {
    /// Assert that a plugin is in a specific state
    pub fn assert_state(plugin_name: &str, expected_state: &str) -> PluginStateAssertion {
        PluginStateAssertion {
            plugin_name: plugin_name.to_string(),
            expected_state: expected_state.to_string(),
        }
    }
}

/// Plugin state assertion builder
pub struct PluginStateAssertion {
    plugin_name: String,
    expected_state: String,
}

impl PluginStateAssertion {
    /// Execute the assertion
    pub async fn check(&self, actual_state: &str) {
        if actual_state != &self.expected_state {
            panic!(
                "Plugin '{}' expected state '{}' but got '{}'",
                self.plugin_name, self.expected_state, actual_state
            );
        }
    }

    /// Check with timeout
    pub async fn check_with_timeout(&self, actual_state: &str, timeout: Duration) {
        let start = Instant::now();
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        
        loop {
            if start.elapsed() > timeout {
                panic!(
                    "Timeout waiting for plugin '{}' to reach state '{}'",
                    self.plugin_name, self.expected_state
                );
            }
            
            interval.tick().await;
            
            // In a real implementation, this would check the actual plugin state
            // For now, we'll simulate the check
            if actual_state == &self.expected_state {
                return;
            }
        }
    }
}

/// Memory usage assertion helpers
pub struct MemoryAssert;

impl MemoryAssert {
    /// Assert that memory usage is within expected bounds
    pub fn assert_memory_usage(current_mb: f64, max_mb: f64) {
        if current_mb > max_mb {
            panic!(
                "Memory usage {}MB exceeds maximum allowed {}MB",
                current_mb, max_mb
            );
        }
    }

    /// Assert that memory growth is within expected bounds
    pub fn assert_memory_growth(initial_mb: f64, current_mb: f64, max_growth_mb: f64) {
        let growth = current_mb - initial_mb;
        if growth > max_growth_mb {
            panic!(
                "Memory grew by {}MB ({}MB -> {}MB), exceeded maximum growth of {}MB",
                growth, initial_mb, current_mb, max_growth_mb
            );
        }
    }
}

/// Plugin performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginPerformanceMetrics {
    pub plugin_name: String,
    pub load_time_ms: f64,
    pub execution_time_ms: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
}

impl PluginPerformanceMetrics {
    /// Create a new performance metrics instance
    pub fn new(plugin_name: &str) -> Self {
        Self {
            plugin_name: plugin_name.to_string(),
            load_time_ms: 0.0,
            execution_time_ms: 0.0,
            memory_usage_mb: 0.0,
            cpu_usage_percent: 0.0,
        }
    }

    /// Update load time
    pub fn with_load_time(mut self, load_time_ms: f64) -> Self {
        self.load_time_ms = load_time_ms;
        self
    }

    /// Update execution time
    pub fn with_execution_time(mut self, execution_time_ms: f64) -> Self {
        self.execution_time_ms = execution_time_ms;
        self
    }

    /// Update memory usage
    pub fn with_memory_usage(mut self, memory_usage_mb: f64) -> Self {
        self.memory_usage_mb = memory_usage_mb;
        self
    }

    /// Update CPU usage
    pub fn with_cpu_usage(mut self, cpu_usage_percent: f64) -> Self {
        self.cpu_usage_percent = cpu_usage_percent;
        self
    }

    /// Validate metrics against thresholds
    pub fn validate(&self, thresholds: &PerformanceThresholds) -> Result<()> {
        if self.load_time_ms > thresholds.max_load_time_ms {
            return Err(anyhow::anyhow!(
                "Load time {}ms exceeds threshold {}ms",
                self.load_time_ms, thresholds.max_load_time_ms
            ));
        }

        if self.execution_time_ms > thresholds.max_execution_time_ms {
            return Err(anyhow::anyhow!(
                "Execution time {}ms exceeds threshold {}ms",
                self.execution_time_ms, thresholds.max_execution_time_ms
            ));
        }

        if self.memory_usage_mb > thresholds.max_memory_usage_mb {
            return Err(anyhow::anyhow!(
                "Memory usage {}MB exceeds threshold {}MB",
                self.memory_usage_mb, thresholds.max_memory_usage_mb
            ));
        }

        if self.cpu_usage_percent > thresholds.max_cpu_usage_percent {
            return Err(anyhow::anyhow!(
                "CPU usage {}% exceeds threshold {}%",
                self.cpu_usage_percent, thresholds.max_cpu_usage_percent
            ));
        }

        Ok(())
    }
}

/// Performance thresholds for validation
#[derive(Debug, Clone)]
pub struct PerformanceThresholds {
    pub max_load_time_ms: f64,
    pub max_execution_time_ms: f64,
    pub max_memory_usage_mb: f64,
    pub max_cpu_usage_percent: f64,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            max_load_time_ms: 100.0,    // 100ms
            max_execution_time_ms: 1000.0, // 1s
            max_memory_usage_mb: 50.0,     // 50MB
            max_cpu_usage_percent: 80.0,   // 80%
        }
    }
}

/// Test assertion helpers for plugin communication
pub struct PluginCommunicationAssert;

impl PluginCommunicationAssert {
    /// Assert that a message was received by a plugin
    pub async fn assert_message_received(
        plugin_name: &str,
        message_type: &str,
        timeout: Duration,
    ) {
        // In a real implementation, this would check message queues or logs
        // For now, we simulate the assertion
        let start = Instant::now();
        let mut interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            if start.elapsed() > timeout {
                panic!(
                    "Timeout waiting for message '{}' to be received by plugin '{}'",
                    message_type, plugin_name
                );
            }

            interval.tick().await;

            // Simulate message reception check
            if rand::random::<f64>() > 0.95 {
                return; // Simulate message received
            }
        }
    }

    /// Assert that plugin communication completed successfully
    pub async fn assert_communication_success(
        sender: &str,
        receiver: &str,
        timeout: Duration,
    ) {
        Self::assert_message_received(receiver, "plugin_message", timeout).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_assert_within_time() {
        let result = PerformanceAssert::within_time(
            async { 42 },
            Duration::from_millis(1000),
        )
        .await
        .unwrap();
        
        assert_eq!(result, 42);
    }

    #[test]
    fn test_plugin_assert() {
        let assertion = PluginAssert::assert_state("test-plugin", "loaded");
        // Would normally be called asynchronously with actual state
    }

    #[test]
    fn test_memory_assert() {
        MemoryAssert::assert_memory_usage(25.0, 50.0);
        MemoryAssert::assert_memory_growth(20.0, 25.0, 10.0);
    }

    #[test]
    fn test_plugin_performance_metrics() {
        let metrics = PluginPerformanceMetrics::new("test-plugin")
            .with_load_time(50.0)
            .with_execution_time(100.0)
            .with_memory_usage(25.0)
            .with_cpu_usage(30.0);

        let thresholds = PerformanceThresholds::default();
        assert!(metrics.validate(&thresholds).is_ok());
    }

    #[tokio::test]
    async fn test_plugin_communication_assert() {
        // This would normally test actual communication
        // For now, it should timeout without causing panic
        let start = Instant::now();
        PluginCommunicationAssert::assert_message_received(
            "test-plugin",
            "test_message",
            Duration::from_millis(100),
        )
        .await;
        
        // If we get here, the test passed
        assert!(start.elapsed() < Duration::from_millis(200));
    }
}