// Common utilities for Skylet plugins - eliminates boilerplate and ensures consistency
// v0.3.0 - Enhanced with support for all plugin types (API, Database, Workflow, etc.) and secrets management
#![allow(dead_code, unused_imports, unused_variables)]
use anyhow::Result;
use skylet_abi::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

// ============================================================================
// Standard Config Paths - RFC-0006
// ============================================================================

/// Standard configuration path resolver for Skylet plugins (RFC-0006)
/// 
/// Provides consistent config file locations across all plugins:
/// - Primary: `~/.config/skylet/plugins/{plugin_name}.toml`
/// - Local project: `data/{plugin_name}.toml`
/// - System: `/etc/skylet/plugins/{plugin_name}.toml`
/// 
/// # Example
/// ```rust
/// use skylet_plugin_common::config_paths;
/// 
/// // Find config for "my-plugin"
/// if let Some(path) = config_paths::find_config("my-plugin") {
///     println!("Config found at: {:?}", path);
/// }
/// 
/// // Get standard config path (may not exist yet)
/// let standard_path = config_paths::get_standard_config_path("my-plugin");
/// println!("Standard config location: {:?}", standard_path);
/// ```
pub mod config_paths {
    use super::*;
    
    /// Get the standard user config directory for Skylet plugins
    /// Returns `~/.config/skylet/plugins/`
    pub fn get_user_config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("skylet")
            .join("plugins")
    }
    
    /// Get the standard local/project config directory
    /// Returns `data/` relative to current directory
    pub fn get_local_config_dir() -> PathBuf {
        PathBuf::from("data")
    }
    
    /// Get the system-wide config directory
    /// Returns `/etc/skylet/plugins/` on Unix systems
    pub fn get_system_config_dir() -> PathBuf {
        PathBuf::from("/etc/skylet/plugins")
    }
    
    /// Get the standard config path for a plugin (user location)
    /// This path may not exist yet - use find_config() to find an existing config
    pub fn get_standard_config_path(plugin_name: &str) -> PathBuf {
        get_user_config_dir().join(format!("{}.toml", plugin_name))
    }
    
    /// Get all possible config paths for a plugin in search order
    /// Returns paths in priority order: local -> user -> system
    pub fn get_config_search_paths(plugin_name: &str) -> Vec<PathBuf> {
        vec![
            // 1. Local project config (highest priority)
            get_local_config_dir().join(format!("{}.toml", plugin_name)),
            // 2. User config directory
            get_user_config_dir().join(format!("{}.toml", plugin_name)),
            // 3. System-wide config (lowest priority)
            get_system_config_dir().join(format!("{}.toml", plugin_name)),
        ]
    }
    
    /// Find an existing config file for a plugin
    /// Searches in order: local -> user -> system
    /// Returns the first existing path, or None if no config exists
    pub fn find_config(plugin_name: &str) -> Option<PathBuf> {
        get_config_search_paths(plugin_name)
            .into_iter()
            .find(|p| p.exists())
    }
    
    /// Find config file with legacy fallback paths
    /// Use this for migrating plugins that have configs in old locations
    pub fn find_config_with_legacy(plugin_name: &str, legacy_paths: &[PathBuf]) -> Option<PathBuf> {
        // First check standard locations
        if let Some(path) = find_config(plugin_name) {
            return Some(path);
        }
        
        // Then check legacy paths
        legacy_paths.iter().find(|p| p.exists()).cloned()
    }
    
    /// Load config from the first available location
    pub fn load_config<T: for<'de> Deserialize<'de>>(plugin_name: &str) -> PluginResult<Option<T>> {
        if let Some(path) = find_config(plugin_name) {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| PluginCommonError::SerializationFailed(
                    format!("Failed to read config {:?}: {}", path, e)
                ))?;
            let config: T = toml::from_str(&content)
                .map_err(|e| PluginCommonError::SerializationFailed(
                    format!("Failed to parse config {:?}: {}", path, e)
                ))?;
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }
    
    /// Load config from a specific path
    pub fn load_config_from_path<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> PluginResult<T> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| PluginCommonError::SerializationFailed(
                format!("Failed to read config {:?}: {}", path, e)
            ))?;
        toml::from_str(&content)
            .map_err(|e| PluginCommonError::SerializationFailed(
                format!("Failed to parse config {:?}: {}", path, e)
            ))
    }
    
    /// Save config to the standard user location
    pub fn save_config<T: Serialize>(plugin_name: &str, config: &T) -> PluginResult<PathBuf> {
        let path = get_standard_config_path(plugin_name);
        
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| PluginCommonError::SerializationFailed(
                    format!("Failed to create config directory: {}", e)
                ))?;
        }
        
        let content = toml::to_string_pretty(config)
            .map_err(|e| PluginCommonError::SerializationFailed(
                format!("Failed to serialize config: {}", e)
            ))?;
        
        std::fs::write(&path, content)
            .map_err(|e| PluginCommonError::SerializationFailed(
                format!("Failed to write config: {}", e)
            ))?;
        
        Ok(path)
    }
    
    /// Get the standard secrets directory for a plugin
    /// Returns `~/.config/skylet/secrets/{plugin_name}/`
    pub fn get_secrets_dir(plugin_name: &str) -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("skylet")
            .join("secrets")
            .join(plugin_name)
    }
    
    /// Get the standard data directory for a plugin
    /// Returns `data/{plugin_name}/`
    pub fn get_data_dir(plugin_name: &str) -> PathBuf {
        PathBuf::from("data").join(plugin_name)
    }
    
    /// Get environment variable with standard prefix
    /// Maps `SKYLET_{PLUGIN_NAME}_{KEY}` to value
    pub fn get_env_var(plugin_name: &str, key: &str) -> Option<String> {
        let env_key = format!("SKYLET_{}_{}", plugin_name.to_uppercase().replace("-", "_"), key);
        std::env::var(&env_key).ok()
    }
    
    /// Get plugin-specific environment variable
    /// Example: get_plugin_env("telegram-bot", "token") -> $SKYLET_TELEGRAM_BOT_TOKEN
    pub fn get_plugin_env(plugin_name: &str, var: &str) -> Option<String> {
        get_env_var(plugin_name, var)
    }
}

