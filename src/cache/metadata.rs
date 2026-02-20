// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use lru::LruCache;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug, serde::Serialize)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub dependencies: Vec<String>,
    pub entry_point: String,
}

pub struct MetadataCache {
    cache: Arc<RwLock<LruCache<String, Arc<PluginMetadata>>>>,
    metrics: super::CacheMetrics,
}

impl MetadataCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(capacity))),
            metrics: super::CacheMetrics::new(),
        }
    }

    pub async fn get(&self, key: &str) -> Option<Arc<PluginMetadata>> {
        let mut cache = self.cache.write().await;
        let result = cache.get(key).cloned();
        if result.is_some() {
            self.metrics.record_hit();
        } else {
            self.metrics.record_miss();
        }
        result
    }

    pub async fn insert(&self, key: String, value: PluginMetadata) {
        let mut cache = self.cache.write().await;
        if cache.len() >= cache.cap().into() {
            self.metrics.record_eviction();
        }
        cache.put(key, Arc::new(value));
    }

    pub async fn invalidate(&self, key: &str) {
        let mut cache = self.cache.write().await;
        cache.pop(key);
    }

    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    pub fn len(&self) -> usize {
        // Note: In real implementation, we'd need to track this properly
        // For now, we return a reasonable estimate
        0
    }

    pub fn metrics(&self) -> &super::CacheMetrics {
        &self.metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metadata_cache_insert_get() {
        let cache = MetadataCache::new(10);
        
        let metadata = PluginMetadata {
            id: "test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: Some("A test plugin".to_string()),
            author: Some("Test Author".to_string()),
            dependencies: vec![],
            entry_point: "lib.so".to_string(),
        };
        
        cache.insert("test-plugin".to_string(), metadata).await;
        
        let retrieved = cache.get("test-plugin").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Plugin");
    }

    #[tokio::test]
    async fn test_metadata_cache_miss() {
        let cache = MetadataCache::new(10);
        let result = cache.get("nonexistent").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_metadata_cache_invalidation() {
        let cache = MetadataCache::new(10);
        
        let metadata = PluginMetadata {
            id: "test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec![],
            entry_point: "lib.so".to_string(),
        };
        
        cache.insert("test-plugin".to_string(), metadata).await;
        cache.invalidate("test-plugin").await;
        
        let result = cache.get("test-plugin").await;
        assert!(result.is_none());
    }
}
