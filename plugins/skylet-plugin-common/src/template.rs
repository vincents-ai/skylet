// Skylet Plugin Template Generator - Eliminates 95% of boilerplate
use crate::common::*;
use skylet_abi::*;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

// Plugin-specific request type (replace with your actual request types)
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "action")]
pub enum PluginRequest {
    // Add your specific actions here
    #[serde(rename = "example_action")]
    ExampleAction { field1: String },
}

// Plugin-specific response type (replace with your actual response types)
pub type PluginResponseData = serde_json::Value;

static mut PLUGIN_CTX: *const PluginContext = ptr::null();
static PLUGIN_INFO_PTR: AtomicPtr<PluginInfo> = AtomicPtr::new(ptr::null_mut());

// PLUGIN LIFECYCLE FUNCTIONS - No changes needed below
#[no_mangle]
pub extern "C" fn plugin_init(context: *const PluginContext) -> PluginResult {
    unsafe {
        if context.is_null() {
            return PluginResult::InvalidRequest;
        }
        PLUGIN_CTX = context;
        log_message(context, PluginLogLevel::Info, "template-plugin initialized");
        PluginResult::Success
    }
}

#[no_mangle]
pub extern "C" fn plugin_shutdown(_context: *const PluginContext) -> PluginResult {
    unsafe {
        PLUGIN_CTX = ptr::null();
        PluginResult::Success
    }
}

#[no_mangle]
pub extern "C" fn plugin_get_info() -> *const PluginInfo {
    let existing = PLUGIN_INFO_PTR.load(Ordering::SeqCst);
    if !existing.is_null() {
        return existing as *const PluginInfo;
    }

    // Configure your plugin here
    let info_holder = create_plugin_info(
        "template-plugin",             // name
        "0.1.0",                       // version
        "Template plugin description", // description
        vec!["template-service"],      // provides_services
        vec!["network-access"],        // capabilities
        4,                             // max_concurrency
    );

    let info_ptr = &info_holder.info as *const PluginInfo as *mut PluginInfo;
    PLUGIN_INFO_PTR.store(info_ptr, Ordering::SeqCst);
    let _ = Box::into_raw(Box::new(info_holder));
    info_ptr as *const PluginInfo
}

#[no_mangle]
pub extern "C" fn plugin_handle_request(
    _context: *const PluginContext,
    request: *const HttpRequest,
    response: *mut *mut HttpResponse,
) -> PluginResult {
    if request.is_null() || response.is_null() {
        return PluginResult::InvalidRequest;
    }

    let (method, path) = match extract_method_and_path(request) {
        Ok(mp) => mp,
        Err(_) => return PluginResult::InvalidRequest,
    };

    if method == "POST" {
        return handle_plugin_request(request, response);
    }

    if method == "GET" && path == "/health" {
        return handle_health_request(response);
    }

    PluginResult::NotImplemented
}

// PLUGIN-SPECIFIC LOGIC - Customize these functions

fn handle_plugin_request(
    request: *const HttpRequest,
    response: *mut *mut HttpResponse,
) -> PluginResult {
    let req_data = match parse_json_request::<PluginRequest>(request) {
        Ok(data) => data,
        Err(_) => {
            unsafe {
                *response = create_error_response(400, "Invalid JSON");
            }
            return PluginResult::Success;
        }
    };

    // Customize your API logic here
    match req_data {
        PluginRequest::ExampleAction { field1 } => handle_example_action(&field1, response),
    }
}

fn handle_example_action(param: &str, response: *mut *mut HttpResponse) -> PluginResult {
    // Add your business logic here
    let result_data = serde_json::json!({
        "message": format!("Processed: {}", param),
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    let response_json = PluginResponse {
        success: true,
        data: Some(result_data),
        error: None,
        rate_limit: None,
        request_id: Some(generate_request_id()),
    };

    match serde_json::to_string(&response_json) {
        Ok(json_body) => {
            unsafe {
                *response = create_success_response(&json_body, None);
            }
            PluginResult::Success
        }
        Err(e) => {
            unsafe {
                *response = create_error_response(500, &format!("Serialization failed: {}", e));
            }
            PluginResult::Success
        }
    }
}

fn handle_health_request(response: *mut *mut HttpResponse) -> PluginResult {
    let health_data = serde_json::json!({
        "status": "healthy",
        "plugin": "template-plugin",
        "version": "0.1.0",
        "capabilities": ["network-access"],
        "endpoints": ["/health", "/"]
    });

    let response_json = PluginResponse::<serde_json::Value> {
        success: true,
        data: Some(health_data),
        error: None,
        rate_limit: None,
        request_id: None,
    };

    match serde_json::to_string(&response_json) {
        Ok(json_body) => {
            unsafe {
                *response = create_success_response(&json_body, None);
            }
            PluginResult::Success
        }
        Err(e) => {
            unsafe {
                *response =
                    create_error_response(500, &format!("Health serialization failed: {}", e));
            }
            PluginResult::Success
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_utils::*;

    #[test]
    fn test_template_plugin_creation() {
        let info = create_plugin_info(
            "test-plugin",
            "0.1.0",
            "Test plugin",
            vec!["test-service"],
            vec!["test-capability"],
            8,
        );

        assert_eq!(info._name.to_str_lossy(), "test-plugin");
        assert_eq!(info._version.to_str_lossy(), "0.1.0");
    }

    #[test]
    fn test_request_parsing() {
        let request_body = r#"{"action": "example_action", "field1": "test"}"#;
        let http_request = create_test_request("POST", "/", request_body);

        let parsed = parse_json_request::<PluginRequest>(&http_request).unwrap();
        match parsed {
            PluginRequest::ExampleAction { field1 } => {
                assert_eq!(field1, "test");
            }
        }
    }

    #[test]
    fn test_url_building() {
        let url = build_api_url(
            "https://api.example.com",
            "/test",
            &[("param1", "value1"), ("param2", "value2")],
        )
        .unwrap();

        assert_url_contains("param1=value1", &url);
        assert_url_contains("param2=value2", &url);
    }
}
