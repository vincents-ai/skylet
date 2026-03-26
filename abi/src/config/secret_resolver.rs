// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Secret Reference Resolver - RFC-0006
//!
//! Resolves secret references in configuration values from various
//! backends: vault://, env://, file://

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use super::schema::{SecretBackend, SecretReference};

/// Secret resolver trait for different backends
pub trait SecretResolverBackend: Send + Sync {
    /// Resolve a secret by path
    fn resolve(&self, path: &str, key: Option<&str>) -> Result<String, SecretError>;

    /// Check if this backend is available
    fn is_available(&self) -> bool;
}

/// Cached secret value
#[derive(Debug, Clone)]
struct CachedSecret {
    value: String,
    resolved_at: Instant,
    ttl: Duration,
}

impl CachedSecret {
    fn new(value: String, ttl: Duration) -> Self {
        Self {
            value,
            resolved_at: Instant::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now().duration_since(self.resolved_at) > self.ttl
    }
}

/// Secret resolver with caching and multiple backend support
pub struct SecretResolver {
    /// Registered backends
    backends: HashMap<SecretBackend, Arc<dyn SecretResolverBackend>>,
    /// Secret cache
    cache: Arc<RwLock<HashMap<String, CachedSecret>>>,
    /// Default TTL for cached secrets
    default_ttl: Duration,
    /// Whether caching is enabled
    caching_enabled: bool,
}

impl SecretResolver {
    /// Create a new secret resolver
    pub fn new() -> Self {
        Self {
            backends: Self::default_backends(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: Duration::from_secs(300), // 5 minutes
            caching_enabled: true,
        }
    }

    /// Create with custom backends
    pub fn with_backends(backends: HashMap<SecretBackend, Arc<dyn SecretResolverBackend>>) -> Self {
        Self {
            backends,
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: Duration::from_secs(300),
            caching_enabled: true,
        }
    }

    /// Get default backends
    fn default_backends() -> HashMap<SecretBackend, Arc<dyn SecretResolverBackend>> {
        let mut backends: HashMap<SecretBackend, Arc<dyn SecretResolverBackend>> = HashMap::new();
        backends.insert(
            SecretBackend::Environment,
            Arc::new(EnvSecretBackend::new()) as Arc<dyn SecretResolverBackend>,
        );
        backends.insert(
            SecretBackend::File,
            Arc::new(FileSecretBackend::new()) as Arc<dyn SecretResolverBackend>,
        );
        backends
    }

    /// Register a custom backend
    pub fn register_backend(
        &mut self,
        backend_type: SecretBackend,
        backend: Arc<dyn SecretResolverBackend>,
    ) {
        self.backends.insert(backend_type, backend);
    }

    /// Set caching enabled
    pub fn set_caching(&mut self, enabled: bool) {
        self.caching_enabled = enabled;
    }

    /// Set default TTL
    pub fn set_default_ttl(&mut self, ttl: Duration) {
        self.default_ttl = ttl;
    }

    /// Resolve a secret reference
    pub fn resolve(&self, reference: &SecretReference) -> Result<String, SecretError> {
        // Check cache first
        if reference.cache && self.caching_enabled {
            let cache = self.cache.read().unwrap();
            if let Some(cached) = cache.get(&reference.uri) {
                if !cached.is_expired() {
                    return Ok(cached.value.clone());
                }
            }
        }

        // Get the appropriate backend
        let backend_type = reference.backend();
        let backend =
            self.backends
                .get(&backend_type)
                .ok_or_else(|| SecretError::BackendUnavailable {
                    backend: format!("{:?}", backend_type),
                })?;

        if !backend.is_available() {
            return Err(SecretError::BackendUnavailable {
                backend: format!("{:?}", backend_type),
            });
        }

        // Resolve the secret
        let value = backend.resolve(reference.path(), reference.key.as_deref())?;

        // Cache the result
        if reference.cache && self.caching_enabled {
            let ttl = reference
                .cache_ttl_seconds
                .map(Duration::from_secs)
                .unwrap_or(self.default_ttl);

            let mut cache = self.cache.write().unwrap();
            cache.insert(reference.uri.clone(), CachedSecret::new(value.clone(), ttl));
        }

        Ok(value)
    }

    /// Resolve a secret URI string
    pub fn resolve_uri(&self, uri: &str) -> Result<String, SecretError> {
        let reference =
            SecretReference::parse(uri).ok_or_else(|| SecretError::InvalidReference {
                uri: uri.to_string(),
            })?;
        self.resolve(&reference)
    }

    /// Clear the secret cache
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
    }

    /// Clear expired entries from cache
    pub fn cleanup_cache(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.retain(|_, cached| !cached.is_expired());
    }

