// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Registry Plugin - V2 ABI Implementation
//!
//! This plugin provides federated plugin registry operations using RFC-0004 v2 ABI.
//!
//! Uses skylet-plugin-common for:
//! - RFC-0006 compliant config paths
//! - Common HTTP client utilities

#![allow(unused_imports)]

use skylet_abi::v2_spec::*;
use skylet_abi::{DependencyInfo, MaturityLevel, MonetizationModel, PluginCategory};
use skylet_plugin_common::config_paths;
use std::ffi::CString;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

// Static storage for plugin state
static PLUGIN_INFO: AtomicPtr<PluginInfoV2> = AtomicPtr::new(ptr::null_mut());
static DEPENDENCIES: AtomicPtr<DependencyInfo> = AtomicPtr::new(ptr::null_mut());

/// Plugin initialization entry point (v2 ABI)
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    // Log initialization
    unsafe {
        if !(*context).logger.is_null() {
            let logger = &*(*context).logger;
            let msg = CString::new("Registry plugin initialized (v2)").unwrap();
            (logger.log)(context, skylet_abi::PluginLogLevel::Info, msg.as_ptr());
        }

        // Register RPC handler
        if !(*context).rpc_service.is_null() {
            let rpc = &*(*context).rpc_service;
            let method = CString::new("registry.info").unwrap();
            (rpc.register_handler)(context, method.as_ptr(), registry_rpc_handler);
        }
    }

    PluginResultV2::Success
}

/// Plugin shutdown entry point (v2 ABI)
#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(_context: *const PluginContextV2) -> PluginResultV2 {
    PluginResultV2::Success
}

/// Plugin shutdown entry point (v1 ABI wrapper for bootstrap compatibility)
#[no_mangle]
pub extern "C" fn plugin_shutdown(
    _context: *const skylet_abi::PluginContext,
) -> skylet_abi::PluginResult {
    skylet_abi::PluginResult::Success
}

/// Plugin information entry point (v2 ABI)
#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    if PLUGIN_INFO.load(Ordering::SeqCst).is_null() {
        initialize_plugin_info();
    }
    PLUGIN_INFO.load(Ordering::SeqCst)
}

/// Initialize plugin info and dependencies
fn initialize_plugin_info() {
    // Initialize dependencies
    if DEPENDENCIES.load(Ordering::SeqCst).is_null() {
        let dep = DependencyInfo {
            name: CString::new("skylet-abi").unwrap().into_raw(),
            version_range: CString::new(">=0.2.0").unwrap().into_raw(),
            required: true,
            service_type: CString::new("core").unwrap().into_raw(),
        };
        DEPENDENCIES.store(Box::into_raw(Box::new(dep)), Ordering::SeqCst);
    }

    // Initialize plugin info
    let info = PluginInfoV2 {
        name: CString::new("registry").unwrap().into_raw(),
        version: CString::new("0.1.0").unwrap().into_raw(),
        description: CString::new("Federated Plugin Registry Service (v2)")
            .unwrap()
            .into_raw(),
        author: CString::new("Skylet").unwrap().into_raw(),
        license: CString::new("MIT OR Apache-2.0").unwrap().into_raw(),
        homepage: CString::new("https://github.com/vincents-ai/skylet")
            .unwrap()
            .into_raw(),
        skylet_version_min: CString::new("0.2.0").unwrap().into_raw(),
        skylet_version_max: ptr::null(),
        abi_version: CString::new("2.0").unwrap().into_raw(),
        dependencies: DEPENDENCIES.load(Ordering::SeqCst),
        num_dependencies: 1,
        provides_services: ptr::null(),
        num_provides_services: 0,
        requires_services: ptr::null(),
        num_requires_services: 0,
        capabilities: ptr::null(),
        num_capabilities: 0,
        min_resources: ptr::null(),
        max_resources: ptr::null(),
        tags: ptr::null(),
        num_tags: 0,
        category: PluginCategory::Utility,
        supports_hot_reload: false,
        supports_async: true,
        supports_streaming: false,
        max_concurrency: 10,
        monetization_model: MonetizationModel::Free,
        price_usd: 0.0,
        purchase_url: ptr::null(),
        subscription_url: ptr::null(),
        marketplace_category: CString::new("Core").unwrap().into_raw(),
        tagline: CString::new("Federated plugin registry")
            .unwrap()
            .into_raw(),
        icon_url: ptr::null(),
        maturity_level: MaturityLevel::Stable,
        build_timestamp: ptr::null(),
        build_hash: ptr::null(),
        git_commit: ptr::null(),
        build_environment: ptr::null(),
        metadata: ptr::null(),
    };

    PLUGIN_INFO.store(Box::into_raw(Box::new(info)), Ordering::SeqCst);
}

/// Handle request (not implemented for registry plugin)
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
    HealthStatus::Healthy
}

/// Query capability
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn plugin_query_capability_v2(
    _context: *const PluginContextV2,
    capability: *const std::ffi::c_char,
) -> bool {
    if capability.is_null() {
        return false;
    }

    unsafe {
        let cap_str = std::ffi::CStr::from_ptr(capability).to_str().unwrap_or("");
        matches!(
            cap_str,
            "registry.list" | "registry.get" | "registry.publish"
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
        get_metrics: None, // PluginMetrics contains raw pointers, not Sync-safe
        query_capability: Some(plugin_query_capability_v2),
        get_config_schema: None,
        get_billing_metrics: None, // Registry plugin doesn't require billing
    };

    &API
}

/// RPC handler for registry operations (v2 ABI)
extern "C" fn registry_rpc_handler(_request: *const RpcRequestV2, response: *mut RpcResponseV2) {
    if response.is_null() {
        return;
    }

    unsafe {
        // Return registry info
        let info = r#"{"name":"registry","version":"0.1.0","type":"v2","status":"healthy"}"#;
        let result = CString::new(info).unwrap();
        (*response).result = result.into_raw();
        (*response).error = ptr::null();
        (*response).status = PluginResultV2::Success;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_info_initialization() {
        initialize_plugin_info();
        let info = plugin_get_info_v2();
        assert!(!info.is_null());

        unsafe {
            assert!(!(*info).name.is_null());
            assert!(!(*info).version.is_null());
            assert_eq!((*info).num_dependencies, 1);
        }
    }

    #[test]
    fn test_plugin_init_v2() {
        // Note: This would need a proper PluginContextV2 setup for full testing
        // For now, just verify it returns InvalidRequest for null context
        let result = plugin_init_v2(ptr::null());
        assert_eq!(result, PluginResultV2::InvalidRequest);
    }
}
