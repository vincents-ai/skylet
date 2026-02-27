//! Unit tests for event system

use super::*;
use crate::plugin_manager::events::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[cfg(test)]
mod event_types_tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({"key": "value"}),
        );

        assert_eq!(event.event_type, "test.event");
        assert_eq!(event.source, "test_plugin");
        assert!(event.payload.is_object());
    }

    #[test]
    fn test_event_priority() {
        let mut event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({}),
        );

        event = event.with_priority(EventPriority::Critical);
        assert_eq!(event.priority, EventPriority::Critical);

        event = event.with_priority(EventPriority::High);
        assert_eq!(event.priority, EventPriority::High);
    }

    #[test]
    fn test_event_headers() {
        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({}),
        )
        .with_header("X-Custom".to_string(), "value123".to_string())
        .with_header("X-Trace".to_string(), "trace-456".to_string());

        assert_eq!(event.headers.len(), 2);
        assert_eq!(event.headers.get("X-Custom"), Some(&"value123".to_string()));
        assert_eq!(event.headers.get("X-Trace"), Some(&"trace-456".to_string()));
    }

    #[test]
    fn test_event_metadata() {
        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({}),
        )
        .with_metadata(
            EventMetadata::default()
                .with_correlation_id("abc123".to_string())
                .with_reply_to("response.event".to_string())
                .with_timeout(5000)
                .with_tag("important".to_string()),
        );

        assert_eq!(event.metadata.correlation_id, Some("abc123".to_string()));
        assert_eq!(event.metadata.reply_to, Some("response.event".to_string()));
        assert_eq!(event.metadata.timeout_ms, Some(5000));
        assert!(event.metadata.tags.contains(&"important".to_string()));
    }

    #[test]
    fn test_event_get_payload() {
        let event = Event::new(
            "test.event".to_string(),
            "test_plugin".to_string(),
            serde_json::json!({"key": "value", "number": 42}),
        );

        let payload = event.get_payload().unwrap();
        assert_eq!(payload.get("key"), Some(&serde_json::json!("value")));
        assert_eq!(payload.get("number"), Some(&serde_json::json!(42)));
    }
}

#[cfg(test)]
mod event_router_tests {
    use super::*;

    #[tokio::test]
    async fn test_subscribe_and_publish() {
        let event_system = EventSystem::new(EventSystemConfig::default());
        let mut received_event = None;

        let callback = Arc::new(move |event: Event| async move {
            received_event = Some(event);
            Ok(())
        });

        let subscriber = EventSubscriber::new(
            "test_plugin".to_string(),
            vec!["test.event".to_string()],
            callback,
        );

        event_system.subscribe(subscriber).await.unwrap();

        let event = Event::new(
            "test.event".to_string(),
            "source_plugin".to_string(),
            serde_json::json!({"data": "test"}),
        );

        event_system.publish(event).await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;
        // In a real test, we'd verify received_event
    }

    #[tokio::test]
    async fn test_wildcard_routing() {
        let event_system = EventSystem::new(EventSystemConfig::default());
        let received_count = Arc::new(Mutex::new(0));

        let callback = {
            let received_count = received_count.clone();
            Arc::new(move |event: Event| {
                let received_count = received_count.clone();
                async move {
                    let mut count = received_count.lock().await;
                    *count += 1;
                    Ok(())
                }
            })
        };

        let subscriber = EventSubscriber::new(
            "test_plugin".to_string(),
            vec!["test.*".to_string()],
            callback,
        );

        event_system.subscribe(subscriber).await.unwrap();

        // Publish multiple events matching wildcard
        for event_type in ["test.event1", "test.event2", "test.event3"] {
            let event = Event::new(
                event_type.to_string(),
                "source_plugin".to_string(),
                serde_json::json!({}),
            );
            event_system.publish(event).await.unwrap();
        }

        tokio::time::sleep(Duration::from_millis(100)).await;

        let count = received_count.lock().await;
        assert_eq!(*count, 3);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let event_system = EventSystem::new(EventSystemConfig::default());
        let count1 = Arc::new(Mutex::new(0));
        let count2 = Arc::new(Mutex::new(0));

        let callback1 = {
            let count = count1.clone();
            Arc::new(move |event: Event| {
                let count = count.clone();
                async move {
                    let mut c = count.lock().await;
                    *c += 1;
                    Ok(())
                }
            })
        };

        let callback2 = {
            let count = count2.clone();
            Arc::new(move |event: Event| {
                let count = count.clone();
                async move {
                    let mut c = count.lock().await;
                    *c += 1;
                    Ok(())
                }
            })
        };

        let subscriber1 = EventSubscriber::new(
            "plugin1".to_string(),
            vec!["test.event".to_string()],
            callback1,
        );

        let subscriber2 = EventSubscriber::new(
            "plugin2".to_string(),
            vec!["test.event".to_string()],
            callback2,
        );

        event_system.subscribe(subscriber1).await.unwrap();
        event_system.subscribe(subscriber2).await.unwrap();

        let event = Event::new(
            "test.event".to_string(),
            "source_plugin".to_string(),
            serde_json::json!({}),
        );

        event_system.publish(event).await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        let c1 = count1.lock().await;
        let c2 = count2.lock().await;
        assert_eq!(*c1, 1);
        assert_eq!(*c2, 1);
    }
}

