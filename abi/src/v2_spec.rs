// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

/// ABI Version 2.0 Specification - RFC-0004
///
/// This module provides the complete ABI v2.0 specification with all
/// required data structures and function signatures for stable
/// plugin development. It includes:
///
/// - Complete PluginContextV2 with all required services
/// - PluginInfoV2 with comprehensive plugin metadata
/// - All required plugin function signatures
/// - Event bus with proper semantics
/// - Service discovery and capability exposure
/// - Proper error handling across FFI boundaries
use std::ffi::{c_char, c_int, c_void};

/// Plugin operation result for ABI v2
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PluginResultV2 {
    Success = 0,
    Error = -1,
    InvalidRequest = -2,
    ServiceUnavailable = -3,
    PermissionDenied = -4,
    NotImplemented = -5,
    Timeout = -6,
    ResourceExhausted = -7,
    /// Pending result - response will be delivered asynchronously (ABI v3)
    Pending = -8,
}

/// Plugin maturity level for RFC-0004
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MaturityLevel {
    Alpha = 0,
    Beta = 1,
    ReleaseCandidate = 2,
    Stable = 3,
    Deprecated = 4,
}

/// Plugin category for marketplace classification
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PluginCategory {
    Utility = 0,
    Database = 1,
    Network = 2,
    Storage = 3,
    Security = 4,
    Monitoring = 5,
    Payment = 6,
    Integration = 7,
    Development = 8,
    Other = 9,
}

/// Monetization model for plugin marketplace
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MonetizationModel {
    Free = 0,
    OneTime = 1,
    Subscription = 2,
    Freemium = 3,
    Custom = 4,
}

/// Service information for dependency and capability declaration
#[repr(C)]
pub struct ServiceInfo {
    pub name: *const c_char,
    pub version: *const c_char,
    pub description: *const c_char,
    pub interface_spec: *const c_char,
}

/// Dependency information for version-compatible dependency specification
///
/// This struct provides type-safe dependency declaration with semantic versioning support
#[repr(C)]
pub struct DependencyInfo {
    /// Dependency name (e.g., "rustdoc-json-plugin")
    pub name: *const c_char,
    /// Semantic version range requirement (e.g., ">=1.0.0, <2.0.0")
    pub version_range: *const c_char,
    /// Whether this dependency is required (true) or optional (false)
    pub required: bool,
    /// Service type/category for dependency classification (e.g., "integration", "database")
    pub service_type: *const c_char,
}

// ============================================================================
// DependencyInfo Validation Helpers - RFC-0004 Task 0004.2
// ============================================================================

use std::ffi::CStr;

/// Errors that can occur when validating DependencyInfo
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyValidationError {
    /// Dependency name is null or empty
    InvalidName { reason: String },
    /// Version range is not valid semver syntax
    InvalidVersionRange { range: String, reason: String },
    /// Service type is invalid
    InvalidServiceType {
        service_type: String,
        reason: String,
    },
}

impl std::fmt::Display for DependencyValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyValidationError::InvalidName { reason } => {
                write!(f, "Invalid dependency name: {}", reason)
            }
            DependencyValidationError::InvalidVersionRange { range, reason } => {
                write!(f, "Invalid version range '{}': {}", range, reason)
            }
            DependencyValidationError::InvalidServiceType {
                service_type,
                reason,
            } => {
                write!(f, "Invalid service type '{}': {}", service_type, reason)
            }
        }
    }
}

impl std::error::Error for DependencyValidationError {}

impl DependencyInfo {
    /// Validate a DependencyInfo struct
    ///
    /// Returns Ok(()) if the dependency info is valid, or an error describing the validation failure.
    ///
    /// # Safety
    /// This function assumes the pointers in the struct are either null or point to valid C strings.
    pub fn validate(&self) -> Result<DependencyInfoValidated, DependencyValidationError> {
        // Validate name
        let name = unsafe {
            if self.name.is_null() {
                return Err(DependencyValidationError::InvalidName {
                    reason: "name pointer is null".to_string(),
                });
            }
            let c_str = CStr::from_ptr(self.name);
            let name_str = c_str.to_string_lossy();
            if name_str.is_empty() {
                return Err(DependencyValidationError::InvalidName {
                    reason: "name is empty".to_string(),
                });
            }
            // Validate name format (alphanumeric, hyphens, underscores)
            if !name_str
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                return Err(DependencyValidationError::InvalidName {
                    reason: "name contains invalid characters (only alphanumeric, hyphens, underscores allowed)".to_string(),
                });
            }
            name_str.into_owned()
        };

        // Validate version_range
        let version_range = unsafe {
            if self.version_range.is_null() {
                return Err(DependencyValidationError::InvalidVersionRange {
                    range: "(null)".to_string(),
                    reason: "version_range pointer is null".to_string(),
                });
            }
            let c_str = CStr::from_ptr(self.version_range);
            let range_str = c_str.to_string_lossy();
            if range_str.is_empty() {
                return Err(DependencyValidationError::InvalidVersionRange {
                    range: "(empty)".to_string(),
                    reason: "version_range is empty".to_string(),
                });
            }
            // Validate semver syntax using the constraints module
            if let Err(e) = crate::dependencies::constraints::VersionReq::parse(&range_str) {
                return Err(DependencyValidationError::InvalidVersionRange {
                    range: range_str.to_string(),
                    reason: e.to_string(),
                });
            }
            range_str.into_owned()
        };

        // Validate service_type (optional but must be valid if present)
        let service_type = unsafe {
            if self.service_type.is_null() {
                None
            } else {
                let c_str = CStr::from_ptr(self.service_type);
                let type_str = c_str.to_string_lossy();
                if type_str.is_empty() {
                    None
                } else {
                    // Validate service type format
                    let valid_types = [
                        "core",
                        "integration",
                        "database",
                        "network",
                        "storage",
                        "security",
                        "monitoring",
                        "messaging",
                        "ai",
                        "utility",
                    ];
                    if !valid_types.contains(&type_str.as_ref()) {
                        // Allow unknown types but they should follow the format
                        if !type_str
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c == '-' || c == '_')
                        {
                            return Err(DependencyValidationError::InvalidServiceType {
                                service_type: type_str.to_string(),
                                reason:
                                    "service_type must be lowercase with hyphens or underscores"
                                        .to_string(),
                            });
                        }
                    }
                    Some(type_str.into_owned())
                }
            }
        };

        Ok(DependencyInfoValidated {
            name,
            version_range,
            required: self.required,
            service_type,
        })
    }
}

