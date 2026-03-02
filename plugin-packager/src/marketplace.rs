// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Marketplace API client for plugin publishing and discovery
///
/// This module provides a client for interacting with the Skylet plugin marketplace.
/// It handles:
/// - Publishing plugins with metadata and ABI information
/// - Searching and discovering plugins by category, tag, or query
/// - Retrieving detailed plugin information and ratings
/// - Managing plugin versions and updates
/// - Handling authentication and authorization
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::abi_compat::ABICompatibleInfo;

/// Marketplace API client
#[derive(Clone)]
pub struct MarketplaceClient {
    base_url: String,
    client: Arc<reqwest::Client>,
    auth_token: Option<String>,
}

/// Plugin published on the marketplace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplacePlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub category: String,
    pub tags: Vec<String>,
    pub downloads: u64,
    pub rating: f32,
    pub maturity: String,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub published_at: String,
    pub updated_at: String,
}

/// Search query parameters
#[derive(Debug, Clone, Serialize)]
pub struct SearchQuery {
    pub query: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub min_maturity: Option<String>,
    pub sort: Option<SearchSort>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Search result sorting options
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSort {
    Relevance,
    Downloads,
    Rating,
    Recent,
}

/// Search results from marketplace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    pub total: u64,
    pub plugins: Vec<MarketplacePlugin>,
    pub limit: u32,
    pub offset: u32,
}

/// Plugin publish request
#[derive(Debug, Clone, Serialize)]
pub struct PublishRequest {
    pub metadata: ABICompatibleInfo,
    pub package_url: String,
    pub checksum: String,
    pub signatures: Option<Vec<PublishSignature>>,
}

/// Plugin signature for verification (lightweight version for publishing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishSignature {
    pub algorithm: String,
    pub signature: String,
    pub key_id: Option<String>,
}

/// Publish response from marketplace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResponse {
    pub id: String,
    pub version: String,
    pub status: PublishStatus,
    pub message: String,
    pub verification_url: Option<String>,
}

/// Status of published plugin
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PublishStatus {
    Pending,
    Verified,
    Active,
    Rejected,
    Suspended,
}

/// Plugin version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginVersionInfo {
    pub version: String,
    pub released_at: String,
    pub downloads: u64,
    pub status: PublishStatus,
    pub abi_version: String,
    pub maturity: String,
    pub changelog: Option<String>,
}

/// Plugin ratings and reviews
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRating {
    pub average: f32,
    pub count: u64,
    pub distribution: RatingDistribution,
}

/// Distribution of ratings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingDistribution {
    pub five_star: u64,
    pub four_star: u64,
    pub three_star: u64,
    pub two_star: u64,
    pub one_star: u64,
}

