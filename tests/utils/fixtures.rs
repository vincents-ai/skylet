//! Test fixtures and data for comprehensive testing

use std::path::PathBuf;

/// Test plugin fixtures for testing plugin loading and execution
pub mod plugin_fixtures {
    use super::*;
    use tempfile::TempDir;

    /// Create a test plugin fixture directory
    pub fn create_test_plugin(name: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join(name);
        std::fs::create_dir_all(&plugin_dir).unwrap();

        // Create Cargo.toml
        let cargo_toml = format!(
            r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-abi = {{ path = "../../../abi" }}
tokio = {{ version = "1.0", features = ["full"] }}
"#,
            name
        );

        std::fs::write(plugin_dir.join("Cargo.toml"), cargo_toml).unwrap();
        std::fs::create_dir_all(plugin_dir.join("src")).unwrap();

        plugin_dir
    }

    /// Create a test plugin source file
    pub fn create_test_plugin_source(dir: &PathBuf, name: &str) {
        let lib_rs = format!(
            r#"
use skylet_abi::prelude::*;

#[skylet_plugin]
pub struct {}Plugin;

#[skylet_plugin_impl]
impl Plugin for {}Plugin {{
    fn name(&self) -> &'static str {{
        "{}"
    }}

    fn version(&self) -> &'static str {{
        "0.1.0"
    }}

    fn init(&mut self, _ctx: &mut PluginInitContext) -> Result<(), PluginError> {{
        Ok(())
    }}

    fn execute(&self, _ctx: &PluginContext) -> Result<PluginResult, PluginError> {{
        Ok(PluginResult::Success("test result".into()))
    }}

    fn cleanup(&mut self, _ctx: &PluginContext) -> Result<(), PluginError> {{
        Ok(())
    }}
}}
"#,
            name, name, name
        );

        let src_dir = dir.join("src");
        std::fs::write(src_dir.join("lib.rs"), lib_rs).unwrap();
    }

    /// Create multiple test plugins
    pub fn create_test_plugins(count: usize) -> Vec<TempDir> {
        (0..count)
            .map(|i| create_test_plugin(&format!("test-plugin-{}", i)))
            .collect()
    }

    /// Create a failing test plugin
    pub fn create_failing_test_plugin() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("failing-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let cargo_toml = r#"[package]
name = "failing-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-abi = { path = "../../../abi" }
tokio = { version = "1.0", features = ["full"] }
"#;

        std::fs::write(plugin_dir.join("Cargo.toml"), cargo_toml).unwrap();
        std::fs::create_dir_all(plugin_dir.join("src")).unwrap();

        let lib_rs = r#"
use skylet_abi::prelude::*;

#[skylet_plugin]
pub struct FailingPlugin;

#[skylet_plugin_impl]
impl Plugin for FailingPlugin {
    fn name(&self) -> &'static str {
        "failing-plugin"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn init(&mut self, _ctx: &mut PluginInitContext) -> Result<(), PluginError> {
        Err(PluginError::InitializationFailed("Plugin intentionally failed to initialize".to_string()))
    }

    fn execute(&self, _ctx: &PluginContext) -> Result<PluginResult, PluginError> {
        Ok(PluginResult::Success("test result".into()))
    }

    fn cleanup(&mut self, _ctx: &PluginContext) -> Result<(), PluginError> {
        Ok(())
    }
}
"#;

        std::fs::write(plugin_dir.join("src").join("lib.rs"), lib_rs).unwrap();
        temp_dir
    }

    /// Create a test plugin that panics
    pub fn create_panicking_test_plugin() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("panicking-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let cargo_toml = r#"[package]
name = "panicking-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-abi = { path = "../../../abi" }
tokio = { version = "1.0", features = ["full"] }
"#;

        std::fs::write(plugin_dir.join("Cargo.toml"), cargo_toml).unwrap();
        std::fs::create_dir_all(plugin_dir.join("src")).unwrap();

        let lib_rs = r#"
use skylet_abi::prelude::*;

#[skylet_plugin]
pub struct PanickingPlugin;

#[skylet_plugin_impl]
impl Plugin for PanickingPlugin {
    fn name(&self) -> &'static str {
        "panicking-plugin"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn init(&mut self, _ctx: &mut PluginInitContext) -> Result<(), PluginError> {
        Ok(())
    }

    fn execute(&self, _ctx: &PluginContext) -> Result<PluginResult, PluginError> {
        panic!("Intentional panic during execution");
    }

    fn cleanup(&mut self, _ctx: &PluginContext) -> Result<(), PluginError> {
        Ok(())
    }
}
"#;

        std::fs::write(plugin_dir.join("src").join("lib.rs"), lib_rs).unwrap();
        temp_dir
    }
}

