// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Advanced Event System Module
//!
//! Provides comprehensive event-driven plugin communication with:
//! - Pattern-based event routing (wildcards, regex)
//! - Event persistence and replay
//! - Advanced filtering and validation
//! - Inter-plugin messaging patterns

pub mod filtering;
pub mod plugin_comm;
pub mod router;
pub mod storage;
pub mod types;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use types::*;

/// Event system configuration
#[derive(Debug, Clone)]
pub struct EventSystemConfig {
    pub enabled: bool,
    pub max_event_size: usize,
    pub retention_period: Duration,
    pub enable_persistence: bool,
    pub enable_replay: bool,
    pub max_subscribers_per_event: usize,
    pub default_event_priority: EventPriority,
}

impl Default for EventSystemConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_event_size: 1024 * 1024, // 1MB
            retention_period: Duration::from_secs(3600), // 1 hour
            enable_persistence: true,
            enable_replay: true,
            max_subscribers_per_event: 100,
            default_event_priority: EventPriority::Normal,
        }
    }
}

/// Main event system manager
pub struct EventSystem {
    config: EventSystemConfig,
    router: Arc<router::EventRouter>,
    storage: Arc<storage::EventStorage>,
    filters: Arc<RwLock<Vec<filtering::EventFilter>>>,
    subscribers: Arc<RwLock<HashMap<String, Vec<EventSubscriber>>>>,
    statistics: Arc<RwLock<EventStatistics>>,
}

impl EventSystem {
    pub fn new(config: EventSystemConfig) -> Self {
        let storage = Arc::new(storage::EventStorage::new(config.retention_period));
        let router = Arc::new(router::EventRouter::new(storage.clone()));

        Self {
            config,
            router,
            storage,
            filters: Arc::new(RwLock::new(Vec::new())),
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            statistics: Arc::new(RwLock::new(EventStatistics::default())),
        }
    }

    pub async fn publish(&self, event: Event) -> Result<EventResult, EventError> {
        if !self.config.enabled {
            return Err(EventError::Disabled);
        }

        let payload_size = serde_json::to_string(&event.payload).map_err(|e| EventError::Callback(e.to_string()))?.len();
        if payload_size > self.config.max_event_size {
            return Err(EventError::TooLarge(payload_size));
        }

        self.update_statistics(&event).await;

        let filtered = self.apply_filters(&event).await;
        if !filtered {
            return Ok(EventResult::Filtered);
        }

        if self.config.enable_persistence {
            let _ = self.storage.store_event(event.clone()).await;
        }

        let subscriber_count = self.router.route_event(event.clone()).await.map_err(|e| EventError::Routing(e.to_string()))?;

        Ok(EventResult::Published { subscriber_count })
    }

    pub async fn subscribe(
        &self,
        subscriber: EventSubscriber,
    ) -> Result<(), EventError> {
        let mut subscribers = self.subscribers.write().await;

        for event_type in &subscriber.event_types {
            let count = subscribers
                .entry(event_type.clone())
                .or_insert_with(Vec::new)
                .len();

            if count >= self.config.max_subscribers_per_event {
                return Err(EventError::TooManySubscribers);
            }
        }

        for event_type in &subscriber.event_types {
            subscribers
                .entry(event_type.clone())
                .or_insert_with(Vec::new)
                .push(subscriber.clone());
        }

        Ok(())
    }

    pub async fn unsubscribe(&self, subscriber_id: &str) -> Result<(), EventError> {
        let mut subscribers = self.subscribers.write().await;

        for subs in subscribers.values_mut() {
            subs.retain(|s| s.id != subscriber_id);
        }

        subscribers.retain(|_, subs| !subs.is_empty());

        Ok(())
    }

    pub async fn add_filter(&self, filter: filtering::EventFilter) {
        let mut filters = self.filters.write().await;
        filters.push(filter);
    }

    pub async fn remove_filter(&self, filter_id: &str) -> bool {
        let mut filters = self.filters.write().await;
        let initial_len = filters.len();
        filters.retain(|f| f.id != filter_id);
        filters.len() < initial_len
    }

    pub async fn replay_events(
        &self,
        event_type: Option<String>,
        start_time: chrono::DateTime<chrono::Utc>,
        end_time: chrono::DateTime<chrono::Utc>,
        callback: Arc<dyn Fn(Event) + Send + Sync>,
    ) -> Result<usize, EventError> {
        if !self.config.enable_replay {
            return Err(EventError::ReplayDisabled);
        }

        let events = self
            .storage
            .query_events(event_type, start_time, end_time)
            .await.map_err(|e| EventError::Storage(e.to_string()))?;

        let mut replayed = 0;
        for event in events {
            callback(event);
            replayed += 1;
        }

        Ok(replayed)
    }

    pub async fn get_statistics(&self) -> EventStatistics {
        let stats = self.statistics.read().await;
        stats.clone()
    }

    pub async fn get_statistics_for_event(&self, event_type: &str) -> EventStatistics {
        let stats = self.statistics.read().await;
        stats.per_event.get(event_type).cloned().unwrap_or_default()
    }

    pub fn router(&self) -> Arc<router::EventRouter> {
        self.router.clone()
    }

    pub fn storage(&self) -> Arc<storage::EventStorage> {
        self.storage.clone()
    }

    pub fn config(&self) -> &EventSystemConfig {
        &self.config
    }

    async fn update_statistics(&self, event: &Event) {
        let mut stats = self.statistics.write().await;

        stats.total_published += 1;
        *stats.per_event
            .entry(event.event_type.clone())
            .or_insert_with(EventStatistics::default)
            .published_mut() += 1;

        if event.priority == EventPriority::High {
            stats.high_priority_published += 1;
        }
    }

    async fn apply_filters(&self, event: &Event) -> bool {
        let filters = self.filters.read().await;
        let mut passed = true;

        for filter in filters.iter() {
            if !filter.matches(event) {
                passed = false;
                break;
            }
        }

        passed
    }
}

impl Default for EventSystem {
    fn default() -> Self {
        Self::new(EventSystemConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_system_config() {
        let config = EventSystemConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_event_size, 1024 * 1024);
        assert_eq!(config.max_subscribers_per_event, 100);
    }

    #[test]
    fn test_event_statistics() {
        let mut stats = EventStatistics::default();
        stats.total_published = 100;
        stats.high_priority_published = 10;

        assert_eq!(stats.total_published, 100);
        assert_eq!(stats.high_priority_published, 10);
    }
}