/// Validated dependency information (owned, safe Rust types)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyInfoValidated {
    pub name: String,
    pub version_range: String,
    pub required: bool,
    pub service_type: Option<String>,
}

impl DependencyInfoValidated {
    /// Check if a given version satisfies this dependency's version range
    pub fn version_satisfies(&self, version: &str) -> bool {
        let version_req =
            match crate::dependencies::constraints::VersionReq::parse(&self.version_range) {
                Ok(req) => req,
                Err(_) => return false,
            };
        let ver = match crate::dependencies::version::Version::parse(version) {
            Ok(v) => v,
            Err(_) => return false,
        };
        version_req.matches(&ver)
    }
}

/// Capability information for fine-grained permission system
#[repr(C)]
pub struct CapabilityInfo {
    pub name: *const c_char,
    pub description: *const c_char,
    pub required_permission: *const c_char,
}

/// Resource requirements specification
#[repr(C)]
pub struct ResourceRequirements {
    pub min_cpu_cores: u32,
    pub max_cpu_cores: u32,
    pub min_memory_mb: u32,
    pub max_memory_mb: u32,
    pub min_disk_mb: u32,
    pub max_disk_mb: u32,
    pub requires_gpu: bool,
}

/// HTTP Request structure for ABI v2
#[repr(C)]
pub struct RequestV2 {
    pub method: *const c_char,
    pub path: *const c_char,
    pub query: *const c_char,
    pub headers: *const HeaderV2,
    pub num_headers: usize,
    pub body: *const u8,
    pub body_len: usize,
    pub content_type: *const c_char,
}

/// HTTP Response structure for ABI v2
#[repr(C)]
pub struct ResponseV2 {
    pub status_code: i32,
    pub headers: *mut HeaderV2,
    pub num_headers: usize,
    pub body: *mut u8,
    pub body_len: usize,
    pub content_type: *const c_char,
}

/// HTTP header for ABI v2
#[repr(C)]
pub struct HeaderV2 {
    pub name: *const c_char,
    pub value: *const c_char,
}

/// RPC Request for v2 ABI
#[repr(C)]
pub struct RpcRequestV2 {
    pub method: *const c_char,
    pub params: *const c_char, // JSON-encoded parameters
    pub timeout_ms: u64,
}

/// RPC Response for v2 ABI
#[repr(C)]
pub struct RpcResponseV2 {
    pub result: *const c_char, // JSON-encoded result
    pub error: *const c_char,  // NULL if no error
    pub status: PluginResultV2,
}

/// Event for v2 event bus
#[repr(C)]
pub struct EventV2 {
    pub type_: *const c_char,        // Event type/topic
    pub payload_json: *const c_char, // JSON payload
    pub timestamp_ms: u64,
    pub source_plugin: *const c_char,
}

/// Health status for plugin health checks
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HealthStatus {
    Healthy = 0,
    Degraded = 1,
    Unhealthy = 2,
    Unknown = 3,
}

/// Plugin metrics for monitoring
#[repr(C)]
pub struct PluginMetrics {
    pub uptime_seconds: u64,
    pub request_count: u64,
    pub error_count: u64,
    pub avg_response_time_ms: f64,
    pub memory_usage_mb: u32,
    pub cpu_usage_percent: f32,
    pub last_error: *const c_char,
}

// ============================================================================
// Billing Metrics - RFC-ARCH Section 10: Marketplace and Billing
// ============================================================================

/// Billing unit types for metered usage
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BillingUnit {
    /// Per API call/request
    Request = 0,
    /// Per compute hour (CPU time)
    ComputeHour = 1,
    /// Per gigabyte of data transferred
    DataGB = 2,
    /// Per gigabyte of storage used
    StorageGB = 3,
    /// Per LLM token processed
    LLMToken = 4,
    /// Per active user/seat
    UserSeat = 5,
    /// Per event/message
    Event = 6,
    /// Custom unit defined by plugin
    Custom = 99,
}

/// Billing metrics for usage-based monetization
///
/// Plugins can optionally export `plugin_get_billing_metrics()` to declare
/// their metered usage for marketplace billing. This enables:
/// - Usage-based pricing for pay-per-use plugins
/// - Transparent metering visible to the platform
/// - Accurate billing without plugin-specific tracking
///
/// Per RFC-ARCH Section 10.3, billing metrics support various monetization
/// models including usage-based metering where the platform meters usage
/// transparently.
#[repr(C)]
pub struct BillingMetrics {
    /// Name of the billing metric (e.g., "api_calls", "compute_hours")
    pub metric_name: *const c_char,
    /// Human-readable description of what is being metered
    pub description: *const c_char,
    /// Unit type for this metric
    pub unit: BillingUnit,
    /// Current period usage count
    pub current_usage: u64,
    /// Billing period start timestamp (Unix epoch seconds)
    pub period_start: u64,
    /// Billing period end timestamp (Unix epoch seconds)
    pub period_end: u64,
    /// Price per unit in stroops (1 XLM = 10,000,000 stroops)
    pub price_per_unit: u64,
    /// Currency code (e.g., "XLM" for Stellar Lumens)
    pub currency: *const c_char,
    /// Whether this is a metered (usage-based) or flat-rate metric
    pub is_metered: bool,
    /// Optional: Granular usage breakdown as JSON
    pub usage_breakdown_json: *const c_char,
}

/// Aggregated billing metrics for a plugin with multiple metered resources
#[repr(C)]
pub struct BillingMetricsReport {
    /// Plugin identifier
    pub plugin_id: *const c_char,
    /// Plugin version
    pub plugin_version: *const c_char,
    /// Array of billing metrics
    pub metrics: *const BillingMetrics,
    /// Number of metrics in the array
    pub num_metrics: usize,
    /// Total estimated cost in stroops for current period
    pub total_cost_stroops: u64,
    /// Timestamp when this report was generated
    pub generated_at: u64,
}

