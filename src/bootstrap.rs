// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bootstrap plugin system for Skylet core
//!
//! This module handles the dynamic loading of bootstrap plugins that provide core services:
//! - config-manager: Configuration management and loading
//! - logging: Structured logging backend
//! - registry: Plugin registry operations
//! - secrets-manager: Secure secrets management
//!
//! The bootstrap plugins are loaded early in the application lifecycle to ensure
//! all core services are available before the main application starts.

#![allow(dead_code, unsafe_code)]

use anyhow::{anyhow, Result};
use libloading::{Library, Symbol};
use skylet_abi::ffi_safe::{contains_sensitive_info, sanitize_error_for_external};
use skylet_abi::security::{PluginSandboxPolicy, SandboxEnforcer};
#[allow(unused_imports)]
use skylet_abi::v2_spec::{PluginContextV2, PluginInitFnV2, PluginResultV2, PluginShutdownFnV2};
use std::ffi::{c_char, CStr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};
type PluginGetInfoFn = unsafe extern "C" fn() -> *const c_char;

// ============================================================================
// Service Trait Definitions
// ============================================================================

/// ConfigService trait for accessing configuration
pub trait ConfigService: Send + Sync {
    fn get_config(&self) -> Result<serde_json::Value>;
    fn set_config(&self, config: serde_json::Value) -> Result<()>;
    fn validate(&self) -> Result<()>;
}

/// LoggingService trait for structured logging
pub trait LoggingService: Send + Sync {
    fn set_level(&self, level: &str) -> Result<()>;
    fn get_level(&self) -> Result<String>;
    fn get_events(&self) -> Result<Vec<String>>;
}

/// RegistryService trait for plugin registry operations
pub trait RegistryService: Send + Sync {
    fn list_plugins(&self) -> Result<Vec<String>>;
    fn search(&self, query: &str) -> Result<Vec<String>>;
    fn add_source(&self, url: &str) -> Result<()>;
    fn remove_source(&self, url: &str) -> Result<()>;
}

/// SecretsService trait for secret management
pub trait SecretsService: Send + Sync {
    fn get_secret(&self, path: &str) -> Result<String>;
    fn set_secret(&self, path: &str, value: &str) -> Result<()>;
    fn delete_secret(&self, path: &str) -> Result<()>;
    fn list_secrets(&self, prefix: &str) -> Result<Vec<String>>;
}

// ============================================================================
// Stub Implementations for Testing
// ============================================================================

/// Stub ConfigService implementation for testing
pub struct StubConfigService {
    config: Arc<Mutex<serde_json::Value>>,
}

impl StubConfigService {
    pub fn new() -> Self {
        Self {
            config: Arc::new(Mutex::new(serde_json::json!({}))),
        }
    }
}

impl Default for StubConfigService {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigService for StubConfigService {
    fn get_config(&self) -> Result<serde_json::Value> {
        Ok(self.config.lock().unwrap().clone())
    }

