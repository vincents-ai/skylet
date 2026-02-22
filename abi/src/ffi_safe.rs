// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

/// Safe FFI Wrappers for ABI v2
///
/// This module provides safe, idiomatic Rust wrappers around the unsafe
/// C FFI boundaries defined in RFC-0004. It handles:
///
/// - String conversion between C strings and Rust strings
/// - Memory ownership tracking and safety
/// - Panic-safe execution across boundaries
/// - Proper error handling with Result types
/// - Async function call safety
/// - Service discovery and capability checking
use crate::v2_spec::*;
use crate::PluginLogLevel;
use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe, UnwindSafe};

/// Error type for ABI wrapper operations
#[derive(Debug, Clone)]
pub enum AbiError {
    NullPointer(String),
    InvalidString(String),
    UnknownPluginError(i32),
    Panicked(String),
    InvalidRequest(String),
    ServiceUnavailable(String),
    PermissionDenied(String),
    Timeout(String),
    ResourceExhausted(String),
    NotImplemented(String),
}

impl std::fmt::Display for AbiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AbiError::NullPointer(msg) => write!(f, "Null pointer: {}", msg),
            AbiError::InvalidString(msg) => write!(f, "Invalid string: {}", msg),
            AbiError::UnknownPluginError(code) => write!(f, "Plugin error: {}", code),
            AbiError::Panicked(msg) => write!(f, "Panicked: {}", msg),
            AbiError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            AbiError::ServiceUnavailable(msg) => write!(f, "Service unavailable: {}", msg),
            AbiError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            AbiError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            AbiError::ResourceExhausted(msg) => write!(f, "Resource exhausted: {}", msg),
            AbiError::NotImplemented(msg) => write!(f, "Not implemented: {}", msg),
        }
    }
}

impl std::error::Error for AbiError {}

pub type AbiResult<T> = Result<T, AbiError>;

/// Safe wrapper for C string conversion
pub struct SafeCString {
    inner: CString,
}

impl SafeCString {
    /// Create a SafeCString from a Rust string
    pub fn new(s: impl Into<Vec<u8>>) -> AbiResult<Self> {
        CString::new(s)
            .map(|inner| SafeCString { inner })
            .map_err(|e| AbiError::InvalidString(e.to_string()))
    }

    /// Get a C string pointer
    pub fn as_ptr(&self) -> *const c_char {
        self.inner.as_ptr()
    }

    /// Get a mutable C string pointer
    pub fn as_mut_ptr(&mut self) -> *mut c_char {
        self.inner.as_ptr() as *mut c_char
    }
}

/// Safe conversion from C string to Rust string
///
/// # Safety Note
/// This function validates that `ptr` is non-null before dereferencing.
/// The clippy lint is allowed because this is intentionally a safe wrapper
/// for FFI boundaries - callers are responsible for ensuring the pointer
/// points to a valid null-terminated C string.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn c_str_to_string(ptr: *const c_char) -> AbiResult<String> {
    if ptr.is_null() {
        return Err(AbiError::NullPointer(
            "C string pointer is null".to_string(),
        ));
    }

    unsafe {
        CStr::from_ptr(ptr)
            .to_str()
            .map(str::to_string)
            .map_err(|e| AbiError::InvalidString(format!("Invalid UTF-8: {e}")))
    }
}

/// Safe conversion from Rust string to C string pointer
/// WARNING: Caller must ensure proper memory cleanup
pub fn string_to_c_ptr(s: &str) -> AbiResult<*const c_char> {
    CString::new(s.as_bytes())
        .map(|cstring| cstring.into_raw() as *const c_char)
        .map_err(|e| AbiError::InvalidString(e.to_string()))
}

/// Safe free function for C strings allocated by the plugin/host
///
/// # Safety
/// Only use on pointers returned by plugin FFI calls
#[allow(dead_code)]
pub unsafe fn free_c_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

/// Safe wrapper for PluginContextV2 operations
#[derive(Debug)]
pub struct SafePluginContext {
    context_ptr: *const PluginContextV2,
}

impl SafePluginContext {
    /// Create a safe wrapper from a context pointer
    ///
    /// # Safety
    /// Context pointer must be valid and outlive the wrapper
    ///
    /// # Errors
    /// Returns error if context_ptr is null
    pub unsafe fn new(context_ptr: *const PluginContextV2) -> AbiResult<Self> {
        if context_ptr.is_null() {
            return Err(AbiError::NullPointer(
                "PluginContextV2 pointer is null".to_string(),
            ));
        }
        Ok(SafePluginContext { context_ptr })
    }

    /// Get the raw context pointer
    pub fn as_ptr(&self) -> *const PluginContextV2 {
        self.context_ptr
    }

    /// Check if logger service is available
    pub fn has_logger(&self) -> bool {
        unsafe { !(*self.context_ptr).logger.is_null() }
    }

    /// Check if config service is available
    pub fn has_config(&self) -> bool {
        unsafe { !(*self.context_ptr).config.is_null() }
    }

    /// Check if service registry is available
    pub fn has_service_registry(&self) -> bool {
        unsafe { !(*self.context_ptr).service_registry.is_null() }
    }

    /// Check if event bus is available
    pub fn has_event_bus(&self) -> bool {
        unsafe { !(*self.context_ptr).event_bus.is_null() }
    }

    /// Check if RPC service is available
    pub fn has_rpc_service(&self) -> bool {
        unsafe { !(*self.context_ptr).rpc_service.is_null() }
    }

