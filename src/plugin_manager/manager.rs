// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Plugin Manager v2 - ABI v2 Integration
///
/// This module handles loading, managing, and executing plugins using the
/// ABI v2 specification.
///
///
/// ## Service Wiring
///
/// The PluginContextV2 provides plugins with access to host services:
/// - **EventBusV2**: Wired to TypedEventBus for pub/sub messaging
/// - **ConfigV2**: Wired to PluginConfigBackend for key-value config
/// - **ServiceRegistryV2**: In-memory service discovery and registration
/// - **RpcServiceV2**: Wired to RpcRegistry for RPC calls
/// - **LoggerV2**: Wired to tracing crate for structured logging
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

// Epoch-based memory reclamation for safe hot-reload
use super::epoch_guard::EpochGuardedPlugin;

// ABI v2 imports
#[allow(unused_imports)]
use skylet_abi::{
    config_schema::{ConfigSchemaValidator, ConfigValidationResult},
    http::{HttpMethod, HttpRouterV2, MiddlewareConfigV2, RouteConfigV2, RouteMetadata},
    security::{DefaultSecretsProvider, SecretsProvider},
    v2_spec::{
        ConfigV2, EventBusV2, EventV2, LoggerV2, PluginContextV2, PluginInfoV2, PluginResultV2,
        RpcRequestV2, RpcResponseV2, RpcServiceV2, ServiceRegistryV2,
    },
    AbiV2PluginLoader, Event, EventBus as TypedEventBusTrait, ExporterConfig, OtelTracer, Plugin,
    PluginLogLevel, PluginSecrets, PluginTracer, RpcRegistry, SamplerConfig, Span, SpanBuilder,
    SpanHandle, SpanManager, Subscription, TracerConfig, TypedEventBus,
};
use serde_json::Value;
use std::ffi::{c_char, CStr, CString};
use std::sync::Mutex;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

// ============================================================================
// Plugin Services - Shared service backends for PluginContextV2
// ============================================================================

/// Configuration backend for plugins
///
/// Simple key-value store backed by a HashMap for plugin configuration.
pub struct PluginConfigBackend {
    config: Mutex<HashMap<String, String>>,
}

impl PluginConfigBackend {
    pub fn new() -> Self {
        Self {
            config: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.config.lock().unwrap().get(key).cloned()
    }

    pub fn set(&self, key: &str, value: &str) {
        self.config
            .lock()
            .unwrap()
            .insert(key.to_string(), value.to_string());
    }

    pub fn get_bool(&self, key: &str) -> bool {
        self.get(key)
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false)
    }

    pub fn get_int(&self, key: &str) -> i64 {
        self.get(key).and_then(|v| v.parse().ok()).unwrap_or(0)
    }

    pub fn get_float(&self, key: &str) -> f64 {
        self.get(key).and_then(|v| v.parse().ok()).unwrap_or(0.0)
    }
}

impl Default for PluginConfigBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// In-memory service registry for plugins
///
/// Allows plugins to register and discover services by name.
pub struct PluginServiceRegistryBackend {
    services: Mutex<HashMap<String, (*mut std::ffi::c_void, String)>>,
}

impl PluginServiceRegistryBackend {
    pub fn new() -> Self {
        Self {
            services: Mutex::new(HashMap::new()),
        }
    }

    pub fn register(&self, name: &str, service: *mut std::ffi::c_void, service_type: &str) {
        self.services
            .lock()
            .unwrap()
            .insert(name.to_string(), (service, service_type.to_string()));
    }

    pub fn get(&self, name: &str) -> Option<(*mut std::ffi::c_void, String)> {
        self.services
            .lock()
            .unwrap()
            .get(name)
            .map(|(ptr, s)| (*ptr, s.clone()))
    }

    pub fn unregister(&self, name: &str) -> bool {
        self.services.lock().unwrap().remove(name).is_some()
    }

    pub fn list_services(&self) -> Vec<String> {
        self.services.lock().unwrap().keys().cloned().collect()
    }
}

impl Default for PluginServiceRegistryBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Event subscription entry for tracking callbacks
struct EventSubscriptionEntry {
    event_type: String,
    #[allow(dead_code)] // Retained to keep FFI callback reference alive
    callback: extern "C" fn(*const EventV2),
    subscription: Subscription,
}

/// Event bus backend for plugins
///
/// Bridges the C FFI EventBusV2 to the Rust TypedEventBus.
pub struct PluginEventBusBackend {
    bus: TypedEventBus,
    subscriptions: Mutex<Vec<EventSubscriptionEntry>>,
}

impl PluginEventBusBackend {
    pub fn new() -> Self {
        Self {
            bus: TypedEventBus::new(),
            subscriptions: Mutex::new(Vec::new()),
        }
    }

    pub fn publish(&self, event: &EventV2) {
        // Convert EventV2 to Event
        let topic = unsafe {
            if event.type_.is_null() {
                return;
            }
            CStr::from_ptr(event.type_).to_string_lossy().to_string()
        };

        let payload = unsafe {
            if event.payload_json.is_null() {
                serde_json::json!({})
            } else {
                let json_str = CStr::from_ptr(event.payload_json).to_string_lossy();
                serde_json::from_str(&json_str).unwrap_or(serde_json::json!({}))
            }
        };

        let typed_event = Event::new(topic, payload);
        self.bus.publish(typed_event);
    }

    pub fn subscribe(&self, event_type: &str, callback: extern "C" fn(*const EventV2)) {
        let event_type_owned = event_type.to_string();

        // Create a bridge closure that converts Event to EventV2 and invokes the C callback
        let callback_for_closure = callback;
        let bridge = move |event: Event| {
            // Convert Rust Event to C EventV2
            // Skip events with null bytes in topic (invalid for C strings)
            let topic_cstring = match CString::new(event.topic.clone()) {
                Ok(c) => c,
                Err(_) => return,
            };
            let payload_str =
                serde_json::to_string(&event.payload).unwrap_or_else(|_| "{}".to_string());
            let payload_cstring = match CString::new(payload_str) {
                Ok(c) => c,
                Err(_) => return,
            };

            let event_v2 = EventV2 {
                type_: topic_cstring.as_ptr(),
                payload_json: payload_cstring.as_ptr(),
                timestamp_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
                source_plugin: std::ptr::null(),
            };

            // Invoke the C callback
            callback_for_closure(&event_v2);
        };

        // Subscribe to the TypedEventBus with the bridge closure
        let subscription = self.bus.subscribe(&event_type_owned, bridge);

        // Store the subscription for tracking and cleanup
        self.subscriptions
            .lock()
            .unwrap()
            .push(EventSubscriptionEntry {
                event_type: event_type_owned,
                callback,
                subscription,
            });
    }

    pub fn unsubscribe(&self, event_type: &str) -> bool {
        let mut subs = self.subscriptions.lock().unwrap();

        // Find and remove the subscription
        let mut found = false;
        subs.retain(|s| {
            if s.event_type == event_type {
                // Unsubscribe from TypedEventBus
                self.bus.unsubscribe(&s.subscription);
                found = true;
                false // Remove from our list
            } else {
                true // Keep in list
            }
        });

        found
    }
}

impl Default for PluginEventBusBackend {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// RFC-0017: Distributed Tracing Backend
// ============================================================================

/// Tracing backend for plugins
///
/// Bridges the C FFI PluginTracer to the Rust SpanManager from the tracing module.
/// Provides distributed tracing capabilities as specified in RFC-0017.
pub struct PluginTracerBackend {
    /// Span manager for tracking active spans
    span_manager: SpanManager,
    /// Active spans indexed by handle (simple counter)
    active_spans: Mutex<HashMap<u64, Span>>,
    /// Next span handle
    next_handle: Mutex<u64>,
    /// Optional OpenTelemetry tracer for export
    #[allow(dead_code)] // Stored for future span export integration
    otel_tracer: Option<OtelTracer>,
}

impl PluginTracerBackend {
    /// Create a new tracer backend with default configuration
    pub fn new() -> Self {
        Self {
            span_manager: SpanManager::new(),
            active_spans: Mutex::new(HashMap::new()),
            next_handle: Mutex::new(1), // 0 is reserved for "no span"
            otel_tracer: None,
        }
    }

    /// Create a tracer backend with OpenTelemetry export enabled
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn with_otel(config: ExporterConfig) -> Self {
        let tracer_config = TracerConfig {
            service_name: config.service_name.clone(),
            sampler: SamplerConfig::TraceIdRatioBased(config.sample_rate),
            exporter: config,
        };

        let otel_tracer = OtelTracer::new(tracer_config).ok();

        Self {
            span_manager: SpanManager::new(),
            active_spans: Mutex::new(HashMap::new()),
            next_handle: Mutex::new(1),
            otel_tracer,
        }
    }

