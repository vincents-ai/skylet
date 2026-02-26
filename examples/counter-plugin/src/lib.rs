use skylet_abi::v2_spec::*;
use std::ffi::{c_char, CStr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

static mut INITIALIZED: bool = false;

lazy_static::lazy_static! {
    static ref COUNTER: AtomicU64 = AtomicU64::new(0);
    static ref MAX_COUNT: Mutex<Option<u64>> = Mutex::new(None);
    static ref ERROR_COUNT: AtomicU64 = AtomicU64::new(0);
}

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    static INFO: PluginInfoV2 = PluginInfoV2 {
        name: b"counter-plugin\0" as *const u8 as *const i8,
        version: b"0.1.0\0" as *const u8 as *const i8,
        author: b"Skylet Team\0" as *const u8 as *const i8,
    };
    &INFO
}

#[no_mangle]
pub extern "C" fn plugin_init_v2(_context: *const PluginContextV2) -> PluginResultV2 {
    if unsafe { INITIALIZED } {
        return PluginResultV2::Error;
    }
    unsafe {
        INITIALIZED = true;
    }

    COUNTER.store(0, Ordering::SeqCst);
    ERROR_COUNT.store(0, Ordering::SeqCst);
    *MAX_COUNT.lock().unwrap() = None;

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
        let req = &*request;
        let resp = &mut *response;

        let action = if !req.query.is_null() {
            CStr::from_ptr(req.query).to_string_lossy().to_string()
        } else {
            "get".to_string()
        };

        let result = match action.as_str() {
            "increment" => increment_counter(),
            "decrement" => decrement_counter(),
            "reset" => reset_counter(),
            "get" => get_counter(),
            "set" => set_counter(req.body, req.body_len),
            _ => format!(r#"{{"error": "Unknown action: {}"}}"#, action),
        };

        resp.status_code = if result.contains("\"error\"") {
            400
        } else {
            200
        };
        resp.content_type = b"application/json\0".as_ptr() as *const c_char;

        let bytes = result.as_bytes();
        resp.body = bytes.as_ptr() as *mut u8;
        resp.body_len = bytes.len();
    }

    PluginResultV2::Success
}

fn increment_counter() -> String {
    let current = COUNTER.fetch_add(1, Ordering::SeqCst) + 1;

    if let Some(max) = *MAX_COUNT.lock().unwrap() {
        if current > max {
            COUNTER.store(max, Ordering::SeqCst);
            ERROR_COUNT.fetch_add(1, Ordering::SeqCst);
            return format!(
                r#"{{"error": "Counter exceeds maximum value of {}", "value": {}}}"#,
                max, max
            );
        }
    }

    format!(r#"{{"value": {}, "action": "incremented"}}"#, current)
}

fn decrement_counter() -> String {
    let current = COUNTER.fetch_sub(1, Ordering::SeqCst).saturating_sub(1);

    if let Some(max) = *MAX_COUNT.lock().unwrap() {
        if current > max {
            COUNTER.store(max, Ordering::SeqCst);
        }
    }

    format!(r#"{{"value": {}, "action": "decremented"}}"#, current)
}

fn reset_counter() -> String {
    COUNTER.store(0, Ordering::SeqCst);
    format!(r#"{{"value": 0, "action": "reset"}}"#)
}

fn get_counter() -> String {
    let current = COUNTER.load(Ordering::SeqCst);
    let max = *MAX_COUNT.lock().unwrap();
    let errors = ERROR_COUNT.load(Ordering::SeqCst);

    format!(
        r#"{{"value": {}, "max": {:?}, "errors": {}}}"#,
        current, max, errors
    )
}

fn set_counter(body: *const u8, body_len: usize) -> String {
    if body.is_null() || body_len == 0 {
        ERROR_COUNT.fetch_add(1, Ordering::SeqCst);
        return r#"{"error": "No value provided"}"#.to_string();
    }

    let slice = std::slice::from_raw_parts(body, body_len);
    let json_str = String::from_utf8_lossy(slice);

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
        if let Some(value) = json.get("value").and_then(|v| v.as_u64()) {
            COUNTER.store(value, Ordering::SeqCst);

            if let Some(max) = json.get("max").and_then(|v| v.as_u64()) {
                *MAX_COUNT.lock().unwrap() = Some(max);
            }

            format!(r#"{{"value": {}, "action": "set"}}"#, value)
        } else {
            ERROR_COUNT.fetch_add(1, Ordering::SeqCst);
            r#"{"error": "Invalid or missing 'value' field"}"#.to_string()
        }
    } else {
        ERROR_COUNT.fetch_add(1, Ordering::SeqCst);
        r#"{"error": "Invalid JSON format"}"#.to_string()
    }
}

#[no_mangle]
pub extern "C" fn plugin_health_check_v2(_context: *const PluginContextV2) -> HealthStatus {
    if unsafe { !INITIALIZED } {
        return HealthStatus::Unhealthy;
    }

    let current = COUNTER.load(Ordering::SeqCst);
    if let Some(max) = *MAX_COUNT.lock().unwrap() {
        if current > max {
            return HealthStatus::Degraded;
        }
    }

    HealthStatus::Healthy
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
        METRICS.error_count = ERROR_COUNT.load(Ordering::SeqCst) as u32;
        &raw const METRICS
    }
}

static PLUGIN_API: PluginApiV2 = PluginApiV2 {
    get_info: plugin_get_info_v2,
    init: plugin_init_v2,
    shutdown: plugin_shutdown_v2,
    handle_request: plugin_handle_request_v2,
    handle_event: None,
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