// Common error types for all plugins
#[derive(Error, Debug)]
pub enum PluginCommonError {
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),

    #[error("Missing request body")]
    MissingBody,

    #[error("Serialization failed: {0}")]
    SerializationFailed(String),

    #[error("URL generation failed: {0}")]
    UrlGenerationFailed(String),

    #[error("HTTP request failed: {0}")]
    HttpRequestFailed(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,
}

// Implement conversion from serde_json::Error for easier error handling
impl From<serde_json::Error> for PluginCommonError {
    fn from(err: serde_json::Error) -> Self {
        PluginCommonError::SerializationFailed(format!("JSON error: {}", err))
    }
}

pub type PluginResult<T> = std::result::Result<T, PluginCommonError>;

// Common response structure for all plugins
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub rate_limit: Option<RateLimitInfo>,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitInfo {
    pub remaining: u32,
    pub limit: u32,
    pub reset: u64,
    pub used: u32,
    pub reset_after: Option<u64>,
}

// Common plugin info structure
pub struct PluginInfoHolder {
    pub info: PluginInfo,
    pub _name: CString,
    pub _version: CString,
    pub _abi: CString,
    pub _description: CString,
}

// Generic plugin context for state management
pub struct PluginContext {
    pub plugin_name: String,
    pub request_id: String,
    pub rate_limiter: Arc<RwLock<RateLimiter>>,
}

// Thread-safe rate limiter
pub struct RateLimiter {
    pub requests_per_minute: u32,
    pub requests: std::collections::VecDeque<u64>,
    pub last_window_start: u64,
    pub current_count: u32,
}