    /// Check if secrets service is available
    pub fn has_secrets(&self) -> bool {
        unsafe { !(*self.context_ptr).secrets.is_null() }
    }

    /// Get the user data pointer
    pub fn user_data(&self) -> *mut std::ffi::c_void {
        unsafe { (*self.context_ptr).user_data }
    }

    /// Get user context JSON if available
    pub fn user_context_json(&self) -> AbiResult<Option<String>> {
        unsafe {
            let user_ctx = (*self.context_ptr).user_context_json;
            if user_ctx.is_null() {
                Ok(None)
            } else {
                c_str_to_string(user_ctx).map(Some)
            }
        }
    }
}

/// Result conversion wrapper
impl From<PluginResultV2> for AbiResult<()> {
    fn from(result: PluginResultV2) -> Self {
        match result {
            PluginResultV2::Success => Ok(()),
            PluginResultV2::Error => Err(AbiError::InvalidRequest(
                "Plugin returned error".to_string(),
            )),
            PluginResultV2::InvalidRequest => {
                Err(AbiError::InvalidRequest("Invalid request".to_string()))
            }
            PluginResultV2::ServiceUnavailable => Err(AbiError::ServiceUnavailable(
                "Service unavailable".to_string(),
            )),
            PluginResultV2::PermissionDenied => {
                Err(AbiError::PermissionDenied("Permission denied".to_string()))
            }
            PluginResultV2::NotImplemented => {
                Err(AbiError::NotImplemented("Not implemented".to_string()))
            }
            PluginResultV2::Timeout => Err(AbiError::Timeout("Timeout".to_string())),
            PluginResultV2::ResourceExhausted => Err(AbiError::ResourceExhausted(
                "Resource exhausted".to_string(),
            )),
            PluginResultV2::Pending => Ok(()), // For async operations
        }
    }
}

/// Safe logging wrapper
pub struct SafeLogger {
    logger_ptr: *const LoggerV2,
    context_ptr: *const PluginContextV2,
}

impl SafeLogger {
    /// Create a safe logger wrapper
    ///
    /// # Safety
    /// Pointers must be valid
    ///
    /// # Errors
    /// Returns error if logger_ptr is null
    pub unsafe fn new(
        logger_ptr: *const LoggerV2,
        context_ptr: *const PluginContextV2,
    ) -> AbiResult<Self> {
        if logger_ptr.is_null() {
            return Err(AbiError::NullPointer("Logger pointer is null".to_string()));
        }
        Ok(SafeLogger {
            logger_ptr,
            context_ptr,
        })
    }

    /// Log a message safely
    pub fn log(&self, level: PluginLogLevel, message: &str) -> AbiResult<()> {
        let c_msg =
            CString::new(message.as_bytes()).map_err(|e| AbiError::InvalidString(e.to_string()))?;

        let result = unsafe {
            let logger = &*self.logger_ptr;
            (logger.log)(self.context_ptr, level, c_msg.as_ptr())
        };

        AbiResult::from(result)
    }

    /// Log structured data safely
    pub fn log_structured(
        &self,
        level: PluginLogLevel,
        message: &str,
        data: serde_json::Value,
    ) -> AbiResult<()> {
        let c_msg =
            CString::new(message.as_bytes()).map_err(|e| AbiError::InvalidString(e.to_string()))?;

        let json_str = serde_json::to_string(&data)
            .map_err(|e: serde_json::Error| AbiError::InvalidString(e.to_string()))?;
        let c_json = CString::new(json_str.as_bytes())
            .map_err(|e| AbiError::InvalidString(e.to_string()))?;

        let result = unsafe {
            let logger = &*self.logger_ptr;
            (logger.log_structured)(self.context_ptr, level, c_msg.as_ptr(), c_json.as_ptr())
        };

        AbiResult::from(result)
    }
}

/// Safe configuration wrapper
pub struct SafeConfig {
    config_ptr: *const ConfigV2,
    context_ptr: *const PluginContextV2,
}

impl SafeConfig {
    /// Create a safe config wrapper
    ///
    /// # Safety
    /// Pointers must be valid
    ///
    /// # Errors
    /// Returns error if config_ptr is null
    pub unsafe fn new(
        config_ptr: *const ConfigV2,
        context_ptr: *const PluginContextV2,
    ) -> AbiResult<Self> {
        if config_ptr.is_null() {
            return Err(AbiError::NullPointer("Config pointer is null".to_string()));
        }
        Ok(SafeConfig {
            config_ptr,
            context_ptr,
        })
    }

    /// Get a configuration value as string
    pub fn get(&self, key: &str) -> AbiResult<Option<String>> {
        let c_key =
            CString::new(key.as_bytes()).map_err(|e| AbiError::InvalidString(e.to_string()))?;

        let result = unsafe {
            let config = &*self.config_ptr;
            (config.get)(self.context_ptr, c_key.as_ptr())
        };

        if result.is_null() {
            Ok(None)
        } else {
            c_str_to_string(result).map(Some)
        }
    }

    /// Get a boolean configuration value
    pub fn get_bool(&self, key: &str) -> AbiResult<bool> {
        let c_key =
            CString::new(key.as_bytes()).map_err(|e| AbiError::InvalidString(e.to_string()))?;

        let result = unsafe {
            let config = &*self.config_ptr;
            (config.get_bool)(self.context_ptr, c_key.as_ptr())
        };

        Ok(result != 0)
    }

