// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct CachedEvent {
    pub event_type: String,
    pub payload: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct EventCache {
    buffer: Arc<RwLock<VecDeque<CachedEvent>>>,
    capacity: usize,
    metrics: super::CacheMetrics,
}

impl EventCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Arc::new(RwLock::new(VecDeque::with_capacity(capacity))),
            capacity,
            metrics: super::CacheMetrics::new(),
        }
    }

    pub async fn push(&self, event_type: impl Into<String>, payload: serde_json::Value) {
        let event = CachedEvent {
            event_type: event_type.into(),
            payload,
            timestamp: chrono::Utc::now(),
        };

        let mut buffer = self.buffer.write().await;
        
        if buffer.len() >= self.capacity {
            buffer.pop_front();
            self.metrics.record_eviction();
        }
        
        buffer.push_back(event);
    }

    pub async fn get_recent(&self, count: usize) -> Vec<CachedEvent> {
        let buffer = self.buffer.read().await;
        buffer.iter().rev().take(count).cloned().collect()
    }

    pub async fn get_by_type(&self, event_type: &str, count: usize) -> Vec<CachedEvent> {
        let buffer = self.buffer.read().await;
        buffer
            .iter()
            .rev()
            .filter(|e| e.event_type == event_type)
            .take(count)
            .cloned()
            .collect()
    }

    pub async fn clear(&self) {
        let mut buffer = self.buffer.write().await;
        buffer.clear();
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
    async fn test_event_cache_push() {
        let cache = EventCache::new(10);
        
        cache.push("test-event", serde_json::json!({"data": 123})).await;
        
        let recent = cache.get_recent(1).await;
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].event_type, "test-event");
    }

    #[tokio::test]
    async fn test_event_cache_overflow() {
        let cache = EventCache::new(3);
        
        for i in 0..5 {
            cache.push(format!("event-{}", i), serde_json::json!({"id": i})).await;
        }
        
        let recent = cache.get_recent(10).await;
        assert_eq!(recent.len(), 3);
    }

    #[tokio::test]
    async fn test_event_cache_filter_by_type() {
        let cache = EventCache::new(10);
        
        cache.push("type-a", serde_json::json!({"id": 1})).await;
        cache.push("type-b", serde_json::json!({"id": 2})).await;
        cache.push("type-a", serde_json::json!({"id": 3})).await;
        
        let type_a_events = cache.get_by_type("type-a", 10).await;
        assert_eq!(type_a_events.len(), 2);
    }
}
