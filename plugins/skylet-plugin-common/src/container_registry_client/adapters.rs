// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

// Container registry adapters for different registry implementations
// Provides implementations for Docker Hub, GCR, ECR, ACR, and other registries
use super::*;
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::io::Read;
use base64::Engine;

// Docker Hub client
pub struct DockerHubClient {
    config: Option<RegistryConfig>,
    token: Option<String>,
    base_url: String,
}

impl DockerHubClient {
    pub fn new() -> Self {
        Self {
            config: None,
            token: None,
            base_url: "https://registry-1.docker.io/v2".to_string(),
        }
    }

    async fn authenticate(&mut self, username: &str, password: &str) -> Result<()> {
        let auth_url = "https://hub.docker.com/v2/users/login";
        let agent = ureq::AgentBuilder::new().build();
        
        let auth_body = json!({
            "username": username,
            "password": password
        });
        
        let auth_body_str = serde_json::to_string(&auth_body)
            .map_err(|e| RegistryError::api(format!("Failed to serialize auth body: {}", e)))?;
        
        let response = agent.post(auth_url)
            .set("Content-Type", "application/json")
            .send_string(&auth_body_str)
            .map_err(|e| RegistryError::network(e.to_string()))?;
        
        let result: serde_json::Value = serde_json::from_reader(response.into_reader())
            .map_err(|e| RegistryError::api(format!("Failed to parse auth response: {}", e)))?;
        
        if let Some(token) = result["token"].as_str() {
            self.token = Some(token.to_string());
            Ok(())
        } else {
            Err(RegistryError::authentication("Invalid Docker Hub credentials"))
        }
    }

    fn build_request(&self, path: &str) -> Result<ureq::Request> {
        let agent = ureq::AgentBuilder::new().build();
        let url = format!("{}{}", self.base_url, path);
        
        let request = agent.get(&url);
        
        if let Some(ref token) = self.token {
            Ok(request.set("Authorization", &format!("Bearer {}", token)))
        } else {
            Ok(request)
        }
    }
}

#[async_trait]
impl ContainerRegistryClient for DockerHubClient {
    async fn initialize(&mut self, config: RegistryConfig) -> Result<()> {
        self.config = Some(config.clone());
        
        // Authenticate if credentials provided
        if let (Some(username), Some(password)) = (&config.username, &config.password) {
            self.authenticate(username, password).await?;
        } else if let Some(ref token) = config.token {
            self.token = Some(token.clone());
        }
        
        Ok(())
    }

