pub mod assertions;
pub mod fixtures;
pub mod helpers;
pub mod mocks;
pub mod performance;
pub mod security;

pub use assertions::*;
pub use fixtures::*;
pub use helpers::*;
pub use mocks::*;
pub use performance::*;
pub use security::*;

/// Common test setup utilities
pub struct TestSetup;

impl TestSetup {
    /// Create a temporary test environment
    pub fn temp_env() -> tempfile::TempDir {
        tempfile::TempDir::new().expect("Failed to create temp directory")
    }

    /// Initialize test logging
    pub fn init_test_logging() {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_test_writer()
            .init();
    }

    /// Create test plugin directory
    pub fn test_plugin_dir() -> tempfile::TempDir {
        tempfile::TempDir::new().expect("Failed to create test plugin directory")
    }

    /// Load test configuration
    pub fn test_config() -> crate::config::AppConfig {
        crate::config::AppConfig::default()
    }
}

/// Test plugin for testing purposes
pub struct TestPlugin;

impl TestPlugin {
    /// Create a simple test plugin binary
    pub fn create_plugin_dir(dir: &tempfile::TempDir) -> std::path::PathBuf {
        let plugin_dir = dir.path().join("test_plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        
        // Create a simple plugin Cargo.toml
        let cargo_toml = r#"[package]
name = "test-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-abi = { path = "../../abi" }
tokio = { version = "1.0", features = ["full"] }
"#;
        
        std::fs::write(plugin_dir.join("Cargo.toml"), cargo_toml).unwrap();
        
        plugin_dir
    }

    /// Create test plugin source code
    pub fn create_plugin_source(dir: &std::path::PathBuf) {
        let lib_rs = r#"
use skylet_abi::prelude::*;

#[skylet_plugin]
pub struct TestPlugin;

#[skylet_plugin_impl]
impl Plugin for TestPlugin {
    fn name(&self) -> &'static str {
        "test-plugin"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn init(&mut self, _ctx: &mut PluginInitContext) -> Result<(), PluginError> {
        Ok(())
    }

    fn execute(&self, _ctx: &PluginContext) -> Result<PluginResult, PluginError> {
        Ok(PluginResult::Success("test result".into()))
    }

    fn cleanup(&mut self, _ctx: &PluginContext) -> Result<(), PluginError> {
        Ok(())
    }
}
"#;
        
        std::fs::write(dir.join("src").join("lib.rs"), lib_rs).unwrap();
        std::fs::create_dir_all(dir.join("src")).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temp_env_creation() {
        let _temp_dir = TestSetup::temp_env();
        // Directory will be cleaned up automatically
    }

    #[test]
    fn test_init_test_logging() {
        TestSetup::init_test_logging();
        // Should not panic
    }

    #[test]
    fn test_plugin_creation() {
        let temp_dir = TestSetup::test_plugin_dir();
        let plugin_dir = TestPlugin::create_plugin_dir(&temp_dir);
        TestPlugin::create_plugin_source(&plugin_dir);
        
        assert!(plugin_dir.exists());
        assert!(plugin_dir.join("Cargo.toml").exists());
        assert!(plugin_dir.join("src").join("lib.rs").exists());
    }

    #[test]
    fn test_config_loading() {
        let config = TestSetup::test_config();
        // Should not panic, basic validation
        assert!(!config.plugins.directory.as_os_str().is_empty());
    }
}