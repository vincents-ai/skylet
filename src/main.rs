// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
mod bootstrap;
mod config;
mod logging;
mod plugin_manager;

use crate::config::AppConfig;
use anyhow::Result;
use axum::{
    extract::Path, extract::State, http::StatusCode, response::Json, routing::get, routing::post,
    Router,
};
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

// Hot-reload service (Phase 7)
use plugin_manager::hot_reload::{HotReloadConfig, HotReloadService};

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

#[derive(Clone)]
pub struct AppState {
    pub lifecycle: Arc<PluginLifecycleManager>,
    pub hot_reload: Arc<HotReloadService>,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("lifecycle", &self.lifecycle)
            .field("started_at", &self.started_at)
            .finish_non_exhaustive()
    }
}

impl AppState {
    pub fn new(lifecycle: Arc<PluginLifecycleManager>, hot_reload: Arc<HotReloadService>) -> Self {
        Self {
            lifecycle,
            hot_reload,
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

async fn plugins_list_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
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

// ============================================================================
// Phase 3: Config endpoint
// ============================================================================

async fn config_plugin_handler(
    State(state): State<Arc<AppState>>,
    Path(plugin_name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let backend = state.lifecycle.config_backend();
    match backend.load_plugin_config(&plugin_name).await {
        Ok(config_val) => Ok(Json(json!({
            "plugin": plugin_name,
            "config": config_val,
            "environment": format!("{:?}", backend.environment()),
        }))),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

// ============================================================================
// Phase 4: Metrics endpoint
// ============================================================================

async fn metrics_handler(State(state): State<Arc<AppState>>) -> Result<String, StatusCode> {
    let manager = state.lifecycle.metrics_manager();
    match manager.export_metrics().await {
        Ok(outputs) => Ok(outputs.join("\n")),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// ============================================================================
// Phase 5: Events stats endpoint
// ============================================================================

async fn events_stats_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let event_system = state.lifecycle.event_system();
    let stats = event_system.get_statistics().await;
    let storage_stats = event_system.storage().get_storage_stats().await;

    Json(json!({
        "event_statistics": stats,
        "storage_statistics": storage_stats,
    }))
}

// ============================================================================
// Phase 6: Circuit breakers endpoint
// ============================================================================

async fn circuit_breakers_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let failover = state.lifecycle.failover();
    let failover_guard = failover.read().await;
    let states = failover_guard.get_all_service_states();

    let services: Vec<serde_json::Value> = states
        .iter()
        .map(|(name, circuit_state)| {
            json!({
                "service": name,
                "state": format!("{}", circuit_state),
            })
        })
        .collect();

    Json(json!({
        "circuit_breakers": services,
        "total": services.len(),
    }))
}

// ============================================================================
// Phase 7: Hot-reload endpoint
// ============================================================================

// Static assertion: AppState must be Send + Sync for axum handlers
const _: () = {
    fn _assert_send_sync<T: Send + Sync>() {}
    fn _check() {
        _assert_send_sync::<AppState>();
        _assert_send_sync::<HotReloadService>();
    }
};

async fn reload_plugin_handler(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.hot_reload.reload_plugin(&name).await {
        Ok(result) => Ok(Json(json!({
            "plugin_id": result.plugin_id,
            "success": result.success,
            "old_version": result.old_version,
            "new_version": result.new_version,
            "state_preserved": result.state_preserved,
            "duration_ms": result.duration_ms,
            "error": result.error,
            "rolled_back": result.rolled_back,
        }))),
        Err(e) => Ok(Json(json!({
            "plugin_id": name,
            "success": false,
            "error": e.to_string(),
        }))),
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

    // Start metrics collection
    if let Err(e) = lifecycle_manager.start_metrics().await {
        warn!("Failed to start metrics collection: {}", e);
    }

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

    // Phase 7: Create hot-reload service and register active plugins
    let hot_reload_service = Arc::new(HotReloadService::new(
        HotReloadConfig::default(),
        lifecycle_manager.clone(),
    ));

    // Register active plugins for hot-reload watching
    let active_plugins = lifecycle_manager.list_plugins().await;
    for plugin in &active_plugins {
        if format!("{}", plugin.status) == "Active" {
            if let Err(e) = hot_reload_service
                .watch_plugin(&plugin.name, &plugin.path)
                .await
            {
                warn!(
                    "Failed to register plugin '{}' for hot-reload: {}",
                    plugin.name, e
                );
            }
        }
    }

    // Start hot-reload service
    if let Err(e) = hot_reload_service.start().await {
        warn!("Failed to start hot-reload service: {}", e);
    }

    let app_state = Arc::new(AppState::new(
        lifecycle_manager.clone(),
        hot_reload_service.clone(),
    ));

    // Simplified server startup, relying on plugins for networking
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/plugins", get(plugins_list_handler))
        .route("/plugins/:name", get(plugin_detail_handler))
        .route("/config/:plugin", get(config_plugin_handler))
        .route("/metrics", get(metrics_handler))
        .route("/events/stats", get(events_stats_handler))
        .route("/circuit-breakers", get(circuit_breakers_handler))
        .route("/reload/:name", post(reload_plugin_handler))
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

    // Phase 7: Stop hot-reload service before shutting down plugins
    info!("Stopping hot-reload service...");
    if let Err(e) = hot_reload_service.stop().await {
        error!("Error stopping hot-reload service: {}", e);
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

// ============================================================================
// Integration Tests for HTTP Endpoints
// ============================================================================

#[cfg(test)]
mod endpoint_tests {
    use super::*;
    use axum::extract::{Path, State};

    /// Build a test AppState with no plugins loaded.
    fn test_state() -> State<Arc<AppState>> {
        let lifecycle_config = LifecycleConfig::default();
        let lifecycle_manager = Arc::new(PluginLifecycleManager::new(lifecycle_config));
        let hot_reload_service = Arc::new(HotReloadService::new(
            HotReloadConfig::default(),
            lifecycle_manager.clone(),
        ));
        State(Arc::new(AppState::new(
            lifecycle_manager,
            hot_reload_service,
        )))
    }

    // ========================================================================
    // GET /health
    // ========================================================================

    #[tokio::test]
    async fn health_returns_ok_with_expected_fields() {
        let state = test_state();
        let result = health_handler(state).await;
        assert!(result.is_ok(), "health_handler returned Err");

        let Json(json) = result.unwrap();
        assert!(json.get("status").is_some(), "missing 'status' field");
        assert!(json.get("timestamp").is_some(), "missing 'timestamp' field");
        assert!(
            json.get("started_at").is_some(),
            "missing 'started_at' field"
        );
        assert!(
            json.get("uptime_seconds").is_some(),
            "missing 'uptime_seconds' field"
        );
        assert!(json.get("plugins").is_some(), "missing 'plugins' field");
    }

    #[tokio::test]
    async fn health_reports_unhealthy_with_no_plugins() {
        let state = test_state();
        let Json(json) = health_handler(state).await.unwrap();

        // With zero plugins: active=0, failed=0, so status should be "unhealthy"
        assert_eq!(json["status"], "unhealthy");
        assert_eq!(json["plugins"]["total"], 0);
        assert_eq!(json["plugins"]["active"], 0);
        assert_eq!(json["plugins"]["failed"], 0);
    }

    // ========================================================================
    // GET /ready
    // ========================================================================

    #[tokio::test]
    async fn ready_returns_unavailable_with_no_active_plugins() {
        let state = test_state();
        let result = ready_handler(state).await;
        assert!(result.is_err(), "expected Err(SERVICE_UNAVAILABLE)");
        assert_eq!(result.unwrap_err(), StatusCode::SERVICE_UNAVAILABLE);
    }

    // ========================================================================
    // GET /plugins
    // ========================================================================

    #[tokio::test]
    async fn plugins_list_returns_empty_list() {
        let state = test_state();
        let Json(json) = plugins_list_handler(state).await;

        assert_eq!(json["total"], 0);
        assert!(json["plugins"].as_array().unwrap().is_empty());
        assert!(json["loading_order"].as_array().unwrap().is_empty());
    }

    // ========================================================================
    // GET /plugins/:name
    // ========================================================================

    #[tokio::test]
    async fn plugin_detail_returns_not_found_for_nonexistent() {
        let state = test_state();
        let result = plugin_detail_handler(state, Path("nonexistent".to_string())).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StatusCode::NOT_FOUND);
    }

    // ========================================================================
    // GET /config/:plugin (Phase 3)
    // ========================================================================

    #[tokio::test]
    async fn config_returns_empty_config_for_unknown_plugin() {
        let state = test_state();
        let result = config_plugin_handler(state, Path("nonexistent".to_string())).await;

        // Config backend returns Ok({}) for missing config files, so the handler succeeds
        assert!(result.is_ok());
        let Json(json) = result.unwrap();
        assert_eq!(json["plugin"], "nonexistent");
        assert!(json.get("config").is_some(), "missing 'config' field");
        assert!(
            json.get("environment").is_some(),
            "missing 'environment' field"
        );
    }

    // ========================================================================
    // GET /metrics (Phase 4)
    // ========================================================================

    #[tokio::test]
    async fn metrics_returns_ok_with_empty_output() {
        let state = test_state();
        let result = metrics_handler(state).await;
        assert!(result.is_ok(), "metrics_handler returned Err");

        // With no exporters registered, export_metrics returns Ok(vec![]) → joined = ""
        let text = result.unwrap();
        assert!(text.is_empty(), "expected empty metrics with no exporters");
    }

    // ========================================================================
    // GET /events/stats (Phase 5)
    // ========================================================================

    #[tokio::test]
    async fn events_stats_returns_expected_fields() {
        let state = test_state();
        let Json(json) = events_stats_handler(state).await;

        assert!(
            json.get("event_statistics").is_some(),
            "missing 'event_statistics' field"
        );
        assert!(
            json.get("storage_statistics").is_some(),
            "missing 'storage_statistics' field"
        );
    }

    #[tokio::test]
    async fn events_stats_has_zero_counts_initially() {
        let state = test_state();
        let Json(json) = events_stats_handler(state).await;

        let stats = &json["event_statistics"];
        assert_eq!(stats["total_published"], 0);
        assert_eq!(stats["high_priority_published"], 0);
    }

    // ========================================================================
    // GET /circuit-breakers (Phase 6)
    // ========================================================================

    #[tokio::test]
    async fn circuit_breakers_returns_empty_list() {
        let state = test_state();
        let Json(json) = circuit_breakers_handler(state).await;

        assert_eq!(json["total"], 0);
        assert!(json["circuit_breakers"].as_array().unwrap().is_empty());
    }

    // ========================================================================
    // POST /reload/:name (Phase 7)
    // ========================================================================

    #[tokio::test]
    async fn reload_returns_failure_for_unknown_plugin() {
        let state = test_state();
        let result = reload_plugin_handler(state, Path("nonexistent".to_string())).await;

        // The handler catches errors and returns Ok with success: false
        assert!(result.is_ok());
        let Json(json) = result.unwrap();
        assert_eq!(json["plugin_id"], "nonexistent");
        assert_eq!(json["success"], false);
        assert!(json.get("error").is_some(), "missing 'error' field");
    }

    // ========================================================================
    // AppState construction
    // ========================================================================

    #[tokio::test]
    async fn app_state_tracks_start_time() {
        let before = chrono::Utc::now();
        let State(state) = test_state();
        let after = chrono::Utc::now();

        assert!(state.started_at >= before);
        assert!(state.started_at <= after);
    }

    #[tokio::test]
    async fn app_state_debug_does_not_panic() {
        let State(state) = test_state();
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("AppState"));
        assert!(debug_str.contains("started_at"));
    }

    // ========================================================================
    // Lifecycle accessor integration
    // ========================================================================

    #[tokio::test]
    async fn lifecycle_accessors_return_valid_instances() {
        let State(state) = test_state();
        let lifecycle = &state.lifecycle;

        // Phase 3: config backend is accessible
        let _config = lifecycle.config_backend();

        // Phase 4: metrics manager is accessible
        let _metrics = lifecycle.metrics_manager();

        // Phase 5: event system is accessible
        let _events = lifecycle.event_system();

        // Phase 6: failover is accessible
        let _failover = lifecycle.failover();

        // All subsystems initialized without panic
    }

    #[tokio::test]
    async fn lifecycle_starts_with_empty_plugin_list() {
        let State(state) = test_state();

        let plugins = state.lifecycle.list_plugins().await;
        assert!(plugins.is_empty());

        let order = state.lifecycle.loading_order().await;
        assert!(order.is_empty());

        let active = state.lifecycle.active_count().await;
        assert_eq!(active, 0);
    }

    #[tokio::test]
    async fn status_summary_empty_with_no_plugins() {
        let State(state) = test_state();
        let summary = state.lifecycle.status_summary().await;

        // With no plugins, summary should be an empty map or have all zeros
        let total: usize = summary.values().sum();
        assert_eq!(total, 0);
    }
}