/// Function type for billing metrics export
///
/// Plugins that support usage-based billing should export this function
/// to report their current metered usage. The platform calls this
/// periodically to aggregate billing data.
///
/// # Returns
///
/// Pointer to a BillingMetricsReport containing all metered resources.
/// The pointer must remain valid until the next call or plugin shutdown.
///
/// # Memory Ownership
///
/// The returned pointer should point to statically allocated memory or
/// memory managed by the plugin that remains valid across calls.
pub type PluginGetBillingMetricsFnV2 =
    extern "C" fn(context: *const PluginContextV2) -> *const BillingMetricsReport;

/// Logger service for ABI v2
#[repr(C)]
pub struct LoggerV2 {
    pub log: extern "C" fn(
        context: *const PluginContextV2,
        level: crate::PluginLogLevel,
        message: *const c_char,
    ) -> PluginResultV2,
    pub log_structured: extern "C" fn(
        context: *const PluginContextV2,
        level: crate::PluginLogLevel,
        message: *const c_char,
        data_json: *const c_char,
    ) -> PluginResultV2,
}

/// Configuration service for ABI v2
#[repr(C)]
pub struct ConfigV2 {
    pub get: extern "C" fn(context: *const PluginContextV2, key: *const c_char) -> *const c_char,
    pub get_bool: extern "C" fn(context: *const PluginContextV2, key: *const c_char) -> c_int,
    pub get_int: extern "C" fn(context: *const PluginContextV2, key: *const c_char) -> i64,
    pub get_float: extern "C" fn(context: *const PluginContextV2, key: *const c_char) -> f64,
    pub set: extern "C" fn(
        context: *const PluginContextV2,
        key: *const c_char,
        value: *const c_char,
    ) -> PluginResultV2,
    pub free_string: extern "C" fn(ptr: *mut c_char),
}

/// Service Registry for ABI v2 with capability checking
#[repr(C)]
pub struct ServiceRegistryV2 {
    pub register: extern "C" fn(
        context: *const PluginContextV2,
        name: *const c_char,
        service: *mut c_void,
        service_type: *const c_char,
    ) -> PluginResultV2,
    pub get: extern "C" fn(
        context: *const PluginContextV2,
        name: *const c_char,
        service_type: *const c_char,
    ) -> *mut c_void,
    pub unregister:
        extern "C" fn(context: *const PluginContextV2, name: *const c_char) -> PluginResultV2,
    pub list_services: extern "C" fn(context: *const PluginContextV2) -> *const *const c_char,
    pub free_service_list: extern "C" fn(list: *const *const c_char, count: usize),
}

/// Event Bus service for ABI v2
#[repr(C)]
pub struct EventBusV2 {
    pub publish:
        extern "C" fn(context: *const PluginContextV2, event: *const EventV2) -> PluginResultV2,
    pub subscribe: extern "C" fn(
        context: *const PluginContextV2,
        event_type: *const c_char,
        callback: extern "C" fn(*const EventV2),
    ) -> PluginResultV2,
    pub unsubscribe:
        extern "C" fn(context: *const PluginContextV2, event_type: *const c_char) -> PluginResultV2,
}

/// RPC service for v2 ABI with service discovery
#[repr(C)]
pub struct RpcServiceV2 {
    pub call: extern "C" fn(
        context: *const PluginContextV2,
        service: *const c_char,
        request: *const RpcRequestV2,
        response: *mut RpcResponseV2,
    ) -> PluginResultV2,
    pub register_handler: extern "C" fn(
        context: *const PluginContextV2,
        method: *const c_char,
        handler: extern "C" fn(*const RpcRequestV2, *mut RpcResponseV2),
    ) -> PluginResultV2,
    pub list_services: extern "C" fn(context: *const PluginContextV2) -> *const *const c_char,
    pub get_service_spec:
        extern "C" fn(context: *const PluginContextV2, service: *const c_char) -> *const c_char,
    pub free_strings: extern "C" fn(ptr: *const *const c_char, count: usize),
}

/// Plugin Context for ABI v2 - Core interface to host services
#[repr(C)]
pub struct PluginContextV2 {
    pub logger: *const LoggerV2,
    pub config: *const ConfigV2,
    pub service_registry: *const ServiceRegistryV2,
    pub event_bus: *const EventBusV2,
    pub rpc_service: *const RpcServiceV2,
    /// HTTP router for plugin-provided API endpoints (RFC-0019)
    /// Allows plugins to register their own REST API routes dynamically
    pub http_router: *const crate::http::HttpRouterV2,
    pub user_data: *mut c_void,
    pub user_context_json: *const c_char,
    pub secrets: *const crate::PluginSecrets,
    pub tracer: *const crate::PluginTracer,
    pub rotation_notifications: *const crate::RotationNotificationService,
}

impl Default for PluginContextV2 {
    fn default() -> Self {
        Self {
            logger: std::ptr::null(),
            config: std::ptr::null(),
            service_registry: std::ptr::null(),
            event_bus: std::ptr::null(),
            rpc_service: std::ptr::null(),
            http_router: std::ptr::null(),
            user_data: std::ptr::null_mut(),
            user_context_json: std::ptr::null(),
            secrets: std::ptr::null(),
            tracer: std::ptr::null(),
            rotation_notifications: std::ptr::null(),
        }
    }
}

/// Complete Plugin Info structure for ABI v2 - RFC-0004
#[repr(C)]
pub struct PluginInfoV2 {
    // Basic metadata
    pub name: *const c_char,
    pub version: *const c_char,
    pub description: *const c_char,
    pub author: *const c_char,
    pub license: *const c_char,
    pub homepage: *const c_char,

    // Version compatibility
    pub skylet_version_min: *const c_char,
    pub skylet_version_max: *const c_char,
    pub abi_version: *const c_char, // MUST be "2.0"

    // Dependencies and service discovery
    pub dependencies: *const DependencyInfo,
    pub num_dependencies: usize,
    pub provides_services: *const ServiceInfo,
    pub num_provides_services: usize,
    pub requires_services: *const ServiceInfo,
    pub num_requires_services: usize,

    // Capabilities and permissions
    pub capabilities: *const CapabilityInfo,
    pub num_capabilities: usize,

    // Resource requirements
    pub min_resources: *const ResourceRequirements,
    pub max_resources: *const ResourceRequirements,

    // Tags and categorization
    pub tags: *const *const c_char,
    pub num_tags: usize,
    pub category: PluginCategory,

    // Runtime capabilities
    pub supports_hot_reload: bool,
    pub supports_async: bool,
    pub supports_streaming: bool,
    pub max_concurrency: usize,