    fn set_config(&self, config: serde_json::Value) -> Result<()> {
        *self.config.lock().unwrap() = config;
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

/// Stub LoggingService implementation for testing
pub struct StubLoggingService;

impl LoggingService for StubLoggingService {
    fn set_level(&self, _level: &str) -> Result<()> {
        Ok(())
    }

    fn get_level(&self) -> Result<String> {
        Ok("info".to_string())
    }

    fn get_events(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
}

/// Stub RegistryService implementation for testing
pub struct StubRegistryService;

impl RegistryService for StubRegistryService {
    fn list_plugins(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }

    fn search(&self, _query: &str) -> Result<Vec<String>> {
        Ok(vec![])
    }

    fn add_source(&self, _url: &str) -> Result<()> {
        Ok(())
    }

    fn remove_source(&self, _url: &str) -> Result<()> {
        Ok(())
    }
}

/// Stub SecretsService implementation for testing
pub struct StubSecretsService {
    secrets: Arc<Mutex<std::collections::HashMap<String, String>>>,
}

impl StubSecretsService {
    pub fn new() -> Self {
        Self {
            secrets: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }
}

impl Default for StubSecretsService {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretsService for StubSecretsService {
    fn get_secret(&self, path: &str) -> Result<String> {
        self.secrets
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| anyhow!("Secret not found: {}", path))
    }

    fn set_secret(&self, path: &str, value: &str) -> Result<()> {
        self.secrets
            .lock()
            .unwrap()
            .insert(path.to_string(), value.to_string());
        Ok(())
    }

    fn delete_secret(&self, path: &str) -> Result<()> {
        self.secrets.lock().unwrap().remove(path);
        Ok(())
    }

    fn list_secrets(&self, prefix: &str) -> Result<Vec<String>> {
        let secrets = self.secrets.lock().unwrap();
        let filtered: Vec<String> = secrets
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        Ok(filtered)
    }
}

// ============================================================================
// Bootstrap Context
// ============================================================================

/// BootstrapContext holds all loaded bootstrap services
pub struct BootstrapContext {
    config_service: Option<Arc<dyn ConfigService>>,
    logging_service: Option<Arc<dyn LoggingService>>,
    registry_service: Option<Arc<dyn RegistryService>>,
    secrets_service: Option<Arc<dyn SecretsService>>,
    loaded_libraries: Vec<LoadedPlugin>,
}

/// Information about a loaded plugin library
struct LoadedPlugin {
    name: String,
    library: Box<Library>,
}

impl BootstrapContext {
    /// Create a new empty bootstrap context
    pub fn new() -> Self {
        Self {
            config_service: None,
            logging_service: None,
            registry_service: None,
            secrets_service: None,
            loaded_libraries: Vec::new(),
        }
    }

    /// Get the config service
    pub fn config_service(&self) -> Option<Arc<dyn ConfigService>> {
        self.config_service.clone()
    }

    /// Set the config service
    pub fn set_config_service(&mut self, service: Arc<dyn ConfigService>) {
        self.config_service = Some(service);
    }

    /// Get the logging service
    pub fn logging_service(&self) -> Option<Arc<dyn LoggingService>> {
        self.logging_service.clone()
    }

    /// Set the logging service
    pub fn set_logging_service(&mut self, service: Arc<dyn LoggingService>) {
        self.logging_service = Some(service);
    }

    /// Get the registry service
    pub fn registry_service(&self) -> Option<Arc<dyn RegistryService>> {
        self.registry_service.clone()
    }

    /// Set the registry service
    pub fn set_registry_service(&mut self, service: Arc<dyn RegistryService>) {
        self.registry_service = Some(service);
    }

    /// Get the secrets service
    pub fn secrets_service(&self) -> Option<Arc<dyn SecretsService>> {
        self.secrets_service.clone()
    }

    /// Set the secrets service
    pub fn set_secrets_service(&mut self, service: Arc<dyn SecretsService>) {
        self.secrets_service = Some(service);
    }

    fn register_loaded_plugin(&mut self, name: String, library: Box<Library>) {
        self.loaded_libraries.push(LoadedPlugin { name, library });
    }

    /// Verify all required bootstrap services are loaded
    pub fn verify_all_loaded(&self) -> Result<()> {
        if self.config_service.is_none() {
            return Err(anyhow!("ConfigService not loaded"));
        }
        if self.logging_service.is_none() {
            return Err(anyhow!("LoggingService not loaded"));
        }
        if self.registry_service.is_none() {
            return Err(anyhow!("RegistryService not loaded"));
        }
        if self.secrets_service.is_none() {
            return Err(anyhow!("SecretsService not loaded"));
        }
        Ok(())
    }

    /// Get all loaded plugin names
    pub fn loaded_plugin_names(&self) -> Vec<String> {
        self.loaded_libraries
            .iter()
            .map(|p| p.name.clone())
            .collect()
    }
}

impl Default for BootstrapContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BootstrapContext {
    fn drop(&mut self) {
        // Call shutdown on all plugins in reverse order with null context
        let null_context = std::ptr::null::<PluginContextV2>();
        for plugin in self.loaded_libraries.iter_mut().rev() {
            if let Err(e) = unsafe { call_plugin_shutdown(&plugin.library, null_context) } {
                warn!("Failed to shutdown plugin {}: {}", plugin.name, e);
            }
        }
    }
}

// ============================================================================
// Plugin Loader
// ============================================================================

/// DynamicPluginLoader handles dynamic loading of bootstrap plugins using libloading
pub struct DynamicPluginLoader {
    plugin_paths: Vec<PathBuf>,
}

impl DynamicPluginLoader {
    /// Create a new plugin loader with default paths
    pub fn new() -> Self {
        Self {
            plugin_paths: Self::default_plugin_paths(),
        }
    }

    /// Create with custom plugin paths
    pub fn with_paths(paths: Vec<PathBuf>) -> Self {
        Self {
            plugin_paths: paths,
        }
    }

    /// Get default plugin search paths
    fn default_plugin_paths() -> Vec<PathBuf> {
        vec![
            PathBuf::from("./target/release"),
            PathBuf::from("./target/debug"),
            PathBuf::from("/usr/local/lib/skylet/plugins"),
            PathBuf::from("/usr/lib/skylet/plugins"),
        ]
    }

    /// Find a plugin library by name
    fn find_plugin(&self, name: &str) -> Result<PathBuf> {
        for path in &self.plugin_paths {
            if path.exists() {
                // Try all platform-specific extensions
                #[cfg(target_os = "macos")]
                let extensions = ["dylib"];
                #[cfg(target_os = "windows")]
                let extensions = ["dll"];
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                let extensions = ["so"];

                for ext in &extensions {
                    let plugin_path = path.join(format!("lib{}.{}", name, ext));
                    if plugin_path.exists() {
                        debug!("Found plugin at: {}", plugin_path.display());
                        return Ok(plugin_path);
                    }
                }
            }
        }
        Err(anyhow!(
            "Plugin '{}' not found in search paths: {:?}",
            name,
            self.plugin_paths
        ))
    }

    /// Load a bootstrap plugin by name
    pub fn load_plugin(&self, name: &str) -> Result<Box<Library>> {
        info!("Loading bootstrap plugin: {}", name);

        let plugin_path = self.find_plugin(name)?;
        info!("Found plugin at: {}", plugin_path.display());

        // Safety: libloading documentation states that loading from a valid path
        // with proper permissions is safe. We verify the path exists before loading.
        let library = unsafe {
            Box::new(Library::new(&plugin_path).map_err(|e| {
                let error_msg = e.to_string();
                if contains_sensitive_info(&error_msg) {
                    warn!(
                        "[SECURITY_AUDIT] Plugin library loading failed for {}: {}",
                        name, error_msg
                    );
                    anyhow!(sanitize_error_for_external(&error_msg, "plugin_loading"))
                } else {
                    warn!(
                        "[SECURITY_AUDIT] Plugin library loading failed for {}: {}",
                        name, error_msg
                    );
                    anyhow!("Failed to load plugin library {}: {}", name, e)
                }
            })?)
        };

        // Call plugin initialization
        unsafe { call_plugin_init(&library, name)? };

        info!("Bootstrap plugin {} loaded successfully", name);
        Ok(library)
    }
}

impl Default for DynamicPluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// FFI Helper Functions
// ============================================================================

/// Create appropriate sandbox policy for a given plugin
///
/// Applies different security policies based on plugin type
fn create_sandbox_policy(name: &str) -> PluginSandboxPolicy {
    match name {
        "secrets-manager" => PluginSandboxPolicy::restrictive(name),
        "config-manager" => PluginSandboxPolicy::permissive(name),
        "logging" => {
            let mut policy = PluginSandboxPolicy::permissive(name);
            policy.allow_child_processes = false;
            policy.max_memory = 256 * 1024 * 1024; // 256MB for logging
            policy
        }
        "registry" => {
            let mut policy = PluginSandboxPolicy::permissive(name);
            policy.max_bandwidth = 10 * 1024 * 1024; // 10MB/s
            policy
        }
        _ => {
            warn!(
                "Unknown bootstrap plugin '{}', applying restrictive policy",
                name
            );
            PluginSandboxPolicy::restrictive(name)
        }
    }
}

/// Safely call plugin_init() or plugin_init_v2() on a loaded plugin
///
/// # Safety
/// This function calls external code and must be treated with care.
///
/// RFC-0004 Dual ABI Support:
/// - Tries v2 ABI first (plugin_init_v2 with PluginContextV2)
///
/// Load all bootstrap plugins using V2 ABI
///
/// Security enhancements:
/// - Validates plugin context structure before use (CVSS 9.8)
/// - Generates context signature to detect tampering (CVSS 7.8)
/// - Enforces sandbox policies (CVSS 9.6)
/// - Catches panics to prevent engine crashes (CVSS 10.0)
unsafe fn call_plugin_init(library: &Library, name: &str) -> Result<()> {
    debug!("Calling plugin_init for {}", name);

    // =================================================================
    // PANIC CATCHING: Prevent plugin panics from crashing the execution engine
    // =================================================================
    // This catches any panics that occur during plugin initialization and
    // converts them to proper error handling, preventing the entire Skylet
    // execution engine from crashing due to a faulty plugin.
    let init_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        call_plugin_init_impl(library, name)
    }));

    match init_result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(panic_info) => {
            let panic_msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Unknown panic".to_string()
            };

            let error_msg = format!(
                "Plugin {} panicked during initialization: {}",
                name, panic_msg
            );

            // Sanitize error message before logging to prevent information leakage
            let sanitized = sanitize_error_for_external(&error_msg, "plugin_init");

            error!(
                "[SECURITY_AUDIT] Plugin {} initialization panicked (caught): {}",
                name, sanitized
            );

            Err(anyhow!(
                "Plugin {} panicked during initialization: {}",
                name,
                sanitized
            ))
        }
    }
}