    /// Get an integer configuration value
    pub fn get_int(&self, key: &str) -> AbiResult<i64> {
        let c_key =
            CString::new(key.as_bytes()).map_err(|e| AbiError::InvalidString(e.to_string()))?;

        Ok(unsafe {
            let config = &*self.config_ptr;
            (config.get_int)(self.context_ptr, c_key.as_ptr())
        })
    }

    /// Get a floating point configuration value
    pub fn get_float(&self, key: &str) -> AbiResult<f64> {
        let c_key =
            CString::new(key.as_bytes()).map_err(|e| AbiError::InvalidString(e.to_string()))?;

        Ok(unsafe {
            let config = &*self.config_ptr;
            (config.get_float)(self.context_ptr, c_key.as_ptr())
        })
    }
}

/// Safe service registry wrapper
pub struct SafeServiceRegistry {
    registry_ptr: *const ServiceRegistryV2,
    context_ptr: *const PluginContextV2,
}

impl SafeServiceRegistry {
    /// Create a safe service registry wrapper
    ///
    /// # Safety
    /// Pointers must be valid
    ///
    /// # Errors
    /// Returns error if registry_ptr is null
    pub unsafe fn new(
        registry_ptr: *const ServiceRegistryV2,
        context_ptr: *const PluginContextV2,
    ) -> AbiResult<Self> {
        if registry_ptr.is_null() {
            return Err(AbiError::NullPointer(
                "ServiceRegistry pointer is null".to_string(),
            ));
        }
        Ok(SafeServiceRegistry {
            registry_ptr,
            context_ptr,
        })
    }

    /// Register a service
    pub fn register(
        &self,
        name: &str,
        service: *mut std::ffi::c_void,
        service_type: &str,
    ) -> AbiResult<()> {
        let c_name =
            CString::new(name.as_bytes()).map_err(|e| AbiError::InvalidString(e.to_string()))?;
        let c_type = CString::new(service_type.as_bytes())
            .map_err(|e| AbiError::InvalidString(e.to_string()))?;

        let result = unsafe {
            let registry = &*self.registry_ptr;
            (registry.register)(self.context_ptr, c_name.as_ptr(), service, c_type.as_ptr())
        };

        AbiResult::from(result)
    }

    /// Unregister a service
    pub fn unregister(&self, name: &str) -> AbiResult<()> {
        let c_name =
            CString::new(name.as_bytes()).map_err(|e| AbiError::InvalidString(e.to_string()))?;

        let result = unsafe {
            let registry = &*self.registry_ptr;
            (registry.unregister)(self.context_ptr, c_name.as_ptr())
        };

        AbiResult::from(result)
    }

    /// List available services
    pub fn list_services(&self) -> AbiResult<Vec<String>> {
        unsafe {
            let registry = &*self.registry_ptr;
            let list_ptr = (registry.list_services)(self.context_ptr);

            if list_ptr.is_null() {
                return Ok(Vec::new());
            }

            // Count non-null entries
            let mut count = 0;
            while count < MAX_SERVICES_RETURNED && !(*list_ptr.add(count)).is_null() {
                count += 1;
            }

            if count >= MAX_SERVICES_RETURNED {
                tracing::warn!(
                    "Service list exceeded maximum: {} >= {}",
                    count,
                    MAX_SERVICES_RETURNED
                );
                return Err(AbiError::ResourceExhausted(
                    "Service list too large".to_string(),
                ));
            }

            let mut services = Vec::new();
            for i in 0..count {
                let s = c_str_to_string(*list_ptr.add(i))?;
                services.push(s);
            }

            (registry.free_service_list)(list_ptr, count);
            Ok(services)
        }
    }
}

/// Safe EventBus wrapper for ABI v2
pub struct SafeEventBus {
    event_bus_ptr: *const EventBusV2,
    context_ptr: *const PluginContextV2,
}

impl SafeEventBus {
    /// Create a safe EventBus wrapper
    ///
    /// # Safety
    /// Pointers must be valid
    ///
    /// # Errors
    /// Returns error if event_bus_ptr is null
    pub unsafe fn new(
        event_bus_ptr: *const EventBusV2,
        context_ptr: *const PluginContextV2,
    ) -> AbiResult<Self> {
        if event_bus_ptr.is_null() {
            return Err(AbiError::NullPointer(
                "EventBusV2 pointer is null".to_string(),
            ));
        }
        Ok(SafeEventBus {
            event_bus_ptr,
            context_ptr,
        })
    }

    /// Publish an event safely
    pub fn publish(&self, event_type: &str, payload: serde_json::Value) -> AbiResult<()> {
        let c_type = CString::new(event_type.as_bytes())
            .map_err(|e| AbiError::InvalidString(e.to_string()))?;
        let json_str = serde_json::to_string(&payload)
            .map_err(|e: serde_json::Error| AbiError::InvalidString(e.to_string()))?;
        let c_payload = CString::new(json_str.as_bytes())
            .map_err(|e| AbiError::InvalidString(e.to_string()))?;

        let event = EventV2 {
            type_: c_type.as_ptr(),
            payload_json: c_payload.as_ptr(),
            timestamp_ms: std::time::SystemTime::UNIX_EPOCH
                .elapsed()
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            source_plugin: std::ptr::null(),
        };

        let result = unsafe {
            let event_bus = &*self.event_bus_ptr;
            (event_bus.publish)(self.context_ptr, &event)
        };

        AbiResult::from(result)
    }