#[cfg(test)]
mod event_filtering_tests {
    use super::*;

    #[test]
    fn test_filter_by_event_type() {
        let filter = EventFilter::new(
            "filter-1".to_string(),
            "Filter by event type".to_string(),
        )
        .with_condition(FilterCondition::EventTypeEquals("production.*".to_string()));

        let event1 = Event::new(
            "production.log".to_string(),
            "source".to_string(),
            serde_json::json!({}),
        );

        let event2 = Event::new(
            "debug.log".to_string(),
            "source".to_string(),
            serde_json::json!({}),
        );

        assert!(filter.matches(&event1));
        assert!(!filter.matches(&event2));
    }

    #[test]
    fn test_filter_by_priority() {
        let filter = EventFilter::new(
            "filter-1".to_string(),
            "Filter by priority".to_string(),
        )
        .with_condition(FilterCondition::PriorityAtLeast(EventPriority::High));

        let high_event = Event::new(
            "test.event".to_string(),
            "source".to_string(),
            serde_json::json!({}),
        )
        .with_priority(EventPriority::High);

        let low_event = Event::new(
            "test.event".to_string(),
            "source".to_string(),
            serde_json::json!({}),
        )
        .with_priority(EventPriority::Low);

        assert!(filter.matches(&high_event));
        assert!(!filter.matches(&low_event));
    }

    #[test]
    fn test_filter_by_payload() {
        let filter = EventFilter::new(
            "filter-1".to_string(),
            "Filter by payload".to_string(),
        )
        .with_condition(FilterCondition::PayloadContains("important".to_string()));

        let event1 = Event::new(
            "test.event".to_string(),
            "source".to_string(),
            serde_json::json!({"message": "This is important"}),
        );

        let event2 = Event::new(
            "test.event".to_string(),
            "source".to_string(),
            serde_json::json!({"message": "Regular message"}),
        );

        assert!(filter.matches(&event1));
        assert!(!filter.matches(&event2));
    }

    #[test]
    fn test_filter_transformation() {
        let filter = EventFilter::new(
            "transformer".to_string(),
            "Add timestamp".to_string(),
        )
        .with_action(FilterAction::Transform(Arc::new(|event: &mut Event| {
            event.headers.insert(
                "processed_at".to_string(),
                chrono::Utc::now().to_rfc3339(),
            );
        })));

        let mut event = Event::new(
            "test.event".to_string(),
            "source".to_string(),
            serde_json::json!({}),
        );

        filter.apply(&mut event).unwrap();

        assert!(event.headers.contains_key("processed_at"));
    }
}

#[cfg(test)]
mod rate_limiter_tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiting() {
        let limiter = RateLimiter::new(
            "limiter-1".to_string(),
            "test.event".to_string(),
            2.0, // 2 events per second
        );

        let event = Event::new(
            "test.event".to_string(),
            "source".to_string(),
            serde_json::json!({}),
        );

        // First event should pass
        assert!(limiter.check(&event).await.is_ok());

        // Second event should pass
        assert!(limiter.check(&event).await.is_ok());

        // Third event should be rate limited
        assert!(limiter.check(&event).await.is_err());
    }

    #[tokio::test]
    async fn test_rate_limit_reset() {
        let limiter = RateLimiter::new(
            "limiter-1".to_string(),
            "test.event".to_string(),
            1.0, // 1 event per second
        );

        let event = Event::new(
            "test.event".to_string(),
            "source".to_string(),
            serde_json::json!({}),
        );

        assert!(limiter.check(&event).await.is_ok());
        assert!(limiter.check(&event).await.is_err());

        // Wait for rate limit to reset
        tokio::time::sleep(Duration::from_secs(1)).await;

        assert!(limiter.check(&event).await.is_ok());
    }
}

