// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(dead_code)] // Event filtering infrastructure - not yet wired into production

use super::types::*;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Event filter for conditional event processing
#[derive(Debug, Clone)]
pub struct EventFilter {
    pub id: String,
    pub name: String,
    pub conditions: Vec<FilterCondition>,
    pub action: FilterAction,
}

#[derive(Clone)]
pub enum FilterCondition {
    EventTypeEquals(String),
    EventTypeMatches(String),
    SourceEquals(String),
    PayloadContains(String),
    PayloadEquals(String),
    PriorityAtLeast(EventPriority),
    HeaderEquals(String, String),
    Custom(Arc<dyn Fn(&Event) -> bool + Send + Sync>),
}

impl std::fmt::Debug for FilterCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterCondition::EventTypeEquals(s) => write!(f, "EventTypeEquals({})", s),
            FilterCondition::EventTypeMatches(s) => write!(f, "EventTypeMatches({})", s),
            FilterCondition::SourceEquals(s) => write!(f, "SourceEquals({})", s),
            FilterCondition::PayloadContains(s) => write!(f, "PayloadContains({})", s),
            FilterCondition::PayloadEquals(s) => write!(f, "PayloadEquals({})", s),
            FilterCondition::PriorityAtLeast(p) => write!(f, "PriorityAtLeast({:?})", p),
            FilterCondition::HeaderEquals(k, v) => write!(f, "HeaderEquals({}, {})", k, v),
            FilterCondition::Custom(_) => write!(f, "Custom(<function>)"),
        }
    }
}

#[derive(Clone)]
pub enum FilterAction {
    Allow,
    Block,
    Transform(Arc<dyn Fn(&mut Event) + Send + Sync>),
}

impl PartialEq for FilterAction {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (FilterAction::Allow, FilterAction::Allow) => true,
            (FilterAction::Block, FilterAction::Block) => true,
            (FilterAction::Transform(_), FilterAction::Transform(_)) => false,
            _ => false,
        }
    }
}

impl std::fmt::Debug for FilterAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterAction::Allow => write!(f, "Allow"),
            FilterAction::Block => write!(f, "Block"),
            FilterAction::Transform(_) => write!(f, "Transform(<function>)"),
        }
    }
}

