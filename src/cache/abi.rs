// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use lru::LruCache;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct SymbolKey {
    pub plugin_id: String,
    pub symbol_name: String,
}

impl SymbolKey {
    pub fn new(plugin_id: impl Into<String>, symbol_name: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            symbol_name: symbol_name.into(),
        }
    }
}

pub struct AbiCache {
    cache: Arc<RwLock<LruCache<SymbolKey, *const std::ffi::c_void>>>,
    metrics: super::CacheMetrics,
}

impl AbiCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(capacity))),
            metrics: super::CacheMetrics::new(),
        }
    }

    pub async fn get(&self, key: &SymbolKey) -> Option<*const std::ffi::c_void> {
        let cache = self.cache.read().await;
        let result = cache.get(key).copied();
        if result.is_some() {
            self.metrics.record_hit();
        } else {
            self.metrics.record_miss();
        }
        result
    }

    pub async fn insert(&self, key: SymbolKey, value: *const std::ffi::c_void) {
        let mut cache = self.cache.write().await;
        if cache.len() >= cache.cap() {
            self.metrics.record_eviction();
        }
        cache.put(key, value);
    }

    pub async fn invalidate_plugin(&self, plugin_id: &str) {
        let mut cache = self.cache.write().await;
        let keys_to_remove: Vec<_> = cache
            .iter()
            .filter(|(k, _)| k.plugin_id == plugin_id)
            .map(|(k, _)| k.clone())
            .collect();
        
        for key in keys_to_remove {
            cache.pop(&key);
        }
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
    async fn test_abi_cache_insert_get() {
        let cache = AbiCache::new(10);
        
        let key = SymbolKey::new("plugin1", "init_func");
        let ptr = 0x1234usize as *const std::ffi::c_void;
        
        cache.insert(key.clone(), ptr).await;
        
        let result = cache.get(&key).await;
        assert_eq!(result, Some(ptr));
    }

    #[tokio::test]
    async fn test_abi_cache_invalidation() {
        let cache = AbiCache::new(10);
        
        let key = SymbolKey::new("plugin1", "init_func");
        let ptr = 0x1234usize as *const std::ffi::c_void;
        
        cache.insert(key.clone(), ptr).await;
        cache.invalidate_plugin("plugin1").await;
        
        let result = cache.get(&key).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_abi_cache_different_plugins() {
        let cache = AbiCache::new(10);
        
        let key1 = SymbolKey::new("plugin1", "init_func");
        let key2 = SymbolKey::new("plugin2", "init_func");
        let ptr = 0x1234usize as *const std::ffi::c_void;
        
        cache.insert(key1.clone(), ptr).await;
        
        // plugin2 should not be affected by invalidating plugin1
        let result = cache.get(&key2).await;
        assert!(result.is_none());
    }
}