impl RateLimiter {
    pub fn new(requests_per_minute: u32) -> Self {
        Self {
            requests_per_minute,
            requests: std::collections::VecDeque::with_capacity(requests_per_minute as usize),
            last_window_start: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            current_count: 0,
        }
    }

    pub fn check_rate_limit(&mut self) -> Result<(), PluginCommonError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let window_start = now.saturating_sub(60);
        while let Some(&request_time) = self.requests.front() {
            if request_time < window_start {
                self.requests.pop_front();
            } else {
                break;
            }
        }

        if self.current_count >= self.requests_per_minute {
            Err(PluginCommonError::RateLimitExceeded)
        } else {
            self.requests.push_back(now);
            self.current_count += 1;
            Ok(())
        }
    }

    pub fn get_status(&self) -> RateLimitInfo {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        RateLimitInfo {
            remaining: self.requests_per_minute.saturating_sub(self.current_count),
            limit: self.requests_per_minute,
            reset: now + 60,
            used: self.current_count,
            reset_after: Some(now + 60),
        }
    }
}

// Common HTTP client with proper headers and error handling
pub fn create_http_client(_user_agent: &str) -> Result<ureq::Agent, PluginCommonError> {
    let agent = ureq::AgentBuilder::new().try_proxy_from_env(true).build();
    Ok(agent)
}

// URL building utilities
pub fn build_api_url(
    base_url: &str,
    endpoint: &str,
    params: &[(&str, &str)],
) -> Result<String, PluginCommonError> {
    let mut url = format!("{}{}", base_url, endpoint);

    if !params.is_empty() {
        url.push('?');
        let param_strings: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect();
        url.push_str(&param_strings.join("&"));
    }

    Ok(url)
}

// Common response creation helpers
pub fn create_success_response<T: Serialize>(
    body: &str,
    _rate_limit: Option<RateLimitInfo>,
) -> *mut HttpResponse {
    let body_bytes = body.as_bytes();
    
    // Security: Validate body size to prevent DoS attacks
    const MAX_RESPONSE_SIZE: usize = 100 * 1024 * 1024; // 100MB limit
    if body_bytes.len() > MAX_RESPONSE_SIZE {
        eprintln!("Security: Response body exceeds maximum size limit");
        return Box::into_raw(Box::new(HttpResponse {
            status_code: 413,
            headers: std::ptr::null_mut(),
            num_headers: 0,
            body: std::ptr::null_mut(),
            body_len: 0,
        }));
    }
    
    // Security: Use Vec instead of raw alloc for automatic bounds checking
    let mut body_vec = body_bytes.to_vec();
    let body_ptr = body_vec.as_mut_ptr();
    std::mem::forget(body_vec); // Leak the vec so the raw pointer remains valid
    
    let resp = Box::new(HttpResponse {
        status_code: 200,
        headers: std::ptr::null_mut(),
        num_headers: 0,
        body: body_ptr,
        body_len: body_bytes.len(),
    });
    Box::into_raw(resp)
}