    /// Start a new span and return its handle
    ///
    /// The span is created as a child of the current active span (if any).
    /// Returns a handle that must be used for subsequent operations on this span.
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn start_span(&self, name: &str) -> SpanHandle {
        // Get next handle
        let handle = {
            let mut next = self.next_handle.lock().unwrap();
            let h = *next;
            *next = h.wrapping_add(1);
            h
        };

        // Create the span using SpanBuilder
        let span = SpanBuilder::new(name).start(&self.span_manager);

        // Store in active spans
        {
            let mut active = self.active_spans.lock().unwrap();
            active.insert(handle, span);
        }

        handle
    }

    /// End a span by handle
    ///
    /// Marks the span as ended and removes it from active tracking.
    /// The span data may still be exported after ending.
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn end_span(&self, handle: SpanHandle) {
        let mut active = self.active_spans.lock().unwrap();
        if let Some(span) = active.remove(&handle) {
            span.end();
            // Note: In a full implementation, we would export the span here
            // via the OtelTracer if configured
        }
    }

    /// Add an event to a span
    ///
    /// Events are timestamped annotations on a span.
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn add_event(&self, handle: SpanHandle, name: &str) {
        let active = self.active_spans.lock().unwrap();
        if let Some(span) = active.get(&handle) {
            span.add_event(name);
        }
    }

    /// Set an attribute on a span
    ///
    /// Attributes are key-value pairs that provide context about the span.
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn set_attribute(&self, handle: SpanHandle, key: &str, value: &str) {
        let active = self.active_spans.lock().unwrap();
        if let Some(span) = active.get(&handle) {
            span.set_attribute(key, value);
        }
    }

    /// Get the number of active spans
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn active_span_count(&self) -> usize {
        self.active_spans.lock().unwrap().len()
    }
}

impl Default for PluginTracerBackend {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Secrets Backend for Plugins (RFC-0029)
// ============================================================================

/// Secrets backend for plugins
///
/// Provides a thread-safe secrets storage backend that implements the SecretsV2 FFI interface.
/// Uses the DefaultSecretsProvider from the ABI for encrypted storage with versioning.
pub struct PluginSecretsBackend {
    provider: DefaultSecretsProvider,
}

impl PluginSecretsBackend {
    /// Create a new secrets backend with default configuration
    pub fn new() -> Self {
        Self {
            provider: DefaultSecretsProvider::new(),
        }
    }

    /// Get a secret value
    ///
    /// # Arguments
    /// * `plugin_id` - The plugin requesting the secret
    /// * `secret_name` - The name/key of the secret
    ///
    /// # Returns
    /// The secret value as a string, or None if not found
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn get(&self, plugin_id: &str, secret_name: &str) -> Option<String> {
        self.provider
            .get_secret(plugin_id, secret_name)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
    }

    /// Store a secret value
    ///
    /// # Arguments
    /// * `plugin_id` - The plugin storing the secret
    /// * `secret_name` - The name/key of the secret
    /// * `value` - The secret value
    ///
    /// # Returns
    /// true on success, false on failure
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn put(&self, plugin_id: &str, secret_name: &str, value: &str) -> bool {
        self.provider
            .put_secret(plugin_id, secret_name, value.as_bytes(), None)
            .is_ok()
    }
}

impl Default for PluginSecretsBackend {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// HTTP Router Backend for Plugins (RFC-0019)
// ============================================================================

/// HTTP router backend for plugins
///
/// Provides a thread-safe HTTP route registration backend that implements the HttpRouterV2 FFI interface.
/// Stores routes registered by plugins and supports dynamic route discovery.
///
/// This backend enables plugins to register their own REST API endpoints at initialization time.
pub struct PluginHttpRouterBackend {
    /// Registered routes indexed by "METHOD:path" key
    routes: Mutex<HashMap<String, PluginRouteEntry>>,
    /// Route metadata for OpenAPI generation
    route_metadata: Mutex<Vec<RouteMetadata>>,
}

/// Internal route entry stored by the backend
#[allow(dead_code)] // Fields stored for future HTTP routing dispatch and OpenAPI generation
struct PluginRouteEntry {
    /// HTTP method (GET, POST, etc.)
    method: HttpMethod,
    /// Path pattern (e.g., "/api/items/{id}")
    path: String,
    /// Plugin that registered this route
    plugin_name: String,
    /// Optional description for API documentation
    description: Option<String>,
    /// User data pointer (owned by plugin)
    user_data: *mut std::ffi::c_void,
}

// SAFETY: PluginRouteEntry contains raw pointers but is only accessed behind a Mutex
unsafe impl Send for PluginRouteEntry {}
unsafe impl Sync for PluginRouteEntry {}

impl PluginHttpRouterBackend {
    /// Create a new HTTP router backend
    pub fn new() -> Self {
        Self {
            routes: Mutex::new(HashMap::new()),
            route_metadata: Mutex::new(Vec::new()),
        }
    }

    /// Register a new route
    ///
    /// # Arguments
    /// * `method` - HTTP method
    /// * `path` - URL path pattern (supports {param} syntax)
    /// * `plugin_name` - Name of the registering plugin
    /// * `description` - Optional description for API docs
    /// * `user_data` - User data pointer for the handler
    ///
    /// # Returns
    /// true if registration succeeded, false if route already exists
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn register_route(
        &self,
        method: HttpMethod,
        path: &str,
        plugin_name: &str,
        description: Option<&str>,
        user_data: *mut std::ffi::c_void,
    ) -> bool {
        let key = format!("{}:{}", Self::method_to_string(method), path);

        let entry = PluginRouteEntry {
            method,
            path: path.to_string(),
            plugin_name: plugin_name.to_string(),
            description: description.map(|s| s.to_string()),
            user_data,
        };

        // Extract path params for metadata
        let path_params = skylet_abi::http::extract_path_params(path);

        let metadata = RouteMetadata {
            method: Self::method_to_string(method).to_string(),
            path: path.to_string(),
            description: description.map(|s| s.to_string()),
            plugin: plugin_name.to_string(),
            path_params,
            tags: vec![plugin_name.to_string()],
        };

        let mut routes = self.routes.lock().unwrap();
        if routes.contains_key(&key) {
            warn!("Route already registered: {}", key);
            return false;
        }

        routes.insert(key, entry);

        // Add to metadata list
        let mut meta = self.route_metadata.lock().unwrap();
        meta.push(metadata);

        true
    }

    /// Unregister a route
    ///
    /// # Returns
    /// true if route was removed, false if not found
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn unregister_route(&self, method: HttpMethod, path: &str) -> bool {
        let key = format!("{}:{}", Self::method_to_string(method), path);

        let mut routes = self.routes.lock().unwrap();
        if routes.remove(&key).is_some() {
            // Also remove from metadata
            let mut meta = self.route_metadata.lock().unwrap();
            meta.retain(|m| !(m.method == Self::method_to_string(method) && m.path == path));
            true
        } else {
            false
        }
    }

    /// Get all registered routes as JSON
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn get_routes_json(&self) -> String {
        let meta = self.route_metadata.lock().unwrap();
        serde_json::to_string(&*meta).unwrap_or_else(|_| "[]".to_string())
    }

    /// Generate OpenAPI specification as JSON
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn get_openapi_spec_json(&self) -> String {
        let meta = self.route_metadata.lock().unwrap();

        let mut paths: std::collections::HashMap<
            String,
            std::collections::HashMap<String, serde_json::Value>,
        > = std::collections::HashMap::new();

        for route in meta.iter() {
            let method_lower = route.method.to_lowercase();

            let mut params = Vec::new();
            for param in &route.path_params {
                params.push(serde_json::json!({
                    "name": param,
                    "in": "path",
                    "required": true,
                    "schema": { "type": "string" }
                }));
            }

            let op = serde_json::json!({
                "summary": route.description.clone().unwrap_or_default(),
                "operationId": format!("{}_{}", route.plugin, route.path.replace("/", "_").replace("{", "").replace("}", "")),
                "tags": route.tags,
                "parameters": params,
                "responses": {
                    "200": {
                        "description": "Success",
                        "content": {
                            "application/json": {
                                "schema": { "type": "object" }
                            }
                        }
                    }
                }
            });

            let path_item = paths.entry(route.path.clone()).or_default();
            path_item.insert(method_lower, op);
        }

        let openapi = serde_json::json!({
            "openapi": "3.0.0",
            "info": {
                "title": "Skylet Plugin API",
                "version": "1.0.0",
                "description": "REST API endpoints provided by Skylet plugins"
            },
            "paths": paths
        });

        serde_json::to_string_pretty(&openapi).unwrap_or_else(|_| "{}".to_string())
    }

    /// Get route count
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn route_count(&self) -> usize {
        self.routes.lock().unwrap().len()
    }