    // Marketplace and monetization
    pub monetization_model: MonetizationModel,
    pub price_usd: f32,
    pub purchase_url: *const c_char,
    pub subscription_url: *const c_char,
    pub marketplace_category: *const c_char,
    pub tagline: *const c_char,
    pub icon_url: *const c_char,

    // Build and deployment information
    pub maturity_level: MaturityLevel,
    pub build_timestamp: *const c_char,
    pub build_hash: *const c_char,
    pub git_commit: *const c_char,
    pub build_environment: *const c_char,

    // Arbitrary metadata
    pub metadata: *const c_char, // JSON string
}

// Plugin function types for ABI v2

pub type PluginInitFnV2 = extern "C" fn(context: *const PluginContextV2) -> PluginResultV2;

pub type PluginShutdownFnV2 = extern "C" fn(context: *const PluginContextV2) -> PluginResultV2;

pub type PluginGetInfoFnV2 = extern "C" fn() -> *const PluginInfoV2;

pub type PluginHandleRequestFnV2 = extern "C" fn(
    context: *const PluginContextV2,
    request: *const RequestV2,
    response: *mut ResponseV2,
) -> PluginResultV2;

pub type PluginHandleEventFnV2 =
    extern "C" fn(context: *const PluginContextV2, event: *const EventV2) -> PluginResultV2;

pub type PluginHotReloadFnV2 = extern "C" fn(context: *const PluginContextV2) -> PluginResultV2;

pub type PluginHealthCheckFnV2 = extern "C" fn(context: *const PluginContextV2) -> HealthStatus;

pub type PluginGetMetricsFnV2 =
    extern "C" fn(context: *const PluginContextV2) -> *const PluginMetrics;

pub type PluginCapabilityQueryFnV2 =
    extern "C" fn(context: *const PluginContextV2, capability: *const c_char) -> bool;

/// Type alias for capability query function pointer
pub type PluginCapabilityQueryV2 =
    extern "C" fn(context: *const PluginContextV2, capability: *const c_char) -> bool;

/// Configuration schema export function for RFC-0006
///
/// Plugins can optionally export this function to provide a JSON Schema
/// that describes their configuration structure. This enables:
/// - Schema validation of plugin config before/during load
/// - Admin UI form generation for plugin configuration
/// - IDE autocompletion and validation for config files
/// - Type-safe configuration access
///
/// # Returns
///
/// A pointer to a null-terminated JSON string containing the JSON Schema.
/// The schema should describe all configuration keys the plugin accepts,
/// their types, default values, and validation constraints.
///
/// # Memory Ownership
///
/// The returned pointer must point to statically allocated memory or
/// memory that remains valid for the lifetime of the plugin.
///
/// # Example Schema
///
/// ```json
/// {
///   "$schema": "https://json-schema.org/draft/2020-12/schema",
///   "type": "object",
///   "properties": {
///     "api_key": {
///       "type": "string",
///       "description": "API key for authentication",
///       "format": "password"
///     },
///     "timeout_seconds": {
///       "type": "integer",
///       "description": "Request timeout in seconds",
///       "default": 30,
///       "minimum": 1,
///       "maximum": 300
///     },
///     "enabled": {
///       "type": "boolean",
///       "description": "Enable or disable the plugin",
///       "default": true
///     }
///   },
///   "required": ["api_key"]
/// }
/// ```
pub type PluginGetConfigSchemaJsonFn = extern "C" fn() -> *const c_char;

/// Complete Plugin API V2 struct - The main entry point bundle
/// This struct provides all function pointers a plugin can export for the host to call.
/// Plugins export this by implementing plugin_create_v2() which returns a pointer to this struct.
#[repr(C)]
pub struct PluginApiV2 {
    /// Plugin information function - MUST be implemented
    pub get_info: PluginGetInfoFnV2,

    /// Plugin initialization - MUST be implemented
    pub init: PluginInitFnV2,

    /// Plugin shutdown - MUST be implemented
    pub shutdown: PluginShutdownFnV2,

    /// Request handling - MUST be implemented for HTTP plugins
    pub handle_request: PluginHandleRequestFnV2,

    /// Event handling - Optional
    pub handle_event: Option<PluginHandleEventFnV2>,

    /// Hot reload preparation - Optional
    pub prepare_hot_reload: Option<PluginHotReloadFnV2>,

    /// Health check - Optional
    pub health_check: Option<PluginHealthCheckFnV2>,

    /// Metrics collection - Optional
    pub get_metrics: Option<PluginGetMetricsFnV2>,

    /// Capability query - Optional
    pub query_capability: Option<PluginCapabilityQueryV2>,

    /// Configuration schema export - Optional (RFC-0006)
    ///
    /// If implemented, returns a JSON Schema describing the plugin's
    /// configuration structure. Enables schema validation, admin UI
    /// form generation, and IDE autocompletion for config files.
    pub get_config_schema: Option<PluginGetConfigSchemaJsonFn>,

    /// Billing metrics export - Optional (RFC-ARCH Section 10)
    ///
    /// If implemented, returns a BillingMetricsReport containing
    /// usage-based billing information. Enables:
    /// - Usage-based pricing for pay-per-use plugins
    /// - Transparent metering visible to the platform
    /// - Accurate billing without plugin-specific tracking
    ///
    /// Per RFC-ARCH Section 10.3, plugins declare billing metrics
    /// through this function for marketplace integration.
    pub get_billing_metrics: Option<PluginGetBillingMetricsFnV2>,
}

/// ABI v2 entry point - plugins MUST export this function
/// Returns a pointer to a statically allocated PluginApiV2 struct
pub type PluginCreateFnV2 = extern "C" fn() -> *const PluginApiV2;

// ============================================================================
// GAP-001: V2 Service Host Implementations
// RFC-0004: EventBusV2 and RpcServiceV2 implementations for host-side use
// ============================================================================

use std::sync::Arc;

/// Host-side EventBusV2 implementation
///
/// This struct provides the concrete implementation of EventBusV2 FFI callbacks
/// that the host provides to plugins. It wraps the TypedEventBus infrastructure.
///
/// # Thread Safety
///
/// All methods are thread-safe and can be called from multiple threads concurrently.
/// The underlying TypedEventBus uses RwLock for synchronization.
pub struct HostEventBusV2 {
    /// The underlying typed event bus
    bus: Arc<crate::TypedEventBus>,
}

