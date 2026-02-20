// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

/// ABI Loader - RFC-0004 Plugin Loading with Complete ABI Validation
/// This module handles the complete lifecycle of loading and validating plugins
/// against the RFC-0004 ABI v2.0 specification, including:
///
/// - ABI version negotiation
/// - Complete ABI validation
/// - Safe function resolution
/// - Symbol loading with proper error handling
/// - Plugin metadata inspection
/// - Capability discovery
use crate::v2_spec::*;
use libloading::{Library, Symbol};
use std::ffi::{c_char, CStr};
use std::ptr::addr_of;

/// Error type for ABI loader operations
#[derive(Debug, Clone)]
pub enum AbiLoaderError {
    InvalidLibrary(String),
    SymbolNotFound(String),
    InvalidSymbol(String),
    UnsupportedAbiVersion(String),
    MissingRequiredSymbol(String),
    ValidationFailed(String),
    LoadError(String),
    /// RFC-0006: Schema validation error
    SchemaValidationError(String),
    /// RFC-0006: Invalid schema format
    InvalidSchema(String),
    /// RFC-0006: Config validation failed against schema
    ConfigValidationFailed { errors: Vec<String> },
}

impl std::fmt::Display for AbiLoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AbiLoaderError::InvalidLibrary(s) => write!(f, "Invalid library: {}", s),
            AbiLoaderError::SymbolNotFound(s) => write!(f, "Symbol not found: {}", s),
            AbiLoaderError::InvalidSymbol(s) => write!(f, "Invalid symbol: {}", s),
            AbiLoaderError::UnsupportedAbiVersion(s) => write!(f, "Unsupported ABI version: {}", s),
            AbiLoaderError::MissingRequiredSymbol(s) => write!(f, "Missing required symbol: {}", s),
            AbiLoaderError::ValidationFailed(s) => write!(f, "Validation failed: {}", s),
            AbiLoaderError::LoadError(s) => write!(f, "Load error: {}", s),
            AbiLoaderError::SchemaValidationError(s) => {
                write!(f, "Schema validation error: {}", s)
            }
            AbiLoaderError::InvalidSchema(s) => write!(f, "Invalid schema: {}", s),
            AbiLoaderError::ConfigValidationFailed { errors } => {
                write!(f, "Config validation failed: {}", errors.join(", "))
            }
        }
    }
}

impl std::error::Error for AbiLoaderError {}

pub type AbiLoaderResult<T> = Result<T, AbiLoaderError>;

/// ABI Version Information
#[derive(Debug, Clone, PartialEq)]
pub enum AbiVersion {
    V1,
    V2,
    Unknown(String),
}

impl AbiVersion {
    /// Parse ABI version from string
    pub fn from_str(s: &str) -> Self {
        match s {
            "1.0" | "1" => AbiVersion::V1,
            "2.0" | "2" => AbiVersion::V2,
            other => AbiVersion::Unknown(other.to_string()),
        }
    }

    /// Check if version is supported
    pub fn is_supported(&self) -> bool {
        matches!(self, AbiVersion::V2 | AbiVersion::V1)
    }
}

/// Plugin capabilities discovered from metadata
#[derive(Debug, Clone)]
pub struct PluginCapabilities {
    pub supports_hot_reload: bool,
    pub supports_async: bool,
    pub supports_streaming: bool,
    pub max_concurrency: usize,
    pub requires_services: Vec<String>,
    pub provides_services: Vec<String>,
    pub capabilities: Vec<String>,
}

/// ABI V2 Plugin Loader
#[allow(dead_code)]
pub struct AbiV2PluginLoader {
    library: Library,
    info: *const PluginInfoV2,
    init_fn: PluginInitFnV2,
    shutdown_fn: PluginShutdownFnV2,
    handle_request_fn: PluginHandleRequestFnV2,
    handle_event_fn: Option<PluginHandleEventFnV2>,
    health_check_fn: Option<PluginHealthCheckFnV2>,
    metrics_fn: Option<PluginGetMetricsFnV2>,
    /// Configuration schema function - Optional (RFC-0006)
    config_schema_fn: Option<PluginGetConfigSchemaJsonFn>,
    /// Billing metrics function - Optional (RFC-ARCH Section 10.3)
    billing_metrics_fn: Option<PluginGetBillingMetricsFnV2>,
}

