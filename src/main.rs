// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
mod bootstrap;
mod config;
mod logging;
mod plugin_manager;

use crate::config::AppConfig;
use anyhow::Result;
use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use bootstrap::{load_bootstrap_plugins, shutdown_bootstrap_plugins, BootstrapContext};
use permissions::{auth_router, AuthState};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

// CQ-003: Import dynamic plugin discovery
use plugin_manager::discovery::{DiscoveryConfig, PluginDiscovery};

#[derive(Parser)]
#[command(name = "skylet")]
#[command(about = "Execution Engine of Skylet")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    Server,
    MigrateSource,
    MigrateTarget,
    Maintenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHealth {
    pub name: String,
    pub status: String,
    pub abi_version: String,
    pub loaded_at: String,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub plugins: Arc<RwLock<Vec<PluginHealth>>>,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(Vec::new())),
            started_at: chrono::Utc::now(),
        }
    }

    pub async fn add_plugin(&self, name: &str, status: &str, abi_version: &str) {
        let mut plugins = self.plugins.write().await;
        plugins.push(PluginHealth {
            name: name.to_string(),
            status: status.to_string(),
            abi_version: abi_version.to_string(),
            loaded_at: chrono::Utc::now().to_rfc3339(),
        });
    }
}

async fn health_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let plugins = state.plugins.read().await;
    let healthy_count = plugins.iter().filter(|p| p.status == "healthy").count();
    let total_count = plugins.len();

    let status = if healthy_count == total_count && total_count > 0 {
        "healthy"
    } else if healthy_count > 0 {
        "degraded"
    } else {
        "unhealthy"
    };

    Ok(Json(json!({
        "status": status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "started_at": state.started_at.to_rfc3339(),
        "uptime_seconds": (chrono::Utc::now() - state.started_at).num_seconds(),
        "plugins": {
            "total": total_count,
            "healthy": healthy_count,
            "unhealthy": total_count - healthy_count,
            "list": plugins.iter().map(|p| json!({
                "name": p.name,
                "status": p.status,
                "abi_version": p.abi_version,
                "loaded_at": p.loaded_at
            })).collect::<Vec<_>>()
        }
    })))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured JSON logging per RFC-0018
    {
        use std::sync::Arc;
        use std::sync::Mutex;
        let buf = Arc::new(Mutex::new(Vec::new()));
        let subscriber = crate::logging::subscriber_with_buffer(buf);
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set global subscriber");
    }
    let cli = Cli::parse();
    let config = AppConfig::load()?;

    // Create data directory if it doesn't exist
    std::fs::create_dir_all(&config.data.directory)?;
    std::fs::create_dir_all(&config.plugins.directory)?;

    match cli.command {
        Commands::Server => run_server(config).await,
        Commands::MigrateSource => run_migrate_source(config),
        Commands::MigrateTarget => run_migrate_target(config),
        Commands::Maintenance => run_maintenance(config).await,
    }
}

async fn run_server(config: AppConfig) -> Result<()> {
    info!("Starting autonomous marketplace server...");

    let app_state = Arc::new(AppState::new());

    // Load bootstrap plugins
    info!("Loading bootstrap plugins...");
    let bootstrap_context = match load_bootstrap_plugins(None) {
        Ok(ctx) => {
            info!("Bootstrap plugins loaded successfully");
            app_state
                .add_plugin("config-manager", "healthy", "v2")
                .await;
            app_state.add_plugin("logging", "healthy", "v2").await;
            app_state.add_plugin("registry", "healthy", "v2").await;
            app_state
                .add_plugin("secrets-manager", "healthy", "v2")
                .await;
            ctx
        }
        Err(e) => {
            warn!("Failed to load some bootstrap plugins (non-fatal): {}", e);
            BootstrapContext::new()
        }
    };

    // CQ-003: Dynamic plugin discovery - discover plugins from filesystem
    info!("Discovering application plugins...");

    // Create discovery config from AppConfig.plugins settings
    let discovery_config = DiscoveryConfig {
        search_paths: vec![config.plugins.directory.clone()],
        exclude_patterns: config.plugins.exclude_patterns.clone(),
        include_patterns: config.plugins.include_patterns.clone(),
        probe_abi_version: config.plugins.probe_abi_version,
        include_debug_builds: config.plugins.include_debug_builds,
    };

    let discovery = PluginDiscovery::new(discovery_config);
    let app_plugins = match discovery.discover_for_loading() {
        Ok(plugins) => {
            info!("Discovered {} application plugins", plugins.len());
            plugins
        }
        Err(e) => {
            warn!("Plugin discovery failed, using empty list: {}", e);
            vec![]
        }
    };

    let loader = bootstrap::DynamicPluginLoader::new();
    for (plugin_name, abi_version) in app_plugins {
        match loader.load_plugin(&plugin_name) {
            Ok(_) => {
                info!("Loaded application plugin: {}", plugin_name);
                app_state
                    .add_plugin(&plugin_name, "healthy", &abi_version)
                    .await;
            }
            Err(e) => {
                warn!("Failed to load application plugin '{}': {}", plugin_name, e);
                app_state
                    .add_plugin(&plugin_name, "failed", &abi_version)
                    .await;
            }
        }
    }

    // Simplified server startup, relying on plugins for networking
    let app = Router::new()
        .route("/health", get(health_handler))
        .with_state(app_state.clone())
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
                .into_inner(),
        );

    // GAP-003: Create separate auth server on internal port
    let auth_state = Arc::new(AuthState::new());
    let auth_app = auth_router(auth_state);

    // Use config server settings
    let port = config.server.port;
    let host: std::net::IpAddr = config
        .server
        .host
        .parse()
        .unwrap_or_else(|_| "0.0.0.0".parse().unwrap());
    let addr = SocketAddr::from((host, port));
    info!("Server listening on {}", addr);

    // Auth server on port + 1 (internal API)
    let auth_port = port + 1;
    let auth_addr = SocketAddr::from((host, auth_port));
    info!("Auth server listening on {}", auth_addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let auth_listener = tokio::net::TcpListener::bind(auth_addr).await?;

    // Set up graceful shutdown signal handler
    let shutdown_signal = async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                info!("Received SIGINT, initiating graceful shutdown");
            }
            Err(e) => {
                error!("Failed to listen for SIGINT: {}", e);
            }
        }
    };

    // GAP-003: Run both servers concurrently with shared shutdown
    tokio::select! {
        result = axum::serve(listener, app) => {
            if let Err(e) = result {
                error!("Main server error: {}", e);
            }
        }
        result = axum::serve(auth_listener, auth_app) => {
            if let Err(e) = result {
                error!("Auth server error: {}", e);
            }
        }
        _ = shutdown_signal => {
            info!("Shutdown signal received");
        }
    }

    // Shutdown bootstrap plugins before exiting
    info!("Shutting down bootstrap plugins...");
    if let Err(e) = shutdown_bootstrap_plugins(bootstrap_context) {
        error!("Error during bootstrap plugin shutdown: {}", e);
    }

    info!("Server shutdown complete");
    Ok(())
}

fn run_migrate_source(_config: AppConfig) -> Result<()> {
    tracing::info!("Source migration: not implemented");
    Ok(())
}

fn run_migrate_target(_config: AppConfig) -> Result<()> {
    tracing::info!("Target migration: not implemented");
    Ok(())
}

async fn run_maintenance(_config: AppConfig) -> Result<()> {
    tracing::info!("Maintenance: not implemented");
    Ok(())
}