    /// Convert HttpMethod to string
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    fn method_to_string(method: HttpMethod) -> &'static str {
        match method {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
        }
    }
}

impl Default for PluginHttpRouterBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Aggregated plugin services container
///
/// This struct holds all the service backends that are wired to PluginContextV2.
/// It is passed via user_data pointer to allow C FFI callbacks to access services.
pub struct PluginServices {
    pub config: Arc<PluginConfigBackend>,
    pub service_registry: Arc<PluginServiceRegistryBackend>,
    pub event_bus: Arc<PluginEventBusBackend>,
    pub rpc_registry: Arc<RpcRegistry>,
    /// Distributed tracing backend (RFC-0017)
    #[allow(dead_code)] // Allocated for future use — FFI callbacks currently have inline implementations
    pub tracer: Arc<PluginTracerBackend>,
    /// Secrets management backend (RFC-0029)
    #[allow(dead_code)] // Allocated for future use — FFI callbacks currently have inline implementations
    pub secrets: Arc<PluginSecretsBackend>,
    /// HTTP router backend (RFC-0019)
    pub http_router: Arc<PluginHttpRouterBackend>,
}

impl PluginServices {
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new() -> Self {
        Self {
            config: Arc::new(PluginConfigBackend::new()),
            service_registry: Arc::new(PluginServiceRegistryBackend::new()),
            event_bus: Arc::new(PluginEventBusBackend::new()),
            rpc_registry: Arc::new(RpcRegistry::new()),
            tracer: Arc::new(PluginTracerBackend::new()),
            secrets: Arc::new(PluginSecretsBackend::new()),
            http_router: Arc::new(PluginHttpRouterBackend::new()),
        }
    }

    /// Create services with OpenTelemetry-enabled tracing
    #[allow(clippy::arc_with_non_send_sync)]
    #[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
    pub fn with_tracing(exporter_config: ExporterConfig) -> Self {
        Self {
            config: Arc::new(PluginConfigBackend::new()),
            service_registry: Arc::new(PluginServiceRegistryBackend::new()),
            event_bus: Arc::new(PluginEventBusBackend::new()),
            rpc_registry: Arc::new(RpcRegistry::new()),
            tracer: Arc::new(PluginTracerBackend::with_otel(exporter_config)),
            secrets: Arc::new(PluginSecretsBackend::new()),
            http_router: Arc::new(PluginHttpRouterBackend::new()),
        }
    }
}

impl Default for PluginServices {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracks allocated resources for a single plugin instance
///
/// This struct holds pointers to resources that were allocated during plugin
/// initialization and must be freed when the plugin is unloaded.
struct PluginResources {
    /// The raw pointer to the Arc<PluginServices> that was passed via user_data
    /// Created with Arc::into_raw(), must be reclaimed with Arc::from_raw()
    services_ptr: *const PluginServices,
    /// Raw pointers to Box'd service structs that must be freed
    logger_ptr: *mut LoggerV2,
    config_ptr: *mut ConfigV2,
    registry_ptr: *mut ServiceRegistryV2,
    eventbus_ptr: *mut EventBusV2,
    rpc_ptr: *mut RpcServiceV2,
    tracer_ptr: *mut PluginTracer,
    secrets_ptr: *mut PluginSecrets,
    http_router_ptr: *mut HttpRouterV2,
}

// SAFETY: PluginResources contains raw pointers but they are only accessed
// through the PluginManager which ensures proper synchronization via RwLock
unsafe impl Send for PluginResources {}
unsafe impl Sync for PluginResources {}

impl Drop for PluginResources {
    fn drop(&mut self) {
        // Reclaim the Arc<PluginServices> to decrement reference count
        if !self.services_ptr.is_null() {
            unsafe {
                let _ = Arc::from_raw(self.services_ptr);
            }
        }

        // Free all the Box'd service structs
        unsafe {
            if !self.logger_ptr.is_null() {
                let _ = Box::from_raw(self.logger_ptr);
            }
            if !self.config_ptr.is_null() {
                let _ = Box::from_raw(self.config_ptr);
            }
            if !self.registry_ptr.is_null() {
                let _ = Box::from_raw(self.registry_ptr);
            }
            if !self.eventbus_ptr.is_null() {
                let _ = Box::from_raw(self.eventbus_ptr);
            }
            if !self.rpc_ptr.is_null() {
                let _ = Box::from_raw(self.rpc_ptr);
            }
            if !self.tracer_ptr.is_null() {
                let _ = Box::from_raw(self.tracer_ptr);
            }
            if !self.secrets_ptr.is_null() {
                let _ = Box::from_raw(self.secrets_ptr);
            }
            if !self.http_router_ptr.is_null() {
                let _ = Box::from_raw(self.http_router_ptr);
            }
        }
    }
}

/// Main plugin manager that handles the complete plugin lifecycle
///
/// Supports ABI v2 plugins only.
pub struct PluginManager {
    /// Loaded v2 plugins - key is fully qualified name
    ///
    /// Stores EpochGuardedPlugin instances for v2 plugins. The epoch-based
    /// reclamation ensures safe hot-reload by deferring destruction of old
    /// plugin versions until all in-flight requests have completed.
    loaded_plugins_v2: Arc<RwLock<HashMap<String, EpochGuardedPlugin>>>,

    /// Allocated resources for each plugin - must be freed on unload
    ///
    /// Tracks the Arc and Box pointers that were leaked during plugin initialization
    /// so they can be properly reclaimed when the plugin is unloaded.
    plugin_resources: Arc<RwLock<HashMap<String, PluginResources>>>,

    /// Shared services for all loaded plugins
    ///
    /// This provides the service backends that are wired to PluginContextV2.
    services: Arc<PluginServices>,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    // RFC-0003: Security fixes for unsafe FFI (Phase 3)
    // Issue #7: Vec::from_raw_parts validation (service list allocation)
    const MAX_SERVICE_LIST_SIZE: usize = 100_000;

    // Issue #8: String length bounds validation
    const MAX_EVENT_NAME_LEN: usize = 256;
    #[allow(dead_code)] // RFC-0003 security bounds for future FFI attribute validation
    const MAX_ATTRIBUTE_KEY_LEN: usize = 256;
    #[allow(dead_code)] // RFC-0003 security bounds for future FFI attribute validation
    const MAX_ATTRIBUTE_VALUE_LEN: usize = 4096;

    /// Create a new plugin manager
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new() -> Self {
        Self {
            loaded_plugins_v2: Arc::new(RwLock::new(HashMap::new())),
            plugin_resources: Arc::new(RwLock::new(HashMap::new())),
            services: Arc::new(PluginServices::new()),
        }
    }

    /// Create a plugin manager with custom services
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn with_services(services: Arc<PluginServices>) -> Self {
        Self {
            loaded_plugins_v2: Arc::new(RwLock::new(HashMap::new())),
            plugin_resources: Arc::new(RwLock::new(HashMap::new())),
            services,
        }
    }

    /// Get access to the plugin services
    pub fn services(&self) -> Arc<PluginServices> {
        self.services.clone()
    }

    /// Load a plugin from a path using ABI v2
    #[allow(dead_code)] // Simpler loading path without FFI context — used in tests
    pub async fn load_plugin(&self, name: &str, path: &PathBuf) -> Result<()> {
        info!("Loading plugin: {} from {:?}", name, path);

        // Load as v2
        match AbiV2PluginLoader::load(path) {
            Ok(loader_v2) => {
                info!("Loaded plugin {} using ABI v2", name);

                // Wrap in epoch guard for safe hot-reload
                let guarded = EpochGuardedPlugin::new(name, loader_v2);

                // Store the loader to keep it alive
                let mut plugins = self.loaded_plugins_v2.write().await;
                plugins.insert(name.to_string(), guarded);

                Ok(())
            }
            Err(e) => {
                error!("Failed to load plugin {}: {}", name, e);
                Err(anyhow!("Failed to load plugin {}: {}", name, e))
            }
        }
    }