/// Configuration fixtures for testing
pub mod config_fixtures {
    use super::*;
    use std::fs;

    /// Create a test configuration file
    pub fn create_test_config(config_content: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        fs::write(&config_path, config_content).unwrap();
        temp_dir
    }

    /// Create a minimal test configuration
    pub fn minimal_test_config() -> TempDir {
        let config = r#"
[plugins]
directory = "./plugins"
exclude_patterns = ["test_*"]
include_patterns = []
probe_abi_version = true
include_debug_builds = true

[database]
path = "./data/test.db"

[server]
port = 8080
host = "0.0.0.0"

[log]
level = "info"
format = "json"
"#;
        create_test_config(config)
    }

    /// Create a test configuration with invalid settings
    pub fn invalid_test_config() -> TempDir {
        let config = r#"
[plugins]
directory = "/invalid/path"
exclude_patterns = ["test_*"]
include_patterns = []
probe_abi_version = true
include_debug_builds = true

[database]
path = ""

[server]
port = "invalid_port"
host = "invalid_host"

[log]
level = "invalid_level"
format = "invalid_format"
"#;
        create_test_config(config)
    }

    /// Create a test configuration with security settings
    pub fn security_test_config() -> TempDir {
        let config = r#"
[plugins]
directory = "./plugins"
exclude_patterns = ["test_*"]
include_patterns = []
probe_abi_version = true
include_debug_builds = true

[security]
plugin_isolation = true
memory_limit_mb = 50
cpu_limit_percent = 80
network_access = false
file_system_access = false

[database]
path = "./data/test.db"

[server]
port = 8080
host = "0.0.0.0"

[log]
level = "info"
format = "json"
"#;
        create_test_config(config)
    }
}

/// Performance test fixtures
pub mod performance_fixtures {
    use super::*;

    /// Generate test data for performance benchmarks
    pub fn generate_test_plugin_names(count: usize) -> Vec<String> {
        (0..count)
            .map(|i| format!("performance-test-plugin-{}", i))
            .collect()
    }

    /// Generate test plugin dependencies
    pub fn generate_test_dependencies(count: usize) -> Vec<(String, Vec<String>)> {
        let mut dependencies = Vec::new();
        for i in 0..count {
            let deps: Vec<String> = (0..i)
                .map(|j| format!("performance-test-plugin-{}", j))
                .collect();
            dependencies.push((format!("performance-test-plugin-{}", i), deps));
        }
        dependencies
    }

    /// Create test scenarios for stress testing
    pub fn stress_test_scenarios() -> Vec<StressTestScenario> {
        vec![
            StressTestScenario {
                name: "single_plugin".to_string(),
                plugin_count: 1,
                concurrent_requests: 10,
                duration_seconds: 30,
            },
            StressTestScenario {
                name: "multiple_plugins".to_string(),
                plugin_count: 5,
                concurrent_requests: 50,
                duration_seconds: 60,
            },
            StressTestScenario {
                name: "high_load".to_string(),
                plugin_count: 10,
                concurrent_requests: 100,
                duration_seconds: 120,
            },
            StressTestScenario {
                name: "extreme_load".to_string(),
                plugin_count: 20,
                concurrent_requests: 200,
                duration_seconds: 180,
            },
        ]
    }
}

/// Stress test scenario definition
#[derive(Debug, Clone)]
pub struct StressTestScenario {
    pub name: String,
    pub plugin_count: usize,
    pub concurrent_requests: usize,
    pub duration_seconds: u64,
}

/// Security test fixtures
pub mod security_fixtures {
    use super::*;

