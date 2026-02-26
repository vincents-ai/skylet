//! BDD Test World for Plugin Testing
//!
//! Provides cucumber-rs integration for behavior-driven plugin testing.
//! This module implements the World trait from cucumber-rs and provides
//! all necessary state management for BDD scenarios.

use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use cucumber::World;
use tempfile::TempDir;

use skylet_abi::*;

use crate::{LoadedPluginV2, MockPluginContextV2, PluginTestConfig, TestResult, TestStatus};

/// Log entry captured during test execution
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub structured_data: Option<String>,
}

/// BDD test world for plugin testing
///
/// This is the main state container for cucumber scenarios.
/// It holds the loaded plugin, mock context, and test state.
#[derive(World)]
#[world(init = Self::new)]
pub struct PluginTestWorld {
    /// Plugin test configuration
    #[world(skip)]
    pub config: PluginTestConfig,

    /// Loaded V2 plugin
    #[world(skip)]
    pub plugin: Option<Arc<LoadedPluginV2>>,

    /// Mock V2 context
    #[world(skip)]
    pub context: Option<Box<MockPluginContextV2>>,

    /// Current plugin path
    pub plugin_path: Option<String>,

    /// Test results collected during scenario
    #[world(skip)]
    pub results: Vec<TestResult>,

    /// Last action response (JSON string)
    pub last_response: Option<String>,

    /// Last error message
    pub last_error: Option<String>,

    /// Custom test data for scenario state
    pub test_data: HashMap<String, String>,

    /// Temporary directory for test files
    #[world(skip)]
    pub temp_dir: Option<TempDir>,

    /// Log entries captured during execution
    #[world(skip)]
    pub log_entries: Arc<Mutex<Vec<LogEntry>>>,

    /// Configuration values set during test
    pub config_values: HashMap<String, String>,

    /// Registered mock services
    #[world(skip)]
    pub mock_services: HashMap<String, Box<dyn MockServiceHandler>>,
}

impl std::fmt::Debug for PluginTestWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginTestWorld")
            .field("plugin_path", &self.plugin_path)
            .field("last_response", &self.last_response)
            .field("last_error", &self.last_error)
            .field("test_data", &self.test_data)
            .field("config_values", &self.config_values)
            .finish()
    }
}

impl Default for PluginTestWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginTestWorld {
    /// Create a new test world
    pub fn new() -> Self {
        Self {
            config: PluginTestConfig::default(),
            plugin: None,
            context: None,
            plugin_path: None,
            results: Vec::new(),
            last_response: None,
            last_error: None,
            test_data: HashMap::new(),
            temp_dir: TempDir::new().ok(),
            log_entries: Arc::new(Mutex::new(Vec::new())),
            config_values: HashMap::new(),
            mock_services: HashMap::new(),
        }
    }

    /// Set up the test world with a plugin path
    pub fn setup(&mut self, plugin_path: &str) -> Result<()> {
        self.config = PluginTestConfig {
            plugin_path: plugin_path.to_string(),
            dependencies: Vec::new(),
            mock_services: HashMap::new(),
            test_timeout_ms: 5000,
            enable_logging: true,
        };
        self.plugin_path = Some(plugin_path.to_string());
        Ok(())
    }

    /// Load the plugin from the configured path
    pub async fn load_plugin(&mut self) -> Result<()> {
        let plugin_path = self
            .plugin_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Plugin path not set"))?;

        let path = PathBuf::from(plugin_path);

        // Create mock V2 context
        let mock_ctx = Box::new(MockPluginContextV2::new());
        self.context = Some(mock_ctx);

        // Load plugin
        let plugin = LoadedPluginV2::load(&path)?;

        // Initialize plugin
        let ctx_ptr = self.context.as_ref().unwrap().as_context_ptr();
        let result = plugin.init(ctx_ptr)?;
        if result != PluginResultV2::Success {
            return Err(anyhow::anyhow!(
                "Plugin initialization failed: {:?}",
                result
            ));
        }

        self.plugin = Some(Arc::new(plugin));
        self.add_result(TestResult::passed(
            "Plugin Loading".to_string(),
            std::time::Duration::from_millis(10),
        ));

        Ok(())
    }