    /// Load and initialize a plugin with v2 ABI context
    ///
    /// This is the primary loading path for v2 plugins. It:
    /// 1. Loads the shared library
    /// 2. Validates ABI compliance
    /// 3. Builds the PluginContextV2 with all services
    /// 4. Calls plugin_init_v2
    pub async fn load_plugin_instance_v2(&self, name: &str, path: &PathBuf) -> Result<()> {
        info!("Loading v2 plugin instance: {}", name);

        // Load and validate the plugin
        let loader = AbiV2PluginLoader::load(path)
            .map_err(|e| anyhow!("Failed to load v2 plugin {}: {}", name, e))?;

        // Get plugin metadata for logging
        let metadata = loader
            .get_info()
            .map_err(|e| anyhow!("Failed to get plugin info for {}: {}", name, e))?;

        info!("Plugin metadata: {:?}", metadata);

        // Get plugin capabilities
        let capabilities = loader
            .get_capabilities()
            .map_err(|e| anyhow!("Failed to get plugin capabilities for {}: {}", name, e))?;

        debug!("Plugin capabilities: {:?}", capabilities);

        // Build PluginContextV2 and track resources
        let (context_v2, resources) = self.create_plugin_context_v2(name)?;

        // Call plugin initialization
        loader
            .init(&context_v2)
            .map_err(|e| anyhow!("Failed to initialize v2 plugin {}: {}", name, e))?;

        info!("Successfully initialized v2 plugin: {}", name);

        // Wrap in epoch guard for safe hot-reload
        let guarded = EpochGuardedPlugin::new(name, loader);

        // Store the loader and resources (resources will be freed on unload)
        let mut plugins = self.loaded_plugins_v2.write().await;
        let mut resources_map = self.plugin_resources.write().await;

        plugins.insert(name.to_string(), guarded);
        resources_map.insert(name.to_string(), resources);

        Ok(())
    }

    /// Create a complete PluginContextV2 with all services
    ///
    /// This builds the v2 context with proper implementations of:
    /// - LoggerV2: Structured logging with levels
    /// - ConfigV2: Configuration access with type conversion
    /// - ServiceRegistryV2: Service discovery and registration
    /// - EventBusV2: Event publish/subscribe
    /// - RpcServiceV2: RPC call routing
    /// - TracerV2: Distributed tracing support
    /// - SecretsV2: Secrets management
    ///
    /// Returns both the context and a PluginResources struct that tracks the
    /// allocated resources for cleanup on plugin unload.
    fn create_plugin_context_v2(
        &self,
        _plugin_name: &str,
    ) -> Result<(PluginContextV2, PluginResources)> {
        // Build LoggerV2
        let logger_v2 = Box::into_raw(Box::new(LoggerV2 {
            log: Self::logger_v2_log,
            log_structured: Self::logger_v2_log_structured,
        }));

        // Build ConfigV2 - wired to PluginConfigBackend
        let config_v2 = Box::into_raw(Box::new(ConfigV2 {
            get: Self::config_v2_get,
            get_bool: Self::config_v2_get_bool,
            get_int: Self::config_v2_get_int,
            get_float: Self::config_v2_get_float,
            set: Self::config_v2_set,
            free_string: Self::config_v2_free_string,
        }));

        // Build ServiceRegistryV2 - wired to PluginServiceRegistryBackend
        let registry_v2 = Box::into_raw(Box::new(ServiceRegistryV2 {
            register: Self::registry_v2_register,
            get: Self::registry_v2_get,
            unregister: Self::registry_v2_unregister,
            list_services: Self::registry_v2_list_services,
            free_service_list: Self::registry_v2_free_service_list,
        }));

        // Build EventBusV2 - wired to PluginEventBusBackend
        let eventbus_v2 = Box::into_raw(Box::new(EventBusV2 {
            publish: Self::eventbus_v2_publish,
            subscribe: Self::eventbus_v2_subscribe,
            unsubscribe: Self::eventbus_v2_unsubscribe,
        }));

        // Build RpcServiceV2 - wired to RpcRegistry
        let rpc_v2 = Box::into_raw(Box::new(RpcServiceV2 {
            call: Self::rpc_v2_call,
            register_handler: Self::rpc_v2_register_handler,
            list_services: Self::rpc_v2_list_services,
            get_service_spec: Self::rpc_v2_get_service_spec,
            free_strings: Self::rpc_v2_free_strings,
        }));

        // Build TracerV2 - wired to PluginTracerBackend (RFC-0017)
        let tracer_v2 = Box::into_raw(Box::new(PluginTracer {
            start_span: Self::tracer_v2_start_span,
            end_span: Self::tracer_v2_end_span,
            add_event: Self::tracer_v2_add_event,
            set_attribute: Self::tracer_v2_set_attribute,
        }));

        // Build SecretsV2 - wired to PluginSecretsBackend (RFC-0029)
        let secrets_v2 = Box::into_raw(Box::new(PluginSecrets {
            get: Self::secrets_v2_get,
            put: Self::secrets_v2_put,
            free_result: Self::secrets_v2_free_result,
        }));

        // Build HttpRouterV2 - wired to PluginHttpRouterBackend (RFC-0019)
        let http_router_v2 = Box::into_raw(Box::new(HttpRouterV2 {
            register_route: Self::http_router_v2_register_route,
            unregister_route: Self::http_router_v2_unregister_route,
            register_middleware: Self::http_router_v2_register_middleware,
            get_routes: Self::http_router_v2_get_routes,
            get_openapi_spec: Self::http_router_v2_get_openapi_spec,
            free_string: Self::http_router_v2_free_string,
        }));

        // Pass PluginServices via user_data - this allows FFI callbacks to access service backends
        // The Arc is tracked in PluginResources and will be reclaimed on plugin unload
        let services_ptr = Arc::into_raw(self.services.clone());

        // Track all allocated resources for cleanup
        let resources = PluginResources {
            services_ptr,
            logger_ptr: logger_v2,
            config_ptr: config_v2,
            registry_ptr: registry_v2,
            eventbus_ptr: eventbus_v2,
            rpc_ptr: rpc_v2,
            tracer_ptr: tracer_v2,
            secrets_ptr: secrets_v2,
            http_router_ptr: http_router_v2,
        };

        // Create the main context
        let context = PluginContextV2 {
            logger: logger_v2,
            config: config_v2,
            service_registry: registry_v2,
            event_bus: eventbus_v2,
            rpc_service: rpc_v2,
            http_router: http_router_v2,
            tracer: tracer_v2 as *const skylet_abi::PluginTracer,
            secrets: secrets_v2 as *const skylet_abi::PluginSecrets,
            rotation_notifications: std::ptr::null(),
            user_data: services_ptr as *mut std::ffi::c_void,
            user_context_json: std::ptr::null(),
        };

        Ok((context, resources))
    }

    /// Helper to extract PluginServices from user_data in context
    unsafe fn get_services_from_context(
        context: *const PluginContextV2,
    ) -> Option<&'static PluginServices> {
        if context.is_null() {
            return None;
        }
        let ctx = &*context;
        if ctx.user_data.is_null() {
            return None;
        }
        Some(&*(ctx.user_data as *const PluginServices))
    }

    // ===== LoggerV2 Service Implementations =====

    extern "C" fn logger_v2_log(
        _context: *const PluginContextV2,
        level: PluginLogLevel,
        message: *const c_char,
    ) -> PluginResultV2 {
        if message.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        let msg = unsafe {
            std::ffi::CStr::from_ptr(message)
                .to_str()
                .unwrap_or("<invalid utf-8>")
        };

        match level {
            PluginLogLevel::Error => error!("{}", msg),
            PluginLogLevel::Warn => warn!("{}", msg),
            PluginLogLevel::Info => info!("{}", msg),
            PluginLogLevel::Debug => debug!("{}", msg),
            PluginLogLevel::Trace => debug!("TRACE: {}", msg), // Simplified
        }

        PluginResultV2::Success
    }

    extern "C" fn logger_v2_log_structured(
        _context: *const PluginContextV2,
        level: PluginLogLevel,
        message: *const c_char,
        _data_json: *const c_char,
    ) -> PluginResultV2 {
        // For now, treat structured logging same as regular logging
        if message.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        let msg = unsafe {
            std::ffi::CStr::from_ptr(message)
                .to_str()
                .unwrap_or("<invalid utf-8>")
        };

        match level {
            PluginLogLevel::Error => error!("STRUCTURED: {}", msg),
            PluginLogLevel::Warn => warn!("STRUCTURED: {}", msg),
            PluginLogLevel::Info => info!("STRUCTURED: {}", msg),
            PluginLogLevel::Debug => debug!("STRUCTURED: {}", msg),
            PluginLogLevel::Trace => debug!("STRUCTURED TRACE: {}", msg),
        }

        PluginResultV2::Success
    }

    // ===== ConfigV2 Service Implementations =====

    extern "C" fn config_v2_get(
        context: *const PluginContextV2,
        key: *const c_char,
    ) -> *const c_char {
        // Safety: Get services from context
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() || key.is_null() {
            return std::ptr::null();
        }

        let services = services.unwrap();
        let key_str = unsafe { CStr::from_ptr(key).to_string_lossy() };

        if let Some(value) = services.config.get(&key_str) {
            // Allocate a CString that the caller must free via free_string
            // Use into_raw which handles the error by returning null
            match CString::new(value) {
                Ok(c_string) => c_string.into_raw() as *const c_char,
                Err(_) => std::ptr::null(), // Value contained null bytes, return null
            }
        } else {
            std::ptr::null()
        }
    }