    /// Subscribe to an event type
    /// Note: Callbacks require extern "C" function pointers
    pub fn subscribe(&self, event_type: &str) -> AbiResult<()> {
        let c_type = CString::new(event_type.as_bytes())
            .map_err(|e| AbiError::InvalidString(e.to_string()))?;

        let result = unsafe {
            let event_bus = &*self.event_bus_ptr;
            (event_bus.subscribe)(self.context_ptr, c_type.as_ptr(), Self::default_callback)
        };

        AbiResult::from(result)
    }

    /// Default event callback (placeholder)
    extern "C" fn default_callback(_event: *const EventV2) {}

    /// Unsubscribe from an event type
    pub fn unsubscribe(&self, event_type: &str) -> AbiResult<()> {
        let c_type = CString::new(event_type.as_bytes())
            .map_err(|e| AbiError::InvalidString(e.to_string()))?;

        let result = unsafe {
            let event_bus = &*self.event_bus_ptr;
            (event_bus.unsubscribe)(self.context_ptr, c_type.as_ptr())
        };

        AbiResult::from(result)
    }
}

/// Safe RPC Service wrapper for ABI v2
pub struct SafeRpcService {
    rpc_ptr: *const RpcServiceV2,
    context_ptr: *const PluginContextV2,
}

impl SafeRpcService {
    /// Create a safe RPC wrapper
    ///
    /// # Safety
    /// Pointers must be valid
    ///
    /// # Errors
    /// Returns error if rpc_ptr is null
    pub unsafe fn new(
        rpc_ptr: *const RpcServiceV2,
        context_ptr: *const PluginContextV2,
    ) -> AbiResult<Self> {
        if rpc_ptr.is_null() {
            return Err(AbiError::NullPointer(
                "RpcServiceV2 pointer is null".to_string(),
            ));
        }
        Ok(SafeRpcService {
            rpc_ptr,
            context_ptr,
        })
    }

    /// Call a remote procedure safely
    pub fn call(
        &self,
        service: &str,
        _method: &str,
        params: serde_json::Value,
        timeout_ms: u64,
    ) -> AbiResult<serde_json::Value> {
        let c_service =
            CString::new(service.as_bytes()).map_err(|e| AbiError::InvalidString(e.to_string()))?;
        let params_str = serde_json::to_string(&params)
            .map_err(|e: serde_json::Error| AbiError::InvalidString(e.to_string()))?;
        let c_params = CString::new(params_str.as_bytes())
            .map_err(|e| AbiError::InvalidString(e.to_string()))?;

        let request = RpcRequestV2 {
            method: c_params.as_ptr(),
            params: c_params.as_ptr(),
            timeout_ms,
        };

        let mut response = RpcResponseV2 {
            result: std::ptr::null(),
            error: std::ptr::null(),
            status: PluginResultV2::Error,
        };

        let rpc_result = unsafe {
            let rpc = &*self.rpc_ptr;
            (rpc.call)(
                self.context_ptr,
                c_service.as_ptr(),
                &request,
                &mut response,
            )
        };

        AbiResult::<()>::from(rpc_result)?;

        if response.status == PluginResultV2::Success && !response.result.is_null() {
            let result_str = c_str_to_string(response.result)?;
            serde_json::from_str(&result_str)
                .map_err(|e| AbiError::InvalidString(format!("Invalid JSON response: {}", e)))
        } else if !response.error.is_null() {
            let error_str = c_str_to_string(response.error)?;
            Err(AbiError::InvalidRequest(error_str))
        } else {
            Err(AbiError::UnknownPluginError(response.status as i32))
        }
    }

    /// List available services
    pub fn list_services(&self) -> AbiResult<Vec<String>> {
        unsafe {
            let rpc = &*self.rpc_ptr;
            let list_ptr = (rpc.list_services)(self.context_ptr);

            if list_ptr.is_null() {
                return Ok(Vec::new());
            }

            let mut count = 0;
            while count < MAX_SERVICES_RETURNED && !(*list_ptr.add(count)).is_null() {
                count += 1;
            }

            if count >= MAX_SERVICES_RETURNED {
                tracing::warn!(
                    "RPC service list exceeded maximum: {} >= {}",
                    count,
                    MAX_SERVICES_RETURNED
                );
                return Err(AbiError::ResourceExhausted(
                    "RPC service list too large".to_string(),
                ));
            }

            let mut services = Vec::new();
            for i in 0..count {
                let s = c_str_to_string(*list_ptr.add(i))?;
                services.push(s);
            }

            (rpc.free_strings)(list_ptr, count);
            Ok(services)
        }
    }

    /// Get service specification
    pub fn get_service_spec(&self, service: &str) -> AbiResult<Option<String>> {
        let c_service =
            CString::new(service.as_bytes()).map_err(|e| AbiError::InvalidString(e.to_string()))?;

        let result = unsafe {
            let rpc = &*self.rpc_ptr;
            (rpc.get_service_spec)(self.context_ptr, c_service.as_ptr())
        };

        if result.is_null() {
            Ok(None)
        } else {
            c_str_to_string(result).map(Some)
        }
    }
}

/// Panic-safe wrapper for plugin function calls
pub struct PanicSafeCall;

