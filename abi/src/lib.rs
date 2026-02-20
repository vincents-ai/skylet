// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::ffi::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

// RFC-0006: Plugin Configuration Schema
pub mod config;
pub use config::{
    ConfigError, ConfigField, ConfigFieldType, ConfigFormat, ConfigManager, ConfigSchema,
    ConfigSection, ConfigValidator, EnvSecretBackend, FileSecretBackend, GlobalValidationRule,
    SchemaError, SecretBackend, SecretError, SecretReference, SecretResolver,
    SecretResolverBackend, UIComponent, UIComponentType, UIConstraints, UIGenerator, UIHints,
    ValidationError, ValidationRule, ValidationWarning, VaultSecretBackend, WidgetType,
};

// RFC-0006: JSON Schema Validation for Plugin Config
pub mod config_schema;
pub use config_schema::{
    validate_config, ConfigSchemaError, ConfigSchemaResult, ConfigSchemaValidator,
    ConfigValidationOptions, ConfigValidationResult,
};

// RFC-0005: Plugin Dependency Resolution
pub mod dependencies;
pub use dependencies::{
    Comparator,
    ConflictRequirement,
    ConstraintError,
    Dependency,
    DependencyEdge,
    // Graph types
    DependencyGraph,
    // Resolution
    DependencyResolver,
    GraphError,
    PluginId,
    PluginNode,
    Prerelease,
    PrereleaseIdentifier,
    RequirementSource,
    Resolution,
    ResolutionError,
    ResolutionStrategy,
    ResolutionWarning,
    ResolvedPlugin,
    // Version types
    Version,
    VersionConflict,
    VersionConstraint,
    VersionError,
    // Constraints
    VersionReq,
};

// Security validation module for FFI boundaries
pub mod security;

// RFC-0008: Security and Capabilities Implementation
pub mod security_rfc;
pub use security_rfc::{
    host_matches_pattern, ApprovalStatus, CapabilityApproval, CapabilityCheckResult,
    CapabilityInfo, CapabilityStatus, CapabilityType, CommandExecution, FilesystemAccess,
    FilesystemAccessError, FilesystemAccessMode, FilesystemEnforcer, HostPattern, NetworkAccess,
    NetworkAccessError, NetworkEnforcer, PathPermission, PolicyError, RiskLevel, SecurityPolicy,
    SecurityPolicyEngine,
};

// Unified MCP tool schema types shared by all plugins
pub mod mcp_schema;
pub use mcp_schema::{InputSchema, PropertySchema, ToolSchema};

// RFC-0018: Structured Logging Schema
pub mod logging;
pub use logging::{
    rfc0018_json_schema, ErrorInfo, LogEvent, LogLevel, RequestContext, SourceLocation,
    TracingContext, RFC0018_JSON_SCHEMA,
};

// RFC-0100 (Phase 2.1): Key Management Abstraction
pub mod key_management;
pub use key_management::{
    DefaultKeyManagement, KeyManagement, KeyManagementError, KeyManagementResult, KeyPair, KeyType,
};

// RFC-0100 (Phase 2.1): Instance Management Abstraction
pub mod instance_management;
pub use instance_management::{
    InstanceManagementError, InstanceManagementResult, InstanceManager, InstancePeerInfo,
    InstanceRole, StandaloneInstanceManager,
};

// RFC-0019: Plugin-Provided API Endpoints
pub mod http;
pub use http::{
    extract_path_params,
    path_pattern_to_regex,
    HttpMethod,
    HttpRouter,
    // V2 types for ABI v2 compatibility
    HttpRouterV2,
    MiddlewareConfig,
    MiddlewareConfigV2,
    MiddlewareFn,
    MiddlewareFnV2,
    OpenApiComponents,
    OpenApiDocument,
    OpenApiInfo,
    OpenApiMediaType,
    OpenApiOperation,
    OpenApiParameter,
    OpenApiRequestBody,
    OpenApiResponse,
    OpenApiSchema,
    RouteConfig,
    RouteConfigV2,
    RouteHandlerFn,
    RouteMetadata,
};

// RFC-0017: Distributed Tracing and Telemetry
// Note: Only re-export specific types to avoid conflict with the `tracing` crate.
// The module itself remains private to prevent `use skylet_abi::*` from
// bringing in a `tracing` module that shadows the `tracing` crate.
mod tracing_mod;
pub use tracing_mod::{
    init_tracing, ExporterConfig, MetricCollector, MetricType, OtelTracer, SamplerConfig, Span,
    SpanBuilder, SpanContext, SpanId, SpanManager, TraceContextExt, TraceId, TracerConfig,
    TracingError, TracingExporter, W3CTraceContext,
};

// RFC-0068: Overlay Network Plugin Unification
pub mod network_transport;
pub use network_transport::{
    OverlayMetrics,
    OverlayMetricsFFI,
    OverlayNetwork,
    OverlayNetworkV2,
    OverlayPeerListResult,
    OverlayResult,
    OverlayStringResult,
    OverlayTransportType,
    OverlayTunnelListResult,
    PeerInfo,
    PeerInfoFFI,
    ServiceAdvertisement,
    TunnelConfig,
    TunnelInfo,
    // FFI types
    TunnelInfoFFI,
    TunnelStatus,
};

