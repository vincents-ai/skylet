// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct CachedConfig {
    pub value: serde_json::Value,
    pub cached_at: Instant,
    pub ttl: Duration,
}

impl CachedConfig {
    pub fn new(value: serde_json::Value, ttl: Duration) -> Self {
        Self {
            value,
            cached_at: Instant::now(),
            ttl,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.cached_at.elapsed() > self.ttl
    }
}

pub struct ConfigCache {
    cache: Arc<RwLock<HashMap<String, CachedConfig>>>,
    default_ttl: Duration,
    metrics: super::CacheMetrics,
}

impl ConfigCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: Duration::from_secs(60),
            metrics: super::CacheMetrics::new(),
        }
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = ttl;
        self
    }

    pub async fn get(&self, key: &str) -> Option<serde_json::Value> {
        let cache = self.cache.read().await;
        
        if let Some(cached) = cache.get(key) {
            if cached.is_expired() {
                drop(cache);
                self.cache.write().await.remove(key);
                self.metrics.record_miss();
                return None;
            }
            self.metrics.record_hit();
            Some(cached.value.clone())
        } else {
            self.metrics.record_miss();
            None
        }
    }

    pub async fn insert(&self, key: String, value: serde_json::Value) {
        let mut cache = self.cache.write().await;
        
        if cache.len() >= 100 {
            self.metrics.record_eviction();
        }
        
        cache.insert(key, CachedConfig::new(value, self.default_ttl));
    }

    pub async fn insert_with_ttl(&self, key: String, value: serde_json::Value, ttl: Duration) {
        let mut cache = self.cache.write().await;
        
        if cache.len() >= 100 {
            self.metrics.record_eviction();
        }
        
        cache.insert(key, CachedConfig::new(value, ttl));
    }

    pub async fn invalidate(&self, key: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(key);
    }

    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    pub fn len(&self) -> usize {
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
    async fn test_config_cache_insert_get() {
        let cache = ConfigCache::new(10);
        
        let config = serde_json::json!({"key": "value"});
        cache.insert("test-config".to_string(), config.clone()).await;
        
        let result = cache.get("test-config").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap(), config);
    }

    #[tokio::test]
    async fn test_config_cache_expiration() {
        let cache = ConfigCache::new(10).with_ttl(Duration::from_millis(10));
        
        let config = serde_json::json!({"key": "value"});
        cache.insert("test-config".to_string(), config).await;
        
        tokio::time::sleep(Duration::from_millis(20)).await;
        
        let result = cache.get("test-config").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_config_cache_invalidation() {
        let cache = ConfigCache::new(10);
        
        let config = serde_json::json!({"key": "value"});
        cache.insert("test-config".to_string(), config).await;
        cache.invalidate("test-config").await;
        
        let result = cache.get("test-config").await;
        assert!(result.is_none());
    }
}