impl AbiV2PluginLoader {
    /// Load a plugin from a shared library and validate ABI v2 compliance
    pub fn load<P: AsRef<std::ffi::OsStr>>(path: P) -> AbiLoaderResult<Self> {
        // Load the library
        let library = unsafe {
            Library::new(&path)
                .map_err(|e: libloading::Error| AbiLoaderError::InvalidLibrary(e.to_string()))?
        };

        // Validate plugin metadata
        let info_fn = unsafe {
            library
                .get::<Symbol<PluginGetInfoFnV2>>(b"plugin_get_info_v2")
                .map_err(|_| {
                    AbiLoaderError::MissingRequiredSymbol("plugin_get_info_v2".to_string())
                })?
        };

        #[allow(unused_unsafe)]
        let info = unsafe { (*info_fn)() };
        if info.is_null() {
            return Err(AbiLoaderError::ValidationFailed(
                "plugin_get_info_v2 returned null".to_string(),
            ));
        }

        // Validate ABI version
        #[allow(unused_unsafe)]
        let abi_version_str = unsafe {
            if (*info).abi_version.is_null() {
                return Err(AbiLoaderError::UnsupportedAbiVersion(
                    "abi_version is null".to_string(),
                ));
            }
            CStr::from_ptr((*info).abi_version)
                .to_str()
                .map_err(|e| AbiLoaderError::ValidationFailed(e.to_string()))?
        };

        let abi_version = AbiVersion::from_str(abi_version_str);
        if !abi_version.is_supported() {
            return Err(AbiLoaderError::UnsupportedAbiVersion(
                abi_version_str.to_string(),
            ));
        }

        // Load required functions - extract function pointers via unsafe transmute to avoid lifetime issues
        let init_fn_sym = unsafe {
            library
                .get::<Symbol<PluginInitFnV2>>(b"plugin_init_v2")
                .map_err(|_| AbiLoaderError::MissingRequiredSymbol("plugin_init_v2".to_string()))?
        };
let init_fn: PluginInitFnV2 = unsafe { *(addr_of!(*init_fn_sym) as *const _) };

        let shutdown_fn_sym = unsafe {
            library
                .get::<Symbol<PluginShutdownFnV2>>(b"plugin_shutdown_v2")
                .map_err(|_| {
                    AbiLoaderError::MissingRequiredSymbol("plugin_shutdown_v2".to_string())
                })?
        };
        let shutdown_fn: PluginShutdownFnV2 =
            unsafe { *(addr_of!(*shutdown_fn_sym) as *const _) };

        let handle_request_fn_sym = unsafe {
            library
                .get::<Symbol<PluginHandleRequestFnV2>>(b"plugin_handle_request_v2")
                .map_err(|_| {
                    AbiLoaderError::MissingRequiredSymbol("plugin_handle_request_v2".to_string())
                })?
        };
        let handle_request_fn: PluginHandleRequestFnV2 =
            unsafe { *(addr_of!(*handle_request_fn_sym) as *const _) };

        // Load optional functions
        let handle_event_fn: Option<PluginHandleEventFnV2> = unsafe {
            library
                .get::<Symbol<PluginHandleEventFnV2>>(b"plugin_handle_event_v2")
                .ok()
                .map(|sym| *(addr_of!(*sym) as *const _))
        };

        let health_check_fn: Option<PluginHealthCheckFnV2> = unsafe {
            library
                .get::<Symbol<PluginHealthCheckFnV2>>(b"plugin_health_check_v2")
                .ok()
                .map(|sym| *(addr_of!(*sym) as *const _))
        };

        let metrics_fn: Option<PluginGetMetricsFnV2> = unsafe {
            library
                .get::<Symbol<PluginGetMetricsFnV2>>(b"plugin_get_metrics_v2")
                .ok()
                .map(|sym| *(addr_of!(*sym) as *const _))
        };

        // Load optional config schema function (RFC-0006)
        let config_schema_fn: Option<PluginGetConfigSchemaJsonFn> = unsafe {
            library
                .get::<Symbol<PluginGetConfigSchemaJsonFn>>(b"plugin_get_config_schema_json")
                .ok()
                .map(|sym| *(addr_of!(*sym) as *const _))
        };

        // Load optional billing metrics function (RFC-ARCH Section 10.3)
        let billing_metrics_fn: Option<PluginGetBillingMetricsFnV2> = unsafe {
            library
                .get::<Symbol<PluginGetBillingMetricsFnV2>>(b"plugin_get_billing_metrics_v2")
                .ok()
                .map(|sym| *(addr_of!(*sym) as *const _))
        };

        Ok(AbiV2PluginLoader {
            library,
            info,
            init_fn,
            shutdown_fn,
            handle_request_fn,
            handle_event_fn,
            health_check_fn,
            metrics_fn,
            config_schema_fn,
            billing_metrics_fn,
        })
    }

    /// Get plugin metadata
    pub fn get_info(&self) -> AbiLoaderResult<PluginMetadata> {
        unsafe {
            let info = &*self.info;

            let name = if info.name.is_null() {
                String::new()
            } else {
                CStr::from_ptr(info.name)
                    .to_str()
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            };

            let version = if info.version.is_null() {
                String::new()
            } else {
                CStr::from_ptr(info.version)
                    .to_str()
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            };

            Ok(PluginMetadata {
                name,
                version,
                abi_version: AbiVersion::V2,
            })
        }
    }