// Re-export key security types for convenience
pub use security::{
    rotation_topics,
    secret_topics,
    AuditEvent,
    AuditLogger,
    BackupCodeProvider,
    CredentialRotationManager,
    CredentialStatus,
    CredentialType,
    CredentialVersion,
    DefaultSecretsProvider,
    // RFC-0077 types
    KeyAlgorithm,
    KeyUsage,
    ListSecretsOptions,
    MFAChallenge,
    MFAFactor,
    MFAManager,
    MFAMethod,
    PluginAuthenticator,
    PluginCredential,
    PluginPermissions,
    PluginRole,
    RotationEvent,
    RotationEventSeverity,
    RotationEventType,
    RotationHistory,
    RotationNotificationService,
    RotationNotifier,
    RotationNotifyCallback,
    RotationPolicy,
    RotationResult,
    SecretAuditEntry,
    SecretMetadata,
    SecretOperation,
    SecretVersion,
    // RFC-0029: Secrets Provider Interface
    SecretsProvider,
    SecurityError,
    TOTPProvider,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PluginResult {
    Success = 0,
    Error = -1,
    InvalidRequest = -2,
    ServiceUnavailable = -3,
    PermissionDenied = -4,
    NotImplemented = -5,
    Timeout = -6,
    ResourceExhausted = -7,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PluginLogLevel {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PluginType {
    Utility = 0,
    Database = 1,
    Network = 2,
    Storage = 3,
    Security = 4,
    Monitoring = 5,
    Testing = 6,
    Application = 7,
    Agent = 8,
    Workflow = 9,
    Job = 10,
    Prompt = 11,
    Integration = 12,
    Analytics = 13,
    Template = 14,
    Scheduler = 15,
}

#[repr(C)]
pub struct PluginLogger {
    pub log:
        extern "C" fn(context: *const PluginContext, level: PluginLogLevel, message: *const c_char),
    // New structured logging function as per RFC-0018. Accepts a UTF-8 JSON object as data.
    pub log_structured: extern "C" fn(
        context: *const PluginContext,
        level: PluginLogLevel,
        message: *const c_char,
        data_json: *const c_char,
    ),
}

#[repr(C)]
pub struct PluginConfig {
    pub get: extern "C" fn(context: *const PluginContext, key: *const c_char) -> *const c_char,
    pub get_bool: extern "C" fn(context: *const PluginContext, key: *const c_char) -> c_int,
    pub get_int: extern "C" fn(context: *const PluginContext, key: *const c_char) -> i64,
    pub get_float: extern "C" fn(context: *const PluginContext, key: *const c_char) -> f64,
}

#[repr(C)]
pub struct PluginServiceRegistry {
    pub register: extern "C" fn(
        context: *const PluginContext,
        name: *const c_char,
        service: *mut c_void,
        service_type: *const c_char,
    ) -> PluginResult,
    pub get: extern "C" fn(
        context: *const PluginContext,
        name: *const c_char,
        service_type: *const c_char,
    ) -> *mut c_void,
    pub unregister:
        extern "C" fn(context: *const PluginContext, name: *const c_char) -> PluginResult,
}

#[repr(C)]
pub struct PluginContext {
    pub logger: *const PluginLogger,
    pub config: *const PluginConfig,
    pub service_registry: *const PluginServiceRegistry,
    // Tracer service (optional). Points to a PluginTracer implementation
    // which plugins can call to create spans, add events and set attributes.
    pub tracer: *const PluginTracer,
    pub user_data: *mut c_void,
    // Optional pointer to a JSON-encoded UserContext for the current request.
    // This is nullable and primarily used by the core to pass per-request
    // identity/roles to plugins that need it.
    pub user_context_json: *const c_char,
    // Optional secrets service (optional). Points to a PluginSecrets implementation
    // which plugins can call to get/put secrets via the host's secrets backend.
    pub secrets: *const PluginSecrets,
    // Optional rotation notification service. Points to a RotationNotificationService
    // implementation which plugins can use to register for rotation events.
    pub rotation_notifications: *const RotationNotificationService,
}

// A handle that uniquely identifies a span within a trace.
pub type SpanHandle = u64;

#[repr(C)]
pub struct PluginTracer {
    // Starts a new span as a child of the current active span. Returns a handle
    // to the new span which becomes the active one.
    pub start_span:
        extern "C" fn(context: *const (), name_ptr: *const c_char, name_len: usize) -> SpanHandle,

    // Ends the specified span. The parent span becomes active again.
    pub end_span: extern "C" fn(context: *const (), span_handle: SpanHandle),

    // Adds a named event to the currently active span.
    pub add_event: extern "C" fn(context: *const (), name_ptr: *const c_char, name_len: usize),

    // Adds a key/value attribute to the currently active span.
    pub set_attribute: extern "C" fn(
        context: *const (),
        key_ptr: *const c_char,
        key_len: usize,
        value_ptr: *const c_char,
        value_len: usize,
    ),
}

#[repr(C)]
pub struct PluginSecrets {
    // Get a secret value. Returns a C string pointer (owned by caller) or null if not found.
    // Signature: (context, plugin_ptr, plugin_len, secret_ref_ptr, secret_ref_len) -> *const c_char
    pub get: extern "C" fn(
        context: *const (),
        plugin_ptr: *const c_char,
        plugin_len: usize,
        secret_ref_ptr: *const c_char,
        secret_ref_len: usize,
    ) -> *const c_char,

    // Put a secret value. Returns PluginResultV2::Success on success.
    // Signature: (context, plugin_ptr, plugin_len, secret_ref_ptr, secret_ref_len, value_ptr, value_len) -> PluginResultV2
    pub put: extern "C" fn(
        context: *const (),
        plugin_ptr: *const c_char,
        plugin_len: usize,
        secret_ref_ptr: *const c_char,
        secret_ref_len: usize,
        value_ptr: *const c_char,
        value_len: usize,
    ) -> crate::v2_spec::PluginResultV2,
    // Free a C string previously returned by `get`.
    // Signature: (ptr) -> void
    // The host allocates strings returned by `get` with CString::into_raw(); plugins MUST call
    // this function to free the memory (CString::from_raw).
    pub free_result: extern "C" fn(ptr: *mut c_char),
}

#[repr(C)]
pub struct WorkflowService {
    /// Execute a workflow.
    /// args_json: A JSON array of arguments.
    /// Returns: execution_id as a C string (caller must free via WorkflowService::free_result).
    pub execute: extern "C" fn(
        context: *const PluginContext,
        workflow_name: *const c_char,
        args_json: *const c_char,
    ) -> *const c_char,

    /// Get the status of an execution.
    /// Returns: A JSON object containing status and results (caller must free).
    pub get_status:
        extern "C" fn(context: *const PluginContext, execution_id: *const c_char) -> *const c_char,

    /// Cancel a running execution.
    pub cancel:
        extern "C" fn(context: *const PluginContext, execution_id: *const c_char) -> PluginResult,

    /// Free a string returned by execute or get_status.
    pub free_result: extern "C" fn(ptr: *mut c_char),
}

#[repr(C)]
pub struct PluginInfo {
    pub name: *const c_char,
    pub version: *const c_char,
    pub description: *const c_char,
    pub author: *const c_char,
    pub license: *const c_char,
    pub homepage: *const c_char,
    pub skynet_version_min: *const c_char,
    pub skynet_version_max: *const c_char,
    pub abi_version: *const c_char,
    pub dependencies: *const *const c_char,
    pub num_dependencies: usize,
    pub provides_services: *const *const c_char,
    pub num_provides_services: usize,
    pub requires_services: *const *const c_char,
    pub num_requires_services: usize,
    pub supported_operations: *const *const c_char,
    pub num_supported_operations: usize,
    pub capabilities: *const *const c_char,
    pub num_capabilities: usize,
    pub resource_requirements: *const *const c_char,
    pub max_concurrency: usize,
    pub supports_hot_reload: bool,
    pub supports_async: bool,
    pub supports_streaming: bool,
    pub plugin_type: PluginType,
    pub tags: *const *const c_char,
    pub num_tags: usize,
    pub build_timestamp: *const c_char,
    pub build_hash: *const c_char,
    pub git_commit: *const c_char,
    pub build_environment: *const c_char,
    pub metadata: *const c_char,
}

// PluginState for hot-reload ABI (opaque to host)
#[repr(C)]
pub struct PluginState {
    pub data: *mut u8,
    pub len: usize,
    pub free: extern "C" fn(state: PluginState),
}

// ============================================================================
// RFC-0007: Hot Reload State Management
// ============================================================================

/// Current state format version for RFC-0007
///
/// This version should be incremented when the state format changes in a way
/// that requires migration. Plugins can use this to handle state migrations
/// between different versions of their serialized state.
pub const HOT_RELOAD_STATE_VERSION: u32 = 1;

/// Magic bytes to identify RFC-0007 state blobs
///
/// Each state blob should start with these bytes followed by the version number
/// to allow for state format identification and migration.
pub const HOT_RELOAD_STATE_MAGIC: &[u8; 4] = b"SKSR"; // "Skylet State"

/// Header for versioned plugin state
///
/// This header should be prepended to all serialized state blobs to enable
/// version detection and migration. The format is:
/// - 4 bytes: magic ("SKSR")
/// - 4 bytes: version (little-endian u32)
/// - 4 bytes: plugin state format version (little-endian u32, plugin-defined)
/// - remaining: plugin-specific state data
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct StateHeader {
    /// Magic bytes "SKSR"
    pub magic: [u8; 4],
    /// RFC-0007 state format version
    pub format_version: u32,
    /// Plugin-specific state version (for migration)
    pub plugin_version: u32,
}

impl StateHeader {
    /// Size of the state header in bytes
    pub const SIZE: usize = 12;

    /// Create a new state header with the current format version
    pub fn new(plugin_version: u32) -> Self {
        Self {
            magic: *HOT_RELOAD_STATE_MAGIC,
            format_version: HOT_RELOAD_STATE_VERSION,
            plugin_version,
        }
    }

    /// Parse a state header from bytes
    ///
    /// Returns the header and a slice of the remaining data
    pub fn from_bytes(data: &[u8]) -> Option<(Self, &[u8])> {
        if data.len() < Self::SIZE {
            return None;
        }

        let magic = [data[0], data[1], data[2], data[3]];
        if &magic != HOT_RELOAD_STATE_MAGIC {
            return None;
        }

        let format_version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let plugin_version = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

        Some((
            Self {
                magic,
                format_version,
                plugin_version,
            },
            &data[Self::SIZE..],
        ))
    }

    /// Serialize the header to bytes
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..4].copy_from_slice(&self.magic);
        bytes[4..8].copy_from_slice(&self.format_version.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.plugin_version.to_le_bytes());
        bytes
    }

    /// Check if this header is valid
    pub fn is_valid(&self) -> bool {
        &self.magic == HOT_RELOAD_STATE_MAGIC
    }
}

impl PluginState {
    /// Create a new PluginState from a Vec<u8>
    ///
    /// This allocates memory that must be freed using the `free` function pointer.
    /// The returned state includes a default free function that deallocates the memory.
    pub fn from_vec(data: Vec<u8>) -> Self {
        let len = data.len();
        let boxed = data.into_boxed_slice();
        let data = Box::into_raw(boxed) as *mut u8;

        Self {
            data,
            len,
            free: plugin_state_free_default,
        }
    }

    /// Create an empty PluginState (no state to preserve)
    pub fn empty() -> Self {
        Self {
            data: std::ptr::null_mut(),
            len: 0,
            free: plugin_state_free_noop,
        }
    }

    /// Create a versioned PluginState with header
    ///
    /// This prepends the StateHeader to the plugin-specific state data.
    pub fn versioned(plugin_state_version: u32, data: Vec<u8>) -> Self {
        let header = StateHeader::new(plugin_state_version);
        let header_bytes = header.to_bytes();

        let mut full_data = Vec::with_capacity(header_bytes.len() + data.len());
        full_data.extend_from_slice(&header_bytes);
        full_data.extend(data);

        Self::from_vec(full_data)
    }

    /// Check if this state has data
    pub fn has_data(&self) -> bool {
        !self.data.is_null() && self.len > 0
    }

    /// Get the state data as a slice
    ///
    /// # Safety
    /// The caller must ensure the data is valid and properly aligned
    pub unsafe fn as_slice(&self) -> &[u8] {
        if self.data.is_null() || self.len == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(self.data, self.len)
        }
    }

    /// Parse the state header and return the plugin-specific data
    ///
    /// Returns None if the state is empty or doesn't have a valid header
    pub fn parse_header(&self) -> Option<(StateHeader, &[u8])> {
        let data = unsafe { self.as_slice() };
        StateHeader::from_bytes(data)
    }

    /// Consume the state and return the data as a Vec
    ///
    /// This is useful for testing and when you need ownership of the data.
    /// Note: This consumes the state without calling the free function.
    ///
    /// # Safety
    /// The caller must ensure the data was allocated in a way compatible with Vec
    pub unsafe fn into_vec(self) -> Option<Vec<u8>> {
        if self.data.is_null() || self.len == 0 {
            return None;
        }

        // Reconstruct the Vec from the raw pointer
        // Note: This assumes the data was allocated as a Box<[u8]> or Vec<u8>
        let slice = std::slice::from_raw_parts_mut(self.data, self.len);
        Some(slice.to_vec())
    }
}