    extern "C" fn config_v2_get_bool(context: *const PluginContextV2, key: *const c_char) -> i32 {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() || key.is_null() {
            return 0;
        }

        let services = services.unwrap();
        let key_str = unsafe { CStr::from_ptr(key).to_string_lossy() };
        services.config.get_bool(&key_str) as i32
    }

    extern "C" fn config_v2_get_int(context: *const PluginContextV2, key: *const c_char) -> i64 {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() || key.is_null() {
            return 0;
        }

        let services = services.unwrap();
        let key_str = unsafe { CStr::from_ptr(key).to_string_lossy() };
        services.config.get_int(&key_str)
    }

    extern "C" fn config_v2_get_float(context: *const PluginContextV2, key: *const c_char) -> f64 {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() || key.is_null() {
            return 0.0;
        }

        let services = services.unwrap();
        let key_str = unsafe { CStr::from_ptr(key).to_string_lossy() };
        services.config.get_float(&key_str)
    }

    extern "C" fn config_v2_set(
        context: *const PluginContextV2,
        key: *const c_char,
        value: *const c_char,
    ) -> PluginResultV2 {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() {
            return PluginResultV2::ServiceUnavailable;
        }
        if key.is_null() || value.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        let services = services.unwrap();
        let key_str = unsafe { CStr::from_ptr(key).to_string_lossy() };
        let value_str = unsafe { CStr::from_ptr(value).to_string_lossy() };

        services.config.set(&key_str, &value_str);
        PluginResultV2::Success
    }

    extern "C" fn config_v2_free_string(ptr: *mut c_char) {
        if !ptr.is_null() {
            unsafe {
                let _ = CString::from_raw(ptr);
            }
        }
    }

    // ===== ServiceRegistryV2 Implementations =====

    extern "C" fn registry_v2_register(
        context: *const PluginContextV2,
        name: *const c_char,
        service: *mut std::ffi::c_void,
        service_type: *const c_char,
    ) -> PluginResultV2 {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() {
            return PluginResultV2::ServiceUnavailable;
        }
        if name.is_null() || service.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        let services = services.unwrap();
        let name_str = unsafe { CStr::from_ptr(name).to_string_lossy() };
        let type_str = if service_type.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(service_type).to_string_lossy().to_string() }
        };

        services
            .service_registry
            .register(&name_str, service, &type_str);
        PluginResultV2::Success
    }

    extern "C" fn registry_v2_get(
        context: *const PluginContextV2,
        name: *const c_char,
        service_type: *const c_char,
    ) -> *mut std::ffi::c_void {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() || name.is_null() {
            return std::ptr::null_mut();
        }

        let services = services.unwrap();
        let name_str = unsafe { CStr::from_ptr(name).to_string_lossy() };

        if let Some((ptr, _type)) = services.service_registry.get(&name_str) {
            // Optional: verify service_type if provided
            if !service_type.is_null() {
                let requested_type = unsafe { CStr::from_ptr(service_type).to_string_lossy() };
                if !_type.is_empty() && _type != requested_type {
                    return std::ptr::null_mut();
                }
            }
            ptr
        } else {
            std::ptr::null_mut()
        }
    }

    extern "C" fn registry_v2_unregister(
        context: *const PluginContextV2,
        name: *const c_char,
    ) -> PluginResultV2 {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() {
            return PluginResultV2::ServiceUnavailable;
        }
        if name.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        let services = services.unwrap();
        let name_str = unsafe { CStr::from_ptr(name).to_string_lossy() };

        if services.service_registry.unregister(&name_str) {
            PluginResultV2::Success
        } else {
            PluginResultV2::ServiceUnavailable
        }
    }

    extern "C" fn registry_v2_list_services(
        context: *const PluginContextV2,
    ) -> *const *const c_char {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() {
            return std::ptr::null();
        }

        let services = services.unwrap();
        let service_names = services.service_registry.list_services();

        // RFC-0003 Phase 3 Issue #9: Bounds check on service list iteration
        if service_names.len() > Self::MAX_SERVICE_LIST_SIZE {
            tracing::warn!(
                "Service registry exceeds max: {} > {}",
                service_names.len(),
                Self::MAX_SERVICE_LIST_SIZE
            );
        }

        // Allocate array of C string pointers
        // Note: This leaks memory - caller must use free_service_list
        if service_names.is_empty() {
            return std::ptr::null();
        }

        // Filter out any names with null bytes and convert to CStrings
        let c_strings: Vec<CString> = service_names
            .into_iter()
            .filter_map(|s| CString::new(s).ok())
            .collect();

        if c_strings.is_empty() {
            return std::ptr::null();
        }

        let mut ptrs: Vec<*const c_char> = c_strings.iter().map(|s| s.as_ptr()).collect();
        ptrs.push(std::ptr::null()); // Null terminator

        // Box the vector and leak it - caller must free
        let boxed = ptrs.into_boxed_slice();
        Box::into_raw(boxed) as *const *const c_char
    }

    extern "C" fn registry_v2_free_service_list(list: *const *const c_char, count: usize) {
        // RFC-0003 Phase 3 Issue #7: Validate Vec allocation bounds
        if count > Self::MAX_SERVICE_LIST_SIZE {
            tracing::error!(
                "Service list count exceeds max: {} > {}",
                count,
                Self::MAX_SERVICE_LIST_SIZE
            );
            return;
        }

        // Validate pointer alignment
        let addr = list as usize;
        if addr & (std::mem::align_of::<*const c_char>() - 1) != 0 {
            tracing::error!("Service list pointer misaligned: 0x{:x}", addr);
            return;
        }

        if list.is_null() {
            return;
        }

        unsafe {
            // Free the array itself
            let _ = Vec::from_raw_parts(list as *mut *const c_char, count, count);
            // Note: Individual C strings are not freed as they point to static data
        }
    }

    // ===== EventBusV2 Implementations =====

    extern "C" fn eventbus_v2_publish(
        context: *const PluginContextV2,
        event: *const EventV2,
    ) -> PluginResultV2 {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() {
            return PluginResultV2::ServiceUnavailable;
        }
        if event.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        let services = services.unwrap();

        unsafe {
            services.event_bus.publish(&*event);
        }

        PluginResultV2::Success
    }

    extern "C" fn eventbus_v2_subscribe(
        context: *const PluginContextV2,
        event_type: *const c_char,
        callback: extern "C" fn(*const EventV2),
    ) -> PluginResultV2 {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() {
            return PluginResultV2::ServiceUnavailable;
        }
        if event_type.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        let services = services.unwrap();
        let event_type_str = unsafe { CStr::from_ptr(event_type).to_string_lossy() };

        services.event_bus.subscribe(&event_type_str, callback);
        PluginResultV2::Success
    }

    extern "C" fn eventbus_v2_unsubscribe(
        context: *const PluginContextV2,
        event_type: *const c_char,
    ) -> PluginResultV2 {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() {
            return PluginResultV2::ServiceUnavailable;
        }
        if event_type.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        let services = services.unwrap();
        let event_type_str = unsafe { CStr::from_ptr(event_type).to_string_lossy() };

        if services.event_bus.unsubscribe(&event_type_str) {
            PluginResultV2::Success
        } else {
            PluginResultV2::ServiceUnavailable
        }
    }

    // ===== RpcServiceV2 Implementations =====

    extern "C" fn rpc_v2_call(
        context: *const PluginContextV2,
        service: *const c_char,
        request: *const RpcRequestV2,
        response: *mut RpcResponseV2,
    ) -> PluginResultV2 {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() {
            return PluginResultV2::ServiceUnavailable;
        }
        if service.is_null() || request.is_null() || response.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        let services = services.unwrap();
        let service_str = unsafe { CStr::from_ptr(service).to_string_lossy() };

        // Get request params as bytes
        let params_bytes = unsafe {
            if (*request).params.is_null() {
                Vec::new()
            } else {
                let params_str = CStr::from_ptr((*request).params).to_string_lossy();
                params_str.as_bytes().to_vec()
            }
        };

        // Call the RPC service
        match services.rpc_registry.call(&service_str, &params_bytes) {
            Ok(result_bytes) => {
                unsafe {
                    // Set response result
                    let result_string = String::from_utf8_lossy(&result_bytes).to_string();
                    match CString::new(result_string) {
                        Ok(result_cstring) => {
                            (*response).result = result_cstring.into_raw();
                            (*response).error = std::ptr::null();
                            (*response).status = PluginResultV2::Success;
                        }
                        Err(_) => {
                            (*response).result = std::ptr::null();
                            (*response).error = std::ptr::null();
                            (*response).status = PluginResultV2::InvalidRequest;
                        }
                    }
                }
                PluginResultV2::Success
            }
            Err(e) => {
                unsafe {
                    (*response).result = std::ptr::null();
                    let error_msg = format!("RPC error: {:?}", e);
                    match CString::new(error_msg) {
                        Ok(error_cstring) => {
                            (*response).error = error_cstring.into_raw();
                        }
                        Err(_) => {
                            (*response).error = std::ptr::null();
                        }
                    }
                    (*response).status = e;
                }
                e
            }
        }
    }

