// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::config::AppConfig; // Use AppConfig only, ConfigArgs from crate::config will be used implicitly for Cli
use anyhow::Result;
use clap::{Parser, Subcommand};
use lazy_static::lazy_static;
use serde_json;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::{Arc, RwLock};
use tracing;

mod config;

#[derive(Parser)]
#[command(name = "skylet")]
#[command(about = "Skylet Plugin Execution Engine")]
struct Cli {
    #[command(flatten)]
    config_args: crate::config::ConfigArgs, // Explicitly use ConfigArgs from crate::config
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the server
    Server,
    /// Migrate the source database
    MigrateSource,
    /// Migrate the target database
    MigrateTarget,
    /// Run maintenance tasks
    Maintenance,
    /// Load and run multiple plugins
    Run {
        #[arg(short, long)]
        plugins: Vec<String>,
    },
}

struct ServicePtr(*mut c_void);
// SAFETY: ServicePtr wraps a raw pointer but is only accessed behind a RwLock
// in the SERVICES static, ensuring exclusive mutable access is properly synchronized.
unsafe impl Send for ServicePtr {}
unsafe impl Sync for ServicePtr {}

lazy_static! {
    static ref SERVICES: Arc<RwLock<HashMap<String, ServicePtr>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

extern "C" fn host_log(
    _ctx: *const skylet_abi::PluginContext,
    level: skylet_abi::PluginLogLevel,
    message: *const c_char,
) {
    let msg = unsafe { CStr::from_ptr(message).to_string_lossy() };
    tracing::info!("[Plugin {:?}] {}", level, msg);
}

extern "C" fn host_log_structured(
    _ctx: *const skylet_abi::PluginContext,
    level: skylet_abi::PluginLogLevel,
    message: *const c_char,
    data_json: *const c_char,
) {
    let msg = unsafe { CStr::from_ptr(message).to_string_lossy() };
    let data = unsafe { CStr::from_ptr(data_json).to_string_lossy() };
    tracing::info!("[Plugin {:?}] {} | Data: {}", level, msg, data);
}

static LOGGER: skylet_abi::PluginLogger = skylet_abi::PluginLogger {
    log: host_log,
    log_structured: host_log_structured,
};

extern "C" fn host_register(
    _ctx: *const skylet_abi::PluginContext,
    name: *const c_char,
    service: *mut c_void,
    _service_type: *const c_char,
) -> skylet_abi::PluginResult {
    let name_str = unsafe { CStr::from_ptr(name).to_string_lossy().into_owned() };
    tracing::info!("Host: Registered service '{}'", name_str);
    let mut map = SERVICES.write().unwrap();
    map.insert(name_str, ServicePtr(service));
    skylet_abi::PluginResult::Success
}

extern "C" fn host_get(
    _ctx: *const skylet_abi::PluginContext,
    name: *const c_char,
    _service_type: *const c_char,
) -> *mut c_void {
    let name_str = unsafe { CStr::from_ptr(name).to_string_lossy().into_owned() };
    let map = SERVICES.read().unwrap();
    map.get(&name_str)
        .map(|s| s.0)
        .unwrap_or(std::ptr::null_mut())
}

extern "C" fn host_unregister(
    _ctx: *const skylet_abi::PluginContext,
    name: *const c_char,
) -> skylet_abi::PluginResult {
    let name_str = unsafe { CStr::from_ptr(name).to_string_lossy().into_owned() };
    let mut map = SERVICES.write().unwrap();
    map.remove(&name_str);
    skylet_abi::PluginResult::Success
}

static REGISTRY: skylet_abi::PluginServiceRegistry = skylet_abi::PluginServiceRegistry {
    register: host_register,
    get: host_get,
    unregister: host_unregister,
};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = AppConfig::load(&cli.config_args)?;
    config.create_directories()?;

    let config_json_c_string = CString::new(serde_json::to_string(&config)?).unwrap();
    let config_ptr = config_json_c_string.as_ptr();
    std::mem::forget(config_json_c_string); // Plugin will hold onto this pointer

    let context = skylet_abi::PluginContext {
        logger: &LOGGER,
        config: std::ptr::null(), // No longer using this field for config
        service_registry: &REGISTRY,
        tracer: std::ptr::null(),
        user_data: std::ptr::null_mut(),
        user_context_json: config_ptr, // Pass the host's config to the plugin via user_context_json
        secrets: std::ptr::null(),
    };

    match cli.command {
        Commands::Server => {
            tracing::info!("Running server...");
        }
        Commands::MigrateSource => {
            tracing::info!("Migrating source database...");
        }
        Commands::MigrateTarget => {
            tracing::info!("Migrating target database...");
        }
        Commands::Maintenance => {
            tracing::info!("Running maintenance tasks...");
        }
        Commands::Run { plugins } => {
            if plugins.is_empty() {
                tracing::info!("No plugins specified.");
                return Ok(());
            }
            tracing::info!("Loading plugins: {:?}", plugins);

            let mut loaded_plugins = Vec::new();

            for path in plugins {
                tracing::info!("Loading plugin from {}...", path);
                let plugin = unsafe { skylet_abi::Plugin::load(&path, &context)? };
                tracing::info!("Initializing plugin...");
                unsafe {
                    plugin.init(&context);
                }
                loaded_plugins.push(plugin);
            }

            tracing::info!("All plugins loaded. Press Ctrl+C to stop.");
            // Keep the main runtime alive to allow background plugin tasks to run
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;

            tokio::signal::ctrl_c().await?;
            tracing::info!("Shutting down...");
            for plugin in loaded_plugins {
                unsafe {
                    plugin.shutdown(&context);
                }
            }
        }
    }

    Ok(())
}
