// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Config Manager Plugin - Configuration loading and management service
//!
//! This plugin provides centralized configuration management including:
//! - Configuration loading from TOML/JSON files
//! - Environment variable overrides
//! - CLI argument parsing
//! - Service registry integration
//!
//! ## Migration to v2 ABI
//!
//! This plugin has been migrated from v1 ABI (manual C string handling, unsafe static mut)
//! to v2 ABI (RFC-0004 Phase 1).
//!
//! ### Changes:
//! - Removed v1 ABI functions (plugin_init, plugin_shutdown, plugin_get_info)
//! - Removed unsafe `static mut CONFIG_SERVICE`
//! - Removed manual C string handling functions (config_load, config_get, etc.)
//! - All plugin ABI functions now implemented in v2_ffi.rs module
//! - ConfigService uses thread-safe Arc<RwLock> pattern
//! - PluginInfoV2 structure with 40+ metadata fields
//! - SafePluginContext for type-safe service access
//! - Uses skylet-plugin-common for RFC-0006 compliant config paths

// Export v2 ABI implementation
mod v2_ffi;
pub use v2_ffi::*;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use skylet_plugin_common::config_paths;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tracing::{info, warn};

// ============================================================================
// Configuration Types
// ============================================================================

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: PathBuf,
    pub node_id: u64,
    pub raft_nodes: Vec<String>,
    pub election_timeout_ms: u64,
    pub secret_raft: String,
    pub secret_api: String,
    pub data_dir: PathBuf,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("./data/marketplace.db"),
            node_id: 1,
            raft_nodes: vec!["localhost:8100".to_string(), "localhost:8200".to_string()],
            election_timeout_ms: 5000,
            secret_raft: "MarketplaceRaftSecret1337".to_string(),
            secret_api: "MarketplaceApiSecret1337".to_string(),
            data_dir: PathBuf::from("./data"),
        }
    }
}

/// Tor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorConfig {
    pub socks_port: u16,
    pub control_port: u16,
    pub hidden_service_port: u16,
}

impl Default for TorConfig {
    fn default() -> Self {
        Self {
            socks_port: 9050,
            control_port: 9051,
            hidden_service_port: 8080,
        }
    }
}

/// Monero wallet configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoneroConfig {
    pub daemon_url: String,
    pub wallet_path: PathBuf,
    pub wallet_rpc_port: u16,
    pub network: String,
    pub wallet_password: Option<String>,
    pub auto_refresh: bool,
    pub refresh_interval: u64,
}

impl Default for MoneroConfig {
    fn default() -> Self {
        Self {
            daemon_url: "http://localhost:18081".to_string(),
            wallet_path: PathBuf::from("./data/wallet"),
            wallet_rpc_port: 18083,
            network: "testnet".to_string(),
            wallet_password: None,
            auto_refresh: true,
            refresh_interval: 30,
        }
    }
}

/// Discovery DHT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryDhtConfig {
    pub bootstrap_nodes: Vec<String>,
    pub replication_factor: u32,
    pub timeout_seconds: u64,
}

impl Default for DiscoveryDhtConfig {
    fn default() -> Self {
        Self {
            bootstrap_nodes: vec![
                "/ip4/104.131.131.82/tcp/4001/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ"
                    .to_string(),
                "/ip4/104.131.131.82/udp/4001/quic/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ"
                    .to_string(),
            ],
            replication_factor: 20,
            timeout_seconds: 30,
        }
    }
}

/// Discovery I2P configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryI2pConfig {
    pub enabled: bool,
    pub sam_host: String,
    pub sam_port: u16,
}

impl Default for DiscoveryI2pConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sam_host: "127.0.0.1".to_string(),
            sam_port: 7656,
        }
    }
}

/// Service discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub enabled: bool,
    pub dht: DiscoveryDhtConfig,
    pub i2p: DiscoveryI2pConfig,
    pub fallback_endpoints: Vec<String>,
    pub cache_ttl: u64,
    pub announce_interval: u64,
    pub peer_id: Option<String>,
    pub private_key: Option<String>,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dht: DiscoveryDhtConfig::default(),
            i2p: DiscoveryI2pConfig::default(),
            fallback_endpoints: vec![
                "marketplace.onion".to_string(),
                "marketplace.i2p".to_string(),
                "127.0.0.1:8080".to_string(),
            ],
            cache_ttl: 3600,
            announce_interval: 300,
            peer_id: None,
            private_key: None,
        }
    }
}