// RFC-0003: Security fixes for unsafe FFI (Phase 3)
// Issue #7: Vec::from_raw_parts validation
// Maximum size for plugin state allocation (512MB)
#[allow(dead_code)]
const MAX_PLUGIN_STATE_SIZE: usize = 512 * 1024 * 1024;
// Maximum size for service list allocation (100k entries * 8 bytes per ptr)
#[allow(dead_code)]
const MAX_SERVICE_LIST_SIZE: usize = 100_000;

/// Default free function for PluginState allocated via Box
///
/// This function is used when creating PluginState from Rust code.
/// RFC-0003: Validates allocation before reconstructing Vec (Issue #7)
extern "C" fn plugin_state_free_default(state: PluginState) {
    if !state.data.is_null() && state.len > 0 {
        unsafe {
            // RFC-0003 Phase 3: Validate allocation bounds before reconstruction
            // Issue #7: Prevent double-free via validation
            if state.len > MAX_PLUGIN_STATE_SIZE {
                tracing::error!(
                    "PluginState exceeds max size: {} > {}",
                    state.len,
                    MAX_PLUGIN_STATE_SIZE
                );
                return;
            }

            // Validate pointer alignment - must be at least word-aligned
            let addr = state.data as usize;
            if addr & (std::mem::align_of::<*mut u8>() - 1) != 0 {
                tracing::error!("PluginState pointer misaligned: 0x{:x}", addr);
                return;
            }

            // Reconstruct the Box and let it drop
            let slice = std::slice::from_raw_parts_mut(state.data, state.len);
            let _ = Vec::from_raw_parts(slice.as_mut_ptr(), state.len, state.len);
        }
    }
}