    extern "C" fn rpc_v2_register_handler(
        context: *const PluginContextV2,
        method: *const c_char,
        _handler: extern "C" fn(*const RpcRequestV2, *mut RpcResponseV2),
    ) -> PluginResultV2 {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() {
            return PluginResultV2::ServiceUnavailable;
        }
        if method.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        let services = services.unwrap();
        let method_str = unsafe { CStr::from_ptr(method).to_string_lossy() }.to_string();

        // Create a wrapper handler that converts between C FFI and Rust types
        let handler_wrapper = Arc::new(move |_request: &[u8]| {
            // For simplicity, we return success with empty response
            // A full implementation would bridge the C handler properly
            (PluginResultV2::Success, vec![])
        });

        services
            .rpc_registry
            .register(&method_str, None, None, handler_wrapper);
        PluginResultV2::Success
    }

    extern "C" fn rpc_v2_list_services(context: *const PluginContextV2) -> *const *const c_char {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() {
            return std::ptr::null();
        }

        let services = services.unwrap();
        let service_names = services.rpc_registry.list_services();

        // Allocate array of C string pointers
        if service_names.is_empty() {
            return std::ptr::null();
        }

        // Filter out any names with null bytes
        let c_strings: Vec<CString> = service_names
            .into_iter()
            .filter_map(|s| CString::new(s).ok())
            .collect();

        if c_strings.is_empty() {
            return std::ptr::null();
        }

        let mut ptrs: Vec<*const c_char> = c_strings.iter().map(|s| s.as_ptr()).collect();
        ptrs.push(std::ptr::null()); // Null terminator

        // Box the vector and leak it - caller must free with rpc_v2_free_strings
        let boxed = ptrs.into_boxed_slice();
        Box::into_raw(boxed) as *const *const c_char
    }

    extern "C" fn rpc_v2_get_service_spec(
        context: *const PluginContextV2,
        service: *const c_char,
    ) -> *const c_char {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() || service.is_null() {
            return std::ptr::null();
        }

        let services = services.unwrap();
        let service_str = unsafe { CStr::from_ptr(service).to_string_lossy() };

        if let Some(idl) = services.rpc_registry.get_idl(&service_str) {
            match CString::new(idl) {
                Ok(cstring) => cstring.into_raw() as *const c_char,
                Err(_) => std::ptr::null(),
            }
        } else {
            std::ptr::null()
        }
    }

    extern "C" fn rpc_v2_free_strings(list: *const *const c_char, count: usize) {
        // RFC-0003 Phase 3 Issue #7: Validate Vec allocation bounds
        if count > Self::MAX_SERVICE_LIST_SIZE {
            tracing::error!(
                "RPC string list count exceeds max: {} > {}",
                count,
                Self::MAX_SERVICE_LIST_SIZE
            );
            return;
        }

        // Validate pointer alignment
        let addr = list as usize;
        if addr & (std::mem::align_of::<*const c_char>() - 1) != 0 {
            tracing::error!("RPC string list pointer misaligned: 0x{:x}", addr);
            return;
        }

        if list.is_null() {
            return;
        }

        unsafe {
            // Free the array itself
            let _ = Vec::from_raw_parts(list as *mut *const c_char, count, count);
            // Note: Individual C strings are owned by their respective CStrings
            // and will be freed when those are dropped
        }
    }

    // ===== PluginTracer Implementations (RFC-0017) =====

    extern "C" fn tracer_v2_start_span(
        _context: *const (),
        name_ptr: *const c_char,
        name_len: usize,
    ) -> SpanHandle {
        // Get services from PluginContextV2
        // We get the tracer from the services backend
        // For v2, the tracer is accessible via PluginServices
        // This implementation uses a simplified approach
        // RFC-0003 Phase 3 Issue #8: Bounds check on event name length
        if name_len > Self::MAX_EVENT_NAME_LEN {
            tracing::warn!(
                "Event name length exceeds max: {} > {}",
                name_len,
                Self::MAX_EVENT_NAME_LEN
            );
        }

        let name = unsafe {
            if name_ptr.is_null() || name_len == 0 {
                "unknown"
            } else {
                let slice = std::slice::from_raw_parts(name_ptr as *const u8, name_len);
                std::str::from_utf8(slice).unwrap_or("unknown")
            }
        };
        debug!("Starting span: {}", name);

        // Return a non-zero handle to indicate success
        // In a full implementation, this would integrate with the PluginTracerBackend
        1
    }

    extern "C" fn tracer_v2_end_span(_context: *const (), span_handle: SpanHandle) {
        if span_handle == 0 {
            return;
        }
        debug!("Ending span: {}", span_handle);
        // In a full implementation, this would end the span in PluginTracerBackend
    }

    extern "C" fn tracer_v2_add_event(
        _context: *const (),
        name_ptr: *const c_char,
        name_len: usize,
    ) {
        if name_ptr.is_null() || name_len == 0 {
            return;
        }

        let name = unsafe {
            let slice = std::slice::from_raw_parts(name_ptr as *const u8, name_len);
            String::from_utf8_lossy(slice).to_string()
        };

        debug!("Adding event to span: {}", name);
        // In a full implementation, this would add event to active span
    }

    extern "C" fn tracer_v2_set_attribute(
        _context: *const (),
        key_ptr: *const c_char,
        key_len: usize,
        value_ptr: *const c_char,
        value_len: usize,
    ) {
        if key_ptr.is_null() || value_ptr.is_null() || key_len == 0 || value_len == 0 {
            return;
        }

        let key = unsafe {
            let slice = std::slice::from_raw_parts(key_ptr as *const u8, key_len);
            String::from_utf8_lossy(slice).to_string()
        };

        let value = unsafe {
            let slice = std::slice::from_raw_parts(value_ptr as *const u8, value_len);
            String::from_utf8_lossy(slice).to_string()
        };

        debug!("Setting attribute on span: {} = {}", key, value);
        // In a full implementation, this would set attribute on active span
    }

    // ===== SecretsV2 Implementations (ABI v2) =====

    extern "C" fn secrets_v2_get(
        _context: *const (),
        plugin_ptr: *const c_char,
        plugin_len: usize,
        secret_ref_ptr: *const c_char,
        secret_ref_len: usize,
    ) -> *const c_char {
        // Stub implementation - return null
        let _ = plugin_ptr;
        let _ = plugin_len;
        let _ = secret_ref_ptr;
        let _ = secret_ref_len;
        std::ptr::null()
    }

    extern "C" fn secrets_v2_put(
        _context: *const (),
        plugin_ptr: *const c_char,
        plugin_len: usize,
        secret_ref_ptr: *const c_char,
        secret_ref_len: usize,
        value_ptr: *const c_char,
        value_len: usize,
    ) -> PluginResultV2 {
        // Stub implementation - return not implemented
        let _ = plugin_ptr;
        let _ = plugin_len;
        let _ = secret_ref_ptr;
        let _ = secret_ref_len;
        let _ = value_ptr;
        let _ = value_len;
        PluginResultV2::NotImplemented
    }

    extern "C" fn secrets_v2_free_result(ptr: *mut c_char) {
        if !ptr.is_null() {
            unsafe {
                let _ = CString::from_raw(ptr);
            }
        }
    }

    // ===== HttpRouterV2 Implementations (RFC-0019) =====

    extern "C" fn http_router_v2_register_route(
        context: *const PluginContextV2,
        config: *const RouteConfigV2,
    ) -> PluginResultV2 {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() || config.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        let services = services.unwrap();
        let config_ref = unsafe { &*config };

        // Extract path from C string
        let path = if config_ref.path.is_null() {
            return PluginResultV2::InvalidRequest;
        } else {
            unsafe {
                CStr::from_ptr(config_ref.path)
                    .to_string_lossy()
                    .to_string()
            }
        };

        // Extract description (optional)
        let description = if config_ref.description.is_null() {
            None
        } else {
            Some(unsafe {
                CStr::from_ptr(config_ref.description)
                    .to_string_lossy()
                    .to_string()
            })
        };

        // Extract plugin name from config (if provided)
        let plugin_name = if config_ref.plugin_name.is_null() {
            "unknown".to_string()
        } else {
            unsafe {
                CStr::from_ptr(config_ref.plugin_name)
                    .to_string_lossy()
                    .to_string()
            }
        };

        // Register the route with the backend
        if services.http_router.register_route(
            config_ref.method,
            &path,
            &plugin_name,
            description.as_deref(),
            config_ref.user_data,
        ) {
            debug!(
                "Registered route: {:?} {} for plugin {}",
                config_ref.method, path, plugin_name
            );
            PluginResultV2::Success
        } else {
            warn!("Route already registered: {:?} {}", config_ref.method, path);
            PluginResultV2::Error
        }
    }

