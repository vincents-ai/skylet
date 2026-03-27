// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::Parser;
mod bootstrap;
mod config;
mod logging;
mod plugin_manager;

use crate::config::AppConfig;
use anyhow::Result;
use axum::{extract::{State, Path}, http::StatusCode, response::Json, routing::{get, post}, Router, body::Bytes};
use serde::{Deserialize, Serialize};
use serde_json::json;

use bootstrap::{load_bootstrap_plugins, shutdown_bootstrap_plugins, BootstrapContext};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

// GAP-003: Import auth HTTP handlers from permissions crate
use skylet_permissions::http::{auth_router, AuthState};

// CQ-003: Import dynamic plugin discovery
use plugin_manager::discovery::{DiscoveryConfig, PluginDiscovery};

// Wire PluginManager for application plugins (provides real FFI services)
use plugin_manager::manager::PluginManager;

// CQ-004: Plugin dependency resolution for load ordering
use plugin_manager::dependency_resolver::PluginDependencyResolver;

// HR-008: Import hot reload service
use plugin_manager::hot_reload::{HotReloadConfig, HotReloadService};

// RPC global registry for HTTP endpoint
use plugin_manager::rpc_global::get_global_rpc_registry;

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

async fn ready_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let plugins = state.plugins.read().await;
    let healthy_count = plugins.iter().filter(|p| p.status == "healthy").count();
    let total_count = plugins.len();

    let is_ready = healthy_count > 0;

    if is_ready {
        Ok(Json(json!({
            "status": "ready",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "plugins": {
                "total": total_count,
                "healthy": healthy_count
            }
        })))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

async fn rpc_handler(
    Path(service): Path<String>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let registry = get_global_rpc_registry();
    let params = body.to_vec();
    
    match registry.call(&service, &params) {
        Ok(result_bytes) => {
            let result_str = String::from_utf8_lossy(&result_bytes);
            match serde_json::from_str::<serde_json::Value>(&result_str) {
                Ok(json_val) => Ok(Json(json_val)),
                Err(_) => Ok(Json(json!({
                    "success": true,
                    "result": result_str
                })))
            }
        }
        Err(skylet_abi::v2_spec::PluginResultV2::ServiceUnavailable) => {
            Err((StatusCode::NOT_FOUND, Json(json!({
                "success": false,
                "error": format!("Service '{}' not found", service)
            }))))
        }
        Err(e) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "success": false,
                "error": format!("RPC error: {:?}", e)
            }))))
        }
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
    let app_plugins = match discovery.discover_plugins() {
        Ok(plugins) => {
            info!("Discovered {} application plugins", plugins.len());
            plugins
        }
        Err(e) => {
            warn!("Plugin discovery failed, using empty list: {}", e);
            vec![]
        }
    };

    // CQ-004: Resolve plugin loading order via dependency graph
    // Probe each discovered plugin for its declared dependencies, then
    // topologically sort so that dependencies load before dependents.
    let ordered_plugins = {
        use skylet_abi::AbiV2PluginLoader;
        use std::collections::HashMap;

        let mut resolver = PluginDependencyResolver::new();
        let mut plugin_map: HashMap<String, &plugin_manager::discovery::DiscoveredPlugin> =
            HashMap::new();

        for discovered in &app_plugins {
            plugin_map.insert(discovered.name.clone(), discovered);

            // Probe plugin library once for both dependency metadata and version
            let (deps, version) = match AbiV2PluginLoader::load(&discovered.path) {
                Ok(loader) => {
                    let deps: Vec<String> = match loader.get_dependencies() {
                        Ok(entries) => entries
                            .into_iter()
                            .filter(|d| d.required)
                            .map(|d| match d.version_range {
                                Some(ver) => format!("{}@{}", d.name, ver),
                                None => d.name,
                            })
                            .collect(),
                        Err(e) => {
                            warn!(
                                "Could not read dependencies for plugin '{}': {}",
                                discovered.name, e
                            );
                            vec![]
                        }
                    };
                    let version = loader.get_info().ok().map(|m| m.version);
                    (deps, version)
                }
                Err(e) => {
                    warn!(
                        "Could not probe plugin '{}' for dependencies: {}",
                        discovered.name, e
                    );
                    (vec![], None)
                }
            };

            resolver.register_plugin(
                &discovered.name,
                &discovered.abi_version,
                deps,
                version.as_deref(),
            );
        }

        match resolver.resolve_loading_order() {
            Ok(order) => {
                // resolve_loading_order() returns dependencies-first order (via
                // Kahn's algorithm starting from nodes with zero dependencies).
                // This is already the correct loading order: each plugin's
                // prerequisites are loaded before the plugin itself.

                info!(
                    "Resolved plugin loading order: {:?}",
                    order.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
                );
                // Map back to DiscoveredPlugin references in resolved order
                order
                    .into_iter()
                    .filter_map(|(name, _)| plugin_map.remove(&name).cloned())
                    .collect::<Vec<_>>()
            }
            Err(e) => {
                warn!(
                    "Dependency resolution failed, loading in discovery order: {}",
                    e
                );
                app_plugins.clone()
            }
        }
    };

    // Load application plugins via PluginManager (provides real FFI services:
    // logging, config, event bus, RPC, tracing, secrets, HTTP routing)
    let plugin_manager = Arc::new(PluginManager::new());

    // Re-initialize bootstrap plugins with full PluginManager context
    // This allows them to register services in the shared service registry
    info!("Re-initializing bootstrap plugins with full context...");
    for plugin_name in bootstrap_context.loaded_plugin_names() {
        if let Some(library) = bootstrap_context.get_loaded_library(&plugin_name) {
            match plugin_manager.init_bootstrap_plugin(&plugin_name, library).await {
                Ok(_) => info!("Re-initialized bootstrap plugin '{}' with full context", plugin_name),
                Err(e) => warn!("Failed to re-initialize bootstrap plugin '{}': {}", plugin_name, e),
            }
        }
    }

    for discovered in &ordered_plugins {
        // Skip bootstrap plugins that were already loaded and re-initialized
        if bootstrap_context.loaded_plugin_names().contains(&discovered.name) {
            info!("Skipping '{}' - already loaded as bootstrap plugin", discovered.name);
            continue;
        }
        match plugin_manager
            .load_plugin_instance_v2(&discovered.name, &discovered.path)
            .await
        {
            Ok(_) => {
                info!("Loaded application plugin: {}", discovered.name);
                app_state
                    .add_plugin(&discovered.name, "healthy", &discovered.abi_version)
                    .await;
            }
            Err(e) => {
                warn!(
                    "Failed to load application plugin '{}': {}",
                    discovered.name, e
                );
                app_state
                    .add_plugin(&discovered.name, "failed", &discovered.abi_version)
                    .await;
            }
        }
    }

    // HR-008: Initialize and start hot reload service
    info!("Initializing hot reload service...");
    let hot_reload_config = HotReloadConfig {
        enabled: config.plugins.hot_reload_enabled,
        auto_reload: config.plugins.hot_reload_auto_reload,
        debounce_ms: config.plugins.hot_reload_debounce_ms,
        watch_patterns: config.plugins.hot_reload_watch_patterns.clone(),
        exclude_dirs: config.plugins.hot_reload_exclude_dirs.clone(),
        ..Default::default()
    };
    
    // Create lifecycle manager from plugins directory
    let lifecycle_config = plugin_manager::lifecycle::LifecycleConfig::new(
        config.plugins.directory.clone()
    );
    let lifecycle_manager: Option<Arc<plugin_manager::lifecycle::PluginLifecycleManager>> = 
        match plugin_manager::lifecycle::PluginLifecycleManager::new(lifecycle_config) {
        Ok(manager) => Some(Arc::new(manager)),
        Err(e) => {
            warn!("Failed to create lifecycle manager: {}, hot reload disabled", e);
            None
        }
    };
    
    let hot_reload_service: Option<Arc<HotReloadService>> = if let Some(ref lifecycle_mgr) = lifecycle_manager {
        // HR-ARCH-1: Wire PluginManager to lifecycle_manager
        lifecycle_mgr.set_plugin_manager(Arc::clone(&plugin_manager)).await;
        
        // Register already-loaded plugins for hot-reload tracking
        for plugin_name in plugin_manager.list_plugins().await.unwrap_or_default() {
            let plugin_path = config.plugins.directory.join(&plugin_name);
            if let Err(e) = lifecycle_mgr.register_loaded_plugin(&plugin_name, plugin_path).await {
                warn!("Failed to register loaded plugin '{}' for hot-reload: {}", plugin_name, e);
            }
        }
        
        let service = Arc::new(HotReloadService::new(hot_reload_config, Arc::clone(lifecycle_mgr)));
        match service.start(&config.plugins.directory).await {
            Ok(_) => {
                info!("Hot reload service started, watching: {:?}", config.plugins.directory);
                Some(service)
            }
            Err(e) => {
                warn!("Failed to start hot reload service: {}", e);
                None
            }
        }
    } else {
        None
    };
    
    // Store hot reload service in app state for later use
    // For now, just log the status
    if hot_reload_service.is_some() {
        info!("Hot reload service is active");
    }

    // Simplified server startup, relying on plugins for networking
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/rpc/:service", post(rpc_handler))
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

    // Shutdown application plugins before bootstrap
    info!("Shutting down application plugins...");
    plugin_manager.shutdown_all().await;

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