pub fn create_error_response(status_code: i32, error_message: &str) -> *mut HttpResponse {
    let error_data = serde_json::json!({
        "success": false,
        "error": error_message
    });
    let response_body = match serde_json::to_string(&error_data) {
        Ok(body) => body,
        Err(_) => {
            eprintln!("Security: Failed to serialize error response");
            return Box::into_raw(Box::new(HttpResponse {
                status_code: 500,
                headers: std::ptr::null_mut(),
                num_headers: 0,
                body: std::ptr::null_mut(),
                body_len: 0,
            }));
        }
    };

    let body_bytes = response_body.as_bytes();
    
    // Security: Validate body size to prevent DoS attacks
    const MAX_RESPONSE_SIZE: usize = 100 * 1024 * 1024; // 100MB limit
    if body_bytes.len() > MAX_RESPONSE_SIZE {
        eprintln!("Security: Error response body exceeds maximum size limit");
        return Box::into_raw(Box::new(HttpResponse {
            status_code: 413,
            headers: std::ptr::null_mut(),
            num_headers: 0,
            body: std::ptr::null_mut(),
            body_len: 0,
        }));
    }

    // Security: Use Vec instead of raw alloc for automatic bounds checking
    let mut body_vec = body_bytes.to_vec();
    let body_ptr = body_vec.as_mut_ptr();
    std::mem::forget(body_vec); // Leak the vec so the raw pointer remains valid

    let resp = Box::new(HttpResponse {
        status_code,
        headers: std::ptr::null_mut(),
        num_headers: 0,
        body: body_ptr,
        body_len: body_bytes.len(),
    });
    Box::into_raw(resp)
}

// Request parsing utilities
pub fn parse_json_request<T: for<'de> Deserialize<'de>>(
    request: *const HttpResponse,
) -> PluginResult<T> {
    unsafe {
        if request.is_null() || (*request).body_len == 0 {
            return Err(PluginCommonError::MissingBody);
        }

        let slice = std::slice::from_raw_parts((*request).body, (*request).body_len);
        serde_json::from_slice::<T>(slice)
            .map_err(|e| PluginCommonError::InvalidJson(e.to_string()))
    }
}

// Method and path extraction (simplified for v0.3.0)
pub fn extract_method_and_path(_request: *const HttpResponse) -> PluginResult<(String, String)> {
    Ok(("POST".to_string(), "/api".to_string()))
}

// Plugin info creation (simplified for v0.3.0)
pub fn create_plugin_info(
    name: &str,
    version: &str,
    description: &str,
    _provides_services: Vec<&str>,
    _capabilities: Vec<&str>,
    max_concurrency: u32,
) -> PluginInfoHolder {
    let name_cstring = CString::new(name).unwrap();
    let version_cstring = CString::new(version).unwrap();
    let abi_cstring = CString::new("2.0").unwrap();
    let description_cstring = CString::new(description).unwrap();

    let info = PluginInfo {
        name: name_cstring.as_ptr(),
        version: version_cstring.as_ptr(),
        description: description_cstring.as_ptr(),
        author: std::ptr::null(),
        license: std::ptr::null(),
        homepage: std::ptr::null(),
        skylet_version_min: std::ptr::null(),
        skylet_version_max: std::ptr::null(),
        abi_version: abi_cstring.as_ptr(),
        dependencies: std::ptr::null(),
        num_dependencies: 0,
        provides_services: std::ptr::null(),
        num_provides_services: 0,
        requires_services: std::ptr::null(),
        num_requires_services: 0,
        supported_operations: std::ptr::null(),
        num_supported_operations: 0,
        capabilities: std::ptr::null(),
        num_capabilities: 0,
        resource_requirements: std::ptr::null(),
        max_concurrency: max_concurrency as usize,
        supports_hot_reload: false,
        supports_async: true,
        supports_streaming: false,
        plugin_type: PluginType::Integration,
        tags: std::ptr::null(),
        num_tags: 0,
        build_timestamp: std::ptr::null(),
        build_hash: std::ptr::null(),
        git_commit: std::ptr::null(),
        build_environment: std::ptr::null(),
        metadata: std::ptr::null(),
    };

    PluginInfoHolder {
        info,
        _name: name_cstring,
        _version: version_cstring,
        _abi: abi_cstring,
        _description: description_cstring,
    }
}

// Logging utility (simplified for v0.3.0)
pub fn log_message(_context: *const PluginContext, _level: PluginLogLevel, message: &str) {
    println!("PLUGIN_LOG: {}", message);
}

// Request ID generation
pub fn generate_request_id() -> String {
    Uuid::new_v4().to_string()
}

// Common test utilities
#[cfg(test)]
pub mod test_utils {
    use super::*;

