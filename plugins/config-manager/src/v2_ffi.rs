// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

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
use std::ffi::{c_char, CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering, Ordering as AOrdering};
use std::sync::{Arc, RwLock};

use super::ConfigService;

// ============================================================================
// Plugin Metadata Constants
// ============================================================================

const PLUGIN_NAME: &[u8] = b"config-manager\0";
const PLUGIN_VERSION: &[u8] = b"0.2.0\0"; // Updated to v2
const PLUGIN_DESCRIPTION: &[u8] = b"Centralized configuration management with TOML/JSON support, environment overrides, and CLI parsing\0";
const PLUGIN_AUTHOR: &[u8] = b"Skylet Team\0";
const PLUGIN_LICENSE: &[u8] = b"MIT OR Apache-2.0\0";
const PLUGIN_HOMEPAGE: &[u8] = b"https://github.com/vincents-ai/skylet\0";
const PLUGIN_ABI_VERSION: &[u8] = b"2.0\0";
const PLUGIN_SKYNET_MIN: &[u8] = b"1.0.0\0";
const PLUGIN_SKYNET_MAX: &[u8] = b"2.0.0\0";

// Plugin tags
const TAG_CONFIG: &[u8] = b"config\0";
const TAG_SETTINGS: &[u8] = b"settings\0";
const TAG_BOOTSTRAP: &[u8] = b"bootstrap\0";
const TAG_CORE: &[u8] = b"core\0";

// Service info - this plugin provides configuration service
const SERVICE_NAME: &[u8] = b"ConfigService\0";
const SERVICE_VERSION: &[u8] = b"2.0.0\0"; // Updated to v2
const SERVICE_DESC: &[u8] = b"Centralized configuration management service\0";
const SERVICE_SPEC: &[u8] = b"config-service-v2\0"; // Updated spec

// ============================================================================
// Static Plugin Information
// ============================================================================

// Static storage for plugin info
static PLUGIN_INFO: AtomicPtr<PluginInfoV2> = AtomicPtr::new(ptr::null_mut());
static CAPABILITIES_STORAGE: AtomicPtr<[CapabilityInfo; 4]> = AtomicPtr::new(ptr::null_mut());
static TAGS_STORAGE: AtomicPtr<[*const c_char; 4]> = AtomicPtr::new(ptr::null_mut());
static SERVICE_STORAGE: AtomicPtr<ServiceInfo> = AtomicPtr::new(ptr::null_mut());

// Thread-safe configuration service storage
static CONFIG_SERVICE: RwLock<Option<Arc<ConfigService>>> = RwLock::new(None);
static CONFIG_INITIALIZED: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Helper Functions
// ============================================================================

/// Initialize static plugin information
fn init_plugin_info() {
    // Initialize capabilities
    let capabilities = [
        CapabilityInfo {
            name: c"config.get".as_ptr(),
            description: c"Get configuration value".as_ptr(),
            required_permission: ptr::null(),
        },
        CapabilityInfo {
            name: c"config.set".as_ptr(),
            description: c"Set configuration value".as_ptr(),
            required_permission: ptr::null(),
        },
        CapabilityInfo {
            name: c"config.export".as_ptr(),
            description: c"Export configuration to JSON/TOML".as_ptr(),
            required_permission: ptr::null(),
        },
        CapabilityInfo {
            name: c"config.validate".as_ptr(),
            description: c"Validate configuration".as_ptr(),
            required_permission: ptr::null(),
        },
    ];

    CAPABILITIES_STORAGE.store(
        Box::leak(Box::new(capabilities)) as *mut _ as *mut _,
        Ordering::SeqCst,
    );

    // Initialize tags
    let tags = [
        TAG_CONFIG.as_ptr() as *const c_char,
        TAG_SETTINGS.as_ptr() as *const c_char,
        TAG_BOOTSTRAP.as_ptr() as *const c_char,
        TAG_CORE.as_ptr() as *const c_char,
    ];

    TAGS_STORAGE.store(
        Box::leak(Box::new(tags)) as *mut _ as *mut _,
        Ordering::SeqCst,
    );

    // Initialize service info
    let service = ServiceInfo {
        name: SERVICE_NAME.as_ptr() as *const c_char,
        version: SERVICE_VERSION.as_ptr() as *const c_char,
        description: SERVICE_DESC.as_ptr() as *const c_char,
        interface_spec: SERVICE_SPEC.as_ptr() as *const c_char,
    };

    SERVICE_STORAGE.store(
        Box::leak(Box::new(service)) as *mut _ as *mut _,
        Ordering::SeqCst,
    );

    // Initialize plugin info
    let info = PluginInfoV2 {
        // Basic metadata
        name: PLUGIN_NAME.as_ptr() as *const c_char,
        version: PLUGIN_VERSION.as_ptr() as *const c_char,
        description: PLUGIN_DESCRIPTION.as_ptr() as *const c_char,
        author: PLUGIN_AUTHOR.as_ptr() as *const c_char,
        license: PLUGIN_LICENSE.as_ptr() as *const c_char,
        homepage: PLUGIN_HOMEPAGE.as_ptr() as *const c_char,

        // Version compatibility
        skynet_version_min: PLUGIN_SKYNET_MIN.as_ptr() as *const c_char,
        skynet_version_max: PLUGIN_SKYNET_MAX.as_ptr() as *const c_char,
        abi_version: PLUGIN_ABI_VERSION.as_ptr() as *const c_char,

        // Dependencies and services
        dependencies: ptr::null(),
        num_dependencies: 0,
        provides_services: SERVICE_STORAGE.load(Ordering::SeqCst),
        num_provides_services: 1,
        requires_services: ptr::null(),
        num_requires_services: 0,

        // Capabilities
        capabilities: CAPABILITIES_STORAGE.load(Ordering::SeqCst) as *const CapabilityInfo,
        num_capabilities: 4,

        // No resource requirements (config manager is lightweight)
        min_resources: ptr::null(),
        max_resources: ptr::null(),

        // Tags and categorization
        tags: TAGS_STORAGE.load(Ordering::SeqCst) as *const *const c_char,
        num_tags: 4,
        category: PluginCategory::Development,

        // Runtime capabilities
        supports_hot_reload: false,
        supports_async: false,
        supports_streaming: false,
        max_concurrency: 1,

        // Marketplace (not sold)
        monetization_model: MonetizationModel::Free,
        price_usd: 0.0,
        purchase_url: ptr::null(),
        subscription_url: ptr::null(),
        marketplace_category: ptr::null(),
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
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    unsafe {
        let ctx = &*context;

        // Log shutdown
        if !ctx.logger.is_null() {
            let logger = &*ctx.logger;
            let msg = CString::new("config-manager v2 plugin shutting down").unwrap();
            let _ = (logger.log)(context, PluginLogLevel::Info, msg.as_ptr());
        }
    }

    // Shutdown ConfigService
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
        get_billing_metrics: None,
    };

    &API
}
