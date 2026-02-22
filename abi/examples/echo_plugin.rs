// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

/// Example Echo Plugin - Demonstrates ABI v2 usage
///
/// This example shows how to implement a plugin using the ABI v2 specification.
/// It provides a simple echo service that returns messages back to the caller.
use skylet_abi::v2_spec::*;
use std::ffi::{c_char, CStr};
use std::sync::atomic::{AtomicU64, Ordering};

static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);

static mut INITIALIZED: bool = false;

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    std::ptr::null()
}

#[no_mangle]
pub extern "C" fn plugin_init_v2(_context: *const PluginContextV2) -> PluginResultV2 {
    if unsafe { INITIALIZED } {
        return PluginResultV2::Error;
    }
    unsafe {
        INITIALIZED = true;
    }
    PluginResultV2::Success
}

#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(_context: *const PluginContextV2) -> PluginResultV2 {
    unsafe {
        INITIALIZED = false;
    }
    PluginResultV2::Success
}

#[no_mangle]
pub extern "C" fn plugin_handle_request_v2(
    _context: *const PluginContextV2,
    request: *const RequestV2,
    response: *mut ResponseV2,
) -> PluginResultV2 {
    if request.is_null() || response.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    unsafe {
        REQUEST_COUNT.fetch_add(1, Ordering::SeqCst);

        let req = &*request;

        let message = if !req.body.is_null() && req.body_len > 0 {
            let slice = std::slice::from_raw_parts(req.body, req.body_len);
            String::from_utf8_lossy(slice).to_string()
        } else if !req.query.is_null() {
            CStr::from_ptr(req.query).to_string_lossy().to_string()
        } else {
            "Hello from Echo Plugin!".to_string()
        };

        let resp = &mut *response;
        resp.status_code = 200;
        resp.content_type = b"text/plain\0".as_ptr() as *const c_char;

        let bytes = message.as_bytes();
        resp.body = bytes.as_ptr() as *mut u8;
        resp.body_len = bytes.len();
    }

    PluginResultV2::Success
}

#[no_mangle]
pub extern "C" fn plugin_handle_event_v2(
    _context: *const PluginContextV2,
    event: *const EventV2,
) -> PluginResultV2 {
    if event.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    unsafe {
        let ev = &*event;
        let event_type = CStr::from_ptr(ev.type_).to_string_lossy();

        if event_type == "ping" {
            return PluginResultV2::Success;
        }
    }

    PluginResultV2::Success
}

#[no_mangle]
pub extern "C" fn plugin_health_check_v2(_context: *const PluginContextV2) -> HealthStatus {
    if unsafe { INITIALIZED } {
        HealthStatus::Healthy
    } else {
        HealthStatus::Unhealthy
    }
}

#[no_mangle]
pub extern "C" fn plugin_get_metrics_v2(_context: *const PluginContextV2) -> *const PluginMetrics {
    static mut METRICS: PluginMetrics = PluginMetrics {
        uptime_seconds: 0,
        request_count: 0,
        error_count: 0,
        avg_response_time_ms: 0.0,
        memory_usage_mb: 0,
        cpu_usage_percent: 0.0,
        last_error: std::ptr::null(),
    };

    unsafe {
        METRICS.request_count = REQUEST_COUNT.load(Ordering::SeqCst);
        &raw const METRICS
    }
}

static PLUGIN_API: PluginApiV2 = PluginApiV2 {
    get_info: plugin_get_info_v2,
    init: plugin_init_v2,
    shutdown: plugin_shutdown_v2,
    handle_request: plugin_handle_request_v2,
    handle_event: Some(plugin_handle_event_v2),
    prepare_hot_reload: None,
    health_check: Some(plugin_health_check_v2),
    get_metrics: Some(plugin_get_metrics_v2),
    query_capability: None,
    get_config_schema: None,
    get_billing_metrics: None,
    serialize_state: None,
    deserialize_state: None,
    free_state: None,
};

#[no_mangle]
pub extern "C" fn plugin_create_v2() -> *const PluginApiV2 {
    &PLUGIN_API
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_init() {
        let result = plugin_init_v2(std::ptr::null());
        assert_eq!(result, PluginResultV2::Success);
    }

    #[test]
    fn test_plugin_shutdown() {
        plugin_init_v2(std::ptr::null());
        let result = plugin_shutdown_v2(std::ptr::null());
        assert_eq!(result, PluginResultV2::Success);
    }

    #[test]
    fn test_plugin_health_check() {
        plugin_init_v2(std::ptr::null());
        let health = plugin_health_check_v2(std::ptr::null());
        assert_eq!(health, HealthStatus::Healthy);
        plugin_shutdown_v2(std::ptr::null());
        let health = plugin_health_check_v2(std::ptr::null());
        assert_eq!(health, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_request_counting() {
        assert_eq!(REQUEST_COUNT.load(Ordering::SeqCst), 0);
        plugin_init_v2(std::ptr::null());
        let mut response = std::mem::MaybeUninit::<ResponseV2>::uninit();
        let request = RequestV2 {
            method: std::ptr::null(),
            path: std::ptr::null(),
            query: b"test\0".as_ptr() as *const c_char,
            headers: std::ptr::null(),
            num_headers: 0,
            body: std::ptr::null(),
            body_len: 0,
            content_type: std::ptr::null(),
        };
        let response_ptr = response.as_mut_ptr();
        plugin_handle_request_v2(std::ptr::null(), &request, response_ptr);
        assert_eq!(REQUEST_COUNT.load(Ordering::SeqCst), 1);
    }
}