/// No-op free function for empty PluginState
extern "C" fn plugin_state_free_noop(_state: PluginState) {
    // Nothing to free
}

#[repr(C)]
pub struct HttpRequest {
    pub method: *mut c_char,
    pub path: *mut c_char,
    pub headers: *const HttpHeader,
    pub num_headers: usize,
    pub body: *const u8,
    pub body_len: usize,
    pub query_params: *const HttpQueryParam,
    pub num_query_params: usize,
}

#[repr(C)]
pub struct HttpHeader {
    pub name: *mut c_char,
    pub value: *mut c_char,
}

#[repr(C)]
pub struct HttpQueryParam {
    pub name: *mut c_char,
    pub value: *mut c_char,
}

#[repr(C)]
pub struct HttpResponse {
    pub status_code: i32,
    pub headers: *mut HttpHeader,
    pub num_headers: usize,
    pub body: *mut u8,
    pub body_len: usize,
}

// Plugin interface functions
pub type PluginInitFn = extern "C" fn(context: *const PluginContext) -> PluginResult;
pub type PluginShutdownFn = extern "C" fn(context: *const PluginContext) -> PluginResult;
pub type PluginGetInfoFn = extern "C" fn() -> *const PluginInfo;
pub type PluginHandleRequestFn = extern "C" fn(
    context: *const PluginContext,
    request: *const HttpRequest,
    response: *mut *mut HttpResponse,
) -> PluginResult;

/// Optional ABI export for MCP tool discovery.
///
/// Returns a JSON-encoded `Vec<ToolSchema>` as a NUL-terminated C string.
/// The caller is responsible for freeing the returned pointer via
/// `plugin_free_tools`. If a plugin does not export this symbol, it has
/// no MCP tools.
pub type PluginGetToolsFn = extern "C" fn() -> *const c_char;

/// Optional ABI export to free the string returned by `plugin_get_tools`.
pub type PluginFreeToolsFn = extern "C" fn(ptr: *mut c_char);

// Hot-reload ABI additions (optional)
pub type PluginPrepareHotReloadFn = extern "C" fn(context: *const PluginContext) -> PluginState;
pub type PluginInitFromStateFn =
    extern "C" fn(context: *const PluginContext, state: PluginState) -> PluginResult;

/// Hot-reload hooks are optional and not currently used
#[allow(dead_code)]
pub struct Plugin {
    handle: libloading::Library,
    init_fn: PluginInitFn,
    shutdown_fn: PluginShutdownFn,
    get_info_fn: PluginGetInfoFn,
    handle_request_fn: PluginHandleRequestFn,
    prepare_hot_reload_fn: Option<PluginPrepareHotReloadFn>,
    init_from_state_fn: Option<PluginInitFromStateFn>,
    get_tools_fn: Option<PluginGetToolsFn>,
    free_tools_fn: Option<PluginFreeToolsFn>,
}

impl Plugin {
    pub unsafe fn load<P>(path: P, _context: &PluginContext) -> Result<Self, libloading::Error>
    where
        P: AsRef<std::ffi::OsStr>,
    {
        let lib = libloading::Library::new(path)?;

        let init_fn: PluginInitFn = *lib.get(b"plugin_init")?;
        let shutdown_fn: PluginShutdownFn = *lib.get(b"plugin_shutdown")?;
        let get_info_fn: PluginGetInfoFn = *lib.get(b"plugin_get_info")?;
        let handle_request_fn: PluginHandleRequestFn = *lib.get(b"plugin_handle_request")?;
        // Optional hot-reload symbols
        let prepare_hot_reload_fn = match lib.get(b"plugin_prepare_hot_reload") {
            Ok(sym) => Some(*sym),
            Err(_) => None,
        };

        let init_from_state_fn = match lib.get(b"plugin_init_from_state") {
            Ok(sym) => Some(*sym),
            Err(_) => None,
        };

        let get_tools_fn: Option<PluginGetToolsFn> = match lib.get(b"plugin_get_tools") {
            Ok(sym) => Some(*sym),
            Err(_) => None,
        };

        let free_tools_fn: Option<PluginFreeToolsFn> = match lib.get(b"plugin_free_tools") {
            Ok(sym) => Some(*sym),
            Err(_) => None,
        };

        Ok(Plugin {
            handle: lib,
            init_fn,
            shutdown_fn,
            get_info_fn,
            handle_request_fn,
            prepare_hot_reload_fn,
            init_from_state_fn,
            get_tools_fn,
            free_tools_fn,
        })
    }

    /// Safely validate and call the plugin init function with FFI boundary checks
    ///
    /// This method validates the context pointer before calling the plugin's init function,
    /// preventing unsafe memory access at the FFI boundary.
    pub unsafe fn init(&self, context: *const PluginContext) -> PluginResult {
        // Validate plugin context before calling
        if let Err(err) = security::validate_plugin_context(context) {
            tracing::error!("Plugin init: context validation failed: {:?}", err);
            return PluginResult::Error;
        }
        (self.init_fn)(context)
    }

    /// Safely validate and call the plugin shutdown function with FFI boundary checks
    pub unsafe fn shutdown(&self, context: *const PluginContext) -> PluginResult {
        // Validate plugin context before calling
        if let Err(err) = security::validate_plugin_context(context) {
            tracing::error!("Plugin shutdown: context validation failed: {:?}", err);
            return PluginResult::Error;
        }
        (self.shutdown_fn)(context)
    }