    pub fn create_test_request(_method: &str, _path: &str, body: &str) -> HttpResponse {
        let body_ptr = body.as_bytes().as_ptr() as *mut u8;

        HttpResponse {
            status_code: 200,
            headers: std::ptr::null_mut(),
            num_headers: 0,
            body: body_ptr,
            body_len: body.len(),
        }
    }

    pub fn assert_url_contains(expected: &str, actual: &str) {
        assert!(
            actual.contains(expected),
            "URL should contain '{}'",
            expected
        );
    }
}

// Common API client traits
pub trait ApiClient {
    type ResponseType;

    fn make_request(&self, url: &str) -> Result<Self::ResponseType, PluginCommonError>;
    fn get_base_url(&self) -> &'static str;
}

pub trait RateLimitedClient: ApiClient {
    fn check_rate_limit(&self, limiter: &mut RateLimiter) -> Result<(), PluginCommonError> {
        limiter.check_rate_limit()
    }
}

// ===== ENHANCED FEATURES FOR v0.3.0 =====

// Enhanced API client with authentication support
pub mod api_client;

// Database abstraction layer
pub mod database;

// Query builder for database operations
pub mod query_builder;

// LLM provider abstraction
pub mod llm_provider;

// Plugin template and scaffolding system
pub mod template_system;

// Workflow execution engine
pub mod workflow_engine;

// Tool calling framework
pub mod tool_calling;

// API registry and version management system
pub mod api_registry;

// Messaging platform abstraction
pub mod messaging_platform;

// Schema migration system for database plugins
// pub mod schema_migration;  // Temporarily disabled - DatabaseConnection trait not dyn compatible (missing async methods)

// Kubernetes client for DevOps plugins
pub mod kubernetes_client;

// Container registry client abstraction
pub mod container_registry_client;

// Git operations client for plugin automation
// Requires architectural refactoring - see git_client/mod.rs for details
// The adapters.rs implementations have incompatible API designs (remote vs local)
// that need reconciliation before the module can be enabled.
// pub mod git_client;

// Authentication plugin framework
pub mod auth;

// Database connection utilities
pub struct DatabaseConfig {
    pub connection_string: String,
    pub max_connections: u32,
    pub timeout_seconds: u64,
}

impl DatabaseConfig {
    pub fn new(connection_string: &str) -> Self {
        Self {
            connection_string: connection_string.to_string(),
            max_connections: 10,
            timeout_seconds: 30,
        }
    }
}

