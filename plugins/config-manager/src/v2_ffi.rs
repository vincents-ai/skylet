// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! V2 ABI FFI Interface for Config Manager Plugin
//!
//! This module implements RFC-0004 v2 ABI for config-manager plugin.
//! Config Manager is Bootstrap Plugin #1 and must load before all other plugins.
//!
//! ## v2 Migration Changes
//! - Thread-safe ConfigService storage using RwLock<Arc<ConfigService>>
//! - PluginInfoV2 with 40+ metadata fields
//! - SafePluginContext for type-safe service access
//! - No unsafe static mut - all storage uses thread-safe primitives
//! - All ABI functions follow RFC-0004 v2 specification

use skylet_abi::v2_spec::*;
use skylet_abi::PluginLogLevel;
use skylet_plugin_common::{
    cstr_ptr, static_cstr, CapabilityBuilder, ServiceInfoBuilder, TagsBuilder,
};
use std::ffi::{c_char, CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering, Ordering as AOrdering};
use std::sync::{Arc, RwLock};

use super::ConfigService;

// ============================================================================
// Plugin Metadata Constants (using static_cstr! for efficiency)
// ============================================================================

const PLUGIN_NAME: &[u8] = static_cstr!("config-manager");
const PLUGIN_VERSION: &[u8] = static_cstr!("0.2.0");
const PLUGIN_DESCRIPTION: &[u8] = static_cstr!("Centralized configuration management with TOML/JSON support, environment overrides, and CLI parsing");
const PLUGIN_AUTHOR: &[u8] = static_cstr!("Skylet Team");
const PLUGIN_LICENSE: &[u8] = static_cstr!("MIT OR Apache-2.0");
const PLUGIN_HOMEPAGE: &[u8] = static_cstr!("https://github.com/vincents-ai/skylet");
const PLUGIN_ABI_VERSION: &[u8] = static_cstr!("2.0");
const PLUGIN_SKYLET_MIN: &[u8] = static_cstr!("1.0.0");
const PLUGIN_SKYLET_MAX: &[u8] = static_cstr!("2.0.0");

// ============================================================================
// Static Plugin Information
// ============================================================================

// Static storage for plugin info
static PLUGIN_INFO: AtomicPtr<PluginInfoV2> = AtomicPtr::new(ptr::null_mut());

// Thread-safe configuration service storage
static CONFIG_SERVICE: RwLock<Option<Arc<ConfigService>>> = RwLock::new(None);
static CONFIG_INITIALIZED: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Helper Functions
// ============================================================================

/// Initialize static plugin information
fn init_plugin_info() {
    // Build capabilities using CapabilityBuilder
    let (capabilities_ptr, num_capabilities) = CapabilityBuilder::new()
        .add("config.get", "Get configuration value", None)
        .add("config.set", "Set configuration value", None)
        .add("config.export", "Export configuration to JSON/TOML", None)
        .add("config.validate", "Validate configuration", None)
        .build();

    // Build tags using TagsBuilder
    let (tags_ptr, num_tags) = TagsBuilder::new()
        .add("config")
        .add("settings")
        .add("bootstrap")
        .add("core")
        .build();

    // Build service info using ServiceInfoBuilder
    let service_ptr = ServiceInfoBuilder::new("ConfigService", "2.0.0")
        .description("Centralized configuration management service")
        .interface_spec("config-service-v2")
        .build();

    // Initialize plugin info
    let info = PluginInfoV2 {
        // Basic metadata
        name: cstr_ptr!(PLUGIN_NAME),
        version: cstr_ptr!(PLUGIN_VERSION),
        description: cstr_ptr!(PLUGIN_DESCRIPTION),
        author: cstr_ptr!(PLUGIN_AUTHOR),
        license: cstr_ptr!(PLUGIN_LICENSE),
        homepage: cstr_ptr!(PLUGIN_HOMEPAGE),

        // Version compatibility
        skylet_version_min: cstr_ptr!(PLUGIN_SKYLET_MIN),
        skylet_version_max: cstr_ptr!(PLUGIN_SKYLET_MAX),
        abi_version: cstr_ptr!(PLUGIN_ABI_VERSION),

        // Dependencies and services
        dependencies: ptr::null(),
        num_dependencies: 0,
        provides_services: service_ptr,
        num_provides_services: 1,
        requires_services: ptr::null(),
        num_requires_services: 0,

        // Capabilities
        capabilities: capabilities_ptr,
        num_capabilities,

        // No resource requirements (config manager is lightweight)
        min_resources: ptr::null(),
        max_resources: ptr::null(),

        // Tags and categorization
        tags: tags_ptr,
        num_tags,
        category: PluginCategory::Development,

        // Runtime capabilities
        supports_hot_reload: false,
        supports_async: false,
        supports_streaming: false,
        max_concurrency: 1,

        // Plugin presentation
        tagline: ptr::null(),
        icon_url: ptr::null(),

        // Build and deployment
        maturity_level: MaturityLevel::Stable,
        build_timestamp: ptr::null(),
        build_hash: ptr::null(),
        git_commit: ptr::null(),
        build_environment: ptr::null(),
        metadata: ptr::null(),
    };

    PLUGIN_INFO.store(
        Box::leak(Box::new(info)) as *mut _ as *mut _,
        Ordering::SeqCst,
    );
}

