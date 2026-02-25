use skylet_abi::v2_spec::*;

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
        let resp = &mut *response;
        resp.status_code = 200;
        resp.content_type = b"text/plain\0".as_ptr() as *const std::ffi::c_char;

        let message = b"Hello from Hello Plugin!";
        resp.body = message.as_ptr() as *mut u8;
        resp.body_len = message.len();
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

static PLUGIN_API: PluginApiV2 = PluginApiV2 {
    get_info: plugin_get_info_v2,
    init: plugin_init_v2,
    shutdown: plugin_shutdown_v2,
    handle_request: plugin_handle_request_v2,
    handle_event: None,
    prepare_hot_reload: None,
    health_check: Some(plugin_health_check_v2),
    get_metrics: None,
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
