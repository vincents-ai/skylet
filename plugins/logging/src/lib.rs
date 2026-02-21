// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Logging Plugin - Structured logging backend for Skylet (V2 ABI)
//!
//! This plugin provides structured JSON logging with RFC-0018 compliance.
//! Now migrated to RFC-0004 v2 ABI.

#![allow(dead_code, unused_imports, unused_variables)]

use skylet_abi::v2_spec::*;
use skylet_abi::{
    DependencyInfo, MaturityLevel, MonetizationModel, PluginCategory, PluginLogLevel,
};
use std::ffi::{c_char, CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::sync::Mutex;

use chrono::Utc;
use serde_json::{json, Value};

// Static storage
static PLUGIN_INFO: AtomicPtr<PluginInfoV2> = AtomicPtr::new(ptr::null_mut());
static DEPENDENCIES: AtomicPtr<DependencyInfo> = AtomicPtr::new(ptr::null_mut());
static LOGGING_SERVICE: Mutex<LoggingService> = Mutex::new(LoggingService::new());
static INITIALIZED: AtomicU64 = AtomicU64::new(0);
static CALL_COUNT: AtomicU64 = AtomicU64::new(0);

/// LoggingService for managing structured logging
pub struct LoggingService {
    log_level: tracing::Level,
    event_buffer: Vec<String>,
}

impl LoggingService {
    pub const fn new() -> Self {
        Self {
            log_level: tracing::Level::INFO,
            event_buffer: Vec::new(),
        }
    }

    pub fn get_level(&self) -> tracing::Level {
        self.log_level
    }

    pub fn set_level(&mut self, level: tracing::Level) {
        self.log_level = level;
    }

    pub fn get_events(&self) -> Vec<String> {
        self.event_buffer.clone()
    }

    pub fn add_event(&mut self, event: String) {
        self.event_buffer.push(event);
        if self.event_buffer.len() > 1000 {
            self.event_buffer.remove(0);
        }
    }

    pub fn clear_events(&mut self) {
        self.event_buffer.clear();
    }
}

impl Default for LoggingService {
    fn default() -> Self {
        Self::new()
    }
}

/// Create structured log event in RFC-0018 format
fn create_log_event(level: &str, message: &str, plugin_name: Option<&str>) -> String {
    let mut map = serde_json::Map::new();
    map.insert(
        "timestamp".to_string(),
        Value::String(Utc::now().to_rfc3339()),
    );
    map.insert("level".to_string(), Value::String(level.to_string()));
    map.insert("message".to_string(), Value::String(message.to_string()));
    if let Some(pn) = plugin_name {
        map.insert("plugin_name".to_string(), Value::String(pn.to_string()));
    }
    serde_json::to_string(&Value::Object(map)).unwrap_or_default()
}

/// V2 entry points
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    match LOGGING_SERVICE.lock() {
        Ok(mut svc) => {
            svc.clear_events();
            INITIALIZED.store(1, Ordering::SeqCst);

            // Log initialization
            unsafe {
                if !(*context).logger.is_null() {
                    let logger = &*(*context).logger;
                    let msg = CString::new("Logging plugin initialized (v2)").unwrap();
                    (logger.log)(context, PluginLogLevel::Info, msg.as_ptr());
                }
            }

            PluginResultV2::Success
        }
        Err(_) => PluginResultV2::Error,
    }
}

#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(_context: *const PluginContextV2) -> PluginResultV2 {
    match LOGGING_SERVICE.lock() {
        Ok(mut svc) => {
            svc.clear_events();
        }
        Err(_) => return PluginResultV2::Error,
    }
    INITIALIZED.store(0, Ordering::SeqCst);
    PluginResultV2::Success
}

/// Plugin shutdown entry point (v1 ABI wrapper for bootstrap compatibility)
#[no_mangle]
pub extern "C" fn plugin_shutdown(
    _context: *const skylet_abi::PluginContext,
) -> skylet_abi::PluginResult {
    match LOGGING_SERVICE.lock() {
        Ok(mut svc) => {
            svc.clear_events();
        }
        Err(_) => return skylet_abi::PluginResult::Error,
    }
    INITIALIZED.store(0, Ordering::SeqCst);
    skylet_abi::PluginResult::Success
}

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    if PLUGIN_INFO.load(Ordering::SeqCst).is_null() {
        initialize_plugin_info();
    }
    PLUGIN_INFO.load(Ordering::SeqCst)
}

fn initialize_plugin_info() {
    if DEPENDENCIES.load(Ordering::SeqCst).is_null() {
        let dep = DependencyInfo {
            name: CString::new("skylet-abi").unwrap().into_raw(),
            version_range: CString::new(">=0.2.0").unwrap().into_raw(),
            required: true,
            service_type: CString::new("core").unwrap().into_raw(),
        };
        DEPENDENCIES.store(Box::into_raw(Box::new(dep)), Ordering::SeqCst);
    }

    let info = PluginInfoV2 {
        name: CString::new("logging").unwrap().into_raw(),
        version: CString::new("0.1.0").unwrap().into_raw(),
        description: CString::new("Structured logging backend (v2)")
            .unwrap()
            .into_raw(),
        author: CString::new("Skylet").unwrap().into_raw(),
        license: CString::new("MIT OR Apache-2.0").unwrap().into_raw(),
        homepage: ptr::null(),
        skynet_version_min: CString::new("0.2.0").unwrap().into_raw(),
        skynet_version_max: ptr::null(),
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
        supports_async: false,
        supports_streaming: false,
        max_concurrency: 10,
        monetization_model: MonetizationModel::Free,
        price_usd: 0.0,
        purchase_url: ptr::null(),
        subscription_url: ptr::null(),
        marketplace_category: CString::new("Core").unwrap().into_raw(),
        tagline: CString::new("Structured JSON logging").unwrap().into_raw(),
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

/// Handle request (not implemented for logging plugin)
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

/// Health check
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn plugin_query_capability_v2(
    _context: *const PluginContextV2,
    capability: *const c_char,
) -> bool {
    if capability.is_null() {
        return false;
    }

    unsafe {
        let cap_str = CStr::from_ptr(capability).to_str().unwrap_or("");
        matches!(cap_str, "logging.write" | "logging.read" | "logging.clear")
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
        get_billing_metrics: None,
    };

    &API
}

/// RPC handler for logging operations
extern "C" fn logging_rpc_handler(request: *const RpcRequestV2, response: *mut RpcResponseV2) {
    if response.is_null() {
        return;
    }

    unsafe {
        // Get log stats
        let stats = match LOGGING_SERVICE.lock() {
            Ok(svc) => {
                format!(
                    "{{\"level\":\"{:?}\",\"events\":{}}}",
                    svc.get_level(),
                    svc.get_events().len()
                )
            }
            Err(_) => "{\"error\":\"lock failed\"}".to_string(),
        };

        let result = CString::new(stats).unwrap();
        (*response).result = result.into_raw();
        (*response).error = ptr::null();
        (*response).status = PluginResultV2::Success;
    }

    CALL_COUNT.fetch_add(1, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_info() {
        initialize_plugin_info();
        let info = plugin_get_info_v2();
        assert!(!info.is_null());
        unsafe {
            assert!(!(*info).name.is_null());
            assert_eq!((*info).num_dependencies, 1);
        }
    }

    #[test]
    fn test_logging_service() {
        let mut svc = LoggingService::new();
        svc.add_event("test event".to_string());
        assert_eq!(svc.get_events().len(), 1);
        svc.clear_events();
        assert_eq!(svc.get_events().len(), 0);
    }
}