    /// Execute an action on the loaded plugin
    pub fn execute_action(&mut self, action: &str, args_json: &str) -> Result<()> {
        let plugin = self
            .plugin
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Plugin not loaded"))?;

        let ctx_ptr = self
            .context
            .as_ref()
            .map(|c| c.as_context_ptr())
            .unwrap_or(std::ptr::null());

        let action_cstr = CString::new(action)?;
        let args_cstr = CString::new(args_json)?;

        let result_ptr = plugin.execute(ctx_ptr, action_cstr.as_ptr(), args_cstr.as_ptr())?;

        if result_ptr.is_null() {
            self.last_error = Some("Plugin returned null response".to_string());
            self.last_response = None;
            return Err(anyhow::anyhow!("Plugin returned null response"));
        }

        let result_str = unsafe { CStr::from_ptr(result_ptr) }
            .to_str()?
            .to_string();

        // Free the result string
        unsafe {
            let _ = CString::from_raw(result_ptr as *mut c_char);
        }

        self.last_response = Some(result_str);
        self.last_error = None;

        Ok(())
    }

    /// Shutdown the plugin
    pub fn shutdown_plugin(&mut self) -> Result<()> {
        if let Some(plugin) = &self.plugin {
            let ctx_ptr = self
                .context
                .as_ref()
                .map(|c| c.as_context_ptr())
                .unwrap_or(std::ptr::null());

            let result = plugin.shutdown(ctx_ptr)?;
            if result != PluginResultV2::Success {
                return Err(anyhow::anyhow!("Plugin shutdown failed: {:?}", result));
            }
        }
        self.plugin = None;
        Ok(())
    }

    /// Get plugin info
    pub fn get_plugin_info(&self) -> Result<String> {
        let plugin = self
            .plugin
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Plugin not loaded"))?;

        let info = plugin.get_info()?;

        // Extract info from PluginInfoV2
        let name = if !info.name.is_null() {
            unsafe { CStr::from_ptr(info.name) }
                .to_str()
                .unwrap_or("unknown")
        } else {
            "unknown"
        };

        let version = if !info.version.is_null() {
            unsafe { CStr::from_ptr(info.version) }
                .to_str()
                .unwrap_or("0.0.0")
        } else {
            "0.0.0"
        };

        let abi_version = if !info.abi_version.is_null() {
            unsafe { CStr::from_ptr(info.abi_version) }
                .to_str()
                .unwrap_or("2.0")
        } else {
            "2.0"
        };

        Ok(format!(
            r#"{{"name": "{}", "version": "{}", "abi_version": "{}"}}"#,
            name, version, abi_version
        ))
    }

    /// Check if last response contains expected text
    pub fn response_contains(&self, expected: &str) -> bool {
        self.last_response
            .as_ref()
            .map(|r| r.contains(expected))
            .unwrap_or(false)
    }

    /// Check if there was an error
    pub fn has_error(&self) -> bool {
        self.last_error.is_some()
    }

    /// Get the last response as JSON value
    pub fn response_as_json(&self) -> Result<serde_json::Value> {
        let response = self
            .last_response
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No response available"))?;

        Ok(serde_json::from_str(response)?)
    }

    /// Add a test result
    pub fn add_result(&mut self, result: TestResult) {
        self.results.push(result);
    }

    /// Get all test results
    pub fn get_results(&self) -> &[TestResult] {
        &self.results
    }

    /// Get passed/failed counts
    pub fn get_result_summary(&self) -> (usize, usize) {
        let passed = self
            .results
            .iter()
            .filter(|r| r.status == TestStatus::Passed)
            .count();
        let failed = self
            .results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .count();
        (passed, failed)
    }

    /// Set a test data value
    pub fn set_data(&mut self, key: &str, value: &str) {
        self.test_data.insert(key.to_string(), value.to_string());
    }

