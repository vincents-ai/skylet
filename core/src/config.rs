// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
            database: DataDirConfig::default(),
        }
    }
}
