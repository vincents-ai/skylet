// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// API registry and version management system for skylet-plugin-common v0.3.0
use crate::PluginCommonError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// API definition with version and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiDefinition {
    pub name: String,
    pub base_url: String,
    pub version: String,
    pub auth_type: AuthType,
    pub rate_limits: RateLimits,
    pub endpoints: Vec<EndpointDefinition>,
    pub documentation_url: Option<String>,
    pub changelog_url: Option<String>,
    pub repository_url: Option<String>,
    pub support_email: Option<String>,
    pub license: String,
    pub tags: Vec<String>,
    pub metadata: ApiMetadata,
}

/// Authentication type for APIs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuthType {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "bearer")]
    Bearer,
    #[serde(rename = "api_key")]
    ApiKey,
    #[serde(rename = "basic")]
    Basic,
    #[serde(rename = "oauth2")]
    OAuth2,
    #[serde(rename = "custom")]
    Custom { auth_type: String },
}

/// Rate limits for API calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimits {
    pub requests_per_minute: Option<u32>,
    pub requests_per_hour: Option<u32>,
    pub requests_per_day: Option<u32>,
    pub concurrent_connections: Option<u32>,
    pub max_payload_size_mb: Option<u64>,
    pub burst_limit: Option<u32>,
}

/// API endpoint definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointDefinition {
    pub path: String,
    pub method: HttpMethod,
    pub description: String,
    pub parameters: serde_json::Value,
    pub response_schema: serde_json::Value,
    pub auth_required: bool,
    pub rate_limit_override: Option<RateLimits>,
    pub deprecated: Option<DeprecationInfo>,
    pub version_added: String,
    pub version_removed: Option<String>,
}

/// HTTP method types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HttpMethod {
    #[serde(rename = "GET")]
    Get,
    #[serde(rename = "POST")]
    Post,
    #[serde(rename = "PUT")]
    Put,
    #[serde(rename = "DELETE")]
    Delete,
    #[serde(rename = "PATCH")]
    Patch,
    #[serde(rename = "HEAD")]
    Head,
    #[serde(rename = "OPTIONS")]
    Options,
}

/// Deprecation information for endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecationInfo {
    pub message: String,
    pub removal_date: Option<String>,
    pub alternative_endpoint: Option<String>,
    pub migration_guide: Option<String>,
}

/// API metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMetadata {
    pub category: String,
    pub owner: String,
    pub first_release_date: Option<String>,
    pub last_updated_date: Option<String>,
    pub downloads: Option<u64>,
    pub stars: Option<u64>,
    pub license: String,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub examples: Vec<ApiExample>,
}

/// API usage example
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiExample {
    pub title: String,
    pub description: String,
    pub code_snippet: String,
    pub language: String,
    pub tags: Vec<String>,
}

/// API registry for managing multiple API definitions
pub struct ApiRegistry {
    apis: Arc<RwLock<HashMap<String, ApiDefinition>>>,
    version_constraints: Arc<RwLock<HashMap<String, VersionConstraint>>>,
    default_api: Arc<RwLock<Option<String>>>,
}

impl ApiRegistry {
    /// Create a new API registry
    pub fn new() -> Self {
        Self {
            apis: Arc::new(RwLock::new(HashMap::new())),
            version_constraints: Arc::new(RwLock::new(HashMap::new())),
            default_api: Arc::new(RwLock::new(None)),
        }
    }

    /// Register an API definition
    pub async fn register_api(&self, api: ApiDefinition) -> Result<(), PluginCommonError> {
        let mut apis = self.apis.write().await;

        // Validate API definition
        self.validate_api(&api)?;

        apis.insert(api.name.clone(), api);

        Ok(())
    }

    /// Update an existing API definition
    pub async fn update_api(
        &self,
        name: &str,
        api: ApiDefinition,
    ) -> Result<(), PluginCommonError> {
        let mut apis = self.apis.write().await;

        // Validate API definition
        self.validate_api(&api)?;

        apis.insert(name.to_string(), api);

        Ok(())
    }