    /// Get plugin capabilities
    pub fn get_capabilities(&self) -> AbiLoaderResult<PluginCapabilities> {
        unsafe {
            let info = &*self.info;

            Ok(PluginCapabilities {
                supports_hot_reload: info.supports_hot_reload,
                supports_async: info.supports_async,
                supports_streaming: info.supports_streaming,
                max_concurrency: info.max_concurrency,
                requires_services: Vec::new(), // TODO: Parse from info
                provides_services: Vec::new(), // TODO: Parse from info
                capabilities: Vec::new(),      // TODO: Parse from info
            })
        }
    }

    /// Call plugin init function
    pub fn init(&self, context: *const PluginContextV2) -> AbiLoaderResult<()> {
        let result = (self.init_fn)(context);
        match result {
            PluginResultV2::Success => Ok(()),
            _ => Err(AbiLoaderError::LoadError(format!(
                "Plugin init failed with result: {:?}",
                result
            ))),
        }
    }

    /// Call plugin shutdown function
    pub fn shutdown(&self, context: *const PluginContextV2) -> AbiLoaderResult<()> {
        let result = (self.shutdown_fn)(context);
        match result {
            PluginResultV2::Success => Ok(()),
            _ => Err(AbiLoaderError::LoadError(format!(
                "Plugin shutdown failed with result: {:?}",
                result
            ))),
        }
    }

    /// Call plugin request handler
    pub fn handle_request(
        &self,
        context: *const PluginContextV2,
        request: *const RequestV2,
        response: *mut ResponseV2,
    ) -> AbiLoaderResult<()> {
        let result = (self.handle_request_fn)(context, request, response);
        match result {
            PluginResultV2::Success => Ok(()),
            _ => Err(AbiLoaderError::LoadError(format!(
                "Plugin request handler failed with result: {:?}",
                result
            ))),
        }
    }

    /// Call plugin event handler if available
    pub fn handle_event(
        &self,
        context: *const PluginContextV2,
        event: *const EventV2,
    ) -> AbiLoaderResult<()> {
        match &self.handle_event_fn {
            Some(handler) => {
                let result = (handler)(context, event);
                match result {
                    PluginResultV2::Success => Ok(()),
                    _ => Err(AbiLoaderError::LoadError(format!(
                        "Plugin event handler failed with result: {:?}",
                        result
                    ))),
                }
            }
            None => Err(AbiLoaderError::MissingRequiredSymbol(
                "plugin_handle_event_v2 not available".to_string(),
            )),
        }
    }

    /// Call plugin health check if available
    pub fn check_health(&self, context: *const PluginContextV2) -> AbiLoaderResult<HealthStatus> {
        match &self.health_check_fn {
            Some(fn_ref) => Ok((fn_ref)(context)),
            None => Ok(HealthStatus::Unknown),
        }
    }

    /// Get plugin metrics if available
    pub fn get_metrics(
        &self,
        context: *const PluginContextV2,
    ) -> AbiLoaderResult<Option<*const PluginMetrics>> {
        match &self.metrics_fn {
            Some(fn_ref) => {
                let metrics = (fn_ref)(context);
                if metrics.is_null() {
                    Ok(None)
                } else {
                    Ok(Some(metrics))
                }
            }
            None => Ok(None),
        }
    }

    /// Check if plugin has event handler
    pub fn has_event_handler(&self) -> bool {
        self.handle_event_fn.is_some()
    }

    /// Check if plugin has health check
    pub fn has_health_check(&self) -> bool {
        self.health_check_fn.is_some()
    }

    /// Check if plugin has metrics
    pub fn has_metrics(&self) -> bool {
        self.metrics_fn.is_some()
    }

    // ========================================================================
    // RFC-0006: Configuration Schema Support
    // ========================================================================

    /// Get the plugin's configuration schema if available (RFC-0006)
    ///
    /// Returns a JSON Schema string describing the plugin's configuration
    /// structure, or None if the plugin doesn't export a schema.
    ///
    /// # Example
    /// ```ignore
    /// if let Some(schema_ptr) = loader.get_config_schema() {
    ///     let schema = unsafe { CStr::from_ptr(schema_ptr).to_str().unwrap() };
    ///     // Parse and validate config against schema
    /// }
    /// ```
    pub fn get_config_schema(&self) -> Option<*const c_char> {
        match &self.config_schema_fn {
            Some(fn_ref) => {
                let schema_ptr = (fn_ref)();
                if schema_ptr.is_null() {
                    None
                } else {
                    Some(schema_ptr)
                }
            }
            None => None,
        }
    }