    async fn test_connection(&self) -> Result<bool> {
        let request = self.build_request("/")?;
        match request.call() {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn list_repositories(&self) -> Result<Vec<Repository>> {
        // Docker Hub doesn't have a simple list all repositories endpoint
        // This would require pagination through search or username-specific endpoints
        Ok(vec![])
    }

    async fn search_repositories(&self, query: &str, limit: Option<u32>) -> Result<Vec<Repository>> {
        let search_url = format!("https://hub.docker.com/v2/search/repositories?q={}&page_size={}", 
            urlencoding::encode(query),
            limit.unwrap_or(25)
        );
        
        let agent = ureq::AgentBuilder::new().build();
        let response = agent.get(&search_url).call()?;
        let result: serde_json::Value = serde_json::from_reader(response.into_reader())
            .map_err(|e| RegistryError::api(format!("Failed to parse search response: {}", e)))?;
        
        let repositories: Vec<Repository> = result["results"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|repo| Repository {
                name: repo["name"].as_str().unwrap_or("").to_string(),
                namespace: repo["namespace"].as_str().map(|s| s.to_string()),
                description: repo["description"].as_str().map(|s| s.to_string()),
                is_private: repo["is_private"].as_bool().unwrap_or(false),
                is_official: repo["is_official"].as_bool(),
                star_count: repo["star_count"].as_i64().map(|i| i as i32),
                pull_count: repo["pull_count"].as_i64(),
                last_updated: repo["updated"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()),
                tags: vec![], // Would require separate API call
                size_bytes: None,
            })
            .collect();
        
        Ok(repositories)
    }

    async fn get_repository(&self, namespace: &str, name: &str) -> Result<Option<Repository>> {
        let search_url = format!("https://hub.docker.com/v2/repositories/{}/{}", namespace, name);
        
        let agent = ureq::AgentBuilder::new().build();
        let response = agent.get(&search_url).call()?;
        let result: serde_json::Value = serde_json::from_reader(response.into_reader())
            .map_err(|e| RegistryError::api(format!("Failed to parse repository response: {}", e)))?;
        
        if result["user"].is_null() {
            return Ok(None);
        }
        
        let repo = &result;
        Ok(Some(Repository {
            name: name.to_string(),
            namespace: Some(namespace.to_string()),
            description: repo["description"].as_str().map(|s| s.to_string()),
            is_private: repo["is_private"].as_bool().unwrap_or(false),
            is_official: repo["is_official"].as_bool(),
            star_count: repo["star_count"].as_i64().map(|i| i as i32),
            pull_count: repo["pull_count"].as_i64(),
            last_updated: repo["updated"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()),
            tags: vec![], // Would require separate API call
            size_bytes: None,
        }))
    }

    async fn list_tags(&self, namespace: &str, name: &str) -> Result<Vec<Tag>> {
        let tags_url = format!("/{}/tags/list", urlencoding::encode(&format!("{}/{}", namespace, name)));
        
        let request = self.build_request(&tags_url)?;
        let response = request.call()?;
        let result: serde_json::Value = serde_json::from_reader(response.into_reader())
            .map_err(|e| RegistryError::api(format!("Failed to parse tags response: {}", e)))?;
        
        let tags: Vec<Tag> = result["tags"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|tag| Tag {
                name: tag.as_str().unwrap_or("").to_string(),
                digest: "".to_string(), // Docker Hub doesn't provide digest in list
                size_bytes: None,
                last_modified: None,
                manifest: None,
            })
            .collect();
        
        Ok(tags)
    }

    async fn get_tag(&self, namespace: &str, name: &str, tag: &str) -> Result<Option<Tag>> {
        let manifest_url = format!("/{}/manifests/{}", 
            urlencoding::encode(&format!("{}/{}", namespace, name)), 
            urlencoding::encode(tag)
        );
        
        let request = self.build_request(&manifest_url)?;
        let response = request.call()?;
        
        if response.status() == 404 {
            return Ok(None);
        }
        
        let digest = response.header("Docker-Content-Digest")
            .and_then(|h| h.split(';').next())
            .unwrap_or("")
            .to_string();
        
        Ok(Some(Tag {
            name: tag.to_string(),
            digest,
            size_bytes: None,
            last_modified: None,
            manifest: None, // Would need to parse response body
        }))
    }

    async fn get_image_manifest(&self, namespace: &str, name: &str, tag: &str) -> Result<Option<ImageManifest>> {
        let manifest_url = format!("/{}/manifests/{}", 
            urlencoding::encode(&format!("{}/{}", namespace, name)), 
            urlencoding::encode(tag)
        );
        
        let request = self.build_request(&manifest_url)?
            .set("Accept", "application/vnd.docker.distribution.manifest.v2+json");
        
        let response = request.call()?;
        
        if response.status() == 404 {
            return Ok(None);
        }
        
        let manifest: ImageManifest = serde_json::from_reader(response.into_reader())
            .map_err(|e| RegistryError::api(format!("Failed to parse manifest: {}", e)))?;
        Ok(Some(manifest))
    }

    async fn get_image_config(&self, namespace: &str, name: &str, tag: &str) -> Result<Option<ImageConfig>> {
        // First get the manifest to find the config digest
        let manifest = self.get_image_manifest(namespace, name, tag).await?;
        if let Some(ref manifest) = manifest {
            let config_url = format!("/blobs/{}", manifest.config.digest);
            let request = self.build_request(&config_url)?;
            
            let response = request.call()?;
            if response.status() == 404 {
                return Ok(None);
            }
            
            let config: ImageConfig = serde_json::from_reader(response.into_reader())
                .map_err(|e| RegistryError::api(format!("Failed to parse config: {}", e)))?;
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }

    async fn pull_image(&self, namespace: &str, name: &str, tag: &str) -> Result<ImagePullResult> {
        let manifest = self.get_image_manifest(namespace, name, tag).await?;
        if let Some(ref manifest) = manifest {
            let mut layers = Vec::new();
            let mut total_size = 0u64;
            
            for layer in &manifest.layers {
                let layer_url = format!("/blobs/{}", layer.digest);
                let request = self.build_request(&layer_url)?;
                let response = request.call()?;
                
                if response.status() != 200 {
                    return Err(RegistryError::api(format!("Failed to download layer: {}", layer.digest)));
                }
                
                let mut layer_data = Vec::new();
                response.into_reader().read_to_end(&mut layer_data)
                    .map_err(|e| RegistryError::network(format!("Failed to read layer data: {}", e)))?;
                total_size += layer_data.len() as u64;
                
                layers.push(LayerData {
                    digest: layer.digest.clone(),
                    size: layer.size,
                    data: layer_data,
                    media_type: layer.media_type.clone(),
                });
            }
            
            let image = ContainerImage {
                name: name.to_string(),
                tag: tag.to_string(),
                digest: "".to_string(), // Would calculate from manifest
                manifest: Some(manifest.clone()),
                config: None,
                size_bytes: Some(total_size),
                created_at: None,
                last_modified: None,
                platform: None,
                os: None,
                architecture: None,
            };
            
            Ok(ImagePullResult {
                image,
                download_url: None, // Would generate temporary URL
                layers,
                total_size,
                download_time_ms: 0, // Would track timing
            })
        } else {
            Err(RegistryError::image_not_found(name, tag))
        }
    }

    async fn push_image(&self, image: &ContainerImage, layers: Vec<LayerData>) -> Result<ImagePushResult> {
        // Docker Hub V2 API push implementation
        // Steps: 1. Upload layers (blobs), 2. Upload config, 3. Upload manifest
        
        let namespace = image.name.split('/').next().unwrap_or("library");
        let repo_name = image.name.split('/').last().unwrap_or(&image.name);
        
        if let Some(ref manifest) = image.manifest {
            // Upload config first
            let config_digest = &manifest.config.digest;
            let config_url = format!("/v2/{}/{}/blobs/{}", 
                urlencoding::encode(namespace),
                urlencoding::encode(repo_name),
                urlencoding::encode(config_digest)
            );
            
            let request = self.build_request(&config_url)?;
            let head_response = request.clone().call();
            
            // Only upload if config blob doesn't exist
            if head_response.is_err() || head_response.as_ref().map(|r| r.status() != 200).unwrap_or(true) {
                let upload_url = format!("/v2/{}/{}/blobs/uploads/", 
                    urlencoding::encode(namespace),
                    urlencoding::encode(repo_name)
                );
                
                let request = self.build_request(&upload_url)?;
                let response = request.call()?;
                
                if response.status() != 202 {
                    return Err(RegistryError::api("Failed to initiate blob upload".to_string()));
                }
                
                let location = response.header("Location")
                    .ok_or_else(|| RegistryError::api("Missing Location header in upload response".to_string()))?
                    .to_string();
                
                // Upload the config blob
                let config_json = serde_json::to_vec(&manifest.config)
                    .map_err(|e| RegistryError::api(format!("Failed to serialize config: {}", e)))?;
                
                let agent = ureq::AgentBuilder::new().build();
                let upload_request = agent.post(&location)
                    .set("Content-Type", "application/octet-stream");
                
                let upload_response = upload_request.send_bytes(&config_json)?;
                
                if upload_response.status() != 201 {
                    return Err(RegistryError::api(format!("Failed to upload config: {}", upload_response.status())));
                }
            }
            
            // Upload layers
            for layer in &layers {
                let layer_url = format!("/v2/{}/{}/blobs/{}", 
                    urlencoding::encode(namespace),
                    urlencoding::encode(repo_name),
                    urlencoding::encode(&layer.digest)
                );
                
                let request = self.build_request(&layer_url)?;
                let head_response = request.clone().call();
                
                // Only upload if layer blob doesn't exist
                if head_response.is_err() || head_response.as_ref().map(|r| r.status() != 200).unwrap_or(true) {
                    let upload_url = format!("/v2/{}/{}/blobs/uploads/", 
                        urlencoding::encode(namespace),
                        urlencoding::encode(repo_name)
                    );
                    
                    let request = self.build_request(&upload_url)?;
                    let response = request.call()?;
                    
                    if response.status() != 202 {
                        return Err(RegistryError::api("Failed to initiate layer upload".to_string()));
                    }
                    
                    let location = response.header("Location")
                        .ok_or_else(|| RegistryError::api("Missing Location header".to_string()))?
                        .to_string();
                    
                    // Upload the layer blob
                    let agent = ureq::AgentBuilder::new().build();
                    let upload_request = agent.post(&location)
                        .set("Content-Type", "application/octet-stream");
                    
                    let upload_response = upload_request.send_bytes(&layer.data)?;
                    
                    if upload_response.status() != 201 {
                        return Err(RegistryError::api(format!("Failed to upload layer: {}", upload_response.status())));
                    }
                }
            }
            
            // Upload manifest
            let manifest_url = format!("/v2/{}/{}/manifests/{}", 
                urlencoding::encode(namespace),
                urlencoding::encode(repo_name),
                urlencoding::encode(&image.tag)
            );
            
            let manifest_json = serde_json::to_string(manifest)
                .map_err(|e| RegistryError::api(format!("Failed to serialize manifest: {}", e)))?;
            
            let request = self.build_request(&manifest_url)?
                .set("Content-Type", "application/vnd.docker.distribution.manifest.v2+json");
            
            let response = request.send_string(&manifest_json)?;
            
            if response.status() != 201 && response.status() != 200 {
                return Err(RegistryError::api(format!("Failed to upload manifest: {}", response.status())));
            }
            
            let digest = response.header("Docker-Content-Digest")
                .unwrap_or("")
                .to_string();
            
            Ok(ImagePushResult {
                digest,
                size_bytes: layers.iter().map(|l| l.size).sum::<u64>() + manifest.config.size,
                uploaded_at: Some(chrono::Utc::now().fixed_offset()),
                repository_url: Some(format!("{}/{}", self.base_url, image.name)),
                tag: Some(image.tag.clone()),
            })
        } else {
            Err(RegistryError::api("Image manifest is required for push".to_string()))
        }
    }

    async fn delete_tag(&self, namespace: &str, name: &str, tag: &str) -> Result<()> {
        let manifest_url = format!("/{}/manifests/{}", 
            urlencoding::encode(&format!("{}/{}", namespace, name)), 
            urlencoding::encode(tag)
        );
        
        let request = self.build_request(&manifest_url)?;
        let response = request.call()?;
        
        if response.status() == 404 {
            return Err(RegistryError::tag_not_found(tag));
        }
        
        if response.status() == 202 || response.status() == 204 {
            Ok(())
        } else {
            Err(RegistryError::api(format!("Failed to delete tag: {}", response.status())))
        }
    }

    async fn delete_repository(&self, namespace: &str, name: &str) -> Result<()> {
        let repo_url = format!("https://hub.docker.com/v2/repositories/{}/{}", namespace, name);
        
        let agent = ureq::AgentBuilder::new().build();
        let response = agent.delete(&repo_url).call()?;
        
        if response.status() == 404 {
            return Err(RegistryError::repository_not_found(&format!("{}/{}", namespace, name)));
        }
        
        if response.status() == 202 || response.status() == 204 {
            Ok(())
        } else {
            Err(RegistryError::api(format!("Failed to delete repository: {}", response.status())))
        }
    }

    async fn get_registry_info(&self) -> Result<RegistryInfo> {
        Ok(RegistryInfo {
            registry_type: RegistryType::DockerHub,
            endpoint: self.base_url.clone(),
            api_version: Some("v2".to_string()),
            supported_formats: vec!["docker".to_string(), "oci".to_string()],
            max_layer_size_mb: Some(100), // Docker Hub limit
            rate_limits: Some(RegistryRateLimits {
                pulls_per_hour: Some(200),
                pulls_per_day: None,
                pushes_per_hour: Some(200),
                pushes_per_day: None,
                bandwidth_per_hour_mb: None,
            }),
            features: RegistryFeatures {
                supports_search: true,
                supports_pagination: true,
                supports_webhooks: false,
                supports_anonymous_pulls: true,
                supports_scope_tokens: false,
                supports_manifest_lists: true,
            },
        })
    }

    async fn image_exists(&self, namespace: &str, name: &str, tag: &str) -> Result<bool> {
        match self.get_tag(namespace, name, tag).await {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(_) => Ok(false),
        }
    }

    async fn get_image_metadata(&self, namespace: &str, name: &str, tag: &str) -> Result<Option<ContainerImage>> {
        let tag_info = self.get_tag(namespace, name, tag).await?;
        if let Some(tag_obj) = tag_info {
            let image = ContainerImage {
                name: name.to_string(),
                tag: tag_obj.name.clone(),
                digest: tag_obj.digest.clone(),
                manifest: tag_obj.manifest.clone(),
                config: None,
                size_bytes: tag_obj.size_bytes,
                created_at: None,
                last_modified: tag_obj.last_modified,
                platform: None,
                os: None,
                architecture: None,
            };
            Ok(Some(image))
        } else {
            Ok(None)
        }
    }
}

// Generic OCI registry client (for GCR, ECR, ACR, Harbor)
pub struct GenericOCIRegistryClient {
    config: Option<RegistryConfig>,
    token: Option<String>,
    base_url: String,
    authorization_header: Option<String>,
}

impl GenericOCIRegistryClient {
    pub fn new(base_url: String) -> Self {
        Self {
            config: None,
            token: None,
            base_url,
            authorization_header: None,
        }
    }

    fn build_request(&self, path: &str) -> Result<ureq::Request> {
        let agent = ureq::AgentBuilder::new().build();
        let url = format!("{}{}", self.base_url, path);
        let mut request = agent.get(&url);
        
        if let Some(ref auth_header) = self.authorization_header {
            request = request.set("Authorization", auth_header);
        }
        
        Ok(request)
    }

    async fn get_auth_token(&mut self, config: &RegistryConfig) -> Result<()> {
        // Generic OCI registry authentication
        if let Some(ref token) = config.token {
            self.token = Some(token.clone());
            self.authorization_header = Some(format!("Bearer {}", token));
            return Ok(());
        }
        
        if let (Some(ref username), Some(ref password)) = (&config.username, &config.password) {
            // Basic auth for most registries
            let auth_string = format!("{}:{}", username, password);
            let auth_encoded = base64::engine::general_purpose::STANDARD.encode(&auth_string);
            self.authorization_header = Some(format!("Basic {}", auth_encoded));
            return Ok(());
        }
        
        Err(RegistryError::configuration("Authentication credentials required"))
    }
}

#[async_trait]
impl ContainerRegistryClient for GenericOCIRegistryClient {
    async fn initialize(&mut self, config: RegistryConfig) -> Result<()> {
        self.config = Some(config.clone());
        self.get_auth_token(&config).await?;
        Ok(())
    }

    async fn test_connection(&self) -> Result<bool> {
        let request = self.build_request("/v2/")?;
        match request.call() {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn list_repositories(&self) -> Result<Vec<Repository>> {
        let request = self.build_request("/v2/_catalog")?;
        let response = request.call()?;
        let result: serde_json::Value = serde_json::from_reader(response.into_reader())
            .map_err(|e| RegistryError::api(format!("Failed to parse catalog response: {}", e)))?;
        
        let repositories: Vec<Repository> = result["repositories"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|repo| Repository {
                name: repo.as_str().unwrap_or("").to_string(),
                namespace: None,
                description: None,
                is_private: false,
                is_official: Some(false),
                star_count: None,
                pull_count: None,
                last_updated: None,
                tags: vec![],
                size_bytes: None,
            })
            .collect();
        
        Ok(repositories)
    }

    async fn search_repositories(&self, _query: &str, _limit: Option<u32>) -> Result<Vec<Repository>> {
        // Most OCI registries don't support search
        Ok(vec![])
    }

    async fn get_repository(&self, _namespace: &str, _name: &str) -> Result<Option<Repository>> {
        // OCI registries typically don't have repository metadata
        Ok(None)
    }

    async fn list_tags(&self, namespace: &str, name: &str) -> Result<Vec<Tag>> {
        let tags_url = format!("/v2/{}/tags/list", urlencoding::encode(&format!("{}/{}", namespace, name)));
        
        let request = self.build_request(&tags_url)?;
        let response = request.call()?;
        let result: serde_json::Value = serde_json::from_reader(response.into_reader())
            .map_err(|e| RegistryError::api(format!("Failed to parse tags response: {}", e)))?;
        
        let tags: Vec<Tag> = result["tags"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|tag| Tag {
                name: tag.as_str().unwrap_or("").to_string(),
                digest: "".to_string(),
                size_bytes: None,
                last_modified: None,
                manifest: None,
            })
            .collect();
        
        Ok(tags)
    }

    async fn get_tag(&self, namespace: &str, name: &str, tag: &str) -> Result<Option<Tag>> {
        let manifest_url = format!("/v2/{}/manifests/{}", 
            urlencoding::encode(&format!("{}/{}", namespace, name)), 
            urlencoding::encode(tag)
        );
        
        let request = self.build_request(&manifest_url)?
            .set("Accept", "application/vnd.docker.distribution.manifest.v2+json");
        
        let response = request.call()?;
        
        if response.status() == 404 {
            return Ok(None);
        }
        
        let digest = response.header("Docker-Content-Digest")
            .and_then(|h| h.split(';').next())
            .unwrap_or("")
            .to_string();
        
        Ok(Some(Tag {
            name: tag.to_string(),
            digest,
            size_bytes: None,
            last_modified: None,
            manifest: None,
        }))
    }

    async fn get_image_manifest(&self, namespace: &str, name: &str, tag: &str) -> Result<Option<ImageManifest>> {
        let manifest_url = format!("/v2/{}/manifests/{}", 
            urlencoding::encode(&format!("{}/{}", namespace, name)), 
            urlencoding::encode(tag)
        );
        
        let request = self.build_request(&manifest_url)?
            .set("Accept", "application/vnd.docker.distribution.manifest.v2+json");
        
        let response = request.call()?;
        
        if response.status() == 404 {
            return Ok(None);
        }
        
        let manifest: ImageManifest = serde_json::from_reader(response.into_reader())
            .map_err(|e| RegistryError::api(format!("Failed to parse manifest: {}", e)))?;
        Ok(Some(manifest))
    }

    async fn get_image_config(&self, namespace: &str, name: &str, tag: &str) -> Result<Option<ImageConfig>> {
        let manifest = self.get_image_manifest(namespace, name, tag).await?;
        if let Some(ref manifest) = manifest {
            let config_url = format!("/v2/blobs/{}", manifest.config.digest);
            let request = self.build_request(&config_url)?;
            
            let response = request.call()?;
            if response.status() == 404 {
                return Ok(None);
            }
            
            let config: ImageConfig = serde_json::from_reader(response.into_reader())
                .map_err(|e| RegistryError::api(format!("Failed to parse config: {}", e)))?;
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }

    async fn pull_image(&self, namespace: &str, name: &str, tag: &str) -> Result<ImagePullResult> {
        // OCI Registry V2 API pull implementation
        let manifest = self.get_image_manifest(namespace, name, tag).await?;
        if let Some(ref manifest) = manifest {
            let mut layers = Vec::new();
            let mut total_size = 0u64;
            
            for layer in &manifest.layers {
                let layer_url = format!("/v2/{}/blobs/{}", 
                    urlencoding::encode(&format!("{}/{}", namespace, name)), 
                    urlencoding::encode(&layer.digest));
                let request = self.build_request(&layer_url)?;
                let response = request.call()?;
                
                if response.status() != 200 {
                    return Err(RegistryError::api(format!("Failed to download layer: {}", layer.digest)));
                }
                
                let mut layer_data = Vec::new();
                response.into_reader().read_to_end(&mut layer_data)
                    .map_err(|e| RegistryError::network(format!("Failed to read layer data: {}", e)))?;
                total_size += layer_data.len() as u64;
                
                layers.push(LayerData {
                    digest: layer.digest.clone(),
                    size: layer.size,
                    data: layer_data,
                    media_type: layer.media_type.clone(),
                });
            }
            
            let image = ContainerImage {
                name: name.to_string(),
                tag: tag.to_string(),
                digest: "".to_string(), // Would calculate from manifest
                manifest: Some(manifest.clone()),
                config: None,
                size_bytes: Some(total_size),
                created_at: None,
                last_modified: None,
                platform: None,
                os: None,
                architecture: None,
            };
            
            Ok(ImagePullResult {
                image,
                download_url: None, // Would generate temporary URL
                layers,
                total_size,
                download_time_ms: 0, // Would track timing
            })
        } else {
            Err(RegistryError::image_not_found(name, tag))
        }
    }

    async fn push_image(&self, image: &ContainerImage, layers: Vec<LayerData>) -> Result<ImagePushResult> {
        // Generic OCI Registry V2 API push implementation
        // Works with GCR, ECR, ACR, Harbor, and other OCI-compliant registries
        
        if let Some(ref manifest) = image.manifest {
            // Upload config blob first
            let config_digest = &manifest.config.digest;
            let config_url = format!("/v2/{}/{}/blobs/{}", 
                urlencoding::encode(&format!("{}/{}", image.name, "")), 
                urlencoding::encode(&image.name),
                urlencoding::encode(config_digest)
            );
            
            let request = self.build_request(&config_url)?;
            let head_response = request.clone().call();
            
            // Only upload if config blob doesn't exist
            if head_response.is_err() || head_response.as_ref().map(|r| r.status() != 200).unwrap_or(true) {
                let upload_url = format!("/v2/{}/blobs/uploads/", 
                    urlencoding::encode(&image.name));
                
                let request = self.build_request(&upload_url)?;
                let response = request.call()?;
                
                if response.status() != 202 {
                    return Err(RegistryError::api("Failed to initiate config blob upload".to_string()));
                }
                
                let location = response.header("Location")
                    .ok_or_else(|| RegistryError::api("Missing Location header".to_string()))?
                    .to_string();
                
                // Upload config blob
                let config_json = serde_json::to_vec(&manifest.config)
                    .map_err(|e| RegistryError::api(format!("Failed to serialize config: {}", e)))?;
                
                let agent = ureq::AgentBuilder::new().build();
                let upload_request = agent.post(&location)
                    .set("Content-Type", "application/octet-stream");
                
                let upload_response = upload_request.send_bytes(&config_json)?;
                
                if upload_response.status() != 201 {
                    return Err(RegistryError::api(format!("Failed to upload config: {}", upload_response.status())));
                }
            }
            
            // Upload layer blobs
            for layer in &layers {
                let layer_url = format!("/v2/{}/blobs/{}", 
                    urlencoding::encode(&image.name),
                    urlencoding::encode(&layer.digest)
                );
                
                let request = self.build_request(&layer_url)?;
                let head_response = request.clone().call();
                
                // Only upload if layer blob doesn't exist
                if head_response.is_err() || head_response.as_ref().map(|r| r.status() != 200).unwrap_or(true) {
                    let upload_url = format!("/v2/{}/blobs/uploads/", 
                        urlencoding::encode(&image.name));
                    
                    let request = self.build_request(&upload_url)?;
                    let response = request.call()?;
                    
                    if response.status() != 202 {
                        return Err(RegistryError::api("Failed to initiate layer upload".to_string()));
                    }
                    
                    let location = response.header("Location")
                        .ok_or_else(|| RegistryError::api("Missing Location header".to_string()))?
                        .to_string();
                    
                    // Upload layer blob
                    let agent = ureq::AgentBuilder::new().build();
                    let upload_request = agent.post(&location)
                        .set("Content-Type", "application/octet-stream");
                    
                    let upload_response = upload_request.send_bytes(&layer.data)?;
                    
                    if upload_response.status() != 201 {
                        return Err(RegistryError::api(format!("Failed to upload layer: {}", upload_response.status())));
                    }
                }
            }
            
            // Upload manifest
            let manifest_url = format!("/v2/{}/manifests/{}", 
                urlencoding::encode(&image.name),
                urlencoding::encode(&image.tag)
            );
            
            let manifest_json = serde_json::to_string(manifest)
                .map_err(|e| RegistryError::api(format!("Failed to serialize manifest: {}", e)))?;
            
            let request = self.build_request(&manifest_url)?
                .set("Content-Type", "application/vnd.docker.distribution.manifest.v2+json");
            
            let response = request.send_string(&manifest_json)?;
            
            if response.status() != 201 && response.status() != 200 {
                return Err(RegistryError::api(format!("Failed to upload manifest: {}", response.status())));
            }
            
            let digest = response.header("Docker-Content-Digest")
                .unwrap_or("")
                .to_string();
            
            Ok(ImagePushResult {
                digest,
                size_bytes: layers.iter().map(|l| l.size).sum::<u64>() + manifest.config.size,
                uploaded_at: Some(chrono::Utc::now().fixed_offset()),
                repository_url: Some(format!("{}/{}", self.base_url, image.name)),
                tag: Some(image.tag.clone()),
            })
        } else {
            Err(RegistryError::api("Image manifest is required for push".to_string()))
        }
    }

    async fn delete_tag(&self, namespace: &str, name: &str, tag: &str) -> Result<()> {
        let manifest_url = format!("/v2/{}/manifests/{}", 
            urlencoding::encode(&format!("{}/{}", namespace, name)), 
            urlencoding::encode(tag)
        );
        
        let request = self.build_request(&manifest_url)?;
        let response = request.call()?;
        
        if response.status() == 404 {
            return Err(RegistryError::tag_not_found(tag));
        }
        
        if response.status() == 202 || response.status() == 204 {
            Ok(())
        } else {
            Err(RegistryError::api(format!("Failed to delete tag: {}", response.status())))
        }
    }

    async fn delete_repository(&self, namespace: &str, name: &str) -> Result<()> {
        let repo_url = format!("/v2/{}", urlencoding::encode(&format!("{}/{}", namespace, name)));
        
        let request = self.build_request(&repo_url)?;
        let response = request.call()?;
        
        if response.status() == 404 {
            return Err(RegistryError::repository_not_found(&format!("{}/{}", namespace, name)));
        }
        
        if response.status() == 202 || response.status() == 204 {
            Ok(())
        } else {
            Err(RegistryError::api(format!("Failed to delete repository: {}", response.status())))
        }
    }

    async fn get_registry_info(&self) -> Result<RegistryInfo> {
        Ok(RegistryInfo {
            registry_type: RegistryType::Custom("Generic OCI Registry".to_string()),
            endpoint: self.base_url.clone(),
            api_version: Some("v2".to_string()),
            supported_formats: vec!["oci".to_string(), "docker".to_string()],
            max_layer_size_mb: None, // Varies by registry
            rate_limits: None, // Registry-specific
            features: RegistryFeatures {
                supports_search: false,
                supports_pagination: true,
                supports_webhooks: false,
                supports_anonymous_pulls: true,
                supports_scope_tokens: false,
                supports_manifest_lists: true,
            },
        })
    }

    async fn image_exists(&self, namespace: &str, name: &str, tag: &str) -> Result<bool> {
        match self.get_tag(namespace, name, tag).await {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(_) => Ok(false),
        }
    }

    async fn get_image_metadata(&self, namespace: &str, name: &str, tag: &str) -> Result<Option<ContainerImage>> {
        let tag_info = self.get_tag(namespace, name, tag).await?;
        if let Some(tag_obj) = tag_info {
            let image = ContainerImage {
                name: name.to_string(),
                tag: tag_obj.name.clone(),
                digest: tag_obj.digest.clone(),
                manifest: tag_obj.manifest.clone(),
                config: None,
                size_bytes: tag_obj.size_bytes,
                created_at: None,
                last_modified: tag_obj.last_modified,
                platform: None,
                os: None,
                architecture: None,
            };
            Ok(Some(image))
        } else {
            Ok(None)
        }
    }
}

impl Default for DockerHubClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for GenericOCIRegistryClient {
    fn default() -> Self {
        Self::new("https://registry.example.com/v2".to_string())
    }
}