    /// Get a test data value
    pub fn get_data(&self, key: &str) -> Option<&String> {
        self.test_data.get(key)
    }

    /// Set a config value
    pub fn set_config(&mut self, key: &str, value: &str) {
        self.config_values.insert(key.to_string(), value.to_string());
    }

    /// Get the temp directory path
    pub fn temp_path(&self) -> Option<PathBuf> {
        self.temp_dir.as_ref().map(|d| d.path().to_path_buf())
    }

    /// Create a test file in temp directory
    pub fn create_test_file(&self, name: &str, content: &str) -> Result<PathBuf> {
        let temp_dir = self
            .temp_dir
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No temp directory"))?;

        let path = temp_dir.path().join(name);
        std::fs::write(&path, content)?;
        Ok(path)
    }

    /// Get log entries
    pub fn get_logs(&self) -> Vec<LogEntry> {
        self.log_entries.lock().unwrap().clone()
    }

    /// Check if logs contain a message at a specific level
    pub fn logs_contain(&self, level: &str, message_pattern: &str) -> bool {
        self.log_entries.lock().unwrap().iter().any(|entry| {
            entry.level == level && entry.message.contains(message_pattern)
        })
    }

    /// Register a mock service handler
    pub fn register_mock_service<S: MockServiceHandler + 'static>(&mut self, name: &str, service: S) {
        self.mock_services.insert(name.to_string(), Box::new(service));
    }

    /// Cleanup test state
    pub fn cleanup(&mut self) {
        self.last_response = None;
        self.last_error = None;
        self.test_data.clear();
        self.results.clear();
        self.log_entries.lock().unwrap().clear();
    }
}

impl Drop for PluginTestWorld {
    fn drop(&mut self) {
        // Attempt graceful shutdown
        let _ = self.shutdown_plugin();
    }
}

/// Mock service handler trait
pub trait MockServiceHandler: Send + Sync {
    /// Handle a request to this mock service
    fn handle(&self, method: &str, args: &str) -> Result<String>;
}

/// Simple mock service that returns configured responses
pub struct SimpleMockService {
    responses: HashMap<String, String>,
}

impl SimpleMockService {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    pub fn add_response(&mut self, method: &str, response: &str) {
        self.responses.insert(method.to_string(), response.to_string());
    }
}

impl Default for SimpleMockService {
    fn default() -> Self {
        Self::new()
    }
}

impl MockServiceHandler for SimpleMockService {
    fn handle(&self, method: &str, _args: &str) -> Result<String> {
        self.responses
            .get(method)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No mock response for method: {}", method))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_creation() {
        let world = PluginTestWorld::new();
        assert!(world.plugin.is_none());
        assert!(world.plugin_path.is_none());
        assert!(world.results.is_empty());
    }

    #[test]
    fn test_world_setup() {
        let mut world = PluginTestWorld::new();
        world.setup("/tmp/test-plugin.so").unwrap();
        assert_eq!(world.plugin_path, Some("/tmp/test-plugin.so".to_string()));
    }

    #[test]
    fn test_data_storage() {
        let mut world = PluginTestWorld::new();
        world.set_data("user_id", "12345");
        assert_eq!(world.get_data("user_id"), Some(&"12345".to_string()));
    }

    #[test]
    fn test_config_storage() {
        let mut world = PluginTestWorld::new();
        world.set_config("database.path", "/tmp/test.db");
        assert_eq!(
            world.config_values.get("database.path"),
            Some(&"/tmp/test.db".to_string())
        );
    }

    #[test]
    fn test_response_contains() {
        let mut world = PluginTestWorld::new();
        world.last_response = Some(r#"{"status": "ok", "data": "test"}"#.to_string());
        assert!(world.response_contains("ok"));
        assert!(world.response_contains("data"));
        assert!(!world.response_contains("error"));
    }

    #[test]
    fn test_simple_mock_service() {
        let mut service = SimpleMockService::new();
        service.add_response("get_user", r#"{"id": "123", "name": "Test"}"#);

        let response = service.handle("get_user", "{}").unwrap();
        assert!(response.contains("123"));
    }
}