impl EventFilter {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            conditions: Vec::new(),
            action: FilterAction::Allow,
        }
    }

    pub fn with_condition(mut self, condition: FilterCondition) -> Self {
        self.conditions.push(condition);
        self
    }

    pub fn with_action(mut self, action: FilterAction) -> Self {
        self.action = action;
        self
    }

    pub fn matches(&self, event: &Event) -> bool {
        for condition in &self.conditions {
            if !Self::check_condition(condition, event) {
                return false;
            }
        }
        true
    }

    pub fn apply(&self, event: &mut Event) -> Result<FilterResult, String> {
        if !self.matches(event) {
            return Ok(FilterResult::Filtered);
        }

        match &self.action {
            FilterAction::Allow => Ok(FilterResult::Allowed),
            FilterAction::Block => Ok(FilterResult::Blocked),
            FilterAction::Transform(func) => {
                func(event);
                Ok(FilterResult::Transformed)
            }
        }
    }

    fn check_condition(condition: &FilterCondition, event: &Event) -> bool {
        match condition {
            FilterCondition::EventTypeEquals(expected) => &event.event_type == expected,
            FilterCondition::EventTypeMatches(pattern) => {
                if let Ok(re) = regex::Regex::new(pattern) {
                    re.is_match(&event.event_type)
                } else {
                    false
                }
            }
            FilterCondition::SourceEquals(source) => &event.source_plugin == source,
            FilterCondition::PayloadContains(needle) => {
                let payload_str = serde_json::to_string(&event.payload).unwrap_or_default();
                payload_str.contains(needle)
            }
            FilterCondition::PayloadEquals(expected) => {
                let payload_str = serde_json::to_string(&event.payload).unwrap_or_default();
                &payload_str == expected
            }
            FilterCondition::PriorityAtLeast(min) => event.priority >= *min,
            FilterCondition::HeaderEquals(key, value) => {
                event.get_header(key).map_or(false, |v| v == value)
            }
            FilterCondition::Custom(func) => func(event),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FilterResult {
    Allowed,
    Filtered,
    Blocked,
    Transformed,
}

/// Rate limiter for events
#[derive(Debug, Clone)]
pub struct RateLimiter {
    #[allow(dead_code)] // Public API — not yet called from production code
    pub id: String,
    pub event_type: String,
    pub max_events_per_second: f64,
    pub events: Arc<RwLock<Vec<(std::time::Instant, Event)>>>,
}

impl RateLimiter {
    pub fn new(id: String, event_type: String, max_events_per_second: f64) -> Self {
        Self {
            id,
            event_type,
            max_events_per_second,
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn check(&self, event: &Event) -> Result<bool, String> {
        if event.event_type != self.event_type {
            return Ok(true);
        }

        let mut events = self.events.write().await;
        let now = std::time::Instant::now();

        events
            .retain(|(timestamp, _)| now.saturating_duration_since(*timestamp).as_secs_f64() < 1.0);

        let count = events.len();
        if count as f64 >= self.max_events_per_second {
            return Err(format!(
                "Rate limit exceeded: {}/s",
                self.max_events_per_second
            ));
        }

        events.push((now, event.clone()));
        Ok(true)
    }

    #[allow(dead_code)] // Public API — not yet called from production code
    pub async fn get_current_rate(&self) -> f64 {
        let events = self.events.read().await;
        let now = std::time::Instant::now();

        let recent_count = events
            .iter()
            .filter(|(timestamp, _)| now.saturating_duration_since(*timestamp).as_secs_f64() < 1.0)
            .count();

        recent_count as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_new() {
        let filter = EventFilter::new("test-id".to_string(), "Test Filter".to_string());
        assert_eq!(filter.id, "test-id");
        assert_eq!(filter.name, "Test Filter");
        assert_eq!(filter.action, FilterAction::Allow);
    }

    #[test]
    fn test_filter_with_condition() {
        let filter = EventFilter::new("test-id".to_string(), "Test Filter".to_string())
            .with_condition(FilterCondition::EventTypeEquals("test.event".to_string()));

        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({}),
        );

        assert!(filter.matches(&event));
    }

    #[test]
    fn test_filter_apply() {
        let filter = EventFilter::new("test-id".to_string(), "Test Filter".to_string())
            .with_action(FilterAction::Block);

        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({}),
        );

        let result = filter.apply(&mut event.clone()).unwrap();
        assert_eq!(result, FilterResult::Blocked);
    }

    #[test]
    fn test_rate_limiter() {
        let limiter = RateLimiter::new("test-limiter".to_string(), "test.event".to_string(), 10.0);

        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({}),
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        let limiter = Arc::new(limiter);

        let checks = (0..10)
            .map(|_| {
                let event = event.clone();
                let limiter = limiter.clone();
                rt.block_on(async move { limiter.check(&event).await.unwrap() })
            })
            .collect::<Vec<_>>();

        assert_eq!(checks.iter().filter(|x| **x).count(), 10);
    }

    #[test]
    fn test_condition_payload_contains() {
        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({"message": "hello world"}),
        );

        let condition = FilterCondition::PayloadContains("hello".to_string());
        assert!(EventFilter::check_condition(&condition, &event));

        let condition = FilterCondition::PayloadContains("not found".to_string());
        assert!(!EventFilter::check_condition(&condition, &event));
    }

    #[test]
    fn test_condition_priority() {
        let mut event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({}),
        );

        event.priority = EventPriority::Critical;

        let condition = FilterCondition::PriorityAtLeast(EventPriority::High);
        assert!(EventFilter::check_condition(&condition, &event));

        event.priority = EventPriority::Low;
        assert!(!EventFilter::check_condition(&condition, &event));
    }

    #[test]
    fn test_condition_header_equals() {
        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({}),
        )
        .with_header("X-Custom".to_string(), "value123".to_string());

        let condition =
            FilterCondition::HeaderEquals("X-Custom".to_string(), "value123".to_string());
        assert!(EventFilter::check_condition(&condition, &event));

        let condition = FilterCondition::HeaderEquals("X-Custom".to_string(), "other".to_string());
        assert!(!EventFilter::check_condition(&condition, &event));
    }
}
