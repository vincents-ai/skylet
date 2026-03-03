// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod adapters;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

// Result type for registry operations
pub type Result<T> = std::result::Result<T, RegistryError>;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug, Error, Clone)]
pub enum RegistryError {
    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Registry configuration error: {0}")]
    Configuration(String),

    #[error("Registry API error: {0}")]
    Api(String),

    #[error("Image not found: {0}:{1}")]
    ImageNotFound(String, String),

    #[error("Repository not found: {0}")]
    RepositoryNotFound(String),

    #[error("Tag not found: {0}")]
    TagNotFound(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

impl RegistryError {
    pub fn authentication(msg: impl Into<String>) -> Self {
        RegistryError::Authentication(msg.into())
    }

    pub fn configuration(msg: impl Into<String>) -> Self {
        RegistryError::Configuration(msg.into())
    }

    pub fn api(msg: impl Into<String>) -> Self {
        RegistryError::Api(msg.into())
    }

    pub fn image_not_found(image: &str, tag: &str) -> Self {
        RegistryError::ImageNotFound(image.to_string(), tag.to_string())
    }

    pub fn repository_not_found(repo: &str) -> Self {
        RegistryError::RepositoryNotFound(repo.to_string())
    }

    pub fn tag_not_found(tag: &str) -> Self {
        RegistryError::TagNotFound(tag.to_string())
    }

    pub fn network(msg: impl Into<String>) -> Self {
        RegistryError::Network(msg.into())
    }

    pub fn invalid_response(msg: impl Into<String>) -> Self {
        RegistryError::InvalidResponse(msg.into())
    }
}

// Convert ureq error to RegistryError
impl From<ureq::Error> for RegistryError {
    fn from(e: ureq::Error) -> Self {
        RegistryError::Network(e.to_string())
    }
}

// Convert serde_json error to RegistryError
impl From<serde_json::Error> for RegistryError {
    fn from(e: serde_json::Error) -> Self {
        RegistryError::InvalidResponse(e.to_string())
    }
}

// ============================================================================
// Registry Configuration and Info Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    pub endpoint: String,
    pub registry_type: RegistryType,
    pub username: Option<String>,
    pub password: Option<String>,
    pub token: Option<String>,
    pub insecure: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RegistryType {
    DockerHub,
    GCR,
    ECR,
    ACR,
    Harbor,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryInfo {
    pub registry_type: RegistryType,
    pub endpoint: String,
    pub api_version: Option<String>,
    pub supported_formats: Vec<String>,
    pub max_layer_size_mb: Option<u32>,
    pub rate_limits: Option<RegistryRateLimits>,
    pub features: RegistryFeatures,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryRateLimits {
    pub pulls_per_hour: Option<u32>,
    pub pulls_per_day: Option<u32>,
    pub pushes_per_hour: Option<u32>,
    pub pushes_per_day: Option<u32>,
    pub bandwidth_per_hour_mb: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryFeatures {
    pub supports_search: bool,
    pub supports_pagination: bool,
    pub supports_webhooks: bool,
    pub supports_anonymous_pulls: bool,
    pub supports_scope_tokens: bool,
    pub supports_manifest_lists: bool,
}

// ============================================================================
// Image and Repository Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub name: String,
    pub namespace: Option<String>,
    pub description: Option<String>,
    pub is_private: bool,
    pub is_official: Option<bool>,
    pub star_count: Option<i32>,
    pub pull_count: Option<i64>,
    pub last_updated: Option<DateTime<FixedOffset>>,
    pub tags: Vec<Tag>,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub digest: String,
    pub size_bytes: Option<u64>,
    pub last_modified: Option<DateTime<FixedOffset>>,
    pub manifest: Option<ImageManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerImage {
    pub name: String,
    pub tag: String,
    pub digest: String,
    pub manifest: Option<ImageManifest>,
    pub config: Option<ImageConfig>,
    pub size_bytes: Option<u64>,
    pub created_at: Option<DateTime<FixedOffset>>,
    pub last_modified: Option<DateTime<FixedOffset>>,
    pub platform: Option<String>,
    pub os: Option<String>,
    pub architecture: Option<String>,
}

// ============================================================================
// Image Manifest and Config Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    pub config: ImageManifestConfig,
    pub layers: Vec<ImageManifestLayer>,
    #[serde(rename = "mediaType")]
    pub media_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageManifestConfig {
    pub size: u64,
    pub digest: String,
    #[serde(rename = "mediaType")]
    pub media_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageManifestLayer {
    pub size: u64,
    pub digest: String,
    #[serde(rename = "mediaType")]
    pub media_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    pub architecture: Option<String>,
    pub os: Option<String>,
    pub rootfs: Option<serde_json::Value>,
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerData {
    pub digest: String,
    pub size: u64,
    pub data: Vec<u8>,
    pub media_type: String,
}

// ============================================================================
// Operation Result Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct ImagePullResult {
    pub image: ContainerImage,
    pub download_url: Option<String>,
    pub layers: Vec<LayerData>,
    pub total_size: u64,
    pub download_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagePushResult {
    pub digest: String,
    pub size_bytes: u64,
    pub uploaded_at: Option<DateTime<FixedOffset>>,
    pub repository_url: Option<String>,
    pub tag: Option<String>,
}

// ============================================================================
// Container Registry Client Trait
// ============================================================================

#[async_trait]
pub trait ContainerRegistryClient: Send + Sync {
    // Connection and configuration
    async fn initialize(&mut self, config: RegistryConfig) -> Result<()>;
    async fn test_connection(&self) -> Result<bool>;
    async fn get_registry_info(&self) -> Result<RegistryInfo>;

    // Repository operations
    async fn list_repositories(&self) -> Result<Vec<Repository>>;
    async fn search_repositories(&self, query: &str, limit: Option<u32>)
        -> Result<Vec<Repository>>;
    async fn get_repository(&self, namespace: &str, name: &str) -> Result<Option<Repository>>;
    async fn delete_repository(&self, namespace: &str, name: &str) -> Result<()>;

    // Image operations
    async fn image_exists(&self, namespace: &str, name: &str, tag: &str) -> Result<bool>;
    async fn get_image_metadata(
        &self,
        namespace: &str,
        name: &str,
        tag: &str,
    ) -> Result<Option<ContainerImage>>;
    async fn pull_image(&self, namespace: &str, name: &str, tag: &str) -> Result<ImagePullResult>;
    async fn push_image(
        &self,
        image: &ContainerImage,
        layers: Vec<LayerData>,
    ) -> Result<ImagePushResult>;

    // Tag operations
    async fn delete_tag(&self, namespace: &str, name: &str, tag: &str) -> Result<()>;

    // Helper methods for implementations
    async fn list_tags(&self, namespace: &str, name: &str) -> Result<Vec<Tag>> {
        Ok(vec![])
    }

    async fn get_tag(&self, namespace: &str, name: &str, tag: &str) -> Result<Option<Tag>> {
        Ok(None)
    }

    async fn get_image_manifest(
        &self,
        namespace: &str,
        name: &str,
        tag: &str,
    ) -> Result<Option<ImageManifest>> {
        Ok(None)
    }

    async fn get_image_config(
        &self,
        namespace: &str,
        name: &str,
        tag: &str,
    ) -> Result<Option<ImageConfig>> {
        Ok(None)
    }
}