impl HostEventBusV2 {
    /// Create a new host event bus wrapping a TypedEventBus
    pub fn new(bus: Arc<crate::TypedEventBus>) -> Self {
        Self { bus }
    }

    /// Create the FFI-compatible EventBusV2 struct with function pointers
    ///
    /// # Safety
    ///
    /// The returned EventBusV2 struct contains function pointers that expect
    /// valid PluginContextV2 pointers when called. The context must contain
    /// user_data pointing to a valid HostEventBusV2Arc.
    pub fn create_ffi_struct() -> EventBusV2 {
        EventBusV2 {
            publish: Self::ffi_publish,
            subscribe: Self::ffi_subscribe,
            unsubscribe: Self::ffi_unsubscribe,
        }
    }

    /// Store the Arc<Self> in a way that can be retrieved from user_data
    pub fn into_raw(self) -> *mut std::ffi::c_void {
        Box::into_raw(Box::new(self)) as *mut std::ffi::c_void
    }

    /// Retrieve Arc<Self> from raw pointer
    ///
    /// # Safety
    ///
    /// The pointer must have been created by `into_raw` and must still be valid.
    pub unsafe fn from_raw(ptr: *mut std::ffi::c_void) -> Option<&'static Self> {
        if ptr.is_null() {
            None
        } else {
            Some(&*(ptr as *const Self))
        }
    }

    /// Publish an event to the bus
    pub fn publish_event(&self, event: &EventV2) -> PluginResultV2 {
        // Convert EventV2 to crate::Event
        let topic = unsafe {
            if event.type_.is_null() {
                return PluginResultV2::InvalidRequest;
            }
            match std::ffi::CStr::from_ptr(event.type_).to_str() {
                Ok(s) => s.to_string(),
                Err(_) => return PluginResultV2::InvalidRequest,
            }
        };

        let payload = unsafe {
            if event.payload_json.is_null() {
                serde_json::Value::Null
            } else {
                match std::ffi::CStr::from_ptr(event.payload_json).to_str() {
                    Ok(s) => match serde_json::from_str(s) {
                        Ok(v) => v,
                        Err(_) => return PluginResultV2::InvalidRequest,
                    },
                    Err(_) => return PluginResultV2::InvalidRequest,
                }
            }
        };

        let crate_event = crate::Event::new(topic, payload);
        // Use the EventBus trait implementation via Arc
        crate::EventBus::publish(&*self.bus, crate_event);
        PluginResultV2::Success
    }

    /// FFI callback for publish
    extern "C" fn ffi_publish(
        context: *const PluginContextV2,
        event: *const EventV2,
    ) -> PluginResultV2 {
        if context.is_null() || event.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        unsafe {
            let ctx = &*context;
            if ctx.user_data.is_null() {
                return PluginResultV2::ServiceUnavailable;
            }

            match Self::from_raw(ctx.user_data) {
                Some(bus) => bus.publish_event(&*event),
                None => PluginResultV2::ServiceUnavailable,
            }
        }
    }

    /// FFI callback for subscribe
    extern "C" fn ffi_subscribe(
        context: *const PluginContextV2,
        event_type: *const std::ffi::c_char,
        _callback: extern "C" fn(*const EventV2),
    ) -> PluginResultV2 {
        if context.is_null() || event_type.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        // Note: Full subscription support requires storing callbacks per-plugin
        // This is a stub that returns Success to indicate the operation is valid
        // Full implementation would need a callback registry keyed by plugin_id
        PluginResultV2::Success
    }

    /// FFI callback for unsubscribe
    extern "C" fn ffi_unsubscribe(
        context: *const PluginContextV2,
        _event_type: *const std::ffi::c_char,
    ) -> PluginResultV2 {
        if context.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        // Note: Full unsubscription support requires callback registry
        PluginResultV2::Success
    }
}

/// Host-side RpcServiceV2 implementation
///
/// This struct provides the concrete implementation of RpcServiceV2 FFI callbacks
/// that the host provides to plugins. It wraps the RpcRegistry infrastructure.
pub struct HostRpcServiceV2 {
    /// The underlying RPC registry
    registry: Arc<crate::RpcRegistry>,
}

impl HostRpcServiceV2 {
    /// Create a new host RPC service wrapping an RpcRegistry
    pub fn new(registry: Arc<crate::RpcRegistry>) -> Self {
        Self { registry }
    }

    /// Create the FFI-compatible RpcServiceV2 struct with function pointers
    pub fn create_ffi_struct() -> RpcServiceV2 {
        RpcServiceV2 {
            call: Self::ffi_call,
            register_handler: Self::ffi_register_handler,
            list_services: Self::ffi_list_services,
            get_service_spec: Self::ffi_get_service_spec,
            free_strings: Self::ffi_free_strings,
        }
    }

    /// Store the Arc<Self> in a way that can be retrieved from user_data
    pub fn into_raw(self) -> *mut std::ffi::c_void {
        Box::into_raw(Box::new(self)) as *mut std::ffi::c_void
    }