    /// Get the plugin's configuration schema as a Rust string (RFC-0006)
    ///
    /// Convenience method that converts the C string to a Rust String.
    /// Returns None if the plugin doesn't export a schema or if the
    /// conversion fails.
    pub fn get_config_schema_string(&self) -> Option<String> {
        self.get_config_schema().and_then(|ptr| {
            unsafe { CStr::from_ptr(ptr) }
                .to_str()
                .ok()
                .map(|s| s.to_string())
        })
    }

    /// Check if plugin exports a configuration schema (RFC-0006)
    pub fn has_config_schema(&self) -> bool {
        self.config_schema_fn.is_some()
    }

    // ========================================================================
    // RFC-ARCH Section 10.3: Billing Metrics Support
    // ========================================================================

    /// Get billing metrics from the plugin if available (RFC-ARCH Section 10.3)
    ///
    /// Returns a BillingMetricsReport containing usage-based billing information,
    /// or None if the plugin doesn't support billing metrics.
    ///
    /// # Example
    /// ```ignore
    /// if let Some(report_ptr) = loader.get_billing_metrics(&context) {
    ///     let report = unsafe { &*report_ptr };
    ///     // Process billing metrics
    /// }
    /// ```
    pub fn get_billing_metrics(
        &self,
        context: *const PluginContextV2,
    ) -> Option<*const BillingMetricsReport> {
        match &self.billing_metrics_fn {
            Some(fn_ref) => {
                let report_ptr = (fn_ref)(context);
                if report_ptr.is_null() {
                    None
                } else {
                    Some(report_ptr)
                }
            }
            None => None,
        }
    }

    /// Check if plugin supports billing metrics (RFC-ARCH Section 10.3)
    pub fn has_billing_metrics(&self) -> bool {
        self.billing_metrics_fn.is_some()
    }
}

/// Plugin metadata
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub abi_version: AbiVersion,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abi_version_parsing() {
        assert_eq!(AbiVersion::from_str("2.0"), AbiVersion::V2);
        assert_eq!(AbiVersion::from_str("1.0"), AbiVersion::V1);
        assert_eq!(
            AbiVersion::from_str("3.0"),
            AbiVersion::Unknown("3.0".to_string())
        );
    }

    #[test]
    fn test_abi_version_support() {
        assert!(AbiVersion::V2.is_supported());
        assert!(AbiVersion::V1.is_supported());
        assert!(!AbiVersion::Unknown("3.0".to_string()).is_supported());
    }

    #[test]
    fn test_plugin_capabilities() {
        let caps = PluginCapabilities {
            supports_hot_reload: true,
            supports_async: true,
            supports_streaming: false,
            max_concurrency: 10,
            requires_services: vec!["logger".to_string()],
            provides_services: vec!["database".to_string()],
            capabilities: vec![],
        };

        assert!(caps.supports_hot_reload);
        assert!(caps.supports_async);
        assert!(!caps.supports_streaming);
        assert_eq!(caps.max_concurrency, 10);
    }

    // ========================================================================
    // RFC-0006: Configuration Schema Tests
    // ========================================================================

    #[test]
    fn test_abi_loader_error_display_rfc0006() {
        // Test SchemaValidationError display
        let err = AbiLoaderError::SchemaValidationError("test schema error".to_string());
        assert!(err.to_string().contains("Schema validation error"));
        assert!(err.to_string().contains("test schema error"));

        // Test InvalidSchema display
        let err = AbiLoaderError::InvalidSchema("malformed schema".to_string());
        assert!(err.to_string().contains("Invalid schema"));
        assert!(err.to_string().contains("malformed schema"));

        // Test ConfigValidationFailed display
        let err = AbiLoaderError::ConfigValidationFailed {
            errors: vec!["missing field 'api_key'".to_string(), "invalid type".to_string()],
        };
        let display = err.to_string();
        assert!(display.contains("Config validation failed"));
        assert!(display.contains("missing field 'api_key'"));
        assert!(display.contains("invalid type"));
    }

    #[test]
    fn test_abi_loader_error_is_error() {
        // Verify that AbiLoaderError implements std::error::Error
        fn assert_error<E: std::error::Error>() {}
        assert_error::<AbiLoaderError>();
    }

    #[test]
    fn test_abi_loader_error_clone() {
        // Verify that AbiLoaderError can be cloned
        let err1 = AbiLoaderError::SchemaValidationError("test".to_string());
        let err2 = err1.clone();
        assert_eq!(err1.to_string(), err2.to_string());
    }

    #[test]
    fn test_abi_loader_error_debug() {
        // Verify that AbiLoaderError implements Debug
        let err = AbiLoaderError::ConfigValidationFailed {
            errors: vec!["error1".to_string()],
        };
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("ConfigValidationFailed"));
    }
}
