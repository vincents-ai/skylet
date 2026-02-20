// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use lru::LruCache;
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod metadata;
pub mod abi;
pub mod config;
pub mod event;

pub use metadata::MetadataCache;
pub use abi::AbiCache;
pub use config::ConfigCache;
pub use event::EventCache;

#[derive(Clone)]
pub struct CacheManager {
    pub metadata: MetadataCache,
    pub abi: AbiCache,
    pub config: ConfigCache,
    pub events: EventCache,
}

impl CacheManager {
    pub fn new(
        metadata_capacity: usize,
        abi_capacity: usize,
        config_capacity: usize,
        event_capacity: usize,
    ) -> Self {
        Self {
            metadata: MetadataCache::new(metadata_capacity),
            abi: AbiCache::new(abi_capacity),
            config: ConfigCache::new(config_capacity),
            events: EventCache::new(event_capacity),
        }
    }

    pub async fn clear_all(&self) {
        self.metadata.clear().await;
        self.abi.clear().await;
        self.config.clear().await;
        self.events.clear().await;
    }

    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            metadata_entries: self.metadata.len(),
            abi_entries: self.abi.len(),
            config_entries: self.config.len(),
            event_entries: self.events.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub metadata_entries: usize,
    pub abi_entries: usize,
    pub config_entries: usize,
    pub event_entries: usize,
}

pub struct CacheMetrics {
    hits: Arc<std::sync::atomic::AtomicUsize>,
    misses: Arc<std::sync::atomic::AtomicUsize>,
    evictions: Arc<std::sync::atomic::AtomicUsize>,
}

impl CacheMetrics {
    pub fn new() -> Self {
        Self {
            hits: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            misses: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            evictions: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    pub fn record_hit(&self) {
        self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn record_miss(&self) {
        self.misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn record_eviction(&self) {
        self.evictions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.misses.load(std::sync::atomic::Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }
}

impl Default for CacheMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_manager_creation() {
        let cache = CacheManager::new(100, 200, 50, 1000);
        let stats = cache.cache_stats();
        assert_eq!(stats.metadata_entries, 0);
        assert_eq!(stats.abi_entries, 0);
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let cache = CacheManager::new(10, 10, 10, 10);
        cache.clear_all().await;
        let stats = cache.cache_stats();
        assert_eq!(stats.metadata_entries, 0);
    }

    #[test]
    fn test_cache_metrics() {
        let metrics = CacheMetrics::new();
        metrics.record_hit();
        metrics.record_hit();
        metrics.record_miss();
        
        assert_eq!(metrics.hit_rate(), 2.0 / 3.0);
    }
}