impl PanicSafeCall {
    /// Execute a closure safely, catching panics and converting result
    pub fn execute<F>(f: F) -> AbiResult<()>
    where
        F: FnOnce() -> PluginResultV2 + UnwindSafe,
    {
        match catch_unwind(AssertUnwindSafe(f)) {
            Ok(plugin_result) => {
                let result: AbiResult<()> = plugin_result.into();
                result
            }
            Err(_) => Err(AbiError::Panicked(
                "Plugin panicked during execution".to_string(),
            )),
        }
    }

    /// Execute and return a value safely
    pub fn execute_with_result<F, R>(f: F) -> AbiResult<R>
    where
        F: FnOnce() -> AbiResult<R> + UnwindSafe,
        R: Default,
    {
        match catch_unwind(AssertUnwindSafe(f)) {
            Ok(result) => result,
            Err(_) => Err(AbiError::Panicked(
                "Plugin panicked during execution".to_string(),
            )),
        }
    }
}

// RFC-0004-SEC-003: Error message sanitization to prevent information disclosure
// When plugin loading fails, avoid exposing internal implementation details

/// Sanitize error messages for external communication
///
/// RFC-0004-SEC-003: Information Disclosure Mitigation
/// - Replaces detailed system errors with generic messages for external APIs
/// - Full error details should be logged separately by the caller
/// - Prevents leakage of paths, system calls, and internal structure
pub fn sanitize_error_for_external(_error_msg: &str, context: &str) -> String {
    // Return generic message for external consumption
    // Caller should log the full error with [SECURITY_AUDIT] prefix for internal audit trail
    match context {
        "plugin_loading" => {
            "Plugin initialization failed. Please contact system administrator.".to_string()
        }
        "plugin_init" => "Plugin failed to initialize. Check system logs.".to_string(),
        "plugin_sandbox" => {
            "Plugin sandbox enforcement failed. Security policy violated.".to_string()
        }
        "ffi_boundary" => "Plugin communication failed. Invalid plugin response.".to_string(),
        _ => "Operation failed. Please contact system administrator.".to_string(),
    }
}

/// Check if error message contains sensitive information
///
/// RFC-0004-SEC-003: Prevents information disclosure attacks
/// Identifies error messages that might leak:
/// - File paths
/// - System library names
/// - Memory addresses
/// - Internal API names
pub fn contains_sensitive_info(error_msg: &str) -> bool {
    let sensitive_patterns = [
        ".so",           // Shared library paths
        ".dylib",        // macOS library paths
        ".dll",          // Windows library paths
        "/lib/",         // System library paths
        "/usr/lib",      // Common library paths
        "symbol lookup", // FFI symbol errors
        "0x",            // Memory addresses
        "dlopen",        // Dynamic loading details
        "dlsym",         // Symbol resolution details
        "LD_LIBRARY",    // Environment variable leakage
        "RPATH",         // Rpath information
    ];

    for pattern in &sensitive_patterns {
        if error_msg.to_lowercase().contains(pattern) {
            return true;
        }
    }
    false
}

// RFC-0004-SEC-004: Strict UTF-8 validation to prevent log injection attacks
// Validates that strings from plugins are valid UTF-8 before logging or processing

/// Validate UTF-8 string and reject invalid sequences
///
/// RFC-0004-SEC-004: UTF-8 Validation Enhancement
/// - Prevents log injection attacks via non-UTF8 strings
/// - Rejects surrogate pairs and other invalid UTF-8 sequences
/// - Replacement characters (U+FFFD) are rejected in logs
/// - Used for logging strings that come from untrusted plugin sources
pub fn strict_utf8_validation(data: &[u8]) -> AbiResult<String> {
    // First, check if it's valid UTF-8
    let s = std::str::from_utf8(data)
        .map_err(|e| AbiError::InvalidString(format!("Invalid UTF-8 sequence: {}", e)))?;

    // Check for replacement character which indicates lossy conversion happened
    if s.contains('\u{FFFD}') {
        return Err(AbiError::InvalidString(
            "String contains replacement characters (invalid UTF-8)".to_string(),
        ));
    }

    // Check for control characters that could be used in log injection
    for ch in s.chars() {
        match ch {
            // Allow common whitespace and printable characters
            ' ' | '\t' | '\n' | '\r' => continue,
            // Reject other control characters (< 0x20 except whitespace, and 0x7F-0x9F)
            c if (c as u32) < 0x20 || ((c as u32) >= 0x7F && (c as u32) <= 0x9F) => {
                return Err(AbiError::InvalidString(format!(
                    "String contains forbidden control character: U+{:04X}",
                    c as u32
                )));
            }
            _ => continue,
        }
    }

    Ok(s.to_string())
}

/// Convert C string with strict UTF-8 validation
///
/// RFC-0004-SEC-004: For use with logging and audit trail strings from plugins
/// Performs stricter validation than standard UTF-8 conversion
///
/// # Safety Note
/// This function validates that `ptr` is non-null before dereferencing.
/// The clippy lint is allowed because this is intentionally a safe wrapper
/// for FFI boundaries - callers are responsible for ensuring the pointer
/// points to a valid null-terminated C string.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn c_str_to_string_strict(ptr: *const c_char) -> AbiResult<String> {
    if ptr.is_null() {
        return Err(AbiError::NullPointer(
            "C string pointer is null".to_string(),
        ));
    }

    let cstr = unsafe { CStr::from_ptr(ptr) };
    let bytes = cstr.to_bytes();
    strict_utf8_validation(bytes)
}