/// Security agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enabled: bool,
    pub scan_interval_seconds: u64,
    pub cve_database_url: Option<String>,
    pub auto_patch_threshold: f64,
    pub enable_vulnerability_scanning: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scan_interval_seconds: 3600,
            cve_database_url: None,
            auto_patch_threshold: 0.8,
            enable_vulnerability_scanning: true,
        }
    }
}

/// Maintenance agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceConfig {
    pub enabled: bool,
    pub maintenance_interval: u64,
    pub test_environment: String,
    pub deployment_stages: Vec<String>,
}

impl Default for MaintenanceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            maintenance_interval: 86400,
            test_environment: "production".to_string(),
            deployment_stages: vec![
                "pre".to_string(),
                "staging".to_string(),
                "production".to_string(),
            ],
        }
    }
}

/// Agents configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsConfig {
    pub enabled: bool,
    pub security: SecurityConfig,
    pub maintenance: MaintenanceConfig,
    pub ops_interval: u64,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            security: SecurityConfig::default(),
            maintenance: MaintenanceConfig::default(),
            ops_interval: 3600,
        }
    }
}

/// Hosting provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostingProviderConfig {
    pub provider_type: String,
    pub name: String,
    pub api_key: String,
    pub api_url: Option<String>,
    pub enabled: bool,
    pub payment_address: Option<String>,
    pub monthly_cost_limit: Option<u64>,
}

impl Default for HostingProviderConfig {
    fn default() -> Self {
        Self {
            provider_type: "digitalocean".to_string(),
            name: "default".to_string(),
            api_key: String::new(),
            api_url: None,
            enabled: false,
            payment_address: None,
            monthly_cost_limit: None,
        }
    }
}

/// Escrow configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscrowConfig {
    pub default_release_days: u32,
    pub max_dispute_days: u32,
    pub marketplace_fee_percentage: f64,
    pub min_arbitrator_confidence: f64,
    pub auto_arbitration_enabled: bool,
}

impl Default for EscrowConfig {
    fn default() -> Self {
        Self {
            default_release_days: 7,
            max_dispute_days: 30,
            marketplace_fee_percentage: 2.5,
            min_arbitrator_confidence: 0.8,
            auto_arbitration_enabled: true,
        }
    }
}

/// Payment processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentsConfig {
    pub enabled: bool,
    pub reserve_months: u32,
    pub payment_buffer_days: u32,
    pub check_interval_hours: u32,
    pub max_payment_amount: u64,
    pub retry_attempts: u32,
    pub retry_delay_minutes: u32,
    pub providers: Vec<HostingProviderConfig>,
    pub large_payment_threshold: u64,
}

impl Default for PaymentsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            reserve_months: 3,
            payment_buffer_days: 7,
            check_interval_hours: 24,
            max_payment_amount: 100000000000,
            retry_attempts: 3,
            retry_delay_minutes: 60,
            providers: vec![],
            large_payment_threshold: 500000000000,
        }
    }
}

/// Application configuration (main config struct)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub tor: TorConfig,
    pub monero: MoneroConfig,
    pub agents: AgentsConfig,
    pub discovery: DiscoveryConfig,
    pub escrow: EscrowConfig,
    pub payments: PaymentsConfig,
}

// ============================================================================
// Configuration Service
// ============================================================================

/// ConfigService provides configuration management
#[derive(Debug, Clone)]
pub struct ConfigService {
    config: Arc<RwLock<AppConfig>>,
}

