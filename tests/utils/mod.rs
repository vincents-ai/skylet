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
        use crate::config::ConfigArgs;
        
        let args = ConfigArgs {
            config: None,
            database_path: None,
            database_node_id: None,
            database_raft_nodes: None,
            database_election_timeout_ms: None,
            database_secret_raft: None,
            database_secret_api: None,
            database_data_dir: None,
            tor_socks_port: None,
            tor_control_port: None,
            tor_hidden_service_port: None,
            monero_daemon_url: None,
            monero_wallet_path: None,
            monero_wallet_rpc_port: None,
            monero_network: None,
            monero_wallet_password: None,
            monero_auto_refresh: None,
            monero_refresh_interval: None,
            agents_enabled: None,
            agents_security_scan_interval: None,
            agents_maintenance_interval: None,
            discovery_enabled: None,
            discovery_dht_bootstrap_nodes: None,
            discovery_dht_replication_factor: None,
            discovery_dht_timeout_seconds: None,
            discovery_i2p_enabled: None,
            discovery_i2p_sam_host: None,
            discovery_i2p_sam_port: None,
            discovery_fallback_endpoints: None,
            discovery_cache_ttl: None,
            discovery_announce_interval: None,
            discovery_peer_id: None,
            discovery_private_key: None,
            payments_enabled: None,
            payments_reserve_months: None,
            payments_payment_buffer_days: None,
            payments_check_interval_hours: None,
            payments_max_payment_amount: None,
            payments_retry_attempts: None,
            payments_retry_delay_minutes: None,
            payments_large_payment_threshold: None,
            escrow_default_release_days: None,
            escrow_max_dispute_days: None,
            escrow_marketplace_fee_percentage: None,
            escrow_min_arbiter_confidence: None,
            escrow_auto_arbitration_enabled: None,
        };

        crate::config::AppConfig::load(&args).expect("Failed to load test config")
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