// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::filtering::EventFilter;

/// Event priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EventPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for EventPriority {
    fn default() -> Self {
        EventPriority::Normal
    }
}

/// An event in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub event_type: String,
    pub source_plugin: String,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub priority: EventPriority,
    pub metadata: EventMetadata,
    pub headers: HashMap<String, String>,
}

impl Event {
    pub fn new(event_type: String, source: String, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type,
            source_plugin: source,
            payload,
            timestamp: Utc::now(),
            priority: EventPriority::Normal,
            metadata: EventMetadata::default(),
            headers: HashMap::new(),
        }
    }

    #[allow(dead_code)] // Phase 2 event system — not yet wired up
    pub fn with_priority(mut self, priority: EventPriority) -> Self {
        self.priority = priority;
        self
    }

    #[allow(dead_code)] // Phase 2 event system — not yet wired up
    pub fn with_header(mut self, key: String, value: String) -> Self {
        self.headers.insert(key, value);
        self
    }

    #[allow(dead_code)] // Phase 2 event system — not yet wired up
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers.extend(headers);
        self
    }

    pub fn with_metadata(mut self, metadata: EventMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn get_header(&self, key: &str) -> Option<&String> {
        self.headers.get(key)
    }

    #[allow(dead_code)] // Phase 2 event system — not yet wired up
    pub fn get_payload_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.payload)
    }

    #[allow(dead_code)] // Phase 2 event system — not yet wired up
    pub fn get_payload<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.payload.clone())
    }
}

/// Event metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventMetadata {
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub reply_to: Option<String>,
    pub timeout_ms: Option<u64>,
    pub retry_count: u32,
    pub max_retries: Option<u32>,
    pub tags: Vec<String>,
}

impl EventMetadata {
    pub fn with_correlation_id(mut self, id: String) -> Self {
        self.correlation_id = Some(id);
        self
    }

    #[allow(dead_code)] // Phase 2 event system — not yet wired up
    pub fn with_reply_to(mut self, reply_to: String) -> Self {
        self.reply_to = Some(reply_to);
        self
    }

    #[allow(dead_code)] // Phase 2 event system — not yet wired up
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    #[allow(dead_code)] // Phase 2 event system — not yet wired up
    pub fn with_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }

    #[allow(dead_code)] // Phase 2 event system — not yet wired up
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags.extend(tags);
        self
    }
}

/// Event subscriber information
#[derive(Clone)]
#[allow(dead_code)] // Phase 2 event system — not yet wired up
pub struct EventSubscriber {
    pub id: String,
    pub plugin_name: String,
    pub event_types: Vec<String>,
    pub filter: Option<EventFilter>,
    pub callback: Arc<dyn EventCallback>,
}

impl EventSubscriber {
    pub fn new(
        plugin_name: String,
        event_types: Vec<String>,
        callback: Arc<dyn EventCallback>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            plugin_name,
            event_types,
            filter: None,
            callback,
        }
    }

    #[allow(dead_code)] // Phase 2 event system — not yet wired up
    pub fn with_filter(mut self, filter: EventFilter) -> Self {
        self.filter = Some(filter);
        self
    }
}

/// Event callback trait
#[async_trait::async_trait]
#[allow(dead_code)] // Phase 2 event system — not yet wired up
pub trait EventCallback: Send + Sync {
    async fn on_event(&self, event: Event) -> Result<(), EventError>;
}

/// Event publication result
#[derive(Debug, Clone)]
#[allow(dead_code)] // Phase 2 event system — not yet wired up
pub enum EventResult {
    Published { subscriber_count: usize },
    Filtered,
}

/// Event errors
#[derive(Debug, Clone, thiserror::Error)]
#[allow(dead_code)] // Phase 2 event system — not yet wired up
pub enum EventError {
    #[error("Event system is disabled")]
    Disabled,
    #[error("Event too large: {0} bytes")]
    TooLarge(usize),
    #[error("Too many subscribers for event type")]
    TooManySubscribers,
    #[error("Event replay is disabled")]
    ReplayDisabled,
    #[error("Routing error: {0}")]
    Routing(String),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Filter error: {0}")]
    Filter(String),
    #[error("Callback error: {0}")]
    Callback(String),
}

/// Event statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventStatistics {
    pub total_published: u64,
    pub total_subscribers: usize,
    pub high_priority_published: u64,
    pub per_event: HashMap<String, EventStatistics>,
}

impl EventStatistics {
    #[allow(dead_code)] // Phase 2 event system — not yet wired up
    pub fn published_mut(&mut self) -> &mut u64 {
        &mut self.total_published
    }
}

/// Event pattern for routing
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventPattern {
    Exact(String),
    Wildcard(String),
    Regex(String),
}

#[allow(dead_code)] // Phase 2 event system — not yet wired up
impl EventPattern {
    pub fn matches(&self, event_type: &str) -> bool {
        match self {
            EventPattern::Exact(s) => event_type == s,
            EventPattern::Wildcard(pattern) => Self::match_wildcard(pattern, event_type),
            EventPattern::Regex(regex) => {
                if let Ok(re) = regex::Regex::new(regex) {
                    re.is_match(event_type)
                } else {
                    false
                }
            }
        }
    }

    fn match_wildcard(pattern: &str, event_type: &str) -> bool {
        let pattern_parts: Vec<&str> = pattern.split('.').collect();
        let event_parts: Vec<&str> = event_type.split('.').collect();

        if pattern_parts.len() != event_parts.len() {
            return false;
        }

        for (p, e) in pattern_parts.iter().zip(event_parts.iter()) {
            if *p != "*" && p != e {
                return false;
            }
        }

        true
    }
}

/// Event routing configuration
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // Phase 2 event system — not yet wired up
pub struct RoutingConfig {
    pub enable_pattern_matching: bool,
    pub enable_wildcard_routing: bool,
    pub max_routing_depth: usize,
    pub enable_dead_letter_queue: bool,
    pub dead_letter_max_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_priority() {
        assert!(EventPriority::Critical > EventPriority::High);
        assert!(EventPriority::High > EventPriority::Normal);
        assert!(EventPriority::Normal > EventPriority::Low);
    }

    #[test]
    fn test_event_new() {
        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({"key": "value"}),
        );

        assert_eq!(event.event_type, "test.event");
        assert_eq!(event.source_plugin, "test_plugin");
        assert_eq!(event.priority, EventPriority::Normal);
    }

    #[test]
    fn test_event_with_priority() {
        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({}),
        )
        .with_priority(EventPriority::Critical);

        assert_eq!(event.priority, EventPriority::Critical);
    }

    #[test]
    fn test_event_pattern_exact() {
        let pattern = EventPattern::Exact("test.event".to_string());
        assert!(pattern.matches("test.event"));
        assert!(!pattern.matches("test.other"));
    }

    #[test]
    fn test_event_pattern_wildcard() {
        let pattern = EventPattern::Wildcard("test.*".to_string());
        assert!(pattern.matches("test.event"));
        assert!(pattern.matches("test.other"));
        assert!(!pattern.matches("other.event"));
    }

    #[test]
    fn test_event_pattern_wildcard_nested() {
        let pattern = EventPattern::Wildcard("*.event".to_string());
        assert!(pattern.matches("test.event"));
        assert!(pattern.matches("other.event"));
        assert!(!pattern.matches("test.other"));
    }

    #[test]
    fn test_event_metadata() {
        let metadata = EventMetadata::default()
            .with_correlation_id("test-id".to_string())
            .with_tag("important".to_string());

        assert_eq!(metadata.correlation_id, Some("test-id".to_string()));
        assert_eq!(metadata.tags.len(), 1);
    }
}