    /// Get an API definition by name
    pub async fn get_api(&self, name: &str) -> Option<ApiDefinition> {
        let apis = self.apis.read().await;
        apis.get(name).cloned()
    }

    /// List all registered APIs
    pub async fn list_apis(&self) -> Vec<ApiDefinition> {
        let apis = self.apis.read().await;
        apis.values().cloned().collect()
    }

    /// List APIs by category
    pub async fn list_apis_by_category(&self, category: &str) -> Vec<ApiDefinition> {
        let apis = self.apis.read().await;
        apis.values()
            .filter(|api| api.metadata.category == category)
            .cloned()
            .collect()
    }

    /// Search for APIs by name, description, or tags
    pub async fn search_apis(&self, query: &str) -> Vec<ApiDefinition> {
        let apis = self.apis.read().await;
        let query_lower = query.to_lowercase();

        apis.values()
            .filter(|api| {
                api.name.to_lowercase().contains(&query_lower)
                    || api
                        .metadata
                        .documentation
                        .as_ref()
                        .map_or(false, |docs| docs.to_lowercase().contains(&query_lower))
                    || api
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&query_lower))
                    || api.metadata.category.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect()
    }

    /// Set version constraints for an API
    pub async fn set_version_constraint(
        &self,
        api_name: &str,
        constraint: VersionConstraint,
    ) -> Result<(), PluginCommonError> {
        let mut constraints = self.version_constraints.write().await;
        constraints.insert(api_name.to_string(), constraint);
        Ok(())
    }

    /// Check if an API version is compatible with constraints
    pub async fn is_version_compatible(&self, api_name: &str, version: &str) -> bool {
        let constraints = self.version_constraints.read().await;

        if let Some(constraint) = constraints.get(api_name) {
            self.check_version_compatibility(version, constraint)
        } else {
            true // No constraints means any version is compatible
        }
    }

    /// Set default API
    pub async fn set_default_api(&self, api_name: &str) -> Result<(), PluginCommonError> {
        // Verify API exists
        let apis = self.apis.read().await;
        if !apis.contains_key(api_name) {
            return Err(PluginCommonError::SerializationFailed(format!(
                "API '{}' not found",
                api_name
            )));
        }

        let mut default_api = self.default_api.write().await;
        *default_api = Some(api_name.to_string());

        Ok(())
    }

    /// Get default API
    pub async fn get_default_api(&self) -> Option<String> {
        self.default_api.read().await.clone()
    }

    /// Get available versions of an API
    pub async fn get_api_versions(&self, api_name: &str) -> Vec<String> {
        // This would typically connect to a package registry or version control
        // For now, return a simulated list
        vec![
            "1.0.0".to_string(),
            "1.1.0".to_string(),
            "2.0.0".to_string(),
        ]
    }

    /// Remove an API definition
    pub async fn unregister_api(&self, api_name: &str) -> Result<(), PluginCommonError> {
        let mut apis = self.apis.write().await;

        // Check if API is set as default and remove it
        let mut default_api = self.default_api.write().await;
        if let Some(current_default) = default_api.as_ref() {
            if current_default == api_name {
                *default_api = None;
            }
        }

        apis.remove(api_name);

        Ok(())
    }

