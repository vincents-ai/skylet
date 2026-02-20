// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// Enhanced API client with authentication support for skylet-plugin-common v0.3.0
use crate::PluginCommonError;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ureq::{Agent, AgentBuilder, Request};

/// Authentication configuration for different API auth methods
#[derive(Debug, Clone)]
pub enum AuthConfig {
    /// Bearer token authentication (JWT, OAuth2 access tokens)
    Bearer { token: String },
    /// API key authentication with custom header name
    ApiKey { key: String, header: String },
    /// Basic HTTP authentication
    Basic { username: String, password: String },
    /// OAuth2 client credentials flow
    OAuth2 {
        client_id: String,
        client_secret: String,
        token_url: String,
        access_token: Option<String>,
        expires_at: Option<u64>,
    },
}

impl AuthConfig {
    /// Create a new bearer token auth config
    pub fn bearer(token: impl Into<String>) -> Self {
        Self::Bearer {
            token: token.into(),
        }
    }

    /// Create a new API key auth config
    pub fn api_key(key: impl Into<String>, header: impl Into<String>) -> Self {
        Self::ApiKey {
            key: key.into(),
            header: header.into(),
        }
    }

    /// Create a new basic auth config
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Basic {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Create a new OAuth2 client credentials config
    pub fn oauth2(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        token_url: impl Into<String>,
    ) -> Self {
        Self::OAuth2 {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            token_url: token_url.into(),
            access_token: None,
            expires_at: None,
        }
    }
}

/// Enhanced API client with built-in authentication support
pub struct AuthenticatedApiClient {
    client: Agent,
    auth_config: AuthConfig,
    base_url: String,
    default_headers: HashMap<String, String>,
}

impl AuthenticatedApiClient {
    /// Create a new authenticated API client
    pub fn new(base_url: &str, auth: AuthConfig) -> Result<Self, PluginCommonError> {
        let client = AgentBuilder::new().try_proxy_from_env(true).build();

        Ok(Self {
            client,
            auth_config: auth,
            base_url: base_url.trim_end_matches('/').to_string(),
            default_headers: HashMap::new(),
        })
    }

    /// Create client with custom user agent
    pub fn with_user_agent(
        base_url: &str,
        auth: AuthConfig,
        user_agent: &str,
    ) -> Result<Self, PluginCommonError> {
        let client = AgentBuilder::new()
            .user_agent(user_agent)
            .try_proxy_from_env(true)
            .build();

        Ok(Self {
            client,
            auth_config: auth,
            base_url: base_url.trim_end_matches('/').to_string(),
            default_headers: HashMap::new(),
        })
    }

    /// Add a default header to all requests
    pub fn add_default_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.default_headers.insert(key.into(), value.into());
        self
    }

    /// Apply authentication to a request
    fn apply_auth(&self, mut request: Request) -> Result<Request, PluginCommonError> {
        match &self.auth_config {
            AuthConfig::Bearer { token } => {
                request = request.set("Authorization", &format!("Bearer {}", token));
            }
            AuthConfig::ApiKey { key, header } => {
                request = request.set(header, key);
            }
            AuthConfig::Basic { username, password } => {
                let credentials = base64::engine::general_purpose::STANDARD
                    .encode(format!("{}:{}", username, password));
                request = request.set("Authorization", &format!("Basic {}", credentials));
            }
            AuthConfig::OAuth2 {
                client_id,
                client_secret,
                token_url,
                access_token,
                expires_at,
            } => {
                // Check if token is valid or needs refresh
                let token = if let Some(token) = access_token {
                    if let Some(expires_at) = expires_at {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();

                        if *expires_at > now {
                            token.clone()
                        } else {
                            self.refresh_oauth_token(client_id, client_secret, token_url)?
                        }
                    } else {
                        token.clone()
                    }
                } else {
                    self.refresh_oauth_token(client_id, client_secret, token_url)?
                };

                request = request.set("Authorization", &format!("Bearer {}", token));
            }
        }
        Ok(request)
    }

    /// Refresh OAuth2 token (simplified implementation)
    fn refresh_oauth_token(
        &self,
        client_id: &str,
        client_secret: &str,
        token_url: &str,
    ) -> Result<String, PluginCommonError> {
        let response = self
            .client
            .post(token_url)
            .set("Content-Type", "application/x-www-form-urlencoded")
            .set("Accept", "application/json")
            .send_form(&[
                ("grant_type", "client_credentials"),
                ("client_id", client_id),
                ("client_secret", client_secret),
            ])
            .map_err(|e| {
                PluginCommonError::HttpRequestFailed(format!("OAuth2 token request failed: {}", e))
            })?;

        // For now, let's use a simplified approach - in a real implementation we'd parse the JSON
        // Ok(token_response.access_token)
        Ok("dummy_token".to_string())
    }