impl MarketplaceClient {
    /// Create a new marketplace client
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: Arc::new(reqwest::Client::new()),
            auth_token: None,
        }
    }

    /// Create a new client with authentication token
    pub fn with_auth(base_url: String, token: String) -> Self {
        Self {
            base_url,
            client: Arc::new(reqwest::Client::new()),
            auth_token: Some(token),
        }
    }

    /// Get the base URL of the marketplace
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Check if client has authentication token
    pub fn has_auth(&self) -> bool {
        self.auth_token.is_some()
    }

    /// Set authentication token
    pub fn set_auth_token(&mut self, token: String) {
        self.auth_token = Some(token);
    }

    /// Search for plugins in the marketplace
    pub async fn search(&self, query: SearchQuery) -> Result<SearchResults> {
        let url = format!("{}/api/v1/plugins/search", self.base_url);

        let response = self
            .client
            .get(&url)
            .query(&query)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to search marketplace: {}", e))?;

        match response.status() {
            reqwest::StatusCode::OK => response
                .json()
                .await
                .map_err(|e| anyhow!("Failed to parse search results: {}", e)),
            status => Err(anyhow!("Marketplace search failed with status {}", status)),
        }
    }

    /// Get detailed information about a plugin
    pub async fn get_plugin(&self, plugin_id: &str) -> Result<MarketplacePlugin> {
        let url = format!("{}/api/v1/plugins/{}", self.base_url, plugin_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch plugin: {}", e))?;

        match response.status() {
            reqwest::StatusCode::OK => response
                .json()
                .await
                .map_err(|e| anyhow!("Failed to parse plugin data: {}", e)),
            reqwest::StatusCode::NOT_FOUND => Err(anyhow!("Plugin not found")),
            status => Err(anyhow!("Failed to fetch plugin with status {}", status)),
        }
    }

    /// Get version history for a plugin
    pub async fn get_versions(
        &self,
        plugin_id: &str,
        limit: u32,
    ) -> Result<Vec<PluginVersionInfo>> {
        let url = format!("{}/api/v1/plugins/{}/versions", self.base_url, plugin_id);

        let response = self
            .client
            .get(&url)
            .query(&[("limit", limit.to_string())])
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch versions: {}", e))?;

        match response.status() {
            reqwest::StatusCode::OK => response
                .json()
                .await
                .map_err(|e| anyhow!("Failed to parse version data: {}", e)),
            status => Err(anyhow!("Failed to fetch versions with status {}", status)),
        }
    }

    /// Get ratings for a plugin
    pub async fn get_ratings(&self, plugin_id: &str) -> Result<PluginRating> {
        let url = format!("{}/api/v1/plugins/{}/ratings", self.base_url, plugin_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch ratings: {}", e))?;

        match response.status() {
            reqwest::StatusCode::OK => response
                .json()
                .await
                .map_err(|e| anyhow!("Failed to parse rating data: {}", e)),
            status => Err(anyhow!("Failed to fetch ratings with status {}", status)),
        }
    }

    /// Publish a plugin to the marketplace
    ///
    /// Requires authentication. Returns plugin ID and publish status.
    pub async fn publish(&self, request: PublishRequest) -> Result<PublishResponse> {
        let url = format!("{}/api/v1/plugins/publish", self.base_url);

        let mut req = self.client.post(&url);

        // Add authentication header if available
        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let response = req
            .json(&request)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to publish plugin: {}", e))?;

        match response.status() {
            reqwest::StatusCode::CREATED | reqwest::StatusCode::ACCEPTED => response
                .json()
                .await
                .map_err(|e| anyhow!("Failed to parse publish response: {}", e)),
            reqwest::StatusCode::UNAUTHORIZED => {
                Err(anyhow!("Authentication required for publishing"))
            }
            reqwest::StatusCode::BAD_REQUEST => Err(anyhow!("Invalid publish request")),
            status => Err(anyhow!("Failed to publish plugin with status {}", status)),
        }
    }

    /// Unpublish a plugin (requires authentication)
    pub async fn unpublish(&self, plugin_id: &str) -> Result<()> {
        let url = format!("{}/api/v1/plugins/{}", self.base_url, plugin_id);

        let mut req = self.client.delete(&url);

        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let response = req
            .send()
            .await
            .map_err(|e| anyhow!("Failed to unpublish plugin: {}", e))?;

        match response.status() {
            reqwest::StatusCode::OK | reqwest::StatusCode::NO_CONTENT => Ok(()),
            reqwest::StatusCode::UNAUTHORIZED => {
                Err(anyhow!("Authentication required for unpublishing"))
            }
            reqwest::StatusCode::NOT_FOUND => Err(anyhow!("Plugin not found")),
            status => Err(anyhow!("Failed to unpublish plugin with status {}", status)),
        }
    }

    /// Get plugins by category
    pub async fn get_by_category(
        &self,
        category: &str,
        limit: u32,
    ) -> Result<Vec<MarketplacePlugin>> {
        let query = SearchQuery {
            query: None,
            category: Some(category.to_string()),
            tags: None,
            min_maturity: None,
            sort: Some(SearchSort::Downloads),
            limit: Some(limit),
            offset: Some(0),
        };

        let results = self.search(query).await?;
        Ok(results.plugins)
    }

    /// Get trending plugins
    pub async fn get_trending(&self, limit: u32) -> Result<Vec<MarketplacePlugin>> {
        let query = SearchQuery {
            query: None,
            category: None,
            tags: None,
            min_maturity: Some("beta".to_string()),
            sort: Some(SearchSort::Downloads),
            limit: Some(limit),
            offset: Some(0),
        };

        let results = self.search(query).await?;
        Ok(results.plugins)
    }

    /// Get recently updated plugins
    pub async fn get_recent(&self, limit: u32) -> Result<Vec<MarketplacePlugin>> {
        let query = SearchQuery {
            query: None,
            category: None,
            tags: None,
            min_maturity: None,
            sort: Some(SearchSort::Recent),
            limit: Some(limit),
            offset: Some(0),
        };

        let results = self.search(query).await?;
        Ok(results.plugins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_marketplace_client_creation() {
        let client = MarketplaceClient::new("https://marketplace.example.com".to_string());
        assert_eq!(client.base_url, "https://marketplace.example.com");
        assert!(client.auth_token.is_none());
    }

    #[test]
    fn test_marketplace_client_with_auth() {
        let client = MarketplaceClient::with_auth(
            "https://marketplace.example.com".to_string(),
            "token123".to_string(),
        );
        assert_eq!(client.base_url, "https://marketplace.example.com");
        assert_eq!(client.auth_token, Some("token123".to_string()));
    }

    #[test]
    fn test_search_query_serialization() {
        let query = SearchQuery {
            query: Some("web server".to_string()),
            category: Some("integration".to_string()),
            tags: Some(vec!["api".to_string(), "rest".to_string()]),
            min_maturity: Some("stable".to_string()),
            sort: Some(SearchSort::Rating),
            limit: Some(10),
            offset: Some(0),
        };

        let json = serde_json::to_string(&query).expect("Failed to serialize");
        assert!(json.contains("web server"));
        assert!(json.contains("integration"));
    }

    #[test]
    fn test_publish_status_serialization() {
        let statuses = vec![
            PublishStatus::Pending,
            PublishStatus::Verified,
            PublishStatus::Active,
            PublishStatus::Rejected,
            PublishStatus::Suspended,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).expect("Failed to serialize");
            assert!(!json.is_empty());
        }
    }

    #[test]
    fn test_rating_distribution() {
        let dist = RatingDistribution {
            five_star: 100,
            four_star: 50,
            three_star: 20,
            two_star: 5,
            one_star: 2,
        };

        let rating = PluginRating {
            average: 4.5,
            count: 177,
            distribution: dist,
        };

        let json = serde_json::to_string(&rating).expect("Failed to serialize");
        assert!(json.contains("4.5"));
        assert!(json.contains("177"));
    }

    #[test]
    fn test_search_sort_options() {
        let sorts = vec![
            SearchSort::Relevance,
            SearchSort::Downloads,
            SearchSort::Rating,
            SearchSort::Recent,
        ];

        for sort in sorts {
            let json = serde_json::to_string(&sort).expect("Failed to serialize");
            assert!(!json.is_empty());
        }
    }
}