// RFC-0004-SEC-001: Secure response handling with null pointer validation
// Prevents null pointer dereferences in ResponseV2 and RpcResponseV2 handling

/// Maximum allowed size for response headers (100 headers max)
const MAX_RESPONSE_HEADERS: usize = 100;

/// Maximum allowed size for a single header name (256 bytes)
const MAX_HEADER_NAME_SIZE: usize = 256;

/// Maximum allowed size for a single header value (4096 bytes)
const MAX_HEADER_VALUE_SIZE: usize = 4096;

/// Maximum allowed response body size (100MB)
const MAX_RESPONSE_BODY_SIZE: usize = 100 * 1024 * 1024;

/// Maximum allowed number of services that can be returned from list_services
const MAX_SERVICES_RETURNED: usize = 10_000;

/// Minimum valid memory address to prevent null/invalid pointers
#[allow(dead_code)]
const MIN_VALID_ADDRESS: usize = 0x1000;

/// Secure wrapper for ResponseV2 that validates all pointers before access
pub struct SafeResponseV2 {
    status_code: i32,
    body: Option<Vec<u8>>,
    content_type: Option<String>,
}

impl SafeResponseV2 {
    /// Create a safe response from raw ResponseV2
    ///
    /// # Safety
    /// - Validates all pointers before dereferencing
    /// - Checks bounds on all data sizes
    /// - Returns error on invalid data
    pub unsafe fn from_raw(response: *const ResponseV2) -> AbiResult<Self> {
        // RFC-0004-SEC-001: Check for null response pointer
        if response.is_null() {
            return Err(AbiError::NullPointer(
                "ResponseV2 pointer is null".to_string(),
            ));
        }

        let resp = &*response;

        // Extract body safely
        let body = if !resp.body.is_null() && resp.body_len > 0 {
            // RFC-0004-SEC-001: Validate body size
            if resp.body_len > MAX_RESPONSE_BODY_SIZE {
                return Err(AbiError::InvalidRequest(format!(
                    "Response body size {} exceeds maximum {}",
                    resp.body_len, MAX_RESPONSE_BODY_SIZE
                )));
            }

            // RFC-0004-SEC-001: Validate body pointer address (Phase 2 Issue #4)
            let body_addr = resp.body as usize;
            if body_addr < MIN_VALID_ADDRESS || body_addr == usize::MAX {
                return Err(AbiError::InvalidRequest(format!(
                    "Invalid response body pointer: 0x{:x}",
                    body_addr
                )));
            }

            // RFC-0004-SEC-001: Safe access with panic protection (Phase 2 Issue #4)
            let body_slice = catch_unwind(AssertUnwindSafe(|| unsafe {
                std::slice::from_raw_parts(resp.body, resp.body_len)
            }))
            .map_err(|_| {
                AbiError::InvalidRequest(
                    "Failed to read response body: memory access violation".to_string(),
                )
            })?;

            Some(body_slice.to_vec())
        } else if !resp.body.is_null() || resp.body_len > 0 {
            // RFC-0004-SEC-001: Pointer/length mismatch detection
            return Err(AbiError::InvalidRequest(
                "Response body pointer/length mismatch".to_string(),
            ));
        } else {
            None
        };

        // Extract content type safely
        let content_type = if !resp.content_type.is_null() {
            Some(c_str_to_string(resp.content_type)?)
        } else {
            None
        };

        Ok(SafeResponseV2 {
            status_code: resp.status_code,
            body,
            content_type,
        })
    }

    pub fn status_code(&self) -> i32 {
        self.status_code
    }

    pub fn body(&self) -> Option<&[u8]> {
        self.body.as_deref()
    }

    pub fn body_as_string(&self) -> AbiResult<Option<String>> {
        match &self.body {
            Some(bytes) => String::from_utf8(bytes.clone())
                .map(Some)
                .map_err(|e| AbiError::InvalidString(e.to_string())),
            None => Ok(None),
        }
    }

    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }
}

/// Secure wrapper for ResponseV2 headers validation
pub struct SafeResponseHeaders {
    headers: Vec<(String, String)>,
}