    /// Validate an API definition
    fn validate_api(&self, api: &ApiDefinition) -> Result<(), PluginCommonError> {
        // Check required fields
        if api.name.is_empty() {
            return Err(PluginCommonError::SerializationFailed(
                "API name cannot be empty".to_string(),
            ));
        }

        if api.base_url.is_empty() {
            return Err(PluginCommonError::SerializationFailed(
                "API base URL cannot be empty".to_string(),
            ));
        }

        if !api.base_url.starts_with("http://") && !api.base_url.starts_with("https://") {
            return Err(PluginCommonError::SerializationFailed(
                "API base URL must start with http:// or https://".to_string(),
            ));
        }

        // Validate endpoints
        for endpoint in &api.endpoints {
            if endpoint.path.is_empty() {
                return Err(PluginCommonError::SerializationFailed(
                    "Endpoint path cannot be empty".to_string(),
                ));
            }

            if !endpoint.path.starts_with('/') {
                return Err(PluginCommonError::SerializationFailed(
                    "Endpoint path must start with /".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Check version compatibility with constraint
    fn check_version_compatibility(&self, version: &str, constraint: &VersionConstraint) -> bool {
        match constraint {
            VersionConstraint::Exact(v) => version == v,
            VersionConstraint::Minimum(v) => self.version_ge(version, v),
            VersionConstraint::Maximum(v) => self.version_le(version, v),
            VersionConstraint::Range { min, max } => {
                self.version_ge(version, min) && self.version_le(version, max)
            }
            VersionConstraint::CompatibleWith(v) => self.version_compatible_with(version, v),
        }
    }

    /// Compare versions (greater than or equal)
    fn version_ge(&self, a: &str, b: &str) -> bool {
        // Simple version comparison - in production would use a proper version library
        let a_parts: Vec<&str> = a.split('.').collect();
        let b_parts: Vec<&str> = b.split('.').collect();

        for i in 0..std::cmp::min(a_parts.len(), b_parts.len()) {
            match a_parts
                .get(i)
                .map_or("", |v| v)
                .cmp(b_parts.get(i).map_or("", |v| v))
            {
                std::cmp::Ordering::Less => return false,
                std::cmp::Ordering::Greater => return true,
                std::cmp::Ordering::Equal => continue,
            }
        }

        // If all compared parts are equal or a is greater, a >= b
        a_parts.len() >= b_parts.len()
    }

    /// Compare versions (less than or equal)
    fn version_le(&self, a: &str, b: &str) -> bool {
        let a_parts: Vec<&str> = a.split('.').collect();
        let b_parts: Vec<&str> = b.split('.').collect();

        for i in 0..std::cmp::min(a_parts.len(), b_parts.len()) {
            match a_parts
                .get(i)
                .map_or("", |v| v)
                .cmp(b_parts.get(i).map_or("", |v| v))
            {
                std::cmp::Ordering::Greater => return false,
                std::cmp::Ordering::Less => return true,
                std::cmp::Ordering::Equal => continue,
            }
        }

        // If all compared parts are equal or a is less, a <= b
        a_parts.len() <= b_parts.len()
    }

    /// Check if version is compatible with constraint
    fn version_compatible_with(&self, version: &str, constraint: &str) -> bool {
        if constraint == "*" {
            return true;
        }
        let version_parts: Vec<&str> = version.split('.').collect();
        let constraint_parts: Vec<&str> = constraint.split('.').collect();
        for (i, cp) in constraint_parts.iter().enumerate() {
            if *cp == "x" || *cp == "*" {
                continue;
            }
            match version_parts.get(i) {
                Some(vp) if vp == cp => continue,
                _ => return false,
            }
        }
        true
    }
}

/// Version constraint for APIs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VersionConstraint {
    #[serde(rename = "exact")]
    Exact(String),
    #[serde(rename = "minimum")]
    Minimum(String),
    #[serde(rename = "maximum")]
    Maximum(String),
    #[serde(rename = "range")]
    Range { min: String, max: String },
    #[serde(rename = "compatible_with")]
    CompatibleWith(String),
}

impl Default for ApiRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions for creating API definitions
pub fn create_api_definition(
    name: &str,
    base_url: &str,
    version: &str,
    auth_type: AuthType,
) -> ApiDefinition {
    ApiDefinition {
        name: name.to_string(),
        base_url: base_url.to_string(),
        version: version.to_string(),
        auth_type,
        rate_limits: RateLimits {
            requests_per_minute: Some(60),
            requests_per_hour: Some(1000),
            requests_per_day: Some(10000),
            concurrent_connections: Some(10),
            max_payload_size_mb: Some(10),
            burst_limit: Some(100),
        },
        endpoints: vec![],
        documentation_url: None,
        changelog_url: None,
        repository_url: None,
        support_email: None,
        license: "MIT".to_string(),
        tags: vec![],
        metadata: ApiMetadata {
            category: "api".to_string(),
            owner: "Skylet".to_string(),
            first_release_date: None,
            last_updated_date: None,
            downloads: None,
            stars: None,
            license: "MIT".to_string(),
            homepage: None,
            documentation: None,
            examples: vec![],
        },
    }
}

pub fn create_endpoint_definition(
    path: &str,
    method: HttpMethod,
    description: &str,
    auth_required: bool,
) -> EndpointDefinition {
    EndpointDefinition {
        path: path.to_string(),
        method,
        description: description.to_string(),
        parameters: serde_json::json!({}),
        response_schema: serde_json::json!({}),
        auth_required,
        rate_limit_override: None,
        deprecated: None,
        version_added: "1.0.0".to_string(),
        version_removed: None,
    }
}

pub fn create_api_registry() -> ApiRegistry {
    ApiRegistry::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_definition_creation() {
        let api = create_api_definition(
            "test-api",
            "https://api.example.com",
            "1.0.0",
            AuthType::Bearer,
        );

        assert_eq!(api.name, "test-api");
        assert_eq!(api.base_url, "https://api.example.com");
        assert_eq!(api.version, "1.0.0");
        assert_eq!(api.auth_type, AuthType::Bearer);
    }

    #[test]
    fn test_endpoint_definition_creation() {
        let endpoint =
            create_endpoint_definition("/users", HttpMethod::Get, "Get user information", true);

        assert_eq!(endpoint.path, "/users");
        assert_eq!(endpoint.method, HttpMethod::Get);
        assert_eq!(endpoint.description, "Get user information");
        assert_eq!(endpoint.auth_required, true);
    }

    #[test]
    fn test_version_constraints() {
        let registry = create_api_registry();

        // Test exact version constraint
        assert!(registry.version_ge("1.2.0", "1.2.0"));
        assert!(!registry.version_ge("1.1.0", "1.2.0"));

        // Test minimum version constraint
        assert!(registry.version_ge("1.2.0", "1.0.0"));
        assert!(!registry.version_ge("0.9.0", "1.0.0"));

        // Test compatible with constraint
        assert!(registry.version_compatible_with("1.2.0", "1.x"));
        assert!(!registry.version_compatible_with("2.0.0", "1.x"));
    }

    #[tokio::test]
    async fn test_api_registry_operations() {
        let registry = create_api_registry();
        let api = create_api_definition(
            "test-api",
            "https://api.example.com",
            "1.0.0",
            AuthType::Bearer,
        );

        // Test registration
        let result = registry.register_api(api).await;
        assert!(result.is_ok());

        // Test retrieval
        let retrieved = registry.get_api("test-api").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-api");

        // Test search
        let search_results = registry.search_apis("test").await;
        assert!(!search_results.is_empty());
    }

    #[test]
    fn test_api_validation() {
        let registry = create_api_registry();

        // Valid API should pass
        let valid_api = create_api_definition("valid", "https://api.com", "1.0.0", AuthType::None);
        assert!(registry.validate_api(&valid_api).is_ok());

        // Invalid API with empty name should fail
        let mut invalid_api = valid_api.clone();
        invalid_api.name = String::new();
        assert!(registry.validate_api(&invalid_api).is_err());

        // Invalid API with invalid URL should fail
        invalid_api.name = "invalid".to_string();
        invalid_api.base_url = "not-a-url".to_string();
        assert!(registry.validate_api(&invalid_api).is_err());
    }
}