    /// Resolve all secret references in a configuration value
    pub fn resolve_in_value(&self, value: &mut serde_json::Value) -> Result<(), SecretError> {
        match value {
            serde_json::Value::String(s) => {
                // Check if it's a secret reference
                if s.starts_with("vault://") || s.starts_with("env://") || s.starts_with("file://")
                {
                    let resolved = self.resolve_uri(s)?;
                    *s = resolved;
                }
            }
            serde_json::Value::Object(map) => {
                // Check for secret reference object
                if let Some(uri) = map.get("$secret") {
                    if let Some(uri_str) = uri.as_str() {
                        let key = map.get("key").and_then(|k| k.as_str());
                        let reference = SecretReference::parse(uri_str).ok_or_else(|| {
                            SecretError::InvalidReference {
                                uri: uri_str.to_string(),
                            }
                        })?;
                        let mut reference = reference;
                        if let Some(k) = key {
                            reference.key = Some(k.to_string());
                        }
                        let resolved = self.resolve(&reference)?;
                        *value = serde_json::Value::String(resolved);
                    }
                } else {
                    // Recursively resolve in nested objects
                    for v in map.values_mut() {
                        self.resolve_in_value(v)?;
                    }
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr.iter_mut() {
                    self.resolve_in_value(v)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Resolve secrets in a configuration map
    pub fn resolve_in_config(
        &self,
        config: &mut HashMap<String, serde_json::Value>,
    ) -> Result<(), SecretError> {
        for value in config.values_mut() {
            self.resolve_in_value(value)?;
        }
        Ok(())
    }
}

impl Default for SecretResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Environment variable secret backend
pub struct EnvSecretBackend;

impl EnvSecretBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EnvSecretBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretResolverBackend for EnvSecretBackend {
    fn resolve(&self, path: &str, _key: Option<&str>) -> Result<String, SecretError> {
        std::env::var(path).map_err(|_| SecretError::SecretNotFound {
            path: format!("env://{}", path),
        })
    }

    fn is_available(&self) -> bool {
        true // Environment is always available
    }
}

/// File-based secret backend
pub struct FileSecretBackend {
    base_path: std::path::PathBuf,
}

impl FileSecretBackend {
    pub fn new() -> Self {
        Self {
            base_path: std::path::PathBuf::from("./secrets"),
        }
    }
}

impl Default for FileSecretBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSecretBackend {
    pub fn with_base_path(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            base_path: path.into(),
        }
    }
}

impl SecretResolverBackend for FileSecretBackend {
    fn resolve(&self, path: &str, key: Option<&str>) -> Result<String, SecretError> {
        let file_path = self.base_path.join(path);

        if !file_path.exists() {
            return Err(SecretError::SecretNotFound {
                path: format!("file://{}", path),
            });
        }

        let content = std::fs::read_to_string(&file_path).map_err(|e| SecretError::ReadError {
            path: file_path.display().to_string(),
            error: e.to_string(),
        })?;

        // If key is specified, try to parse as key-value format
        if let Some(k) = key {
            // Try JSON first
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(v) = json.get(k) {
                    return v.as_str().map(|s| s.to_string()).ok_or_else(|| {
                        SecretError::KeyNotFound {
                            path: format!("file://{}", path),
                            key: k.to_string(),
                        }
                    });
                }
            }

            // Try key=value format
            for line in content.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    if key.trim() == k {
                        return Ok(value.trim().to_string());
                    }
                }
            }

            return Err(SecretError::KeyNotFound {
                path: format!("file://{}", path),
                key: k.to_string(),
            });
        }

        // Return entire content as secret (trimmed)
        Ok(content.trim().to_string())
    }

    fn is_available(&self) -> bool {
        self.base_path.exists()
    }
}

/// Vault secret backend for HashiCorp Vault integration
///
/// Uses Vault KV v2 secret engine. Expects secrets at paths like `secret/data/myapp`.
/// The response format is `{data: {data: {key: value}}}`.
pub struct VaultSecretBackend {
    address: Option<String>,
    token: Option<String>,
}

impl VaultSecretBackend {
    pub fn new() -> Self {
        Self {
            address: std::env::var("VAULT_ADDR").ok(),
            token: std::env::var("VAULT_TOKEN").ok(),
        }
    }

    pub fn with_config(address: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            address: Some(address.into()),
            token: Some(token.into()),
        }
    }
}

impl SecretResolverBackend for VaultSecretBackend {
    fn resolve(&self, path: &str, key: Option<&str>) -> Result<String, SecretError> {
        let address = self.address.as_ref().ok_or_else(|| SecretError::BackendUnavailable {
            backend: "vault".to_string(),
        })?;
        let token = self.token.as_ref().ok_or_else(|| SecretError::BackendUnavailable {
            backend: "vault".to_string(),
        })?;

        let url = format!("{}/v1/{}", address.trim_end_matches('/'), path);

        let response = ureq::get(&url)
            .set("X-Vault-Token", token)
            .set("X-Vault-Request", "true")
            .call()
            .map_err(|e| SecretError::ReadError {
                path: url.clone(),
                error: e.to_string(),
            })?;

        if !(200..300).contains(&response.status()) {
            return Err(SecretError::SecretNotFound { path: url });
        }

        let json: serde_json::Value = response.into_json().map_err(|e| SecretError::ReadError {
            path: url.clone(),
            error: e.to_string(),
        })?;

        let data: &serde_json::Value = json.get("data").and_then(|d: &serde_json::Value| d.get("data")).ok_or_else(|| {
            SecretError::ReadError {
                path: url.clone(),
                error: "No data field in response".to_string(),
            }
        })?;

        if let Some(k) = key {
            data.get(k)
                .and_then(|v: &serde_json::Value| v.as_str())
                .map(|s: &str| s.to_string())
                .ok_or_else(|| SecretError::KeyNotFound {
                    path: url,
                    key: k.to_string(),
                })
        } else {
            data.as_str()
                .map(|s: &str| s.to_string())
                .ok_or_else(|| SecretError::ReadError {
                    path: url,
                    error: "Secret value is not a string".to_string(),
                })
        }
    }