    pub unsafe fn get_info(&self) -> *const PluginInfo {
        (self.get_info_fn)()
    }

    /// Returns true if this plugin exports MCP tool schemas.
    pub fn has_tools(&self) -> bool {
        self.get_tools_fn.is_some()
    }

    /// Retrieve tool schemas from the plugin, if it exports `plugin_get_tools`.
    ///
    /// Returns `None` if the plugin does not export the symbol.
    /// Returns `Some(Err(...))` if the returned JSON is invalid.
    /// Returns `Some(Ok(vec))` on success.
    pub unsafe fn get_tools(&self) -> Option<Result<Vec<ToolSchema>, String>> {
        let get_fn = self.get_tools_fn?;
        let ptr = (get_fn)();
        if ptr.is_null() {
            return Some(Ok(Vec::new()));
        }
        let cstr = std::ffi::CStr::from_ptr(ptr);
        let result = match cstr.to_str() {
            Ok(s) => serde_json::from_str::<Vec<ToolSchema>>(s)
                .map_err(|e| format!("Invalid tool schema JSON: {}", e)),
            Err(e) => Err(format!("Invalid UTF-8 in tool schema: {}", e)),
        };
        // Free the string if the plugin provides a free function
        if let Some(free_fn) = self.free_tools_fn {
            (free_fn)(ptr as *mut c_char);
        }
        Some(result)
    }

    /// Safely validate and call the plugin request handler with FFI boundary checks
    ///
    /// This method validates the request pointer and response pointer before calling
    /// the plugin's request handler function, preventing buffer overflow attacks.
    pub unsafe fn handle_request(
        &self,
        request: &HttpRequest,
        response: *mut *mut HttpResponse,
    ) -> PluginResult {
        // Validate response pointer is not null
        if response.is_null() {
            tracing::error!("Plugin handle_request: response pointer is null");
            return PluginResult::InvalidRequest;
        }

        // Validate request pointer alignment
        if (request as *const _ as usize) % std::mem::align_of::<HttpRequest>() != 0 {
            tracing::error!("Plugin handle_request: misaligned request pointer");
            return PluginResult::InvalidRequest;
        }

        // TODO: Pass actual context
        (self.handle_request_fn)(std::ptr::null(), request, response)
    }

    // ===== RFC-0007: Hot Reload Methods =====

    /// Check if this plugin supports hot reload
    ///
    /// Returns true if the plugin exports both `plugin_prepare_hot_reload` and
    /// `plugin_init_from_state` symbols.
    pub fn supports_hot_reload(&self) -> bool {
        self.prepare_hot_reload_fn.is_some() && self.init_from_state_fn.is_some()
    }

    /// Prepare for hot reload by serializing plugin state
    ///
    /// Calls the plugin's `plugin_prepare_hot_reload` function to serialize
    /// its internal state into an opaque byte buffer. The returned state can
    /// be passed to `init_from_state` on the new plugin instance.
    ///
    /// Returns `None` if the plugin doesn't support hot reload.
    ///
    /// # Safety
    /// The context pointer must be valid and properly initialized.
    pub unsafe fn prepare_hot_reload(&self, context: *const PluginContext) -> Option<PluginState> {
        let prepare_fn = self.prepare_hot_reload_fn?;

        // Validate context
        if let Err(err) = security::validate_plugin_context(context) {
            tracing::error!(
                "Plugin prepare_hot_reload: context validation failed: {:?}",
                err
            );
            return None;
        }

        Some((prepare_fn)(context))
    }

    /// Initialize the plugin from a previous state
    ///
    /// Calls the plugin's `plugin_init_from_state` function to restore state
    /// from a serialized buffer produced by a previous plugin instance's
    /// `prepare_hot_reload` call.
    ///
    /// Returns `None` if the plugin doesn't support hot reload.
    /// Returns `Some(PluginResult::Success)` on success.
    /// Returns `Some(PluginResult::Error)` if state restoration fails.
    ///
    /// # Safety
    /// The context pointer must be valid. The state must have been produced
    /// by a compatible version of the plugin.
    pub unsafe fn init_from_state(
        &self,
        context: *const PluginContext,
        state: PluginState,
    ) -> Option<PluginResult> {
        let init_fn = self.init_from_state_fn?;

        // Validate context
        if let Err(err) = security::validate_plugin_context(context) {
            tracing::error!(
                "Plugin init_from_state: context validation failed: {:?}",
                err
            );
            return Some(PluginResult::Error);
        }

        Some((init_fn)(context, state))
    }

    /// Free a PluginState that was not consumed
    ///
    /// If `init_from_state` fails or is not called, the state must be freed
    /// using this method to prevent memory leaks.
    pub fn free_state(state: PluginState) {
        (state.free)(state);
    }
}

// Re-export for convenience
pub use std::ffi::CString;