    /// Create test security policies
    pub fn security_test_policies() -> Vec<SecurityTestPolicy> {
        vec![
            SecurityTestPolicy {
                name: "read_only".to_string(),
                allow_file_read: true,
                allow_file_write: false,
                allow_network: false,
                allow_memory_allocation: true,
                max_memory_mb: 10,
            },
            SecurityTestPolicy {
                name: "network_only".to_string(),
                allow_file_read: false,
                allow_file_write: false,
                allow_network: true,
                allow_memory_allocation: true,
                max_memory_mb: 20,
            },
            SecurityTestPolicy {
                name: "unrestricted".to_string(),
                allow_file_read: true,
                allow_file_write: true,
                allow_network: true,
                allow_memory_allocation: true,
                max_memory_mb: 100,
            },
            SecurityTestPolicy {
                name: "restricted".to_string(),
                allow_file_read: false,
                allow_file_write: false,
                allow_network: false,
                allow_memory_allocation: false,
                max_memory_mb: 5,
            },
        ]
    }
}

/// Security test policy definition
#[derive(Debug, Clone)]
pub struct SecurityTestPolicy {
    pub name: String,
    pub allow_file_read: bool,
    pub allow_file_write: bool,
    pub allow_network: bool,
    pub allow_memory_allocation: bool,
    pub max_memory_mb: usize,
}

/// Integration test fixtures
pub mod integration_fixtures {
    use super::*;

    /// Create test scenarios for integration testing
    pub fn integration_test_scenarios() -> Vec<IntegrationTestScenario> {
        vec![
            IntegrationTestScenario {
                name: "plugin_communication".to_string(),
                description: "Test plugin-to-plugin communication".to_string(),
                plugins: vec!["messenger".to_string(), "receiver".to_string()],
                test_type: "communication".to_string(),
            },
            IntegrationTestScenario {
                name: "plugin_dependency".to_string(),
                description: "Test plugin dependency resolution".to_string(),
                plugins: vec!["dependency".to_string(), "dependent".to_string()],
                test_type: "dependency".to_string(),
            },
            IntegrationTestScenario {
                name: "plugin_lifecycle".to_string(),
                description: "Test plugin lifecycle management".to_string(),
                plugins: vec!["lifecycle-test".to_string()],
                test_type: "lifecycle".to_string(),
            },
        ]
    }
}

/// Integration test scenario definition
#[derive(Debug, Clone)]
pub struct IntegrationTestScenario {
    pub name: String,
    pub description: String,
    pub plugins: Vec<String>,
    pub test_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_fixture_creation() {
        let plugin_dir = plugin_fixtures::create_test_plugin("test-plugin");
        assert!(plugin_dir.path().exists());

        let lib_path = plugin_dir.path().join("src").join("lib.rs");
        assert!(lib_path.exists());
    }

    #[test]
    fn test_config_fixture_creation() {
        let config_dir = config_fixtures::minimal_test_config();
        let config_path = config_dir.path().join("config.toml");
        assert!(config_path.exists());
    }

    #[test]
    fn test_performance_fixtures() {
        let plugin_names = performance_fixtures::generate_test_plugin_names(5);
        assert_eq!(plugin_names.len(), 5);

        let dependencies = performance_fixtures::generate_test_dependencies(3);
        assert_eq!(dependencies.len(), 3);

        let scenarios = performance_fixtures::stress_test_scenarios();
        assert_eq!(scenarios.len(), 4);
    }

    #[test]
    fn test_security_fixtures() {
        let policies = security_fixtures::security_test_policies();
        assert_eq!(policies.len(), 4);

        let policy = &policies[0];
        assert_eq!(policy.name, "read_only");
        assert!(policy.allow_file_read);
        assert!(!policy.allow_file_write);
        assert!(!policy.allow_network);
    }

    #[test]
    fn test_integration_fixtures() {
        let scenarios = integration_fixtures::integration_test_scenarios();
        assert_eq!(scenarios.len(), 3);

        let scenario = &scenarios[0];
        assert_eq!(scenario.name, "plugin_communication");
        assert_eq!(scenario.plugins.len(), 2);
    }
}
