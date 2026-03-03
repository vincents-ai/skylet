// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
mod bootstrap;
mod config;
mod logging;
mod plugin_manager;

use crate::config::AppConfig;
use anyhow::Result;
use axum::{extract::Path, extract::State, http::StatusCode, response::Json, routing::get, Router};
use serde_json::json;

use bootstrap::{load_bootstrap_plugins, shutdown_bootstrap_plugins, BootstrapContext};
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

// GAP-003: Import auth HTTP handlers from permissions crate
use permissions::http::{auth_router, AuthState};

// Plugin lifecycle orchestrator (wraps discovery, dep resolution, and loading)
use plugin_manager::lifecycle::{LifecycleConfig, PluginLifecycleManager};

// Discovery config to build LifecycleConfig from AppConfig
use plugin_manager::discovery::DiscoveryConfig;

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

#[derive(Debug, Clone)]
pub struct AppState {
    pub lifecycle: Arc<PluginLifecycleManager>,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

impl AppState {
    pub fn new(lifecycle: Arc<PluginLifecycleManager>) -> Self {
        Self {
            lifecycle,
            started_at: chrono::Utc::now(),
        }
    }
}

async fn health_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let summary = state.lifecycle.status_summary().await;
    let active = summary.get("active").copied().unwrap_or(0);
    let failed = summary.get("failed").copied().unwrap_or(0);
    let total: usize = summary.values().sum();

    let status = if failed == 0 && active > 0 {
        "healthy"
    } else if active > 0 {
        "degraded"
    } else {
        "unhealthy"
    };

    let plugins = state.lifecycle.list_plugins().await;

    Ok(Json(json!({
        "status": status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "started_at": state.started_at.to_rfc3339(),
        "uptime_seconds": (chrono::Utc::now() - state.started_at).num_seconds(),
        "plugins": {
            "total": total,
            "active": active,
            "failed": failed,
            "list": plugins.iter().map(|p| json!({
                "name": p.name,
                "status": format!("{}", p.status),
                "abi_version": p.abi_version,
                "loaded_at": p.loaded_at.map(|t| t.to_rfc3339())
            })).collect::<Vec<_>>()
        }
    })))
}

async fn ready_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let active = state.lifecycle.active_count().await;
    let summary = state.lifecycle.status_summary().await;
    let total: usize = summary.values().sum();

    let is_ready = active > 0;

    if is_ready {
        Ok(Json(json!({
            "status": "ready",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "plugins": {
                "total": total,
                "active": active
            }
        })))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

async fn plugins_list_handler(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let plugins = state.lifecycle.list_plugins().await;
    let order = state.lifecycle.loading_order().await;

    Json(json!({
        "plugins": plugins.iter().map(|p| json!({
            "name": p.name,
            "status": format!("{}", p.status),
            "abi_version": p.abi_version,
            "path": p.path.display().to_string(),
            "dependencies": p.dependencies,
            "loaded_at": p.loaded_at.map(|t| t.to_rfc3339()),
            "error": p.error,
        })).collect::<Vec<_>>(),
        "loading_order": order,
        "total": plugins.len(),
    }))
}

async fn plugin_detail_handler(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.lifecycle.get_state(&name).await {
        Some(p) => Ok(Json(json!({
            "name": p.name,
            "status": format!("{}", p.status),
            "abi_version": p.abi_version,
            "path": p.path.display().to_string(),
            "dependencies": p.dependencies,
            "loaded_at": p.loaded_at.map(|t| t.to_rfc3339()),
            "error": p.error,
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
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
    info!("Starting Skylet server...");

    // Load bootstrap plugins
    info!("Loading bootstrap plugins...");
    let bootstrap_context = match load_bootstrap_plugins(None) {
        Ok(ctx) => {
            info!("Bootstrap plugins loaded successfully");
            ctx
        }
        Err(e) => {
            warn!("Failed to load some bootstrap plugins (non-fatal): {}", e);
            BootstrapContext::new()
        }
    };

    // Build lifecycle configuration from AppConfig
    let discovery_config = DiscoveryConfig {
        search_paths: vec![config.plugins.directory.clone()],
        exclude_patterns: config.plugins.exclude_patterns.clone(),
        include_patterns: config.plugins.include_patterns.clone(),
        probe_abi_version: config.plugins.probe_abi_version,
        include_debug_builds: config.plugins.include_debug_builds,
    };

    let lifecycle_config = LifecycleConfig {
        discovery: discovery_config,
        continue_on_failure: true,
        health_check_interval_secs: 0,
    };

    // Create lifecycle manager and activate all plugins
    // (handles discovery → dependency resolution → ordered loading)
    let lifecycle_manager = Arc::new(PluginLifecycleManager::new(lifecycle_config));

    info!("Discovering and activating application plugins...");
    match lifecycle_manager.activate_all().await {
        Ok((loaded, failed)) => {
            info!(
                "Plugin activation complete: {} loaded, {} failed",
                loaded, failed
            );
        }
        Err(e) => {
            error!("Plugin activation failed: {}", e);
        }
    }

    let app_state = Arc::new(AppState::new(lifecycle_manager.clone()));

    // Simplified server startup, relying on plugins for networking
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/plugins", get(plugins_list_handler))
        .route("/plugins/:name", get(plugin_detail_handler))
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

    // Shutdown application plugins (reverse dependency order) before bootstrap
    info!("Shutting down application plugins...");
    lifecycle_manager.shutdown_all().await;

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