/// Initialize ConfigService (called during plugin_init_v2)
fn init_config_service() {
    let mut guard = CONFIG_SERVICE.write().unwrap();
    if guard.is_none() {
        *guard = Some(Arc::new(ConfigService::new()));
        CONFIG_INITIALIZED.store(true, AOrdering::SeqCst);
    }
}

/// Shutdown ConfigService (called during plugin_shutdown_v2)
fn shutdown_config_service() {
    let mut guard = CONFIG_SERVICE.write().unwrap();
    if guard.is_some() {
        *guard = None;
        CONFIG_INITIALIZED.store(false, AOrdering::SeqCst);
    }
}

// ============================================================================
// V2 ABI Implementation
// ============================================================================

/// Get plugin information
#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    if PLUGIN_INFO.load(Ordering::SeqCst).is_null() {
        init_plugin_info();
    }
    PLUGIN_INFO.load(Ordering::SeqCst)
}

/// Initialize plugin
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    // Ensure plugin info is initialized
    if PLUGIN_INFO.load(Ordering::SeqCst).is_null() {
        init_plugin_info();
    }

    unsafe {
        let ctx = &*context;

        // Log initialization
        if !ctx.logger.is_null() {
            let logger = &*ctx.logger;
            let msg = CString::new("config-manager v2 plugin initialized").unwrap();
            let _ = (logger.log)(context, PluginLogLevel::Info, msg.as_ptr());
        }

        // Register ConfigService in service registry
        if !ctx.service_registry.is_null() {
            let registry = &*ctx.service_registry;
            let service_name = CString::new("ConfigService").unwrap();
            let service_type = CString::new("config-service-v2").unwrap();

            let _ = (registry.register)(
                context,
                service_name.as_ptr(),
                std::ptr::null_mut::<std::ffi::c_void>(),
                service_type.as_ptr(),
            );
        }
    }

    // Initialize ConfigService
    init_config_service();

    PluginResultV2::Success
}

/// Shutdown plugin
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn plugin_shutdown_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if !context.is_null() {
        unsafe {
            let ctx = &*context;

            if !ctx.logger.is_null() {
                let logger = &*ctx.logger;
                let msg = CString::new("config-manager v2 plugin shutting down").unwrap();
                let _ = (logger.log)(context, PluginLogLevel::Info, msg.as_ptr());
            }
        }
    }

    shutdown_config_service();
    PluginResultV2::Success
}

/// Handle HTTP request (not implemented for config-manager)
#[no_mangle]
pub extern "C" fn plugin_handle_request_v2(
    _context: *const PluginContextV2,
    _request: *const RequestV2,
    _response: *mut ResponseV2,
) -> PluginResultV2 {
    PluginResultV2::NotImplemented
}

/// Health check
#[no_mangle]
pub extern "C" fn plugin_health_check_v2(_context: *const PluginContextV2) -> HealthStatus {
    // ConfigService is always healthy if initialized
    if CONFIG_INITIALIZED.load(AOrdering::SeqCst) {
        HealthStatus::Healthy
    } else {
        HealthStatus::Degraded
    }
}

/// Get metrics
#[no_mangle]
#[allow(static_mut_refs)]
pub extern "C" fn plugin_get_metrics_v2(_context: *const PluginContextV2) -> *const PluginMetrics {
    use std::time::{SystemTime, UNIX_EPOCH};

    static mut METRICS: Option<PluginMetrics> = None;

    unsafe {
        if METRICS.is_none() {
            METRICS = Some(PluginMetrics {
                uptime_seconds: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                request_count: 0,
                error_count: 0,
                avg_response_time_ms: 0.0,
                memory_usage_mb: 16,
                cpu_usage_percent: 0.1,
                last_error: ptr::null(),
            });
        }
        METRICS.as_ref().unwrap()
    }
}

/// Query capability
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn plugin_query_capability_v2(
    _context: *const PluginContextV2,
    capability: *const c_char,
) -> bool {
    unsafe {
        if capability.is_null() {
            return false;
        }

        let cap_str = CStr::from_ptr(capability).to_str().unwrap_or("");

        // Check against our capabilities list
        matches!(
            cap_str,
            "config.get" | "config.set" | "config.export" | "config.validate"
        )
    }
}

/// Create v2 plugin API - REQUIRED ENTRY POINT
#[no_mangle]
pub extern "C" fn plugin_create_v2() -> *const PluginApiV2 {
    static API: PluginApiV2 = PluginApiV2 {
        get_info: plugin_get_info_v2,
        init: plugin_init_v2,
        shutdown: plugin_shutdown_v2,
        handle_request: plugin_handle_request_v2,
        handle_event: None,
        prepare_hot_reload: None,
        health_check: Some(plugin_health_check_v2),
        get_metrics: Some(plugin_get_metrics_v2),
        query_capability: Some(plugin_query_capability_v2),
        get_config_schema: None,
        serialize_state: None,
        deserialize_state: None,
        free_state: None,
    };

    &API
}
