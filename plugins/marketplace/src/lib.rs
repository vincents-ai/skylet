// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Marketplace Plugin - V2 ABI Implementation
//!
//! This plugin provides marketplace operations for listings and transactions.
//! It includes its own database migrations separate from the core engine.

use skylet_abi::v2_spec::*;
use skylet_abi::DependencyInfo;
use std::ffi::CString;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

// Static storage for plugin state
static PLUGIN_INFO: AtomicPtr<PluginInfoV2> = AtomicPtr::new(ptr::null_mut());
static DEPENDENCIES: AtomicPtr<DependencyInfo> = AtomicPtr::new(ptr::null_mut());

/// SQL migrations for the marketplace plugin.
/// These are separate from core engine migrations.
pub const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/2_create_listings.sql"),
    include_str!("../migrations/3_create_transactions.sql"),
];

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
            let msg = CString::new("Marketplace plugin initialized (v2)").unwrap();
            (logger.log)(context, skylet_abi::PluginLogLevel::Info, msg.as_ptr());
        }

        // Register RPC handlers
        if !(*context).rpc_service.is_null() {
            let rpc = &*(*context).rpc_service;

            let method = CString::new("marketplace.list_listings").unwrap();
            (rpc.register_handler)(context, method.as_ptr(), marketplace_rpc_handler);

            let method = CString::new("marketplace.create_listing").unwrap();
            (rpc.register_handler)(context, method.as_ptr(), marketplace_rpc_handler);

            let method = CString::new("marketplace.get_transaction").unwrap();
            (rpc.register_handler)(context, method.as_ptr(), marketplace_rpc_handler);
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
        name: CString::new("marketplace").unwrap().into_raw(),
        version: CString::new("0.1.0").unwrap().into_raw(),
        description: CString::new("Marketplace for listings and transactions (v2)")
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
        category: PluginCategory::Payment, // Using Payment as closest to Marketplace
        supports_hot_reload: false,
        supports_async: true,
        supports_streaming: false,
        max_concurrency: 10,
        monetization_model: MonetizationModel::Free,
        price_usd: 0.0,
        purchase_url: ptr::null(),
        subscription_url: ptr::null(),
        marketplace_category: CString::new("Commerce").unwrap().into_raw(),
        tagline: CString::new("Listings and transactions marketplace")
            .unwrap()
            .into_raw(),
        icon_url: ptr::null(),
        maturity_level: MaturityLevel::Alpha,
        build_timestamp: ptr::null(),
        build_hash: ptr::null(),
        git_commit: ptr::null(),
        build_environment: ptr::null(),
        metadata: ptr::null(),
    };

    PLUGIN_INFO.store(Box::into_raw(Box::new(info)), Ordering::SeqCst);
}

/// Handle request (not implemented for marketplace plugin yet)
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

/// RPC handler for marketplace operations
extern "C" fn marketplace_rpc_handler(_request: *const RpcRequestV2, response: *mut RpcResponseV2) {
    if response.is_null() {
        return;
    }

    // Create a placeholder response
    let resp_json = r#"{"status": "ok", "message": "marketplace handler"}"#;
    let resp = CString::new(resp_json).unwrap();

    unsafe {
        (*response).status = PluginResultV2::Success;
        (*response).result = resp.into_raw();
        (*response).error = ptr::null();
    }
}

/// Run marketplace migrations on the provided database connection
pub fn run_migrations(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    for migration in MIGRATIONS {
        conn.execute_batch(migration)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_included() {
        assert_eq!(MIGRATIONS.len(), 2);
        assert!(MIGRATIONS[0].contains("CREATE TABLE IF NOT EXISTS listings"));
        assert!(MIGRATIONS[1].contains("CREATE TABLE IF NOT EXISTS transactions"));
    }
}
