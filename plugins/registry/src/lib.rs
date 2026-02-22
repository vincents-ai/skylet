// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Registry Plugin - V2 ABI Implementation
//!
//! This plugin provides federated plugin registry operations using RFC-0004 v2 ABI.
//! Now migrated to use skylet_plugin_v2! macro for boilerplate elimination.
//!
//! Uses skylet-plugin-common for:
//! - RFC-0006 compliant config paths
//! - skylet_plugin_v2! macro for V2 ABI boilerplate elimination
//! - Common HTTP client utilities

#![allow(unused_imports)]

use skylet_abi::v2_spec::*;
use skylet_abi::{DependencyInfo, MaturityLevel, MonetizationModel, PluginCategory};
use skylet_plugin_common::config_paths;
use std::ffi::CString;
use std::ptr;

// Use the V2 ABI macro to generate all boilerplate entry points
skylet_plugin_common::skylet_plugin_v2! {
    name: "registry",
    version: "0.1.0",
    description: "Federated Plugin Registry Service (v2)",
    author: "Skylet",
    license: "MIT OR Apache-2.0",
    tagline: "Federated plugin registry",
    category: skylet_abi::PluginCategory::Utility,
    max_concurrency: 10,
    supports_async: true,
    capabilities: ["registry.list", "registry.get", "registry.publish"],
}

// ============================================================================
// Plugin-specific Business Logic
// ============================================================================

/// RPC handler for registry operations (v2 ABI)
///
/// This handler is registered separately since the macro generates a default
/// plugin_init_v2 that doesn't include custom RPC registration.
///
/// For plugins that need custom init logic, consider using the macro for
/// the boilerplate and adding a separate initialization function.
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

/// Register RPC handlers for the registry plugin
///
/// Call this function after plugin initialization to set up RPC handlers.
/// This is kept separate from the macro-generated init since RPC registration
/// requires access to the plugin context.
#[allow(dead_code)]
pub fn register_rpc_handlers(context: *const PluginContextV2) {
    if context.is_null() {
        return;
    }

    unsafe {
        if !(*context).rpc_service.is_null() {
            let rpc = &*(*context).rpc_service;
            let method = CString::new("registry.info").unwrap();
            (rpc.register_handler)(context, method.as_ptr(), registry_rpc_handler);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_info_v2() {
        let info = plugin_get_info_v2();
        assert!(!info.is_null());

        unsafe {
            assert!(!(*info).name.is_null());
            assert!(!(*info).version.is_null());
        }
    }

    #[test]
    fn test_plugin_init_v2() {
        // Verify it returns InvalidRequest for null context
        let result = plugin_init_v2(ptr::null());
        assert_eq!(result, PluginResultV2::InvalidRequest);
    }

    #[test]
    fn test_capability_query() {
        // Test that capabilities are properly returned
        let cap = CString::new("registry.list").unwrap();
        let result = plugin_query_capability_v2(ptr::null(), cap.as_ptr());
        assert!(result);

        let cap2 = CString::new("registry.get").unwrap();
        let result2 = plugin_query_capability_v2(ptr::null(), cap2.as_ptr());
        assert!(result2);

        let invalid_cap = CString::new("invalid.capability").unwrap();
        let result3 = plugin_query_capability_v2(ptr::null(), invalid_cap.as_ptr());
        assert!(!result3);
    }
}