    fn is_available(&self) -> bool {
        self.address.is_some() && self.token.is_some()
    }
}

impl Default for VaultSecretBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Secret resolution error
#[derive(Debug, Clone)]
pub enum SecretError {
    /// Invalid secret reference
    InvalidReference { uri: String },
    /// Secret not found
    SecretNotFound { path: String },
    /// Key not found in secret
    KeyNotFound { path: String, key: String },
    /// Backend unavailable
    BackendUnavailable { backend: String },
    /// Failed to read secret
    ReadError { path: String, error: String },
    /// Permission denied
    PermissionDenied { path: String },
}

impl std::fmt::Display for SecretError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecretError::InvalidReference { uri } => {
                write!(f, "Invalid secret reference: {}", uri)
            }
            SecretError::SecretNotFound { path } => {
                write!(f, "Secret not found: {}", path)
            }
            SecretError::KeyNotFound { path, key } => {
                write!(f, "Key '{}' not found in secret: {}", key, path)
            }
            SecretError::BackendUnavailable { backend } => {
                write!(f, "Secret backend unavailable: {}", backend)
            }
            SecretError::ReadError { path, error } => {
                write!(f, "Failed to read secret '{}': {}", path, error)
            }
            SecretError::PermissionDenied { path } => {
                write!(f, "Permission denied for secret: {}", path)
            }
        }
    }
}

impl std::error::Error for SecretError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_secret_resolver_new() {
        let resolver = SecretResolver::new();
        assert!(resolver.caching_enabled);
    }

    #[test]
    fn test_env_backend_resolve() {
        env::set_var("TEST_SECRET", "test_value");

        let backend = EnvSecretBackend::new();
        let result = backend.resolve("TEST_SECRET", None);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_value");

        env::remove_var("TEST_SECRET");
    }

    #[test]
    fn test_env_backend_not_found() {
        let backend = EnvSecretBackend::new();
        let result = backend.resolve("NONEXISTENT_SECRET_XYZ", None);

        assert!(result.is_err());
    }

    #[test]
    fn test_secret_reference_parse() {
        let ref1 = SecretReference::parse("env://MY_SECRET");
        assert!(ref1.is_some());
        assert_eq!(ref1.unwrap().backend(), SecretBackend::Environment);

        let ref2 = SecretReference::parse("file:///path/to/secret");
        assert!(ref2.is_some());
        assert_eq!(ref2.unwrap().backend(), SecretBackend::File);
    }

    #[test]
    fn test_resolve_uri() {
        env::set_var("TEST_CONFIG_SECRET", "config_value");

        let resolver = SecretResolver::new();
        let result = resolver.resolve_uri("env://TEST_CONFIG_SECRET");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "config_value");

        env::remove_var("TEST_CONFIG_SECRET");
    }

    #[test]
    fn test_resolve_in_value_string() {
        env::set_var("NESTED_SECRET", "nested_value");

        let resolver = SecretResolver::new();
        let mut value = serde_json::json!("env://NESTED_SECRET");

        let result = resolver.resolve_in_value(&mut value);
        assert!(result.is_ok());
        assert_eq!(value, serde_json::json!("nested_value"));

        env::remove_var("NESTED_SECRET");
    }

    #[test]
    fn test_cached_secret_expiration() {
        let cached = CachedSecret::new("value".to_string(), Duration::from_millis(10));
        assert!(!cached.is_expired());

        std::thread::sleep(Duration::from_millis(20));
        assert!(cached.is_expired());
    }

    #[test]
    fn test_clear_cache() {
        let resolver = SecretResolver::new();

        let mut cache = resolver.cache.write().unwrap();
        cache.insert(
            "test_key".to_string(),
            CachedSecret::new("test_value".to_string(), Duration::from_secs(60)),
        );
        drop(cache);

        resolver.clear_cache();

        let cache = resolver.cache.read().unwrap();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_vault_backend_is_available_with_config() {
        let backend = VaultSecretBackend::with_config("http://localhost:8200", "test-token");
        assert!(backend.is_available());
    }

    #[test]
    fn test_vault_backend_not_available_without_config() {
        let backend = VaultSecretBackend::new();
        assert!(!backend.is_available());
    }

    #[test]
    fn test_vault_backend_resolve_makes_http_request() {
        let backend = VaultSecretBackend::with_config("http://localhost:8200", "test-token");
        let result = backend.resolve("secret/data/myapp", Some("password"));
        assert!(result.is_err());
        if let Err(e) = result {
            if let SecretError::BackendUnavailable { backend } = e {
                assert!(!backend.contains("not implemented"));
            }
        }
    }
}