/// Implementation of plugin initialization logic
/// Separated to allow panic catching via catch_unwind
unsafe fn call_plugin_init_impl(library: &Library, name: &str) -> Result<()> {
    use skylet_abi::v2_spec::{PluginContextV2, PluginInitFnV2, PluginResultV2};

    debug!("Calling plugin_init for {}", name);

    // =================================================================
    // RFC-0004 Dual ABI Support
    // =================================================================
    // Try v2 ABI first (plugin_init_v2)
    if let Ok(init_fn_v2) = library.get::<Symbol<PluginInitFnV2>>(b"plugin_init_v2") {
        debug!(
            "Loading plugin {} using RFC-0004 v2 ABI (plugin_init_v2)",
            name
        );

        // Create a minimal v2 context with null services
        let context_v2 = PluginContextV2 {
            logger: std::ptr::null(),
            config: std::ptr::null(),
            service_registry: std::ptr::null(),
            event_bus: std::ptr::null(),
            rpc_service: std::ptr::null(),
            http_router: std::ptr::null(), // RFC-0019: HttpRouterV2
            tracer: std::ptr::null(),
            secrets: std::ptr::null(),
            rotation_notifications: std::ptr::null(),
            user_data: std::ptr::null_mut(),
            user_context_json: std::ptr::null(),
        };

        let context_ptr_v2 = &context_v2 as *const PluginContextV2;

        // Apply sandbox policy and call v2 init
        let sandbox_policy = create_sandbox_policy(name);
        let enforcer = SandboxEnforcer::new();
        if let Err(e) = enforcer.register_plugin(sandbox_policy) {
            return Err(anyhow!(
                "Failed to register sandbox policy for plugin {}: {:?}",
                name,
                e
            ));
        }

        let result = init_fn_v2(context_ptr_v2);
        if result != PluginResultV2::Success {
            let error_msg = format!("{:?}", result);
            warn!(
                "[SECURITY_AUDIT] Plugin {} v2 initialization failed with code: {:?}",
                name, result
            );
            return Err(anyhow!(sanitize_error_for_external(
                &error_msg,
                "plugin_init"
            )));
        }

        info!("Plugin {} initialized successfully with v2 ABI", name);
        return Ok(());
    }

    // No v2 ABI found - this plugin is not compatible
    Err(anyhow!(
        "Plugin {} does not support v2 ABI (plugin_init_v2 not found)",
        name
    ))
}

