// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::types::*;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::storage::EventStorage;

/// Event router with pattern matching
pub struct EventRouter {
    storage: Arc<EventStorage>,
    patterns: Arc<RwLock<HashMap<String, Vec<EventSubscriber>>>>,
    wildcards: Arc<RwLock<Vec<EventSubscriber>>>,
    config: RoutingConfig,
}

impl EventRouter {
    pub fn new(storage: Arc<EventStorage>) -> Self {
        Self {
            storage,
            patterns: Arc::new(RwLock::new(HashMap::new())),
            wildcards: Arc::new(RwLock::new(Vec::new())),
            config: RoutingConfig::default(),
        }
    }

    pub fn with_config(mut self, config: RoutingConfig) -> Self {
        self.config = config;
        self
    }

    pub async fn route_event(&self, event: Event) -> Result<usize> {
        let mut matched_subscribers = Vec::new();

        if self.config.enable_pattern_matching {
            self.route_by_pattern(&event, &mut matched_subscribers).await;
        }

        if self.config.enable_wildcard_routing {
            self.route_by_wildcard(&event, &mut matched_subscribers).await;
        }

        let count = matched_subscribers.len();
        self.deliver_to_subscribers(event.clone(), matched_subscribers).await?;

        Ok(count)
    }

    async fn route_by_pattern(&self, event: &Event, subscribers: &mut Vec<EventSubscriber>) {
        let patterns = self.patterns.read().await;

        if let Some(subs) = patterns.get(&event.event_type) {
            for sub in subs {
                if self.should_deliver(sub, event) {
                    subscribers.push(sub.clone());
                }
            }
        }
    }

    async fn route_by_wildcard(&self, event: &Event, subscribers: &mut Vec<EventSubscriber>) {
        let wildcards = self.wildcards.read().await;

        for sub in wildcards.iter() {
            for pattern_str in &sub.event_types {
                if let Ok(EventPattern::Wildcard(wc)) = self.parse_pattern(pattern_str) {
                    let event_parts: Vec<&str> = event.event_type.split('.').collect();
                    let wc_parts: Vec<&str> = wc.split('.').collect();

                    if event_parts.len() == wc_parts.len()
                        && event_parts.iter().zip(wc_parts.iter()).all(|(e, w)| *w == "*" || w == e)
                    {
                        if self.should_deliver(sub, event) {
                            subscribers.push(sub.clone());
                        }
                    }
                }
            }
        }
    }

    fn should_deliver(&self, subscriber: &EventSubscriber, event: &Event) -> bool {
        if let Some(ref filter) = subscriber.filter {
            return filter.matches(event);
        }
        true
    }

    async fn deliver_to_subscribers(
        &self,
        event: Event,
        subscribers: Vec<EventSubscriber>,
    ) -> Result<()> {
        for subscriber in subscribers {
            if let Err(e) = subscriber.callback.on_event(event.clone()).await {
                tracing::warn!(
                    "Event callback failed for {}: {}",
                    subscriber.plugin_name,
                    e
                );

                if self.config.enable_dead_letter_queue {
                    self.storage
                        .store_dead_letter(event.clone(), subscriber.plugin_name.clone(), e.to_string())
                        .await
                        .ok();
                }
            }
        }

        Ok(())
    }

    pub async fn add_subscriber(&self, subscriber: EventSubscriber) -> Result<()> {
        for event_type in &subscriber.event_types {
            let pattern = self.parse_pattern(event_type)?;

            match pattern {
                EventPattern::Wildcard(_) => {
                    let mut wildcards = self.wildcards.write().await;
                    wildcards.push(subscriber.clone());
                }
                EventPattern::Exact(_) | EventPattern::Regex(_) => {
                    let mut patterns = self.patterns.write().await;
                    patterns
                        .entry(event_type.clone())
                        .or_insert_with(Vec::new)
                        .push(subscriber.clone());
                }
            }
        }

        Ok(())
    }

    pub async fn remove_subscriber(&self, subscriber_id: &str) -> Result<()> {
        let mut patterns = self.patterns.write().await;
        let mut wildcards = self.wildcards.write().await;

        for subs in patterns.values_mut() {
            subs.retain(|s| s.id != subscriber_id);
        }

        wildcards.retain(|s| s.id != subscriber_id);

        patterns.retain(|_, subs| !subs.is_empty());

        Ok(())
    }

    fn parse_pattern(&self, pattern: &str) -> Result<EventPattern> {
        if pattern.starts_with("regex:") {
            Ok(EventPattern::Regex(pattern[6..].to_string()))
        } else if pattern.contains('*') {
            Ok(EventPattern::Wildcard(pattern.to_string()))
        } else {
            Ok(EventPattern::Exact(pattern.to_string()))
        }
    }

    pub async fn subscriber_count(&self, event_type: &str) -> usize {
        let patterns = self.patterns.read().await;
        let wildcards = self.wildcards.read().await;

        let exact_count = patterns.get(event_type).map(|s| s.len()).unwrap_or(0);
        let wildcard_count = wildcards
            .iter()
            .filter(|s| s.event_types.iter().any(|t| self.pattern_matches_wildcard(t, event_type)))
            .count();

        exact_count + wildcard_count
    }

    fn pattern_matches_wildcard(&self, pattern: &str, event_type: &str) -> bool {
        if let Ok(EventPattern::Wildcard(wc)) = self.parse_pattern(pattern) {
            let event_parts: Vec<&str> = event_type.split('.').collect();
            let wc_parts: Vec<&str> = wc.split('.').collect();

            if event_parts.len() == wc_parts.len()
                && event_parts.iter().zip(wc_parts.iter()).all(|(e, w)| *w == "*" || w == e)
            {
                return true;
            }
        }

        false
    }
}

impl Default for EventRouter {
    fn default() -> Self {
        Self::new(Arc::new(EventStorage::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_router_new() {
        let storage = Arc::new(EventStorage::default());
        let router = EventRouter::new(storage);

        assert_eq!(router.subscriber_count("test.event").await, 0);
    }

    #[tokio::test]
    async fn test_router_add_subscriber() {
        use super::super::types::*;

        let storage = Arc::new(EventStorage::default());
        let router = EventRouter::new(storage);

        struct TestCallback;
        #[async_trait::async_trait]
        impl EventCallback for TestCallback {
            async fn on_event(&self, _event: Event) -> Result<(), EventError> {
                Ok(())
            }
        }

        let callback: Arc<dyn EventCallback> = Arc::new(TestCallback);

        let subscriber = EventSubscriber::new(
            "test_plugin".to_string(),
            vec!["test.event".to_string()],
            callback,
        );

        router.add_subscriber(subscriber).await.unwrap();

        assert_eq!(router.subscriber_count("test.event").await, 1);
    }

    #[test]
    fn test_parse_pattern() {
        let router = EventRouter::new(Arc::new(EventStorage::default()));

        let exact = router.parse_pattern("test.event").unwrap();
        assert!(matches!(exact, EventPattern::Exact(_)));

        let wildcard = router.parse_pattern("test.*").unwrap();
        assert!(matches!(wildcard, EventPattern::Wildcard(_)));

        let regex = router.parse_pattern("regex:test.*").unwrap();
        assert!(matches!(regex, EventPattern::Regex(_)));
    }
}