impl ConfigService {
    /// Create a new configuration service with defaults
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(AppConfig::default())),
        }
    }

    /// Load configuration from defaults
    pub fn load_defaults() -> Result<Self> {
        info!("Loading default configuration");
        Ok(Self::new())
    }

    /// Load configuration using RFC-0006 compliant config paths
    /// Searches in order: local -> user -> system
    pub fn load_auto() -> Result<Self> {
        if let Some(path) = config_paths::find_config("config-manager") {
            info!("Found configuration at: {:?}", path);
            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("toml");
            match extension {
                "json" => Self::load_from_json(path.to_str().unwrap_or("")),
                "yaml" | "yml" => Self::load_from_yaml(path.to_str().unwrap_or("")),
                _ => Self::load_from_toml(path.to_str().unwrap_or("")),
            }
        } else {
            info!("No configuration file found, using defaults");
            Self::load_defaults()
        }
    }

    /// Load configuration from TOML file
    pub fn load_from_toml(path: &str) -> Result<Self> {
        info!("Loading configuration from TOML file: {}", path);
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read config file: {}", e))?;
        let config: AppConfig =
            toml::from_str(&content).map_err(|e| anyhow!("Failed to parse TOML config: {}", e))?;
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
        })
    }

    /// Load configuration from JSON file
    pub fn load_from_json(path: &str) -> Result<Self> {
        info!("Loading configuration from JSON file: {}", path);
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read config file: {}", e))?;
        let config: AppConfig = serde_json::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse JSON config: {}", e))?;
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
        })
    }

    /// Load configuration from YAML file
    pub fn load_from_yaml(path: &str) -> Result<Self> {
        info!("Loading configuration from YAML file: {}", path);
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read config file: {}", e))?;
        let config: AppConfig = serde_yaml::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse YAML config: {}", e))?;
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
        })
    }

    /// Get current configuration
    pub fn get_config(&self) -> Result<AppConfig> {
        self.config
            .read()
            .map(|lock| lock.clone())
            .map_err(|e| anyhow!("Failed to read config: {}", e))
    }

    /// Update configuration
    pub fn set_config(&self, config: AppConfig) -> Result<()> {
        *self
            .config
            .write()
            .map_err(|e| anyhow!("Failed to write config: {}", e))? = config;
        info!("Configuration updated");
        Ok(())
    }

    /// Get database configuration
    pub fn get_database_config(&self) -> Result<DatabaseConfig> {
        Ok(self.get_config()?.database)
    }

    /// Set database configuration
    pub fn set_database_config(&self, db_config: DatabaseConfig) -> Result<()> {
        let mut config = self.get_config()?;
        config.database = db_config;
        self.set_config(config)?;
        info!("Database configuration updated");
        Ok(())
    }

    /// Get Tor configuration
    pub fn get_tor_config(&self) -> Result<TorConfig> {
        Ok(self.get_config()?.tor)
    }

    /// Set Tor configuration
    pub fn set_tor_config(&self, tor_config: TorConfig) -> Result<()> {
        let mut config = self.get_config()?;
        config.tor = tor_config;
        self.set_config(config)?;
        info!("Tor configuration updated");
        Ok(())
    }

    /// Get Monero configuration
    pub fn get_monero_config(&self) -> Result<MoneroConfig> {
        Ok(self.get_config()?.monero)
    }

    /// Set Monero configuration
    pub fn set_monero_config(&self, monero_config: MoneroConfig) -> Result<()> {
        let mut config = self.get_config()?;
        config.monero = monero_config;
        self.set_config(config)?;
        info!("Monero configuration updated");
        Ok(())
    }

    /// Get agents configuration
    pub fn get_agents_config(&self) -> Result<AgentsConfig> {
        Ok(self.get_config()?.agents)
    }

    /// Set agents configuration
    pub fn set_agents_config(&self, agents_config: AgentsConfig) -> Result<()> {
        let mut config = self.get_config()?;
        config.agents = agents_config;
        self.set_config(config)?;
        info!("Agents configuration updated");
        Ok(())
    }

    /// Get discovery configuration
    pub fn get_discovery_config(&self) -> Result<DiscoveryConfig> {
        Ok(self.get_config()?.discovery)
    }

    /// Set discovery configuration
    pub fn set_discovery_config(&self, discovery_config: DiscoveryConfig) -> Result<()> {
        let mut config = self.get_config()?;
        config.discovery = discovery_config;
        self.set_config(config)?;
        info!("Discovery configuration updated");
        Ok(())
    }

    /// Get escrow configuration
    pub fn get_escrow_config(&self) -> Result<EscrowConfig> {
        Ok(self.get_config()?.escrow)
    }

    /// Set escrow configuration
    pub fn set_escrow_config(&self, escrow_config: EscrowConfig) -> Result<()> {
        let mut config = self.get_config()?;
        config.escrow = escrow_config;
        self.set_config(config)?;
        info!("Escrow configuration updated");
        Ok(())
    }

    /// Get payments configuration
    pub fn get_payments_config(&self) -> Result<PaymentsConfig> {
        Ok(self.get_config()?.payments)
    }

    /// Set payments configuration
    pub fn set_payments_config(&self, payments_config: PaymentsConfig) -> Result<()> {
        let mut config = self.get_config()?;
        config.payments = payments_config;
        self.set_config(config)?;
        info!("Payments configuration updated");
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        let config = self.get_config()?;

        // Validate database config
        if config.database.path.as_os_str().is_empty() {
            return Err(anyhow!("Database path cannot be empty"));
        }
        if config.database.node_id == 0 {
            return Err(anyhow!("Database node_id must be greater than 0"));
        }
        if config.database.raft_nodes.is_empty() {
            return Err(anyhow!("At least one Raft node must be configured"));
        }

        // Validate Monero config
        if config.monero.daemon_url.is_empty() {
            return Err(anyhow!("Monero daemon URL cannot be empty"));
        }
        if config.monero.wallet_rpc_port == 0 {
            return Err(anyhow!("Monero wallet RPC port must be greater than 0"));
        }

        // Validate discovery config
        if config.discovery.enabled && config.discovery.dht.bootstrap_nodes.is_empty() {
            warn!("Discovery enabled but no DHT bootstrap nodes configured");
        }

        // Validate escrow config
        if config.escrow.marketplace_fee_percentage < 0.0
            || config.escrow.marketplace_fee_percentage > 100.0
        {
            return Err(anyhow!(
                "Marketplace fee percentage must be between 0 and 100"
            ));
        }

        info!("Configuration validation successful");
        Ok(())
    }

    /// Export configuration as TOML
    pub fn export_toml(&self) -> Result<String> {
        let config = self.get_config()?;
        toml::to_string_pretty(&config)
            .map_err(|e| anyhow!("Failed to export config as TOML: {}", e))
    }

    /// Export configuration as JSON
    pub fn export_json(&self) -> Result<String> {
        let config = self.get_config()?;
        serde_json::to_string_pretty(&config)
            .map_err(|e| anyhow!("Failed to export config as JSON: {}", e))
    }

    /// Export configuration as YAML
    pub fn export_yaml(&self) -> Result<String> {
        let config = self.get_config()?;
        serde_yaml::to_string(&config)
            .map_err(|e| anyhow!("Failed to export config as YAML: {}", e))
    }
}