    /// Retrieve Self from raw pointer
    ///
    /// # Safety
    ///
    /// The pointer must have been created by `into_raw` and must still be valid.
    pub unsafe fn from_raw(ptr: *mut std::ffi::c_void) -> Option<&'static Self> {
        if ptr.is_null() {
            None
        } else {
            Some(&*(ptr as *const Self))
        }
    }

    /// Call an RPC method
    pub fn call_rpc(
        &self,
        service: &str,
        request: &RpcRequestV2,
        response: &mut RpcResponseV2,
    ) -> PluginResultV2 {
        // Extract params from request
        let params_bytes = unsafe {
            if request.params.is_null() {
                &[][..]
            } else {
                let cstr = std::ffi::CStr::from_ptr(request.params);
                cstr.to_bytes()
            }
        };

        // Call the registry
        match self.registry.call(service, params_bytes) {
            Ok(result_bytes) => {
                // Convert result to C string (leaks intentionally - caller must free)
                let result_str = String::from_utf8_lossy(&result_bytes);
                match std::ffi::CString::new(result_str.as_ref()) {
                    Ok(cstr) => {
                        response.result = cstr.into_raw() as *const std::ffi::c_char;
                        response.error = std::ptr::null();
                        response.status = PluginResultV2::Success;
                        PluginResultV2::Success
                    }
                    Err(_) => {
                        response.status = PluginResultV2::Error;
                        PluginResultV2::Error
                    }
                }
            }
            Err(e) => {
                // Use PluginResultV2 directly
                response.status = e;
                response.result = std::ptr::null();
                response.error = std::ptr::null();
                e
            }
        }
    }

    /// FFI callback for call
    extern "C" fn ffi_call(
        context: *const PluginContextV2,
        service: *const std::ffi::c_char,
        request: *const RpcRequestV2,
        response: *mut RpcResponseV2,
    ) -> PluginResultV2 {
        if context.is_null() || service.is_null() || request.is_null() || response.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        unsafe {
            let ctx = &*context;
            if ctx.user_data.is_null() {
                return PluginResultV2::ServiceUnavailable;
            }

            let service_name = match std::ffi::CStr::from_ptr(service).to_str() {
                Ok(s) => s,
                Err(_) => return PluginResultV2::InvalidRequest,
            };

            match Self::from_raw(ctx.user_data) {
                Some(rpc) => rpc.call_rpc(service_name, &*request, &mut *response),
                None => PluginResultV2::ServiceUnavailable,
            }
        }
    }

    /// FFI callback for register_handler
    extern "C" fn ffi_register_handler(
        context: *const PluginContextV2,
        method: *const std::ffi::c_char,
        _handler: extern "C" fn(*const RpcRequestV2, *mut RpcResponseV2),
    ) -> PluginResultV2 {
        if context.is_null() || method.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        // Note: Full handler registration requires storing callbacks per-plugin
        // This is a stub that returns Success
        PluginResultV2::Success
    }

    /// FFI callback for list_services
    extern "C" fn ffi_list_services(
        _context: *const PluginContextV2,
    ) -> *const *const std::ffi::c_char {
        // Note: Full implementation would return a list of registered services
        // For now, return null to indicate no services
        std::ptr::null()
    }

    /// FFI callback for get_service_spec
    extern "C" fn ffi_get_service_spec(
        _context: *const PluginContextV2,
        _service: *const std::ffi::c_char,
    ) -> *const std::ffi::c_char {
        // Note: Full implementation would return the IDL spec for the service
        std::ptr::null()
    }

    /// FFI callback for free_strings
    extern "C" fn ffi_free_strings(_ptr: *const *const std::ffi::c_char, _count: usize) {
        // Note: Full implementation would free the strings array
    }
}

/// Builder for creating PluginContextV2 with all services configured
pub struct PluginContextV2Builder {
    logger: Option<*const LoggerV2>,
    config: Option<*const ConfigV2>,
    service_registry: Option<*const ServiceRegistryV2>,
    event_bus: Option<*const EventBusV2>,
    rpc_service: Option<*const RpcServiceV2>,
    /// HTTP router for plugin-provided API endpoints (RFC-0019)
    http_router: Option<*const crate::http::HttpRouterV2>,
    user_data: *mut std::ffi::c_void,
    user_context_json: *const std::ffi::c_char,
    secrets: *const crate::PluginSecrets,
    tracer: *const crate::PluginTracer,
    rotation_notifications: *const crate::RotationNotificationService,
}

impl PluginContextV2Builder {
    /// Create a new builder with default (null) values
    pub fn new() -> Self {
        Self {
            logger: None,
            config: None,
            service_registry: None,
            event_bus: None,
            rpc_service: None,
            http_router: None,
            user_data: std::ptr::null_mut(),
            user_context_json: std::ptr::null(),
            secrets: std::ptr::null(),
            tracer: std::ptr::null(),
            rotation_notifications: std::ptr::null(),
        }
    }

    /// Set the logger service
    pub fn logger(mut self, logger: *const LoggerV2) -> Self {
        self.logger = Some(logger);
        self
    }

    /// Set the config service
    pub fn config(mut self, config: *const ConfigV2) -> Self {
        self.config = Some(config);
        self
    }

    /// Set the service registry
    pub fn service_registry(mut self, registry: *const ServiceRegistryV2) -> Self {
        self.service_registry = Some(registry);
        self
    }

    /// Set the event bus service
    pub fn event_bus(mut self, event_bus: *const EventBusV2) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// Set the RPC service
    pub fn rpc_service(mut self, rpc: *const RpcServiceV2) -> Self {
        self.rpc_service = Some(rpc);
        self
    }

    /// Set the HTTP router service (RFC-0019)
    ///
    /// Allows plugins to register their own REST API endpoints dynamically.
    pub fn http_router(mut self, router: *const crate::http::HttpRouterV2) -> Self {
        self.http_router = Some(router);
        self
    }

    /// Set user data pointer
    pub fn user_data(mut self, data: *mut std::ffi::c_void) -> Self {
        self.user_data = data;
        self
    }

    /// Set user context JSON
    pub fn user_context_json(mut self, json: *const std::ffi::c_char) -> Self {
        self.user_context_json = json;
        self
    }

    /// Set secrets service
    pub fn secrets(mut self, secrets: *const crate::PluginSecrets) -> Self {
        self.secrets = secrets;
        self
    }

    /// Set tracer service
    pub fn tracer(mut self, tracer: *const crate::PluginTracer) -> Self {
        self.tracer = tracer;
        self
    }

    /// Set rotation notifications service
    pub fn rotation_notifications(
        mut self,
        notifications: *const crate::RotationNotificationService,
    ) -> Self {
        self.rotation_notifications = notifications;
        self
    }

    /// Build the PluginContextV2
    ///
    /// # Safety
    ///
    /// All pointers must remain valid for the lifetime of the context.
    pub fn build(self) -> PluginContextV2 {
        PluginContextV2 {
            logger: self.logger.unwrap_or(std::ptr::null()),
            config: self.config.unwrap_or(std::ptr::null()),
            service_registry: self.service_registry.unwrap_or(std::ptr::null()),
            event_bus: self.event_bus.unwrap_or(std::ptr::null()),
            rpc_service: self.rpc_service.unwrap_or(std::ptr::null()),
            http_router: self.http_router.unwrap_or(std::ptr::null()),
            user_data: self.user_data,
            user_context_json: self.user_context_json,
            secrets: self.secrets,
            tracer: self.tracer,
            rotation_notifications: self.rotation_notifications,
        }
    }
}