#[cfg(test)]
mod event_storage_tests {
    use super::*;

    #[tokio::test]
    async fn test_event_storage() {
        let storage = EventStorage::new(chrono::Duration::hours(1));

        let event = Event::new(
            "test.event".to_string(),
            "source".to_string(),
            serde_json::json!({"data": "test"}),
        );

        storage.store_event(event.clone()).await.unwrap();

        let retrieved = storage.get_event(&event.id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().event_type, "test.event");
    }

    #[tokio::test]
    async fn test_event_query_by_type() {
        let storage = EventStorage::new(chrono::Duration::hours(1));

        for i in 0..3 {
            let event = Event::new(
                "test.event".to_string(),
                "source".to_string(),
                serde_json::json!({"index": i}),
            );
            storage.store_event(event).await.unwrap();
        }

        let events = storage.get_events_for_type("test.event").await.unwrap();
        assert_eq!(events.len(), 3);
    }

    #[tokio::test]
    async fn test_event_query_by_time_range() {
        let storage = EventStorage::new(chrono::Duration::hours(1));

        let now = chrono::Utc::now();

        let event1 = Event::new(
            "test.event".to_string(),
            "source".to_string(),
            serde_json::json!({}),
        );
        storage.store_event(event1).await.unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        let event2 = Event::new(
            "test.event".to_string(),
            "source".to_string(),
            serde_json::json!({}),
        );
        storage.store_event(event2).await.unwrap();

        let events = storage
            .query_events(
                Some("test.event".to_string()),
                now - chrono::Duration::seconds(1),
                chrono::Utc::now(),
            )
            .await
            .unwrap();

        assert!(events.len() >= 1);
    }

    #[tokio::test]
    async fn test_event_retention() {
        let storage = EventStorage::new(chrono::Duration::milliseconds(100));

        let event = Event::new(
            "test.event".to_string(),
            "source".to_string(),
            serde_json::json!({}),
        );
        storage.store_event(event.clone()).await.unwrap();

        tokio::time::sleep(Duration::from_millis(150)).await;

        storage.cleanup_old_events().await;

        let retrieved = storage.get_event(&event.id).await;
        assert!(retrieved.is_none());
    }
}

#[cfg(test)]
mod plugin_comm_tests {
    use super::*;

    #[tokio::test]
    async fn test_publish_subscribe() {
        let event_system = Arc::new(EventSystem::new(EventSystemConfig::default()));
        let event_bus = EventBus::new(event_system.clone(), "publisher_plugin".to_string());

        let received = Arc::new(Mutex::new(false));

        let callback = {
            let received = received.clone();
            Arc::new(move |event: Event| async move {
                let mut r = received.lock().await;
                *r = true;
                Ok(())
            })
        };

        event_bus
            .subscribe(vec!["test.event".to_string()], callback)
            .await
            .unwrap();

        event_bus
            .publish("test.event".to_string(), serde_json::json!({"data": "test"}))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        let r = received.lock().await;
        assert!(*r);
    }

    #[tokio::test]
    async fn test_broadcast() {
        let event_system = Arc::new(EventSystem::new(EventSystemConfig::default()));
        let event_bus = EventBus::new(event_system.clone(), "broadcaster".to_string());

        let count = Arc::new(Mutex::new(0));

        // Create multiple subscribers
        for _ in 0..3 {
            let callback = {
                let count = count.clone();
                Arc::new(move |event: Event| {
                    let count = count.clone();
                    async move {
                        let mut c = count.lock().await;
                        *c += 1;
                        Ok(())
                    }
                })
            };

            event_bus
                .subscribe(vec!["broadcast.event".to_string()], callback)
                .await
                .unwrap();
        }

        let broadcast_mgr = event_bus.broadcast_manager();
        let subscriber_count = broadcast_mgr
            .broadcast(
                "broadcast.event".to_string(),
                serde_json::json!({"msg": "hello"}),
            )
            .await
            .unwrap();

        assert_eq!(subscriber_count, 3);

        tokio::time::sleep(Duration::from_millis(100)).await;

        let c = count.lock().await;
        assert_eq!(*c, 3);
    }
}