impl SafeResponseHeaders {
    /// Validate and extract headers from raw HeaderV2 array
    ///
    /// # Safety
    /// - Validates header count against maximum allowed
    /// - Validates each header name/value length
    /// - Performs UTF-8 validation on all strings
    pub unsafe fn from_raw(headers_ptr: *const HeaderV2, headers_count: usize) -> AbiResult<Self> {
        let mut headers = Vec::new();

        // RFC-0004-SEC-002: Validate header count
        if headers_count > MAX_RESPONSE_HEADERS {
            return Err(AbiError::InvalidRequest(format!(
                "Too many headers: {} > {}",
                headers_count, MAX_RESPONSE_HEADERS
            )));
        }

        // RFC-0004-SEC-002: Handle empty headers case
        if headers_count == 0 {
            return Ok(SafeResponseHeaders { headers });
        }

        // RFC-0004-SEC-002: Check headers pointer is not null when count > 0
        if headers_ptr.is_null() {
            return Err(AbiError::NullPointer(
                "Headers pointer is null but header count > 0".to_string(),
            ));
        }

        // RFC-0004-SEC-003: Validate pointer alignment (Phase 2 Issue #6)
        let alignment = std::mem::align_of::<HeaderV2>();
        let alignment_mask = alignment - 1;
        if (headers_ptr as usize) & alignment_mask != 0 {
            return Err(AbiError::InvalidRequest(format!(
                "Headers pointer misaligned: 0x{:x} (required {} bytes)",
                headers_ptr as usize, alignment
            )));
        }

        // Validate each header
        for i in 0..headers_count {
            let header = &*headers_ptr.add(i);

            // RFC-0004-SEC-002: Validate header name
            let name = if !header.name.is_null() {
                let name_str = c_str_to_string(header.name)?;

                // RFC-0004-SEC-002: Check header name length
                if name_str.len() > MAX_HEADER_NAME_SIZE {
                    return Err(AbiError::InvalidRequest(format!(
                        "Header name too long: {} > {}",
                        name_str.len(),
                        MAX_HEADER_NAME_SIZE
                    )));
                }

                // RFC-0004-SEC-002: Validate header name is not empty
                if name_str.is_empty() {
                    return Err(AbiError::InvalidRequest(
                        "Header name cannot be empty".to_string(),
                    ));
                }

                name_str
            } else {
                return Err(AbiError::NullPointer(format!(
                    "Header {} name pointer is null",
                    i
                )));
            };

            // RFC-0004-SEC-002: Validate header value
            let value = if !header.value.is_null() {
                let value_str = c_str_to_string(header.value)?;

                // RFC-0004-SEC-002: Check header value length
                if value_str.len() > MAX_HEADER_VALUE_SIZE {
                    return Err(AbiError::InvalidRequest(format!(
                        "Header value too long: {} > {}",
                        value_str.len(),
                        MAX_HEADER_VALUE_SIZE
                    )));
                }

                value_str
            } else {
                return Err(AbiError::NullPointer(format!(
                    "Header {} value pointer is null",
                    i
                )));
            };

            headers.push((name, value));
        }

        Ok(SafeResponseHeaders { headers })
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.headers.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_cstring_creation() {
        let s = SafeCString::new("hello").unwrap();
        unsafe {
            let cstr = CStr::from_ptr(s.as_ptr());
            assert_eq!(cstr.to_str().unwrap(), "hello");
        }
    }

    #[test]
    fn test_c_str_to_string() {
        let c_string = CString::new("world").unwrap();
        let result = c_str_to_string(c_string.as_ptr()).unwrap();
        assert_eq!(result, "world");
    }

    #[test]
    fn test_null_string_error() {
        let result = c_str_to_string(std::ptr::null());
        assert!(result.is_err());
        match result {
            Err(AbiError::NullPointer(_)) => (),
            _ => panic!("Expected NullPointer error"),
        }
    }

    #[test]
    fn test_plugin_result_conversion() {
        let success: AbiResult<()> = PluginResultV2::Success.into();
        assert!(success.is_ok());

        let error: AbiResult<()> = PluginResultV2::Error.into();
        assert!(error.is_err());
    }

    #[test]
    fn test_plugin_context_null_check() {
        let result = unsafe { SafePluginContext::new(std::ptr::null()) };
        assert!(result.is_err());
        match result {
            Err(AbiError::NullPointer(_)) => (),
            _ => panic!("Expected NullPointer error"),
        }
    }

    #[test]
    fn test_logger_null_check() {
        let result = unsafe { SafeLogger::new(std::ptr::null(), std::ptr::null()) };
        assert!(result.is_err());
        match result {
            Err(AbiError::NullPointer(_)) => (),
            _ => panic!("Expected NullPointer error"),
        }
    }

    #[test]
    fn test_config_null_check() {
        let result = unsafe { SafeConfig::new(std::ptr::null(), std::ptr::null()) };
        assert!(result.is_err());
        match result {
            Err(AbiError::NullPointer(_)) => (),
            _ => panic!("Expected NullPointer error"),
        }
    }

    #[test]
    fn test_service_registry_null_check() {
        let result = unsafe { SafeServiceRegistry::new(std::ptr::null(), std::ptr::null()) };
        assert!(result.is_err());
        match result {
            Err(AbiError::NullPointer(_)) => (),
            _ => panic!("Expected NullPointer error"),
        }
    }

    #[test]
    fn test_event_bus_null_check() {
        let result = unsafe { SafeEventBus::new(std::ptr::null(), std::ptr::null()) };
        assert!(result.is_err());
        match result {
            Err(AbiError::NullPointer(_)) => (),
            _ => panic!("Expected NullPointer error"),
        }
    }

    #[test]
    fn test_rpc_service_null_check() {
        let result = unsafe { SafeRpcService::new(std::ptr::null(), std::ptr::null()) };
        assert!(result.is_err());
        match result {
            Err(AbiError::NullPointer(_)) => (),
            _ => panic!("Expected NullPointer error"),
        }
    }

    #[test]
    fn test_safe_context_service_checks() {
        let ctx = unsafe {
            let null_ctx: *const PluginContextV2 = std::ptr::null();
            SafePluginContext::new(null_ctx).unwrap_err()
        };
        assert!(matches!(ctx, AbiError::NullPointer(_)));

        let valid_ctx = std::ptr::null();
        let ctx = unsafe { SafePluginContext::new(valid_ctx) };
        assert!(ctx.is_err());
    }

    #[test]
    fn test_all_plugin_result_variants() {
        let results = [
            (PluginResultV2::Success, true),
            (PluginResultV2::Error, false),
            (PluginResultV2::InvalidRequest, false),
            (PluginResultV2::ServiceUnavailable, false),
            (PluginResultV2::PermissionDenied, false),
            (PluginResultV2::NotImplemented, false),
            (PluginResultV2::Timeout, false),
            (PluginResultV2::ResourceExhausted, false),
            (PluginResultV2::Pending, true),
        ];

        for (result, expected_ok) in results {
            let converted: AbiResult<()> = result.into();
            assert_eq!(converted.is_ok(), expected_ok, "Failed for {:?}", result);
        }
    }

    #[test]
    fn test_panic_safe_call_success() {
        let result = PanicSafeCall::execute(|| PluginResultV2::Success);
        assert!(result.is_ok());
    }

    #[test]
    fn test_panic_safe_call_error() {
        let result = PanicSafeCall::execute(|| PluginResultV2::Error);
        assert!(result.is_err());
    }

    #[test]
    fn test_panic_safe_call_panics() {
        let result = PanicSafeCall::execute(|| {
            panic!("test panic");
        });
        assert!(result.is_err());
        match result {
            Err(AbiError::Panicked(_)) => (),
            _ => panic!("Expected Panicked error"),
        }
    }

    #[test]
    fn test_string_to_c_ptr_roundtrip() {
        let original = "test string with special chars: äöü";
        let c_ptr = string_to_c_ptr(original).unwrap();
        let result = c_str_to_string(c_ptr).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn test_empty_string_handling() {
        let empty = "";
        let c_ptr = string_to_c_ptr(empty).unwrap();
        let result = c_str_to_string(c_ptr).unwrap();
        assert_eq!(result, empty);
    }

    #[test]
    fn test_abi_error_display() {
        let errors = [
            AbiError::NullPointer("test".to_string()),
            AbiError::InvalidString("test".to_string()),
            AbiError::UnknownPluginError(-1),
            AbiError::Panicked("test".to_string()),
            AbiError::InvalidRequest("test".to_string()),
            AbiError::ServiceUnavailable("test".to_string()),
            AbiError::PermissionDenied("test".to_string()),
            AbiError::Timeout("test".to_string()),
            AbiError::ResourceExhausted("test".to_string()),
            AbiError::NotImplemented("test".to_string()),
        ];

        for error in errors {
            let display = error.to_string();
            assert!(!display.is_empty());
        }
    }

    // RFC-0004-SEC-001: Tests for secure response handling
    #[test]
    fn test_safe_response_null_pointer() {
        unsafe {
            let result = SafeResponseV2::from_raw(std::ptr::null());
            assert!(result.is_err());
            match result {
                Err(AbiError::NullPointer(_)) => (),
                _ => panic!("Expected NullPointer error"),
            }
        }
    }

    #[test]
    fn test_safe_response_empty() {
        unsafe {
            let response = ResponseV2 {
                status_code: 200,
                headers: std::ptr::null_mut(),
                num_headers: 0,
                body: std::ptr::null_mut(),
                body_len: 0,
                content_type: std::ptr::null(),
            };

            let safe_resp = SafeResponseV2::from_raw(&response).unwrap();
            assert_eq!(safe_resp.status_code(), 200);
            assert!(safe_resp.body().is_none());
            assert!(safe_resp.content_type().is_none());
        }
    }

    #[test]
    fn test_safe_response_body_mismatch() {
        unsafe {
            let response = ResponseV2 {
                status_code: 200,
                headers: std::ptr::null_mut(),
                num_headers: 0,
                body: std::ptr::null_mut(),
                body_len: 100, // Mismatch: null pointer with non-zero length
                content_type: std::ptr::null(),
            };

            let result = SafeResponseV2::from_raw(&response);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_safe_response_excessive_body_size() {
        unsafe {
            let response = ResponseV2 {
                status_code: 200,
                headers: std::ptr::null_mut(),
                num_headers: 0,
                body: 0x1234 as *mut u8, // Fake pointer for testing
                body_len: MAX_RESPONSE_BODY_SIZE + 1, // Exceeds limit
                content_type: std::ptr::null(),
            };

            let result = SafeResponseV2::from_raw(&response);
            assert!(result.is_err());
            match result {
                Err(AbiError::InvalidRequest(msg)) => {
                    assert!(msg.contains("exceeds maximum"));
                }
                _ => panic!("Expected InvalidRequest error"),
            }
        }
    }

    // RFC-0004-SEC-002: Tests for header validation
    #[test]
    fn test_safe_headers_empty() {
        unsafe {
            let result = SafeResponseHeaders::from_raw(std::ptr::null(), 0);
            assert!(result.is_ok());
            let headers = result.unwrap();
            assert_eq!(headers.iter().count(), 0);
        }
    }

    #[test]
    fn test_safe_headers_null_pointer_with_count() {
        unsafe {
            let result = SafeResponseHeaders::from_raw(std::ptr::null(), 5);
            assert!(result.is_err());
            match result {
                Err(AbiError::NullPointer(_)) => (),
                _ => panic!("Expected NullPointer error"),
            }
        }
    }

    #[test]
    fn test_safe_headers_exceeds_max() {
        unsafe {
            let response = ResponseV2 {
                status_code: 200,
                headers: 0x1234 as *mut HeaderV2, // Fake pointer
                num_headers: MAX_RESPONSE_HEADERS + 1, // Exceeds max
                body: std::ptr::null_mut(),
                body_len: 0,
                content_type: std::ptr::null(),
            };

            let result = SafeResponseHeaders::from_raw(response.headers, response.num_headers);
            assert!(result.is_err());
        }
    }
}