// ===== Network Endpoint Registration (RFC-0028) =====

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointType {
    OSLevel = 0,
    OverlayLevel = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolType {
    Tcp = 0,
    Udp = 1,
    WebSocket = 2,
}

// Simplified TLS configuration stub
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsConfig {
    pub cert_pem: Option<String>,
    pub key_pem: Option<String>,
    // In a real implementation this would include OCSP, ALPN, cipher suites, etc.
}

// Firewall rule stub
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirewallRule {
    pub allow: bool,
    pub src: Option<String>,
    pub dst_port: Option<u16>,
}

// C-compatible request structure (pointer fields omitted for safety in Rust tests)
#[repr(C)]
pub struct EndpointRequest {
    pub endpoint_type: EndpointType,
    pub protocol: ProtocolType,
    pub port: u16,
    // For overlay: transport name/id simulated as Rust strings in high level API
    pub network_transport_name: *const c_char,
    pub overlay_id_type: *const c_char,
    pub path: *const c_char,
    // TLS and firewall pointers would be provided in full implementation
}

// A simplified, protocol-agnostic NetworkManager suitable for tests and early integration.
pub struct NetworkManager {
    os_ports: Mutex<HashSet<u16>>,
    overlay_ids: Mutex<HashSet<String>>,
    // map allocated port -> owner id (for tests we store a string)
    owners: Mutex<HashMap<u16, String>>,
}

impl NetworkManager {
    pub fn new() -> Self {
        NetworkManager {
            os_ports: Mutex::new(HashSet::new()),
            overlay_ids: Mutex::new(HashSet::new()),
            owners: Mutex::new(HashMap::new()),
        }
    }

    /// Request an endpoint. Returns PluginResult::Success on allocation, PluginResult::ResourceExhausted if unavailable,
    /// or PluginResult::InvalidRequest for unsupported parameters.
    pub fn request_endpoint(&self, owner: &str, req: &EndpointRequest) -> PluginResult {
        match req.endpoint_type {
            EndpointType::OSLevel => {
                // Only support Tcp/Udp for now
                match req.protocol {
                    ProtocolType::Tcp | ProtocolType::Udp => {
                        let mut ports = self.os_ports.lock().unwrap();
                        if ports.contains(&req.port) {
                            return PluginResult::ResourceExhausted;
                        }
                        ports.insert(req.port);
                        let mut owners = self.owners.lock().unwrap();
                        owners.insert(req.port, owner.to_string());
                        PluginResult::Success
                    }
                    _ => PluginResult::InvalidRequest,
                }
            }
            EndpointType::OverlayLevel => {
                // For overlay, require a non-null overlay_id_type pointer
                if req.overlay_id_type.is_null() {
                    return PluginResult::InvalidRequest;
                }
                // read C string
                unsafe {
                    let cstr = std::ffi::CStr::from_ptr(req.overlay_id_type);
                    if let Ok(s) = cstr.to_str() {
                        let mut ids = self.overlay_ids.lock().unwrap();
                        if ids.contains(s) {
                            return PluginResult::ResourceExhausted;
                        }
                        ids.insert(s.to_string());
                        PluginResult::Success
                    } else {
                        PluginResult::InvalidRequest
                    }
                }
            }
        }
    }

    /// Release an OS-level port previously allocated.
    pub fn release_os_port(&self, port: u16) -> bool {
        let mut ports = self.os_ports.lock().unwrap();
        if ports.remove(&port) {
            let mut owners = self.owners.lock().unwrap();
            owners.remove(&port);
            true
        } else {
            false
        }
    }

    /// Install a firewall rule (stub). Returns true when accepted.
    pub fn install_firewall_rule(&self, _rule: FirewallRule) -> bool {
        // No-op in this stub; in real system this would call out to host firewall manager.
        true
    }

    /// Configure TLS for a given port (stub). Returns true on success.
    pub fn configure_tls(&self, _port: u16, _cfg: TlsConfig) -> bool {
        // No-op in this stub. Real implementation would persist and apply certs.
        true
    }
}

// Expose a small RPC runtime inspired by RFC-0021. This is intentionally
// lightweight: it provides a service registry keyed by service name and a
// simple call API that accepts/returns raw bytes. Runtime storage of the
// service's Protobuf IDL (as source text) is supported so codegen tools or a
// runtime descriptor-based dispatcher can be added later.
mod rpc;
pub use rpc::*;

// RFC-0021: Service Discovery and Inter-Plugin RPC
// Provides standardized service discovery API with versioning, capability filtering,
// and IDL retrieval for type-safe inter-plugin communication.
pub mod service_discovery;
pub use service_discovery::{
    ServiceDescriptor, ServiceDiscovery, ServiceDiscoveryError, ServiceFilter, VersionCompatibility,
};

// ABI v2.0 specification - RFC-0004
// Complete plugin runtime ABI with event bus, service discovery, RPC, and more
pub mod v2_spec;
pub use v2_spec::*;

// Safe FFI wrappers for ABI v2
pub mod ffi_safe;
pub use ffi_safe::*;

// ABI v2 Plugin Loader with validation and capability discovery
pub mod abi_loader;
pub use abi_loader::*;

// RFC-0004 Phase 2: Cross-platform dynamic plugin loaders
// Provides platform abstraction for Linux ELF, macOS Mach-O, and Windows PE loading
pub mod loaders;
pub use loaders::{CrossPlatformLoader, LoadedPlugin, PlatformLoaderError, PluginCapabilities};

pub mod symbols;
pub use symbols::{
    FunctionSignature, ResolvedSymbol, SignatureError, SymbolRegistry, SymbolRegistryError,
    SymbolResolutionStatus,
};

pub mod abi_compat;
pub use abi_compat::{
    versions, AbiCompatibility, AbiVersionError, CompatibilityConstraint, SemanticVersion,
};

// RFC-0004 Phase 2: Plugin lifecycle orchestration combining loaders, symbols, and ABI compat
pub mod lifecycle;
pub use lifecycle::{
    LifecycleError, LifecycleErrorType, LifecycleStage, PerformanceMetrics, PipelineStageResult,
    PluginLifecycleState, PluginLoadConfig, PluginLoadPipeline, PluginLoadResult, RecoveryAction,
    RecoveryResult, RecoveryStrategy, SecurityValidationConfig,
};

// RFC-0004 Phase 3: Audit logging for plugin events and recovery tracking
pub mod audit;
#[allow(deprecated)]
pub use audit::{
    AuditEvent as PluginAuditEvent, AuditEventId, AuditEventType, AuditLogBackend, AuditLogError,
    AuditLogFilter, AuditPluginRegistry, AuditSeverity, BackendRegistrar, DefaultAuditRegistry,
    InMemoryAuditLog,
};

pub mod clustering;

// ===== ABI v2: Typed Event Bus and Middleware =====
// The v2 event system provides a typed, JSON-backed Event type, a pluggable
// Middleware trait for intercepting/transforming events, and an EventBus trait
// with a thread-safe implementation `TypedEventBus`.
//
// Event shapes are intentionally flexible (serde_json::Value) to allow hosts
// and plugins to exchange structured data without requiring a rigid Rust type.

/// A v2 Event: topic (string), JSON payload, and optional metadata map.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub topic: String,
    pub payload: Value,
    pub metadata: Option<serde_json::Map<String, Value>>,
}

impl Event {
    /// Construct a new Event
    pub fn new<T: Into<String>>(topic: T, payload: Value) -> Self {
        Event {
            topic: topic.into(),
            payload,
            metadata: None,
        }
    }
}

/// Middleware may observe and optionally mutate or drop events.
/// Returning `Some(event)` continues delivery; `None` stops delivery.
pub trait Middleware: Send + Sync + 'static {
    fn handle(&self, event: Event) -> Option<Event>;
}

impl<F> Middleware for F
where
    F: Fn(Event) -> Option<Event> + Send + Sync + 'static,
{
    fn handle(&self, event: Event) -> Option<Event> {
        (self)(event)
    }
}