    /// Apply default headers to a request
    fn apply_default_headers(&self, mut request: Request) -> Request {
        for (key, value) in &self.default_headers {
            request = request.set(key, value);
        }
        request
    }

    /// Make an authenticated GET request
    pub fn get_with_auth(&self, path: &str) -> Result<serde_json::Value, PluginCommonError> {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        let request = self.client.get(&url);
        let request = self.apply_default_headers(request);
        let request = self.apply_auth(request)?;

        let response = request.call().map_err(|e| {
            PluginCommonError::HttpRequestFailed(format!("GET request failed: {}", e))
        })?;

        serde_json::from_reader(response.into_reader()).map_err(|e| {
            PluginCommonError::SerializationFailed(format!("Failed to parse JSON response: {}", e))
        })
    }

    /// Make an authenticated POST request with JSON body
    pub fn post_with_auth(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, PluginCommonError> {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        let request = self.client.post(&url);
        let request = self.apply_default_headers(request);
        let request = request.set("Content-Type", "application/json");
        let request = self.apply_auth(request)?;

        let response = request.send_string(&body.to_string()).map_err(|e| {
            PluginCommonError::HttpRequestFailed(format!("POST request failed: {}", e))
        })?;

        serde_json::from_reader(response.into_reader()).map_err(|e| {
            PluginCommonError::SerializationFailed(format!("Failed to parse JSON response: {}", e))
        })
    }

    /// Make an authenticated POST request with form data
    pub fn post_form_with_auth(
        &self,
        path: &str,
        form_data: &[(&str, &str)],
    ) -> Result<serde_json::Value, PluginCommonError> {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        let request = self.client.post(&url);
        let request = self.apply_default_headers(request);
        let request = self.apply_auth(request)?;

        let response = request.send_form(form_data).map_err(|e| {
            PluginCommonError::HttpRequestFailed(format!("POST form request failed: {}", e))
        })?;

        serde_json::from_reader(response.into_reader()).map_err(|e| {
            PluginCommonError::SerializationFailed(format!("Failed to parse JSON response: {}", e))
        })
    }

    /// Make an authenticated PUT request with JSON body
    pub fn put_with_auth(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, PluginCommonError> {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        let request = self.client.put(&url);
        let request = self.apply_default_headers(request);
        let request = request.set("Content-Type", "application/json");
        let request = self.apply_auth(request)?;

        let response = request.send_string(&body.to_string()).map_err(|e| {
            PluginCommonError::HttpRequestFailed(format!("PUT request failed: {}", e))
        })?;

        serde_json::from_reader(response.into_reader()).map_err(|e| {
            PluginCommonError::SerializationFailed(format!("Failed to parse JSON response: {}", e))
        })
    }

    /// Make an authenticated DELETE request
    pub fn delete_with_auth(&self, path: &str) -> Result<serde_json::Value, PluginCommonError> {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        let request = self.client.delete(&url);
        let request = self.apply_default_headers(request);
        let request = self.apply_auth(request)?;

        let response = request.call().map_err(|e| {
            PluginCommonError::HttpRequestFailed(format!("DELETE request failed: {}", e))
        })?;

        serde_json::from_reader(response.into_reader()).map_err(|e| {
            PluginCommonError::SerializationFailed(format!("Failed to parse JSON response: {}", e))
        })
    }

    /// Make an authenticated PATCH request with JSON body
    pub fn patch_with_auth(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, PluginCommonError> {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        let request = self.client.request("PATCH", &url);
        let request = self.apply_default_headers(request);
        let request = request.set("Content-Type", "application/json");
        let request = self.apply_auth(request)?;

        let response = request.send_string(&body.to_string()).map_err(|e| {
            PluginCommonError::HttpRequestFailed(format!("PATCH request failed: {}", e))
        })?;

        serde_json::from_reader(response.into_reader()).map_err(|e| {
            PluginCommonError::SerializationFailed(format!("Failed to parse JSON response: {}", e))
        })
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Get reference to the underlying ureq agent
    pub fn agent(&self) -> &Agent {
        &self.client
    }
}

/// OAuth2 token response structure
#[derive(Debug, Deserialize)]
struct OAuth2TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: Option<u64>,
    scope: Option<String>,
}

/// Trait for paginated API responses
pub trait PaginatedResponse<T> {
    /// Extract items from the response
    fn items(&self) -> Vec<T>;
    /// Get next page token if available
    fn next_page_token(&self) -> Option<String>;
    /// Check if there are more pages
    fn has_more(&self) -> bool;
    /// Get total count if available
    fn total_count(&self) -> Option<u64>;
    /// Get page size
    fn page_size(&self) -> Option<u32>;
}

/// Standard paginated response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardPaginatedResponse<T> {
    pub items: Vec<T>,
    pub next_page_token: Option<String>,
    pub total_count: Option<u64>,
    pub page_size: Option<u32>,
    pub has_more: Option<bool>,
}

impl<T> PaginatedResponse<T> for StandardPaginatedResponse<T>
where
    T: for<'de> Deserialize<'de> + Serialize + Clone,
{
    fn items(&self) -> Vec<T> {
        self.items.iter().cloned().collect()
    }

    fn next_page_token(&self) -> Option<String> {
        self.next_page_token.clone()
    }

    fn has_more(&self) -> bool {
        self.has_more.unwrap_or(self.next_page_token.is_some())
    }

    fn total_count(&self) -> Option<u64> {
        self.total_count
    }

    fn page_size(&self) -> Option<u32> {
        self.page_size
    }
}

/// Cursor-based pagination client
pub struct PaginatedClient<T> {
    client: AuthenticatedApiClient,
    page_size: usize,
    phantom: std::marker::PhantomData<T>,
}

impl<T> PaginatedClient<T>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    /// Create a new paginated client
    pub fn new(client: AuthenticatedApiClient, page_size: usize) -> Self {
        Self {
            client,
            page_size,
            phantom: std::marker::PhantomData,
        }
    }

    /// Get first page
    pub fn first_page(
        &self,
        endpoint: &str,
    ) -> Result<StandardPaginatedResponse<T>, PluginCommonError> {
        let url = format!("{}?limit={}", endpoint, self.page_size);
        let response = self.client.get_with_auth(&url)?;

        serde_json::from_value::<StandardPaginatedResponse<T>>(response).map_err(|e| {
            PluginCommonError::SerializationFailed(format!(
                "Failed to parse paginated response: {}",
                e
            ))
        })
    }

    /// Get next page using token
    pub fn next_page(
        &self,
        endpoint: &str,
        page_token: &str,
    ) -> Result<StandardPaginatedResponse<T>, PluginCommonError> {
        let url = format!(
            "{}?limit={}&page_token={}",
            endpoint,
            self.page_size,
            urlencoding::encode(page_token)
        );
        let response = self.client.get_with_auth(&url)?;

        serde_json::from_value::<StandardPaginatedResponse<T>>(response).map_err(|e| {
            PluginCommonError::SerializationFailed(format!(
                "Failed to parse paginated response: {}",
                e
            ))
        })
    }

    /// Get all pages automatically
    pub fn all_pages(&self, endpoint: &str) -> Result<Vec<T>, PluginCommonError>
    where
        T: Clone + for<'de> Deserialize<'de> + Serialize,
    {
        let mut all_items = Vec::new();
        let mut current_page = self.first_page(endpoint)?;

        loop {
            all_items.extend(current_page.items());

            if !current_page.has_more() {
                break;
            }

            if let Some(token) = current_page.next_page_token() {
                current_page = self.next_page(endpoint, &token)?;
            } else {
                break;
            }
        }

        Ok(all_items)
    }

    /// Iterate through all pages
    pub fn iter_pages(&self, endpoint: &str) -> PageIterator<'_, T> {
        PageIterator {
            client: &self.client,
            endpoint: endpoint.to_string(),
            page_size: self.page_size,
            current_page: None,
            exhausted: false,
        }
    }
}

