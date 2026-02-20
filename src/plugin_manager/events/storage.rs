// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::types::*;
use anyhow::Result;
use chrono::{DateTime, TimeDelta, Utc};
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Event storage with persistence
pub struct EventStorage {
    events: Arc<RwLock<HashMap<String, Event>>>,
    retention_period: TimeDelta,
    cache: Arc<RwLock<LruCache<String, Vec<Event>>>>,
    max_cache_size: usize,
    dead_letters: Arc<RwLock<Vec<DeadLetterEvent>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEvent {
    pub event: Event,
    pub subscriber: String,
    pub error: String,
    pub timestamp: DateTime<Utc>,
}

impl Default for EventStorage {
    fn default() -> Self {
        Self::new(Duration::from_secs(3600))
    }
}

impl EventStorage {
    pub fn new(retention_period: Duration) -> Self {
        Self {
            events: Arc::new(RwLock::new(HashMap::new())),
            retention_period: TimeDelta::from_std(retention_period)
                .unwrap_or_else(|_| TimeDelta::hours(1)),
            cache: Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(1000).unwrap()))),
            max_cache_size: 1000,
            dead_letters: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn store_event(&self, event: Event) -> Result<()> {
        let mut events = self.events.write().await;
        events.insert(event.id.to_string(), event.clone());

        self.cleanup_old_events(&mut events).await;
        self.update_cache(event).await;

        Ok(())
    }

    pub async fn get_event(&self, event_id: &str) -> Option<Event> {
        let events = self.events.read().await;
        events.get(event_id).cloned()
    }

    pub async fn get_events_for_type(&self, event_type: &str) -> Result<Vec<Event>> {
        let events = self.events.read().await;
        let filtered: Vec<Event> = events
            .values()
            .filter(|e| e.event_type == event_type)
            .cloned()
            .collect();

        Ok(filtered)
    }

    pub async fn query_events(
        &self,
        event_type: Option<String>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<Event>> {
        let events = self.events.read().await;
        let mut results = Vec::new();

        for event in events.values() {
            if let Some(ref et) = event_type {
                if &event.event_type != et {
                    continue;
                }
            }

            if event.timestamp < start_time {
                continue;
            }

            if event.timestamp > end_time {
                continue;
            }

            results.push(event.clone());
        }

        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(results)
    }

    pub async fn store_dead_letter(
        &self,
        event: Event,
        subscriber: String,
        error: String,
    ) -> Result<()> {
        let dead_letter = DeadLetterEvent {
            event,
            subscriber,
            error,
            timestamp: Utc::now(),
        };

        let mut dead_letters = self.dead_letters.write().await;
        dead_letters.push(dead_letter);

        if dead_letters.len() > 1000 {
            dead_letters.truncate(1000);
        }

        Ok(())
    }

    pub async fn get_dead_letters(&self) -> Vec<DeadLetterEvent> {
        let dead_letters = self.dead_letters.read().await;
        dead_letters.clone()
    }

    pub async fn clear_dead_letters(&self) {
        let mut dead_letters = self.dead_letters.write().await;
        dead_letters.clear();
    }

    async fn cleanup_old_events(&self, events: &mut HashMap<String, Event>) {
        let cutoff = Utc::now() - self.retention_period;

        let to_remove: Vec<String> = events
            .iter()
            .filter(|(_, event)| event.timestamp < cutoff)
            .map(|(id, _)| id.clone())
            .collect();

        for id in to_remove {
            events.remove(&id);
        }
    }

    async fn update_cache(&self, event: Event) {
        let mut cache = self.cache.write().await;

        if let Some(events) = cache.get_mut(&event.event_type) {
            events.push(event);

            if events.len() > 100 {
                events.truncate(100);
            }
        } else {
            cache.put(event.event_type.clone(), vec![event]);
        }

        if cache.len() > self.max_cache_size {
            cache.pop_lru();
        }
    }

    pub async fn get_cached_events(&self, event_type: &str, limit: usize) -> Vec<Event> {
        let mut cache = self.cache.write().await;

        if let Some(events) = cache.get_mut(event_type) {
            events.iter().rev().take(limit).cloned().collect()
        } else {
            Vec::new()
        }
    }

    pub async fn get_event_count(&self) -> usize {
        let events = self.events.read().await;
        events.len()
    }

    pub async fn get_event_count_for_type(&self, event_type: &str) -> usize {
        let events = self.events.read().await;
        events
            .values()
            .filter(|e| e.event_type == event_type)
            .count()
    }

    pub async fn clear(&self) {
        let mut events = self.events.write().await;
        let mut cache = self.cache.write().await;
        let mut dead_letters = self.dead_letters.write().await;

        events.clear();
        cache.clear();
        dead_letters.clear();
    }

    pub async fn get_storage_stats(&self) -> StorageStats {
        let events = self.events.read().await;
        let dead_letters = self.dead_letters.read().await;

        StorageStats {
            total_events: events.len(),
            dead_letter_count: dead_letters.len(),
            retention_hours: self.retention_period.num_hours(),
            cache_size: self.max_cache_size,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_events: usize,
    pub dead_letter_count: usize,
    pub retention_hours: i64,
    pub cache_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_storage_store_and_get() {
        let storage = EventStorage::new(Duration::from_secs(3600));

        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({"key": "value"}),
        );

        storage.store_event(event.clone()).await.unwrap();

        let retrieved = storage.get_event(&event.id.to_string()).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().event_type, "test.event");
    }

    #[tokio::test]
    async fn test_storage_query_by_type() {
        let storage = EventStorage::new(Duration::from_secs(3600));

        let event1 = Event::new(
            "test.event".to_string(),
            "plugin1".to_string(),
            serde_json::json!({}),
        );

        let event2 = Event::new(
            "other.event".to_string(),
            "plugin2".to_string(),
            serde_json::json!({}),
        );

        storage.store_event(event1).await.unwrap();
        storage.store_event(event2).await.unwrap();

        let events = storage.get_events_for_type("test.event").await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "test.event");
    }

    #[tokio::test]
    async fn test_storage_query_by_time() {
        let storage = EventStorage::new(Duration::from_secs(3600));

        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({}),
        );

        storage.store_event(event).await.unwrap();

        let start = Utc::now() - TimeDelta::hours(1);
        let end = Utc::now() + TimeDelta::hours(1);

        let events = storage.query_events(None, start, end).await.unwrap();
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn test_storage_dead_letter() {
        let storage = EventStorage::new(Duration::from_secs(3600));

        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({}),
        );

        storage
            .store_dead_letter(
                event,
                "test_subscriber".to_string(),
                "test error".to_string(),
            )
            .await
            .unwrap();

        let dead_letters = storage.get_dead_letters().await;
        assert_eq!(dead_letters.len(), 1);
        assert_eq!(dead_letters[0].error, "test error");
    }

    #[tokio::test]
    async fn test_storage_cleanup() {
        let storage = EventStorage::new(Duration::from_millis(100));

        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({}),
        );

        storage.store_event(event).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // Trigger cleanup by storing another event
        let event2 = Event::new(
            "trigger.cleanup".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({}),
        );
        storage.store_event(event2).await.unwrap();

        let count = storage.get_event_count_for_type("test.event").await;
        assert_eq!(count, 0);
    }
}