/// Subscription handle returned from `subscribe`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Subscription {
    pub id: u64,
    pub topic: String,
}

/// EventBus trait defines publish/subscribe semantics.
pub trait EventBus: Send + Sync + 'static {
    fn subscribe<F>(&self, topic: &str, handler: F) -> Subscription
    where
        F: Fn(Event) + Send + Sync + 'static;

    fn unsubscribe(&self, sub: &Subscription) -> bool;

    fn publish(&self, event: Event);

    fn add_middleware<M: Middleware>(&self, middleware: M);
}

type Handler = Arc<dyn Fn(Event) + Send + Sync>;

struct Subscriber {
    id: u64,
    handler: Handler,
}

/// Thread-safe typed event bus implementation.
pub struct TypedEventBus {
    inner: Arc<RwLock<HashMap<String, Vec<Subscriber>>>>,
    middlewares: Arc<RwLock<Vec<Arc<dyn Middleware>>>>,
    id_counter: AtomicU64,
}

impl TypedEventBus {
    pub fn new() -> Self {
        TypedEventBus {
            inner: Arc::new(RwLock::new(HashMap::new())),
            middlewares: Arc::new(RwLock::new(Vec::new())),
            id_counter: AtomicU64::new(1),
        }
    }
}

impl EventBus for TypedEventBus {
    fn subscribe<F>(&self, topic: &str, handler: F) -> Subscription
    where
        F: Fn(Event) + Send + Sync + 'static,
    {
        let id = self.id_counter.fetch_add(1, Ordering::SeqCst);
        let sub = Subscriber {
            id,
            handler: Arc::new(handler),
        };
        let mut map = self.inner.write().unwrap();
        map.entry(topic.to_string()).or_default().push(sub);
        Subscription {
            id,
            topic: topic.to_string(),
        }
    }

    fn unsubscribe(&self, sub: &Subscription) -> bool {
        let mut map = self.inner.write().unwrap();
        if let Some(list) = map.get_mut(&sub.topic) {
            let before = list.len();
            list.retain(|s| s.id != sub.id);
            return list.len() != before;
        }
        false
    }

    fn publish(&self, mut event: Event) {
        // Pass through middleware chain
        let mws = { self.middlewares.read().unwrap().clone() };
        for mw in mws.iter() {
            match mw.handle(event) {
                Some(e) => event = e,
                None => return, // middleware dropped event
            }
        }

        // Collect subscribers for exact topic and wildcard "*"
        let subs = {
            let map = self.inner.read().unwrap();
            let mut out: Vec<Handler> = Vec::new();
            if let Some(list) = map.get(&event.topic) {
                for s in list.iter() {
                    out.push(s.handler.clone());
                }
            }
            if let Some(list) = map.get("*") {
                for s in list.iter() {
                    out.push(s.handler.clone());
                }
            }
            out
        };

        for handler in subs.into_iter() {
            // deliver cloned event to each handler to avoid sharing issues
            let delivered = event.clone();
            (handler)(delivered);
        }
    }

    fn add_middleware<M: Middleware>(&self, middleware: M) {
        let mut mws = self.middlewares.write().unwrap();
        mws.push(Arc::new(middleware));
    }
}

// ===== Compatibility Shim for v1 plugins =====
// Older (v1) plugins may emit events as JSON objects with a different shape.
// The compatibility shim provides helpers to adapt those shapes into v2 Events.