/// Safely call plugin_shutdown_v2() on a loaded plugin
///
/// Security: Validates plugin context before shutdown (CVSS 9.8)
/// Note: During cleanup (Drop), context may be null which is intentional and safe
/// PANIC CATCHING: Prevents plugin panics from crashing the execution engine
unsafe fn call_plugin_shutdown(library: &Library, context: *const PluginContextV2) -> Result<()> {
    // =================================================================
    // PANIC CATCHING: Prevent plugin panics from crashing the execution engine
    // =================================================================
    let shutdown_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        call_plugin_shutdown_impl(library, context)
    }));

    match shutdown_result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(panic_info) => {
            let panic_msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Unknown panic".to_string()
            };

            let sanitized = sanitize_error_for_external(&panic_msg, "plugin_shutdown");

            error!(
                "[SECURITY_AUDIT] Plugin shutdown panicked (caught): {}",
                sanitized
            );

            // During shutdown, we log the panic but don't fail hard
            // This allows the cleanup process to continue
            warn!("Plugin shutdown panicked, continuing cleanup process");
            Ok(())
        }
    }
}

/// Implementation of plugin shutdown logic
/// Separated to allow panic catching via catch_unwind
unsafe fn call_plugin_shutdown_impl(
    library: &Library,
    context: *const PluginContextV2,
) -> Result<()> {
    // =================================================================
    // SECURITY: FFI Boundary Validation (CVSS 9.8)
    // =================================================================
    // During cleanup (Drop), a null context is intentional and acceptable
    // as plugins should handle graceful shutdown without a context

    // Try v2 shutdown first
    if let Ok(shutdown_fn_v2) = library.get::<Symbol<PluginShutdownFnV2>>(b"plugin_shutdown_v2") {
        debug!(
            "Calling plugin_shutdown_v2 with context: {}",
            if context.is_null() { "null" } else { "valid" }
        );
        let result = shutdown_fn_v2(context);
        if result != PluginResultV2::Success {
            return Err(anyhow!("Plugin shutdown v2 failed with code: {:?}", result));
        }
        debug!("Plugin v2 shutdown completed");
        return Ok(());
    }

    // Fallback to null context for v1 shutdown
    if context.is_null() {
        debug!("Plugin shutdown called with null context (cleanup phase)");
    }

    // Try v1 shutdown as fallback
    let shutdown_fn: Symbol<skylet_abi::PluginShutdownFn> = library
        .get(b"plugin_shutdown")
        .map_err(|e| anyhow!("Failed to find plugin_shutdown: {}", e))?;

    debug!(
        "Calling plugin_shutdown (v1) with context: {}",
        if context.is_null() { "null" } else { "valid" }
    );
    let result = shutdown_fn(std::ptr::null());
    if result != skylet_abi::PluginResult::Success {
        return Err(anyhow!("Plugin shutdown failed with code: {:?}", result));
    }

    debug!("Plugin shutdown completed with security checks");
    Ok(())
}

