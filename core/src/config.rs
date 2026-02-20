// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub enabled: bool,
    pub dht_bootstrap_nodes: Vec<String>,
    pub dht_replication_factor: u32,
    pub dht_timeout_seconds: u64,
    pub i2p_enabled: bool,
    pub i2p_sam_host: Option<String>,
    pub i2p_sam_port: Option<u16>,
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
            dht_bootstrap_nodes: vec![],
            dht_replication_factor: 20,
            dht_timeout_seconds: 30,
            i2p_enabled: false,
            i2p_sam_host: None,
            i2p_sam_port: None,
            fallback_endpoints: vec![],
            cache_ttl: 3600,
            announce_interval: 300,
            peer_id: None,
            private_key: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { port: 8080 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDirConfig {
    pub data_dir: PathBuf,
}

impl Default for DataDirConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub tor: TorConfig,
    pub discovery: DiscoveryConfig,
    pub database: DataDirConfig,
}

#[derive(Debug, Clone, clap::Parser)]
pub struct ConfigArgs {
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    #[arg(long, default_value = "./data")]
    pub data_dir: Option<PathBuf>,
}

impl AppConfig {
    pub fn load(args: &ConfigArgs) -> Result<Self> {
        let mut config = if let Some(config_path) = &args.config {
            let config_content = std::fs::read_to_string(config_path)?;
            toml::from_str(&config_content)?
        } else {
            AppConfig::default()
        };

        // Override with command-line arguments if provided
        if args.port != ServerConfig::default().port {
            config.server.port = args.port;
        }
        if let Some(data_dir) = &args.data_dir {
            config.database.data_dir = data_dir.clone();
        }

        Ok(config)
    }

    pub fn create_directories(&self) -> Result<()> {
        std::fs::create_dir_all(&self.database.data_dir)?;
        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            tor: TorConfig::default(),
            discovery: DiscoveryConfig::default(),
            database: DataDirConfig::default(),
        }
    }
}