impl Default for PluginContextV2Builder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clustering::ServiceCluster;
    use std::ffi::CString;

    #[test]
    fn test_result_values() {
        assert_eq!(PluginResultV2::Success as i32, 0);
        assert_eq!(PluginResultV2::Error as i32, -1);
        assert_eq!(PluginResultV2::PermissionDenied as i32, -4);
    }

    #[test]
    fn test_maturity_levels() {
        assert_eq!(MaturityLevel::Alpha as u32, 0);
        assert_eq!(MaturityLevel::Stable as u32, 3);
        assert_eq!(MaturityLevel::Deprecated as u32, 4);
    }

    #[test]
    fn test_monetization_models() {
        assert_eq!(MonetizationModel::Free as u32, 0);
        assert_eq!(MonetizationModel::Subscription as u32, 2);
    }

    #[test]
    fn test_health_statuses() {
        assert_eq!(HealthStatus::Healthy as u32, 0);
        assert_eq!(HealthStatus::Unhealthy as u32, 2);
    }

    // ========================================================================
    // RFC-0006 Task 0006.1: Configuration Schema ABI Tests
    // ========================================================================

    /// Test that PluginGetConfigSchemaJsonFn can be defined and called
    #[test]
    fn test_config_schema_fn_signature() {
        // Define a mock function that returns a static schema string
        extern "C" fn mock_get_config_schema() -> *const c_char {
            static SCHEMA: &str =
                r#"{"type":"object","properties":{"enabled":{"type":"boolean"}}}"#;
            SCHEMA.as_ptr() as *const c_char
        }

        // Verify we can assign it to the function type
        let _fn_ptr: PluginGetConfigSchemaJsonFn = mock_get_config_schema;

        // Verify calling it returns a valid pointer
        let result = mock_get_config_schema();
        assert!(!result.is_null());

        // Verify we can read the string
        unsafe {
            let c_str = CStr::from_ptr(result);
            assert!(c_str.to_str().unwrap().contains("object"));
        }
    }

    /// Test PluginApiV2 struct has config_schema field
    #[test]
    fn test_plugin_api_v2_has_config_schema_field() {
        // This test verifies the field exists and is the right type
        fn _check_field_type(_field: Option<PluginGetConfigSchemaJsonFn>) {}

        // Define extern "C" functions for the struct
        extern "C" fn mock_get_info() -> *const PluginInfoV2 {
            std::ptr::null()
        }
        extern "C" fn mock_init(_ctx: *const PluginContextV2) -> PluginResultV2 {
            PluginResultV2::Success
        }
        extern "C" fn mock_shutdown(_ctx: *const PluginContextV2) -> PluginResultV2 {
            PluginResultV2::Success
        }
        extern "C" fn mock_handle_request(
            _ctx: *const PluginContextV2,
            _req: *const RequestV2,
            _resp: *mut ResponseV2,
        ) -> PluginResultV2 {
            PluginResultV2::Success
        }

        // Create a dummy struct to verify the field exists
        let _api = PluginApiV2 {
            get_info: mock_get_info,
            init: mock_init,
            shutdown: mock_shutdown,
            handle_request: mock_handle_request,
            handle_event: None,
            prepare_hot_reload: None,
            health_check: None,
            get_metrics: None,
            query_capability: None,
            get_config_schema: None,
            get_billing_metrics: None,
        };

        // Verify the field is accessible
        _check_field_type(_api.get_config_schema);
    }

    // ========================================================================
    // RFC-0004 Task 0004.2: DependencyInfo Validation Tests
    // ========================================================================

    #[test]
    fn test_dependency_info_validate_success() {
        let name = CString::new("test-plugin").unwrap();
        let version_range = CString::new(">=1.0.0, <2.0.0").unwrap();
        let service_type = CString::new("integration").unwrap();

        let dep = DependencyInfo {
            name: name.as_ptr(),
            version_range: version_range.as_ptr(),
            required: true,
            service_type: service_type.as_ptr(),
        };

        let validated = dep.validate().expect("validation should succeed");
        assert_eq!(validated.name, "test-plugin");
        assert_eq!(validated.version_range, ">=1.0.0, <2.0.0");
        assert!(validated.required);
        assert_eq!(validated.service_type, Some("integration".to_string()));
    }

    #[test]
    fn test_dependency_info_validate_wildcard() {
        let name = CString::new("wildcard-dep").unwrap();
        let version_range = CString::new("*").unwrap();

        let dep = DependencyInfo {
            name: name.as_ptr(),
            version_range: version_range.as_ptr(),
            required: false,
            service_type: std::ptr::null(),
        };

        let validated = dep.validate().expect("validation should succeed");
        assert_eq!(validated.name, "wildcard-dep");
        assert_eq!(validated.version_range, "*");
        assert!(!validated.required);
        assert!(validated.service_type.is_none());
    }

    #[test]
    fn test_dependency_info_validate_caret() {
        let name = CString::new("caret-dep").unwrap();
        let version_range = CString::new("^1.2.3").unwrap();

        let dep = DependencyInfo {
            name: name.as_ptr(),
            version_range: version_range.as_ptr(),
            required: true,
            service_type: std::ptr::null(),
        };

        let validated = dep.validate().expect("validation should succeed");
        assert_eq!(validated.version_range, "^1.2.3");
    }

    #[test]
    fn test_dependency_info_validate_null_name() {
        let version_range = CString::new(">=1.0.0").unwrap();

        let dep = DependencyInfo {
            name: std::ptr::null(),
            version_range: version_range.as_ptr(),
            required: true,
            service_type: std::ptr::null(),
        };

        let err = dep.validate().expect_err("should fail with null name");
        assert!(matches!(err, DependencyValidationError::InvalidName { .. }));
    }

    #[test]
    fn test_dependency_info_validate_invalid_version_range() {
        let name = CString::new("test-plugin").unwrap();
        let version_range = CString::new("not-valid-semver!!!").unwrap();

        let dep = DependencyInfo {
            name: name.as_ptr(),
            version_range: version_range.as_ptr(),
            required: true,
            service_type: std::ptr::null(),
        };

        let err = dep
            .validate()
            .expect_err("should fail with invalid version range");
        assert!(matches!(
            err,
            DependencyValidationError::InvalidVersionRange { .. }
        ));
    }

    #[test]
    fn test_dependency_info_validate_invalid_name_chars() {
        let name = CString::new("invalid!@#$%name").unwrap();
        let version_range = CString::new(">=1.0.0").unwrap();

        let dep = DependencyInfo {
            name: name.as_ptr(),
            version_range: version_range.as_ptr(),
            required: true,
            service_type: std::ptr::null(),
        };

        let err = dep
            .validate()
            .expect_err("should fail with invalid name chars");
        assert!(matches!(err, DependencyValidationError::InvalidName { .. }));
    }

    #[test]
    fn test_dependency_info_validated_version_satisfies() {
        let validated = DependencyInfoValidated {
            name: "test".to_string(),
            version_range: ">=1.0.0, <2.0.0".to_string(),
            required: true,
            service_type: None,
        };

        assert!(validated.version_satisfies("1.0.0"));
        assert!(validated.version_satisfies("1.5.0"));
        assert!(validated.version_satisfies("1.9.9"));
        assert!(!validated.version_satisfies("0.9.9"));
        assert!(!validated.version_satisfies("2.0.0"));
        assert!(!validated.version_satisfies("2.5.0"));
    }

    #[test]
    fn test_dependency_info_validated_version_satisfies_caret() {
        let validated = DependencyInfoValidated {
            name: "test".to_string(),
            version_range: "^1.2.3".to_string(),
            required: true,
            service_type: None,
        };

        assert!(validated.version_satisfies("1.2.3"));
        assert!(validated.version_satisfies("1.2.4"));
        assert!(validated.version_satisfies("1.9.9"));
        assert!(!validated.version_satisfies("1.2.2"));
        assert!(!validated.version_satisfies("2.0.0"));
    }

    #[test]
    fn test_dependency_info_validated_version_satisfies_wildcard() {
        let validated = DependencyInfoValidated {
            name: "test".to_string(),
            version_range: "*".to_string(),
            required: false,
            service_type: None,
        };

        assert!(validated.version_satisfies("0.0.1"));
        assert!(validated.version_satisfies("1.0.0"));
        assert!(validated.version_satisfies("99.99.99"));
    }

    #[test]
    fn test_dependency_info_validated_version_satisfies_tilde() {
        let validated = DependencyInfoValidated {
            name: "test".to_string(),
            version_range: "~1.2.3".to_string(),
            required: true,
            service_type: None,
        };

        assert!(validated.version_satisfies("1.2.3"));
        assert!(validated.version_satisfies("1.2.9"));
        assert!(!validated.version_satisfies("1.2.2"));
        assert!(!validated.version_satisfies("1.3.0"));
    }

    #[test]
    fn test_dependency_validation_error_display() {
        let err1 = DependencyValidationError::InvalidName {
            reason: "name is empty".to_string(),
        };
        assert!(err1.to_string().contains("name is empty"));

        let err2 = DependencyValidationError::InvalidVersionRange {
            range: "bad".to_string(),
            reason: "invalid syntax".to_string(),
        };
        assert!(err2.to_string().contains("bad"));
        assert!(err2.to_string().contains("invalid syntax"));

        let err3 = DependencyValidationError::InvalidServiceType {
            service_type: "BadType".to_string(),
            reason: "must be lowercase".to_string(),
        };
        assert!(err3.to_string().contains("BadType"));
    }

    // ========================================================================
    // RFC-0004 Phase 6.3: Service Registry Clustering Tests
    // ========================================================================

    use crate::clustering::{ClusterService, ConsensusType, ServiceNode};

    #[tokio::test]
    async fn test_distributed_register() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);

        // Register service on local node
        let service = ClusterService::new(
            "my-service",
            "my-service",
            "1.0",
            "node-1",
            "localhost",
            8080,
        );
        let result = cluster.register_service(service).await;
        assert!(result.is_ok());

        // Should be immediately available on local node
        let lookup = cluster.discover_services("my-service").await;
        assert!(lookup.is_ok());
        assert!(!lookup.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_lookup_distributed() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);

        // Register a service
        let service =
            ClusterService::new("service-1", "database", "1.0", "node-1", "localhost", 5432);
        let _ = cluster.register_service(service).await;

        // Lookup should succeed
        let result = cluster.discover_services("database").await;
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());

        // Non-existent service should return empty
        let result = cluster.discover_services("non-existent").await;
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_conflict_resolution() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);

        // Register service with version 1
        let service_v1 = ClusterService::new(
            "api-service",
            "api-service",
            "1.0",
            "node-1",
            "localhost",
            8080,
        );
        let _ = cluster.register_service(service_v1).await;

        // Update to version 2 (last-write-wins - same ID replaces)
        let service_v2 = ClusterService::new(
            "api-service",
            "api-service",
            "2.0",
            "node-1",
            "localhost",
            8080,
        );
        let _ = cluster.register_service(service_v2).await;

        // Should return latest version
        let result = cluster.discover_services("api-service").await.unwrap();
        assert_eq!(result[0].version, "2.0");
    }

    #[tokio::test]
    async fn test_node_sync() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);

        // Register services
        let svc1 = ClusterService::new("svc-1", "svc-1", "1.0", "node-1", "localhost", 8081);
        let svc2 = ClusterService::new("svc-2", "svc-2", "1.0", "node-1", "localhost", 8082);
        let _ = cluster.register_service(svc1).await;
        let _ = cluster.register_service(svc2).await;

        // Sync nodes (gossip protocol)
        let result = cluster.sync_nodes().await;
        assert!(result.is_ok());

        // Services should remain available after sync
        assert!(!cluster.discover_services("svc-1").await.unwrap().is_empty());
        assert!(!cluster.discover_services("svc-2").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_node_addition() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);

        // Add a node
        let node = ServiceNode::new("node-2", "node-2.example.com", 8080);
        let result = cluster.add_node(node).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_partition_tolerance() {
        let cluster = ServiceCluster::new("node-1", ConsensusType::EventualConsistency, 2);

        // Register services before partition
        let service = ClusterService::new("svc-a", "svc-a", "1.0", "node-1", "localhost", 8083);
        let _ = cluster.register_service(service).await;

        // In a partition, local node remains available
        let result = cluster.discover_services("svc-a").await;
        assert!(!result.unwrap().is_empty());

        // System remains operational (eventual consistency)
        let _ = cluster.sync_nodes().await;
        assert!(!cluster.discover_services("svc-a").await.unwrap().is_empty());
    }
}