impl Default for ConfigService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.database.node_id, 1);
        assert!(!config.payments.enabled);
        assert!(config.agents.enabled);
    }

    #[test]
    fn test_config_service_creation() {
        let service = ConfigService::new();
        let config = service.get_config().unwrap();
        assert_eq!(config.database.node_id, 1);
    }

    #[test]
    fn test_config_validation() {
        let service = ConfigService::new();
        assert!(service.validate().is_ok());
    }

    #[test]
    fn test_config_export_json() {
        let service = ConfigService::new();
        let json = service.export_json().unwrap();
        assert!(!json.is_empty());
        assert!(json.contains("database"));
    }

    #[test]
    fn test_config_export_toml() {
        let service = ConfigService::new();
        let toml = service.export_toml().unwrap();
        assert!(!toml.is_empty());
        assert!(toml.contains("[database]"));
    }

    #[test]
    fn test_database_config_update() {
        let service = ConfigService::new();
        let mut db_config = service.get_database_config().unwrap();
        db_config.node_id = 42;
        service.set_database_config(db_config).unwrap();

        let updated = service.get_database_config().unwrap();
        assert_eq!(updated.node_id, 42);
    }

    #[test]
    fn test_tor_config_update() {
        let service = ConfigService::new();
        let mut tor_config = service.get_tor_config().unwrap();
        tor_config.socks_port = 9999;
        service.set_tor_config(tor_config).unwrap();

        let updated = service.get_tor_config().unwrap();
        assert_eq!(updated.socks_port, 9999);
    }
}