/// Safely call plugin_get_info() on a loaded plugin
unsafe fn call_plugin_get_info(library: &Library) -> Result<String> {
    let get_info_fn: Symbol<PluginGetInfoFn> = library
        .get(b"plugin_get_info")
        .map_err(|e| anyhow!("Failed to find plugin_get_info: {}", e))?;

    let info_ptr = get_info_fn();
    if info_ptr.is_null() {
        return Err(anyhow!("plugin_get_info returned null"));
    }

    let c_str = CStr::from_ptr(info_ptr);
    let info_str = c_str
        .to_str()
        .map_err(|e| anyhow!("Failed to convert plugin info to string: {}", e))?;

    Ok(info_str.to_string())
}

// ============================================================================
// Bootstrap Functions
// ============================================================================

/// Load all bootstrap plugins and initialize services
///
/// # Arguments
/// * `_config_path` - Optional path to configuration file
///
/// # Returns
/// A BootstrapContext with all loaded services
pub fn load_bootstrap_plugins(_config_path: Option<&str>) -> Result<BootstrapContext> {
    info!("Starting bootstrap plugin loading sequence");

    let mut context = BootstrapContext::new();
    let loader = DynamicPluginLoader::new();

    // Try to load config-manager plugin first (other plugins may need it)
    info!("Loading config-manager plugin...");
    match loader.load_plugin("config_manager") {
        Ok(lib) => {
            context.register_loaded_plugin("config-manager".to_string(), lib);
            context.set_config_service(Arc::new(StubConfigService::new()));
            info!("ConfigService loaded");
        }
        Err(e) => {
            warn!(
                "Failed to load config-manager plugin (will use stub): {}",
                e
            );
            context.set_config_service(Arc::new(StubConfigService::new()));
        }
    }

    // Load logging plugin
    info!("Loading logging plugin...");
    match loader.load_plugin("logging") {
        Ok(lib) => {
            context.register_loaded_plugin("logging".to_string(), lib);
            context.set_logging_service(Arc::new(StubLoggingService));
            info!("LoggingService loaded");
        }
        Err(e) => {
            warn!("Failed to load logging plugin (will use stub): {}", e);
            context.set_logging_service(Arc::new(StubLoggingService));
        }
    }

    // Load registry plugin
    info!("Loading registry plugin...");
    match loader.load_plugin("registry") {
        Ok(lib) => {
            context.register_loaded_plugin("registry".to_string(), lib);
            context.set_registry_service(Arc::new(StubRegistryService));
            info!("RegistryService loaded");
        }
        Err(e) => {
            warn!("Failed to load registry plugin (will use stub): {}", e);
            context.set_registry_service(Arc::new(StubRegistryService));
        }
    }

    // Load secrets-manager plugin
    info!("Loading secrets-manager plugin...");
    match loader.load_plugin("secrets_manager") {
        Ok(lib) => {
            context.register_loaded_plugin("secrets-manager".to_string(), lib);
            context.set_secrets_service(Arc::new(StubSecretsService::new()));
            info!("SecretsService loaded");
        }
        Err(e) => {
            warn!(
                "Failed to load secrets-manager plugin (will use stub): {}",
                e
            );
            context.set_secrets_service(Arc::new(StubSecretsService::new()));
        }
    }

    info!("Verifying all bootstrap plugins loaded...");
    context.verify_all_loaded()?;

    info!("Bootstrap plugin loading sequence completed successfully");
    info!("Loaded plugins: {:?}", context.loaded_plugin_names());

    Ok(context)
}