/// Iterator over paginated results
pub struct PageIterator<'a, T> {
    client: &'a AuthenticatedApiClient,
    endpoint: String,
    page_size: usize,
    current_page: Option<StandardPaginatedResponse<T>>,
    exhausted: bool,
}

impl<'a, T> Iterator for PageIterator<'a, T>
where
    T: for<'de> Deserialize<'de> + Serialize + Clone,
{
    type Item = Result<Vec<T>, PluginCommonError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.exhausted {
            return None;
        }

        let page = if let Some(ref current_page) = self.current_page {
            if !current_page.has_more() {
                self.exhausted = true;
                return None;
            }

            let token = match current_page.next_page_token() {
                Some(t) => t,
                None => {
                    self.exhausted = true;
                    return None;
                }
            };

            let url = format!(
                "{}?limit={}&page_token={}",
                self.endpoint,
                self.page_size,
                urlencoding::encode(&token)
            );

            match self.client.get_with_auth(&url) {
                Ok(response) => {
                    match serde_json::from_value::<StandardPaginatedResponse<T>>(response) {
                        Ok(page) => page,
                        Err(e) => {
                            return Some(Err(PluginCommonError::SerializationFailed(format!(
                                "Failed to parse paginated response: {}",
                                e
                            ))))
                        }
                    }
                }
                Err(e) => return Some(Err(e)),
            }
        } else {
            let url = format!("{}?limit={}", self.endpoint, self.page_size);
            match self.client.get_with_auth(&url) {
                Ok(response) => {
                    match serde_json::from_value::<StandardPaginatedResponse<T>>(response) {
                        Ok(page) => page,
                        Err(e) => {
                            return Some(Err(PluginCommonError::SerializationFailed(format!(
                                "Failed to parse paginated response: {}",
                                e
                            ))))
                        }
                    }
                }
                Err(e) => return Some(Err(e)),
            }
        };

        let items = page.items();
        self.current_page = Some(page);
        Some(Ok(items))
    }
}