// Configuration management utilities
pub struct ConfigManager {
    configs: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl ConfigManager {
    pub fn new() -> Self {
        Self {
            configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set(&self, key: &str, value: serde_json::Value) {
        let mut configs = self.configs.write().await;
        configs.insert(key.to_string(), value);
    }

    pub async fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> PluginResult<T> {
        let configs = self.configs.read().await;
        let value = configs
            .get(key)
            .ok_or_else(|| PluginCommonError::SerializationFailed(format!("Config key '{}' not found", key)))?
            .clone();
        
        serde_json::from_value(value)
            .map_err(|e| PluginCommonError::SerializationFailed(format!("Failed to deserialize config: {}", e)))
    }
}

// State management abstraction
pub struct PluginState<T> {
    inner: Arc<RwLock<T>>,
}

impl<T> PluginState<T> {
    pub fn new(initial: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(initial)),
        }
    }

    pub async fn read<F, R>(&self, f: F) -> R 
    where 
        F: FnOnce(&T) -> R,
    {
        let guard = self.inner.read().await;
        f(&*guard)
    }

    pub async fn write<F, R>(&self, f: F) -> R 
    where 
        F: FnOnce(&mut T) -> R,
    {
        let mut guard = self.inner.write().await;
        f(&mut *guard)
    }

    pub fn clone_handle(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

// ===== SECRETS MANAGEMENT =====

pub struct SecretsManager {
    /// In-memory secrets storage using std::sync::RwLock for sync access
    secrets: Arc<std::sync::RwLock<HashMap<String, String>>>,
    /// Pending secrets to be stored asynchronously (key, value, callback)
    pending_stores: Arc<Mutex<Vec<(String, String, Option<Arc<dyn Fn(&str, &str) -> Result<()> + Send + Sync>>)>>>,
}

impl SecretsManager {
    pub fn new() -> Self {
        Self {
            secrets: Arc::new(std::sync::RwLock::new(HashMap::new())),
            pending_stores: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Store a secret synchronously (stored in memory, queued for async persistence)
    pub fn store_secret_sync(&self, key: &str, value: &str) -> Result<()> {
        let mut secrets = self.secrets.write().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
        secrets.insert(key.to_string(), value.to_string());
        Ok(())
    }

    /// Get a secret synchronously from in-memory cache
    pub fn get_secret_sync(&self, key: &str) -> Option<String> {
        let secrets = self.secrets.read().ok()?;
        secrets.get(key).cloned()
    }

    /// Queue a secret for async storage with an optional callback
    pub fn queue_secret_store(&self, key: &str, value: &str, callback: Option<Arc<dyn Fn(&str, &str) -> Result<()> + Send + Sync>>) {
        let mut pending = self.pending_stores.lock().unwrap();
        pending.push((key.to_string(), value.to_string(), callback));
        // Also store in memory immediately
        if let Ok(mut secrets) = self.secrets.write() {
            secrets.insert(key.to_string(), value.to_string());
        }
    }

    /// Process pending secret stores asynchronously
    pub async fn flush_pending_stores(&self) -> Result<usize> {
        let pending: Vec<_> = {
            let mut pending_guard = self.pending_stores.lock().unwrap();
            std::mem::take(&mut *pending_guard)
        };

        let mut success_count = 0;
        for (key, value, callback) in pending {
            // Store via sync method (already in memory)
            if self.store_secret_sync(&key, &value).is_ok() {
                // Call callback if provided
                if let Some(cb) = callback {
                    if cb(&key, &value).is_ok() {
                        success_count += 1;
                    }
                } else {
                    success_count += 1;
                }
            }
        }
        Ok(success_count)
    }

    pub async fn store_secret(&self, key: &str, value: &str) -> PluginResult<()> {
        let mut secrets = self.secrets.write()
            .map_err(|e| PluginCommonError::SerializationFailed(format!("Lock poisoned: {}", e)))?;
        secrets.insert(key.to_string(), value.to_string());
        Ok(())
    }

    pub async fn get_secret(&self, key: &str) -> PluginResult<Option<String>> {
        let secrets = self.secrets.read()
            .map_err(|e| PluginCommonError::SerializationFailed(format!("Lock poisoned: {}", e)))?;
        Ok(secrets.get(key).cloned())
    }

    pub async fn delete_secret(&self, key: &str) -> PluginResult<bool> {
        let mut secrets = self.secrets.write()
            .map_err(|e| PluginCommonError::SerializationFailed(format!("Lock poisoned: {}", e)))?;
        Ok(secrets.remove(key).is_some())
    }

    pub async fn list_secrets(&self, prefix: Option<&str>) -> PluginResult<Vec<String>> {
        let secrets = self.secrets.read()
            .map_err(|e| PluginCommonError::SerializationFailed(format!("Lock poisoned: {}", e)))?;
        let mut keys: Vec<String> = secrets.keys().cloned().collect();
        
        if let Some(prefix) = prefix {
            keys.retain(|k| k.starts_with(prefix));
        }
        
        keys.sort();
        Ok(keys)
    }
}

impl Default for SecretsManager {
    fn default() -> Self {
        Self::new()
    }
}

// Utility functions for secrets
pub fn generate_secure_password(length: usize) -> String {
    use rand::Rng;
    
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";
    let mut rng = rand::thread_rng();
    
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

pub fn hash_secret(secret: &str, salt: &str) -> PluginResult<String> {
    use sha2::{Sha256, Digest};
    
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(salt.as_bytes());
    
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn get_env_secret(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

pub async fn load_config_with_secrets<T: for<'de> Deserialize<'de>>(
    config_path: &str,
    secrets: &SecretsManager,
) -> PluginResult<T> {
    let config_content = std::fs::read_to_string(config_path)
        .map_err(|e| PluginCommonError::SerializationFailed(format!("Failed to read config: {}", e)))?;
    
    let mut processed_content = config_content;
    
    if let Ok(all_keys) = secrets.list_secrets(None).await {
        for secret_key in all_keys {
            if let Ok(Some(secret_value)) = secrets.get_secret(&secret_key).await {
                let placeholder = format!("${{secret:{}}}", secret_key);
                processed_content = processed_content.replace(&placeholder, &secret_value);
            }
        }
    }
    
    serde_json::from_str(&processed_content)
        .map_err(|e| PluginCommonError::SerializationFailed(format!("Failed to parse config: {}", e)))
}

// Simple JSON request handler utility
pub fn handle_json_request<T: Serialize>(
    handler: impl Fn(serde_json::Value) -> PluginResult<T>,
    args_json: *const c_char,
) -> *mut c_char {
    unsafe {
        if args_json.is_null() {
            let error_response = serde_json::json!({
                "success": false,
                "error": "Missing arguments"
            });
            return CString::new(error_response.to_string()).unwrap().into_raw();
        }

        let args_str = CStr::from_ptr(args_json).to_string_lossy().into_owned();
        
        match serde_json::from_str::<serde_json::Value>(&args_str) {
            Ok(args) => {
                match handler(args) {
                    Ok(result) => {
                        let success_response = serde_json::json!({
                            "success": true,
                            "data": result
                        });
                        CString::new(success_response.to_string()).unwrap().into_raw()
                    }
                    Err(e) => {
                        let error_response = serde_json::json!({
                            "success": false,
                            "error": e.to_string()
                        });
                        CString::new(error_response.to_string()).unwrap().into_raw()
                    }
                }
            }
            Err(e) => {
                let error_response = serde_json::json!({
                    "success": false,
                    "error": format!("Invalid JSON arguments: {}", e)
                });
                CString::new(error_response.to_string()).unwrap().into_raw()
            }
        }
    }
}

/// Master plugin declaration macro - eliminates 100+ lines of boilerplate
#[macro_export]
macro_rules! skylet_plugin {
    (
        name: $name:expr,
        version: $version:expr,
        description: $description:expr,
        plugin_type: $plugin_type:expr,
        max_concurrency: $max_concurrency:expr,
    ) => {
        static mut PLUGIN_INFO: Option<$crate::PluginInfoHolder> = None;

        #[no_mangle]
        pub extern "C" fn plugin_get_info() -> *const skylet_abi::PluginInfo {
            unsafe {
                if PLUGIN_INFO.is_none() {
                    let info = $crate::create_plugin_info(
                        $name,
                        $version,
                        $description,
                        vec![],
                        vec!["json", "async"],
                        $max_concurrency,
                    );
                    PLUGIN_INFO = Some(info);
                }
                &PLUGIN_INFO.as_ref().unwrap().info
            }
        }

        #[no_mangle]
        pub extern "C" fn plugin_init(context: *const skylet_abi::PluginContext) -> skylet_abi::PluginResult {
            unsafe {
                if context.is_null() {
                    return skylet_abi::PluginResult::InvalidRequest;
                }
                
                skylet_abi::PluginResult::Success
            }
        }

        #[no_mangle]
        pub extern "C" fn plugin_shutdown(_context: *const skylet_abi::PluginContext) -> skylet_abi::PluginResult {
            skylet_abi::PluginResult::Success
        }
    };
}
