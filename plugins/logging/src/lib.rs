// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Logging Plugin - Structured logging backend for Skylet (V2 ABI)
//!
//! This plugin provides structured JSON logging with RFC-0018 compliance.
//! Now migrated to RFC-0004 v2 ABI using skylet_plugin_v2! macro.
//!
//! Uses skylet-plugin-common for:
//! - RFC-0006 compliant config paths
//! - skylet_plugin_v2! macro for V2 ABI boilerplate elimination
//! - Common response helpers

#![allow(dead_code, unused_imports, unused_variables)]

use skylet_abi::v2_spec::*;
use skylet_abi::PluginLogLevel;
use skylet_plugin_common::config_paths;
use std::ffi::{c_char, CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use chrono::Utc;
use serde_json::{json, Value};

// Use the V2 ABI macro to generate all boilerplate entry points
skylet_plugin_common::skylet_plugin_v2! {
    name: "logging",
    version: "0.1.0",
    description: "Structured logging backend (v2)",
    author: "Skylet",
    license: "MIT OR Apache-2.0",
    tagline: "Structured JSON logging",
    category: skylet_abi::PluginCategory::Utility,
    max_concurrency: 10,
    supports_async: false,
    capabilities: ["logging.write", "logging.read", "logging.clear"],
}

// ============================================================================
// Plugin-specific Business Logic
// ============================================================================

static LOGGING_SERVICE: Mutex<LoggingService> = Mutex::new(LoggingService::new());
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

/// RPC handler for logging operations
#[allow(dead_code)]
extern "C" fn logging_rpc_handler(_request: *const RpcRequestV2, response: *mut RpcResponseV2) {
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
        let info = plugin_get_info_v2();
        assert!(!info.is_null());
        unsafe {
            assert!(!(*info).name.is_null());
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

    #[test]
    fn test_capability_query() {
        // Test that capabilities are properly returned
        let cap = CString::new("logging.write").unwrap();
        let result = plugin_query_capability_v2(ptr::null(), cap.as_ptr());
        assert!(result);

        let invalid_cap = CString::new("invalid.capability").unwrap();
        let result = plugin_query_capability_v2(ptr::null(), invalid_cap.as_ptr());
        assert!(!result);
    }
}