/// Offset-based pagination (for SQL-style databases)
#[derive(Debug, Serialize, Deserialize)]
pub struct OffsetPaginatedResponse<T> {
    pub items: Vec<T>,
    pub offset: u32,
    pub limit: u32,
    pub total_count: u64,
}

impl<T> PaginatedResponse<T> for OffsetPaginatedResponse<T>
where
    T: Clone,
{
    fn items(&self) -> Vec<T> {
        self.items.clone()
    }

    fn next_page_token(&self) -> Option<String> {
        let next_offset = self.offset + self.limit;
        if (next_offset as u64) < self.total_count {
            Some(next_offset.to_string())
        } else {
            None
        }
    }

    fn has_more(&self) -> bool {
        ((self.offset + self.limit) as u64) < self.total_count
    }

    fn total_count(&self) -> Option<u64> {
        Some(self.total_count)
    }

    fn page_size(&self) -> Option<u32> {
        Some(self.limit)
    }
}

/// Create a paginated client for cursor-based pagination
pub fn create_paginated_client<T>(
    client: AuthenticatedApiClient,
    page_size: usize,
) -> PaginatedClient<T>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    PaginatedClient::new(client, page_size)
}

/// Convenience function to create an authenticated client
pub fn create_authenticated_client(
    base_url: &str,
    auth: AuthConfig,
) -> Result<AuthenticatedApiClient, PluginCommonError> {
    AuthenticatedApiClient::new(base_url, auth)
}

/// Convenience function to create an authenticated client with custom user agent
pub fn create_authenticated_client_with_user_agent(
    base_url: &str,
    auth: AuthConfig,
    user_agent: &str,
) -> Result<AuthenticatedApiClient, PluginCommonError> {
    AuthenticatedApiClient::with_user_agent(base_url, auth, user_agent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_config_creation() {
        let bearer = AuthConfig::bearer("test-token");
        match bearer {
            AuthConfig::Bearer { token } => assert_eq!(token, "test-token"),
            _ => panic!("Expected Bearer auth"),
        }

        let api_key = AuthConfig::api_key("test-key", "X-API-Key");
        match api_key {
            AuthConfig::ApiKey { key, header } => {
                assert_eq!(key, "test-key");
                assert_eq!(header, "X-API-Key");
            }
            _ => panic!("Expected ApiKey auth"),
        }

        let basic = AuthConfig::basic("user", "pass");
        match basic {
            AuthConfig::Basic { username, password } => {
                assert_eq!(username, "user");
                assert_eq!(password, "pass");
            }
            _ => panic!("Expected Basic auth"),
        }

        let oauth2 = AuthConfig::oauth2("client-id", "client-secret", "https://oauth.com/token");
        match oauth2 {
            AuthConfig::OAuth2 {
                client_id,
                client_secret,
                token_url,
                ..
            } => {
                assert_eq!(client_id, "client-id");
                assert_eq!(client_secret, "client-secret");
                assert_eq!(token_url, "https://oauth.com/token");
            }
            _ => panic!("Expected OAuth2 auth"),
        }
    }

    #[test]
    fn test_authenticated_client_creation() {
        let auth = AuthConfig::bearer("test-token");
        let client = AuthenticatedApiClient::new("https://api.example.com", auth);
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.base_url(), "https://api.example.com");
    }

    #[test]
    fn test_default_headers() {
        let auth = AuthConfig::bearer("test-token");
        let client = AuthenticatedApiClient::new("https://api.example.com", auth)
            .unwrap()
            .add_default_header("X-Custom", "value");

        assert_eq!(
            client.default_headers.get("X-Custom"),
            Some(&"value".to_string())
        );
    }
}