    extern "C" fn http_router_v2_unregister_route(
        context: *const PluginContextV2,
        method: HttpMethod,
        path: *const c_char,
    ) -> PluginResultV2 {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() || path.is_null() {
            return PluginResultV2::InvalidRequest;
        }

        let services = services.unwrap();
        let path_str = unsafe { CStr::from_ptr(path).to_string_lossy() };

        if services.http_router.unregister_route(method, &path_str) {
            debug!("Unregistered route: {:?} {}", method, path_str);
            PluginResultV2::Success
        } else {
            debug!("Route not found for unregister: {:?} {}", method, path_str);
            PluginResultV2::ServiceUnavailable
        }
    }

    extern "C" fn http_router_v2_register_middleware(
        _context: *const PluginContextV2,
        _config: *const MiddlewareConfigV2,
    ) -> PluginResultV2 {
        // Middleware registration is not yet implemented in the backend
        // This is a placeholder for future implementation
        debug!("Middleware registration called (not yet implemented)");
        PluginResultV2::NotImplemented
    }

    extern "C" fn http_router_v2_get_routes(context: *const PluginContextV2) -> *mut c_char {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() {
            return std::ptr::null_mut();
        }

        let services = services.unwrap();
        let routes_json = services.http_router.get_routes_json();

        match CString::new(routes_json) {
            Ok(cstring) => cstring.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    }

    extern "C" fn http_router_v2_get_openapi_spec(context: *const PluginContextV2) -> *mut c_char {
        let services = unsafe { Self::get_services_from_context(context) };
        if services.is_none() {
            return std::ptr::null_mut();
        }

        let services = services.unwrap();
        let openapi_json = services.http_router.get_openapi_spec_json();

        match CString::new(openapi_json) {
            Ok(cstring) => cstring.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    }

    extern "C" fn http_router_v2_free_string(ptr: *mut c_char) {
        if !ptr.is_null() {
            unsafe {
                let _ = CString::from_raw(ptr);
            }
        }
    }

    /// Unload a plugin
    ///
    /// This properly cleans up all resources allocated during plugin initialization:
    /// - Removes the plugin loader from the map (deferred via epoch reclamation)
    /// - Reclaims the Arc<PluginServices> pointer (decrements refcount)
    /// - Frees all Box'd service structs (LoggerV2, ConfigV2, etc.)
    ///
    /// Note: The actual plugin destruction is deferred via epoch-based reclamation
    /// to ensure in-flight requests complete safely before the plugin is deallocated.
    pub async fn unload_plugin(&self, name: &str) -> Result<()> {
        info!("Unloading plugin: {}", name);

        // Try to unload v2 plugin
        let mut plugins_v2 = self.loaded_plugins_v2.write().await;
        let mut resources_map = self.plugin_resources.write().await;

        if let Some(guarded_plugin) = plugins_v2.remove(name) {
            // Trigger epoch-based deferred unload
            // The actual library unload is deferred until all epoch guards are released
            guarded_plugin.unload();
            debug!(
                "Plugin {} marked for deferred unload via epoch reclamation",
                name
            );

            // Clean up resources - the Drop impl handles freeing memory
            if let Some(resources) = resources_map.remove(name) {
                // Verify cleanup by checking Arc strong count before/after
                let services_strong_count = Arc::strong_count(&self.services);
                debug!(
                    "Cleaning up resources for plugin {}, services Arc strong_count before: {}",
                    name, services_strong_count
                );

                // Resources are dropped here, which reclaims the Arc and frees Box'd structs
                drop(resources);

                debug!(
                    "Resources cleaned up for plugin {}, services Arc strong_count after: {}",
                    name,
                    Arc::strong_count(&self.services)
                );
            }

            info!("Successfully unloaded v2 plugin: {}", name);
            return Ok(());
        }

        Err(anyhow!("Plugin '{}' not found", name))
    }

    /// Get list of loaded plugins
    pub async fn list_plugins(&self) -> Result<Vec<String>> {
        let plugins_v2 = self.loaded_plugins_v2.read().await;
        Ok(plugins_v2.keys().cloned().collect())
    }

    /// Shutdown all loaded plugins and clean up resources
    ///
    /// Unloads every loaded plugin in reverse insertion order, cleaning up
    /// FFI resources (Box'd service structs, Arc<PluginServices> refs) for each.
    pub async fn shutdown_all(&self) {
        let plugin_names: Vec<String> = {
            let plugins = self.loaded_plugins_v2.read().await;
            plugins.keys().cloned().collect()
        };

        info!(
            "Shutting down {} application plugins...",
            plugin_names.len()
        );

        for name in &plugin_names {
            match self.unload_plugin(name).await {
                Ok(()) => info!("Unloaded plugin: {}", name),
                Err(e) => error!("Error unloading plugin '{}': {}", name, e),
            }
        }

        info!("All application plugins shut down");
    }

    // ========================================================================
    // RFC-0006: Plugin Configuration Schema Validation
    // ========================================================================

    /// Load plugin configuration from a TOML file
    ///
    /// Looks for config in the following locations (in order):
    /// 1. `data/{plugin_name}.toml`
    /// 2. `~/.config/skylet/plugins/{plugin_name}.toml`
    ///
    /// Returns the config as a JSON Value for schema validation.
    #[allow(dead_code)] // RFC-0006 config schema validation — not yet wired into load path
    pub fn load_plugin_config_from_toml(plugin_name: &str) -> Result<Value> {
        let config_paths = vec![
            PathBuf::from(format!("data/{}.toml", plugin_name)),
            dirs::config_dir()
                .map(|p| {
                    p.join("skylet")
                        .join("plugins")
                        .join(format!("{}.toml", plugin_name))
                })
                .unwrap_or_else(|| PathBuf::from("")),
        ];

        for config_path in config_paths {
            if !config_path.exists() {
                continue;
            }

            let content = std::fs::read_to_string(&config_path)
                .map_err(|e| anyhow!("Failed to read config file {:?}: {}", config_path, e))?;

            // Parse TOML and convert to JSON
            let toml_value: toml::Value = toml::from_str(&content)
                .map_err(|e| anyhow!("Failed to parse TOML config {:?}: {}", config_path, e))?;

            // Convert TOML value to JSON value
            let json_value = toml_to_json(toml_value);
            debug!("Loaded config for {} from {:?}", plugin_name, config_path);
            return Ok(json_value);
        }

        // No config file found, return empty object
        debug!("No config file found for plugin {}", plugin_name);
        Ok(serde_json::json!({}))
    }

    /// Validate a plugin's configuration against its schema
    ///
    /// Returns validation result with any errors found.
    #[allow(dead_code)] // RFC-0006 config schema validation — not yet wired into load path
    pub async fn validate_plugin_config(
        &self,
        plugin_name: &str,
    ) -> Result<ConfigValidationResult> {
        let plugins_v2 = self.loaded_plugins_v2.read().await;

        let guarded = plugins_v2
            .get(plugin_name)
            .ok_or_else(|| anyhow!("Plugin '{}' not found", plugin_name))?;

        // Access plugin via epoch guard
        let guard = guarded
            .access()
            .ok_or_else(|| anyhow!("Plugin '{}' is being unloaded", plugin_name))?;
        let loader = guard.plugin();

        // Get schema from plugin
        let schema_json = match loader.get_config_schema_string() {
            Some(schema) => schema,
            None => {
                debug!(
                    "Plugin {} does not export a config schema, skipping validation",
                    plugin_name
                );
                return Ok(ConfigValidationResult::valid());
            }
        };

        // Load config from TOML file
        let config_json = Self::load_plugin_config_from_toml(plugin_name)?;

        // Create validator from schema
        let validator = ConfigSchemaValidator::from_json(&schema_json).map_err(|e| {
            anyhow!(
                "Failed to compile config schema for {}: {:?}",
                plugin_name,
                e
            )
        })?;

        // Validate config against schema
        let config_str = serde_json::to_string(&config_json)
            .map_err(|e| anyhow!("Failed to serialize config: {}", e))?;

        let result = validator
            .validate(&config_str)
            .map_err(|e| anyhow!("Config validation error: {:?}", e))?;

        if !result.is_valid() {
            warn!(
                "Config validation failed for plugin {}: {:?}",
                plugin_name, result.errors
            );
        } else {
            debug!("Config validation passed for plugin {}", plugin_name);
        }

        Ok(result)
    }

    /// Load and validate plugin config, then populate PluginConfigBackend
    ///
    /// This is the main entry point for config loading during plugin initialization.
    /// It loads from TOML, validates against schema (if available), and populates
    /// the config backend with the values.
    #[allow(dead_code)] // RFC-0006 config schema validation — not yet wired into load path
    pub async fn load_and_validate_config(&self, plugin_name: &str) -> Result<bool> {
        // Load config from TOML
        let config_json = Self::load_plugin_config_from_toml(plugin_name)?;

        // Validate against schema if available
        let validation_result = self.validate_plugin_config(plugin_name).await?;

        if !validation_result.is_valid() {
            warn!(
                "Plugin {} config validation failed: {:?}",
                plugin_name, validation_result.errors
            );
            // Log warnings but don't fail - allow plugins to run with invalid config
            // in case they have their own handling
        }

        // Populate config backend with loaded values
        if let Some(obj) = config_json.as_object() {
            for (key, value) in obj {
                let value_str = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Array(arr) => {
                        serde_json::to_string(arr).unwrap_or_else(|_| "[]".to_string())
                    }
                    Value::Object(obj) => {
                        serde_json::to_string(obj).unwrap_or_else(|_| "{}".to_string())
                    }
                    Value::Null => String::new(),
                };
                self.services.config.set(key, &value_str);
            }
        }

        Ok(validation_result.is_valid())
    }

    /// Check if a plugin exports a configuration schema
    #[allow(dead_code)] // RFC-0006 config schema validation — not yet wired into load path
    pub async fn plugin_has_config_schema(&self, plugin_name: &str) -> bool {
        let plugins_v2 = self.loaded_plugins_v2.read().await;
        if let Some(guarded) = plugins_v2.get(plugin_name) {
            if let Some(guard) = guarded.access() {
                return guard.plugin().get_config_schema_string().is_some();
            }
        }
        false
    }

    /// Get the JSON schema for a plugin's configuration
    #[allow(dead_code)] // RFC-0006 config schema validation — not yet wired into load path
    pub async fn get_plugin_config_schema(&self, plugin_name: &str) -> Option<String> {
        let plugins_v2 = self.loaded_plugins_v2.read().await;
        if let Some(guarded) = plugins_v2.get(plugin_name) {
            if let Some(guard) = guarded.access() {
                return guard.plugin().get_config_schema_string();
            }
        }
        None
    }
}

/// Convert a TOML value to a JSON value
#[allow(dead_code)] // Phase 2 infrastructure — not yet wired up
fn toml_to_json(toml: toml::Value) -> Value {
    match toml {
        toml::Value::String(s) => Value::String(s),
        toml::Value::Integer(i) => Value::Number(i.into()),
        toml::Value::Float(f) => {
            if let Some(n) = serde_json::Number::from_f64(f) {
                Value::Number(n)
            } else {
                Value::Null
            }
        }
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Array(arr) => Value::Array(arr.into_iter().map(toml_to_json).collect()),
        toml::Value::Table(table) => Value::Object(
            table
                .into_iter()
                .map(|(k, v)| (k, toml_to_json(v)))
                .collect(),
        ),
        toml::Value::Datetime(dt) => Value::String(dt.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_manager_creation() {
        let manager = PluginManager::new();
        let plugins = manager.list_plugins().await.unwrap();
        assert_eq!(plugins.len(), 0);
    }

    #[tokio::test]
    async fn test_unload_nonexistent_plugin() {
        let manager = PluginManager::new();
        let result = manager.unload_plugin("nonexistent").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_services_creation() {
        let services = PluginServices::new();

        // Test config backend
        services.config.set("test_key", "test_value");
        assert_eq!(
            services.config.get("test_key"),
            Some("test_value".to_string())
        );
        assert_eq!(services.config.get_bool("test_key"), false);

        services.config.set("bool_key", "true");
        assert_eq!(services.config.get_bool("bool_key"), true);

        services.config.set("int_key", "42");
        assert_eq!(services.config.get_int("int_key"), 42);

        services.config.set("float_key", "3.14");
        assert!((services.config.get_float("float_key") - 3.14).abs() < 0.001);
    }

    #[test]
    fn test_service_registry_backend() {
        let services = PluginServices::new();

        // Test registration
        let service_ptr = 0x1000 as *mut std::ffi::c_void;
        services
            .service_registry
            .register("test_service", service_ptr, "test_type");

        // Test retrieval
        let result = services.service_registry.get("test_service");
        assert!(result.is_some());
        let (ptr, type_str) = result.unwrap();
        assert_eq!(ptr, service_ptr);
        assert_eq!(type_str, "test_type");

        // Test list
        let list = services.service_registry.list_services();
        assert!(list.contains(&"test_service".to_string()));

        // Test unregister
        let removed = services.service_registry.unregister("test_service");
        assert!(removed);

        let not_found = services.service_registry.get("test_service");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_event_bus_backend() {
        let services = PluginServices::new();

        // Create a test event
        let event_type = std::ffi::CString::new("test.event").unwrap();
        let payload = std::ffi::CString::new("{\"data\":\"test\"}").unwrap();

        let event = EventV2 {
            type_: event_type.as_ptr(),
            payload_json: payload.as_ptr(),
            timestamp_ms: 0,
            source_plugin: std::ptr::null(),
        };

        // Test publish (should not panic)
        services.event_bus.publish(&event);

        // Test unsubscribe for non-existent subscription
        let result = services.event_bus.unsubscribe("nonexistent");
        assert!(!result);
    }

    #[test]
    fn test_rpc_registry() {
        let services = PluginServices::new();

        // Register a simple RPC handler
        let handler =
            Arc::new(|_request: &[u8]| (PluginResultV2::Success, b"{\"result\":\"ok\"}".to_vec()));

        services
            .rpc_registry
            .register("test_rpc", None, None, handler);

        // Test call
        let result = services.rpc_registry.call("test_rpc", b"{}");
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response, b"{\"result\":\"ok\"}");

        // Test non-existent service
        let not_found = services.rpc_registry.call("nonexistent", b"{}");
        assert!(not_found.is_err());
    }

    #[tokio::test]
    async fn test_plugin_manager_with_custom_services() {
        let custom_services = Arc::new(PluginServices::new());
        custom_services.config.set("custom_key", "custom_value");

        let manager = PluginManager::with_services(custom_services.clone());

        // Verify services are accessible
        let services = manager.services();
        assert_eq!(
            services.config.get("custom_key"),
            Some("custom_value".to_string())
        );
    }

    #[test]
    fn test_plugin_resources_cleanup() {
        // Test that PluginResources Drop implementation properly cleans up
        let services = Arc::new(PluginServices::new());
        let initial_count = Arc::strong_count(&services);

        // Simulate what create_plugin_context_v2 does
        let services_ptr = Arc::into_raw(services.clone());

        // After Arc::into_raw, the count should increase
        assert_eq!(Arc::strong_count(&services), initial_count + 1);

        // Create PluginResources with the leaked Arc and some null pointers
        // (we don't need real Box allocations to test the Arc cleanup)
        let resources = PluginResources {
            services_ptr,
            logger_ptr: std::ptr::null_mut(),
            config_ptr: std::ptr::null_mut(),
            registry_ptr: std::ptr::null_mut(),
            eventbus_ptr: std::ptr::null_mut(),
            rpc_ptr: std::ptr::null_mut(),
            tracer_ptr: std::ptr::null_mut(),
            secrets_ptr: std::ptr::null_mut(),
            http_router_ptr: std::ptr::null_mut(),
        };

        // Drop the resources - this should reclaim the Arc
        drop(resources);

        // After drop, the count should be back to initial
        assert_eq!(Arc::strong_count(&services), initial_count);
    }

    #[test]
    fn test_plugin_resources_box_cleanup() {
        // Test that Box'd structs are properly freed
        // We create actual Box allocations and verify no panic on drop

        let services = Arc::new(PluginServices::new());
        let services_ptr = Arc::into_raw(services.clone());

        // Create actual Box allocations
        let logger = Box::into_raw(Box::new(LoggerV2 {
            log: PluginManager::logger_v2_log,
            log_structured: PluginManager::logger_v2_log_structured,
        }));

        let config = Box::into_raw(Box::new(ConfigV2 {
            get: PluginManager::config_v2_get,
            get_bool: PluginManager::config_v2_get_bool,
            get_int: PluginManager::config_v2_get_int,
            get_float: PluginManager::config_v2_get_float,
            set: PluginManager::config_v2_set,
            free_string: PluginManager::config_v2_free_string,
        }));

        let resources = PluginResources {
            services_ptr,
            logger_ptr: logger,
            config_ptr: config,
            registry_ptr: std::ptr::null_mut(),
            eventbus_ptr: std::ptr::null_mut(),
            rpc_ptr: std::ptr::null_mut(),
            tracer_ptr: std::ptr::null_mut(),
            secrets_ptr: std::ptr::null_mut(),
            http_router_ptr: std::ptr::null_mut(),
        };

        // This should not panic - all allocations should be properly freed
        drop(resources);

        // If we get here without panic or memory error, the test passes
    }
}