/// Adapt a v1 JSON event into a v2 Event.
/// v1 expected shape: { "name": "topic.name", "data": ... }
/// This converts to Event.topic = name, Event.payload = data
pub fn adapt_v1_json_to_v2(value: &Value) -> Option<Event> {
    match value {
        Value::Object(map) => {
            let name = map.get("name")?.as_str()?.to_string();
            let data = map.get("data").cloned().unwrap_or(Value::Null);
            Some(Event {
                topic: name,
                payload: data,
                metadata: None,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_publish_subscribe() {
        let bus = TypedEventBus::new();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_c = seen.clone();

        bus.subscribe("test.topic", move |e: Event| {
            let mut v = seen_c.lock().unwrap();
            v.push((e.topic, e.payload));
        });

        let ev = Event::new("test.topic", serde_json::json!({"k":"v"}));
        bus.publish(ev);

        let v = seen.lock().unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0, "test.topic");
        assert_eq!(v[0].1, serde_json::json!({"k":"v"}));
    }

    #[test]
    fn test_middleware_chain() {
        let bus = TypedEventBus::new();
        // middleware that injects field
        bus.add_middleware(|mut e: Event| {
            if let Value::Object(ref mut map) = e.payload {
                map.insert("injected".to_string(), Value::String("yes".to_string()));
            }
            Some(e)
        });

        // middleware that blocks a topic
        bus.add_middleware(|e: Event| {
            if e.topic == "blocked" {
                return None;
            }
            Some(e)
        });

        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_c = seen.clone();
        bus.subscribe("ok", move |e: Event| {
            seen_c.lock().unwrap().push(e.payload);
        });

        let ev = Event::new("ok", serde_json::json!({"a":1}));
        bus.publish(ev);

        let v = seen.lock().unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].get("injected").and_then(|x| x.as_str()), Some("yes"));

        // blocked event should not be delivered
        let seen2 = Arc::new(Mutex::new(Vec::new()));
        let seen2c = seen2.clone();
        bus.subscribe("blocked", move |_e: Event| {
            seen2c.lock().unwrap().push(1);
        });
        bus.publish(Event::new("blocked", serde_json::json!({})));
        assert_eq!(seen2.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_v1_compat_shim() {
        let bus = TypedEventBus::new();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_c = seen.clone();
        bus.subscribe("legacy.topic", move |e: Event| {
            seen_c.lock().unwrap().push((e.topic, e.payload));
        });

        // v1 payload shape
        let v1 = serde_json::json!({"name":"legacy.topic","data":{"x":42}});
        let ev = adapt_v1_json_to_v2(&v1).expect("should adapt");
        bus.publish(ev);

        let v = seen.lock().unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0, "legacy.topic");
        assert_eq!(v[0].1, serde_json::json!({"x":42}));
    }

    #[test]
    fn test_network_manager_os_allocation_and_release() {
        let nm = NetworkManager::new();
        let req = EndpointRequest {
            endpoint_type: EndpointType::OSLevel,
            protocol: ProtocolType::Tcp,
            port: 8080,
            network_transport_name: std::ptr::null(),
            overlay_id_type: std::ptr::null(),
            path: std::ptr::null(),
        };

        let r1 = nm.request_endpoint("plugin.a", &req);
        assert_eq!(r1, PluginResult::Success);

        // Duplicate allocation should fail
        let r2 = nm.request_endpoint("plugin.b", &req);
        assert_eq!(r2, PluginResult::ResourceExhausted);

        // Release and allocate again
        assert!(nm.release_os_port(8080));
        let r3 = nm.request_endpoint("plugin.b", &req);
        assert_eq!(r3, PluginResult::Success);
    }

    #[test]
    fn test_network_manager_overlay_allocation() {
        let nm = NetworkManager::new();

        let empty_req = EndpointRequest {
            endpoint_type: EndpointType::OverlayLevel,
            protocol: ProtocolType::Tcp,
            port: 0,
            network_transport_name: std::ptr::null(),
            overlay_id_type: std::ptr::null(),
            path: std::ptr::null(),
        };

        // Missing overlay id -> invalid
        let r1 = nm.request_endpoint("plugin.x", &empty_req);
        assert_eq!(r1, PluginResult::InvalidRequest);

        let overlay_id = CString::new("route-alpha").unwrap();
        let req = EndpointRequest {
            endpoint_type: EndpointType::OverlayLevel,
            protocol: ProtocolType::Tcp,
            port: 0,
            network_transport_name: std::ptr::null(),
            overlay_id_type: overlay_id.as_ptr(),
            path: std::ptr::null(),
        };

        let r2 = nm.request_endpoint("plugin.x", &req);
        assert_eq!(r2, PluginResult::Success);

        // Duplicate overlay id should be exhausted
        let r3 = nm.request_endpoint("plugin.y", &req);
        assert_eq!(r3, PluginResult::ResourceExhausted);
    }

    #[test]
    fn test_tls_and_firewall_stubs() {
        let nm = NetworkManager::new();
        let tls = TlsConfig {
            cert_pem: Some("cert".to_string()),
            key_pem: Some("key".to_string()),
        };
        assert!(nm.configure_tls(443, tls));

        let rule = FirewallRule {
            allow: true,
            src: Some("0.0.0.0/0".to_string()),
            dst_port: Some(443),
        };
        assert!(nm.install_firewall_rule(rule));
    }

    // ===== RFC-0007: Hot Reload State Tests =====

    #[test]
    fn test_state_header_creation() {
        let header = StateHeader::new(1);
        assert!(header.is_valid());
        // Copy fields to avoid unaligned reference to packed struct
        let format_ver = header.format_version;
        let plugin_ver = header.plugin_version;
        assert_eq!(format_ver, HOT_RELOAD_STATE_VERSION);
        assert_eq!(plugin_ver, 1);
    }

    #[test]
    fn test_state_header_serialization() {
        let header = StateHeader::new(42);
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), StateHeader::SIZE);

        // Check magic bytes
        assert_eq!(&bytes[0..4], HOT_RELOAD_STATE_MAGIC);

        // Check format version
        let format_ver = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        assert_eq!(format_ver, HOT_RELOAD_STATE_VERSION);

        // Check plugin version
        let plugin_ver = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        assert_eq!(plugin_ver, 42);
    }

    #[test]
    fn test_state_header_parsing() {
        let header = StateHeader::new(123);
        let bytes = header.to_bytes();

        let (parsed, remaining) = StateHeader::from_bytes(&bytes).unwrap();
        assert!(parsed.is_valid());
        // Copy fields to avoid unaligned reference to packed struct
        let format_ver = parsed.format_version;
        let plugin_ver = parsed.plugin_version;
        assert_eq!(format_ver, HOT_RELOAD_STATE_VERSION);
        assert_eq!(plugin_ver, 123);
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_state_header_parsing_with_data() {
        let header = StateHeader::new(1);
        let header_bytes = header.to_bytes();
        let plugin_data = b"plugin state data here";

        let mut full_data = Vec::with_capacity(header_bytes.len() + plugin_data.len());
        full_data.extend_from_slice(&header_bytes);
        full_data.extend_from_slice(plugin_data);

        let (parsed, remaining) = StateHeader::from_bytes(&full_data).unwrap();
        assert!(parsed.is_valid());
        assert_eq!(remaining, plugin_data);
    }

    #[test]
    fn test_state_header_invalid_magic() {
        let mut bytes = [0u8; StateHeader::SIZE];
        bytes[0..4].copy_from_slice(b"BADM"); // Invalid magic
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&1u32.to_le_bytes());

        let result = StateHeader::from_bytes(&bytes);
        assert!(result.is_none());
    }

    #[test]
    fn test_state_header_too_short() {
        let bytes = [0u8; 4];
        let result = StateHeader::from_bytes(&bytes);
        assert!(result.is_none());
    }

    #[test]
    fn test_plugin_state_empty() {
        let state = PluginState::empty();
        assert!(!state.has_data());
        assert!(state.data.is_null());
        assert_eq!(state.len, 0);
    }

    #[test]
    fn test_plugin_state_from_vec() {
        let data = vec![1, 2, 3, 4, 5];
        let state = PluginState::from_vec(data.clone());

        assert!(state.has_data());
        assert_eq!(state.len, 5);

        unsafe {
            let slice = state.as_slice();
            assert_eq!(slice, &data[..]);
        }
    }

    #[test]
    fn test_plugin_state_versioned() {
        let plugin_data = b"my plugin state".to_vec();
        let state = PluginState::versioned(2, plugin_data.clone());

        assert!(state.has_data());

        // Should have header + data
        assert_eq!(state.len, StateHeader::SIZE + plugin_data.len());

        // Parse header
        let (header, remaining) = state.parse_header().unwrap();
        assert!(header.is_valid());
        // Copy field to avoid unaligned reference to packed struct
        let plugin_ver = header.plugin_version;
        assert_eq!(plugin_ver, 2);
        assert_eq!(remaining, &plugin_data[..]);
    }

    #[test]
    fn test_plugin_state_parse_header_empty() {
        let state = PluginState::empty();
        let result = state.parse_header();
        assert!(result.is_none());
    }

    #[test]
    fn test_plugin_state_free() {
        let state = PluginState::from_vec(vec![1, 2, 3]);
        // Just verify free doesn't panic - call the free function directly
        (state.free)(state);
    }
}

// Documentation validation tests
#[cfg(test)]
mod docs_validation;