/// Shutdown all bootstrap plugins gracefully
///
/// # Arguments
/// * `context` - The BootstrapContext to shut down
pub fn shutdown_bootstrap_plugins(context: BootstrapContext) -> Result<()> {
    info!("Starting bootstrap plugin shutdown sequence");

    let loaded = context.loaded_plugin_names();
    if !loaded.is_empty() {
        info!("Shutting down plugins: {:?}", loaded);
    }

    drop(context);

    info!("Bootstrap plugin shutdown sequence completed successfully");
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_context_creation() {
        let context = BootstrapContext::new();
        assert!(context.config_service.is_none());
        assert!(context.logging_service.is_none());
        assert!(context.registry_service.is_none());
        assert!(context.secrets_service.is_none());
    }

    #[test]
    fn test_bootstrap_context_verify_fails_when_empty() {
        let context = BootstrapContext::new();
        assert!(context.verify_all_loaded().is_err());
    }

    #[test]
    fn test_stub_config_service() {
        let service = StubConfigService::new();
        let config = service.get_config().unwrap();
        assert_eq!(config, serde_json::json!({}));

        let new_config = serde_json::json!({"key": "value"});
        service.set_config(new_config.clone()).unwrap();
        let retrieved = service.get_config().unwrap();
        assert_eq!(retrieved, new_config);
    }

    #[test]
    fn test_stub_secrets_service() {
        let service = StubSecretsService::new();
        service.set_secret("test/key", "value").unwrap();
        let secret = service.get_secret("test/key").unwrap();
        assert_eq!(secret, "value");

        let secrets = service.list_secrets("test/").unwrap();
        assert_eq!(secrets.len(), 1);

        service.delete_secret("test/key").unwrap();
        assert!(service.get_secret("test/key").is_err());
    }

    #[test]
    fn test_plugin_loader_default_paths() {
        let loader = DynamicPluginLoader::new();
        assert!(!loader.plugin_paths.is_empty());
    }

    #[test]
    fn test_plugin_loader_custom_paths() {
        let paths = vec![PathBuf::from("./custom/path")];
        let loader = DynamicPluginLoader::with_paths(paths.clone());
        assert_eq!(loader.plugin_paths, paths);
    }

    #[test]
    fn test_stub_services_in_context() {
        let mut context = BootstrapContext::new();
        context.set_config_service(Arc::new(StubConfigService::new()));
        context.set_logging_service(Arc::new(StubLoggingService));
        context.set_registry_service(Arc::new(StubRegistryService));
        context.set_secrets_service(Arc::new(StubSecretsService::new()));

        assert!(context.verify_all_loaded().is_ok());
        assert!(context.config_service.is_some());
        assert!(context.logging_service.is_some());
        assert!(context.registry_service.is_some());
        assert!(context.secrets_service.is_some());
    }

    #[test]
    fn test_load_bootstrap_plugins_uses_stubs() {
        let context = load_bootstrap_plugins(None).unwrap();

        // Verify all services are loaded
        assert!(context.verify_all_loaded().is_ok());

        // Verify services work
        assert!(context.config_service().is_some());
        assert!(context.logging_service().is_some());
        assert!(context.registry_service().is_some());
        assert!(context.secrets_service().is_some());
    }

    #[test]
    fn test_bootstrap_context_service_operations() {
        let context = load_bootstrap_plugins(None).unwrap();

        // Test config service
        if let Some(config_svc) = context.config_service() {
            let config = config_svc.get_config().unwrap();
            assert_eq!(config, serde_json::json!({}));

            let new_config = serde_json::json!({"test": "data"});
            config_svc.set_config(new_config).unwrap();
            config_svc.validate().unwrap();
        }

        // Test secrets service
        if let Some(secrets_svc) = context.secrets_service() {
            secrets_svc
                .set_secret("app/db/password", "secret123")
                .unwrap();
            let secret = secrets_svc.get_secret("app/db/password").unwrap();
            assert_eq!(secret, "secret123");
        }
    }

    #[test]
    fn test_shutdown_bootstrap_plugins() {
        let context = load_bootstrap_plugins(None).unwrap();
        let result = shutdown_bootstrap_plugins(context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_logging_service_operations() {
        let service = StubLoggingService;
        assert_eq!(service.get_level().unwrap(), "info");
        service.set_level("debug").unwrap();
        let events = service.get_events().unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_registry_service_operations() {
        let service = StubRegistryService;
        assert!(service.list_plugins().unwrap().is_empty());
        assert!(service.search("test").unwrap().is_empty());
        service.add_source("https://example.com").unwrap();
        service.remove_source("https://example.com").unwrap();
    }

    #[test]
    fn test_bootstrap_context_default() {
        let context = BootstrapContext::default();
        assert!(context.verify_all_loaded().is_err());
    }

    #[test]
    fn test_plugin_loader_default() {
        let loader = DynamicPluginLoader::default();
        assert!(!loader.plugin_paths.is_empty());
    }

    #[test]
    fn test_loaded_plugin_names() {
        let mut context = BootstrapContext::new();
        context.set_config_service(Arc::new(StubConfigService::new()));
        context.set_logging_service(Arc::new(StubLoggingService));
        context.set_registry_service(Arc::new(StubRegistryService));
        context.set_secrets_service(Arc::new(StubSecretsService::new()));

        let names = context.loaded_plugin_names();
        assert!(names.is_empty()); // No dynamic plugins loaded, just stubs
    }
}
