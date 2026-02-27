//! Integration tests for plugin communication and service integration

use super::*;
use crate::plugin_manager::events::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_plugin_to_plugin_communication() {
    let event_system = Arc::new(EventSystem::new(EventSystemConfig::default()));

    let received_messages = Arc::new(Mutex::new(Vec::new()));

    // Plugin A: Publisher
    let publisher = EventBus::new(
        event_system.clone(),
        "plugin_a".to_string(),
    );

    // Plugin B: Subscriber
    let subscriber = EventBus::new(
        event_system.clone(),
        "plugin_b".to_string(),
    );

    let callback = {
        let received = received_messages.clone();
        Arc::new(move |event: Event| {
            let received = received.clone();
            async move {
                let mut messages = received.lock().await;
                messages.push(event);
                Ok(())
            }
        })
    };

    subscriber
        .subscribe(vec!["plugin_a.message".to_string()], callback)
        .await
        .unwrap();

    // Publish message
    publisher
        .publish(
            "plugin_a.message".to_string(),
            serde_json::json!({"text": "Hello from plugin A"}),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let messages = received_messages.lock().await;
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].source, "plugin_a");
    assert_eq!(messages[0].event_type, "plugin_a.message");
}

#[tokio::test]
async fn test_request_response_pattern() {
    let event_system = Arc::new(EventSystem::new(EventSystemConfig::default()));

    // Plugin B: Responds to requests
    let subscriber = EventBus::new(
        event_system.clone(),
        "plugin_b".to_string(),
    );

    let callback = Arc::new(move |event: Event| async move {
        let response_data = if let Some(reply_to) = &event.metadata.reply_to {
            serde_json::json!({
                "correlation_id": event.metadata.correlation_id,
                "response": "Hello from plugin B"
            })
        } else {
            serde_json::json!({})
        };

        let response_event = Event::new(
            reply_to.unwrap_or("default.response".to_string()),
            "plugin_b".to_string(),
            response_data,
        );

        event_system.publish(response_event).await
    });

    subscriber
        .subscribe(vec!["request.to.plugin_b".to_string()], callback)
        .await
        .unwrap();

    // Plugin A: Sends request
    let publisher = EventBus::new(
        event_system.clone(),
        "plugin_a".to_string(),
    );

    let correlation_id = uuid::Uuid::new_v4().to_string();

    let request = Request::new(
        serde_json::json!({"message": "Hello plugin B"}),
        "plugin_b.response".to_string(),
    )
    .with_correlation_id(correlation_id.clone());

    let response = tokio::time::timeout(
        Duration::from_secs(5),
        publisher.send_request(&event_system, "plugin_b".to_string(), request),
    )
    .await;

    assert!(response.is_ok());
    let response = response.unwrap();
    assert!(matches!(response, Response::Success(_)));
}

#[tokio::test]
async fn test_broadcast_to_multiple_plugins() {
    let event_system = Arc::new(EventSystem::new(EventSystemConfig::default()));

    let receiver_counts = Arc::new(Mutex::new(std::collections::HashMap::new()));

    // Create multiple subscribers
    for i in 0..3 {
        let subscriber = EventBus::new(
            event_system.clone(),
            format!("plugin_{}", i),
        );

        let receiver_counts = receiver_counts.clone();
        let plugin_id = format!("plugin_{}", i);

        let callback = Arc::new(move |event: Event| {
            let receiver_counts = receiver_counts.clone();
            let plugin_id = plugin_id.clone();
            async move {
                let mut counts = receiver_counts.lock().await;
                *counts.entry(plugin_id).or_insert(0) += 1;
                Ok(())
            }
        });

        subscriber
            .subscribe(vec!["broadcast.event".to_string()], callback)
            .await
            .unwrap();
    }

    // Broadcast event
    let publisher = EventBus::new(
        event_system.clone(),
        "broadcaster".to_string(),
    );

    let broadcast_mgr = publisher.broadcast_manager();
    broadcast_mgr
        .broadcast(
            "broadcast.event".to_string(),
            serde_json::json!({"message": "Hello all plugins"}),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let counts = receiver_counts.lock().await;
    assert_eq!(counts.len(), 3);
    for (_, count) in counts.iter() {
        assert_eq!(*count, 1);
    }
}

#[tokio::test]
async fn test_event_filtering() {
    let event_system = Arc::new(EventSystem::new(EventSystemConfig::default()));

    let high_priority_received = Arc::new(Mutex::new(0));
    let all_received = Arc::new(Mutex::new(0));

    // Subscriber that only receives high-priority events
    let high_priority_subscriber = EventBus::new(
        event_system.clone(),
        "priority_subscriber".to_string(),
    );

    let callback = {
        let high_priority_received = high_priority_received.clone();
        let all_received = all_received.clone();
        Arc::new(move |event: Event| {
            let high_priority_received = high_priority_received.clone();
            let all_received = all_received.clone();
            async move {
                let mut all = all_received.lock().await;
                *all += 1;

                if event.priority == EventPriority::High {
                    let mut high = high_priority_received.lock().await;
                    *high += 1;
                }
                Ok(())
            }
        })
    };

    high_priority_subscriber
        .subscribe(vec!["test.event".to_string()], callback)
        .await
        .unwrap();

    // Add filter to event system
    let filter = EventFilter::new(
        "priority-filter".to_string(),
        "Filter by priority".to_string(),
    )
    .with_condition(FilterCondition::PriorityAtLeast(EventPriority::High))
    .with_action(FilterAction::Allow);

    event_system.add_filter(filter).await;

    let publisher = EventBus::new(
        event_system.clone(),
        "publisher".to_string(),
    );

    // Publish low priority event (should be filtered)
    let low_priority_event = Event::new(
        "test.event".to_string(),
        "publisher".to_string(),
        serde_json::json!({}),
    )
    .with_priority(EventPriority::Low);

    event_system.publish(low_priority_event).await.unwrap();

    // Publish high priority event (should pass)
    let high_priority_event = Event::new(
        "test.event".to_string(),
        "publisher".to_string(),
        serde_json::json!({}),
    )
    .with_priority(EventPriority::High);

    event_system.publish(high_priority_event).await.unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let high = high_priority_received.lock().await;
    let all = all_received.lock().await;

    assert_eq!(*high, 1); // Only high-priority event
    assert_eq!(*all, 1); // Only high-priority event was received
}

#[tokio::test]
async fn test_event_persistence_and_replay() {
    let event_system = Arc::new(EventSystem::new(EventSystemConfig {
        enable_persistence: true,
        enable_replay: true,
        ..EventSystemConfig::default()
    }));

    // Publish events
    for i in 0..3 {
        let event = Event::new(
            "test.event".to_string(),
            "publisher".to_string(),
            serde_json::json!({"index": i}),
        );
        event_system.publish(event).await.unwrap();
    }

    // Replay events
    let replayed_count = Arc::new(Mutex::new(0));

    let callback = {
        let replayed_count = replayed_count.clone();
        Arc::new(move |event: Event| {
            let replayed_count = replayed_count.clone();
            async move {
                let mut count = replayed_count.lock().await;
                *count += 1;
                Ok(())
            }
        })
    };

    let start = chrono::Utc::now() - chrono::Duration::seconds(10);
    let end = chrono::Utc::now();

    let count = event_system
        .replay_events(Some("test.event".to_string()), start, end, callback)
        .await;

    assert!(count >= 3);
}

#[tokio::test]
async fn test_dead_letter_queue() {
    let event_system = Arc::new(EventSystem::new(EventSystemConfig {
        enable_dead_letter_queue: true,
        dead_letter_max_size: 100,
        ..EventSystemConfig::default()
    }));

    // Subscribe with a failing handler
    let subscriber = EventBus::new(
        event_system.clone(),
        "failing_subscriber".to_string(),
    );

    let callback = Arc::new(|event: Event| async move {
        // Always fail
        Err(anyhow::anyhow!("Intentional failure"))
    });

    subscriber
        .subscribe(vec!["failing.event".to_string()], callback)
        .await
        .unwrap();

    // Publish event that will fail
    let event = Event::new(
        "failing.event".to_string(),
        "publisher".to_string(),
        serde_json::json!({}),
    );

    event_system.publish(event).await.unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check dead letter queue
    let dead_letters = event_system.storage().get_dead_letters().await;

    assert!(!dead_letters.is_empty());
    assert_eq!(dead_letters[0].event.event_type, "failing.event");
}

#[tokio::test]
async fn test_service_integration() {
    let event_system = Arc::new(EventSystem::new(EventSystemConfig::default()));

    let responses = Arc::new(Mutex::new(Vec::new()));

    // Create mock service that responds to events
    let service_callback = {
        let responses = responses.clone();
        Arc::new(move |event: Event| {
            let responses = responses.clone();
            async move {
                let mut resp = responses.lock().await;
                resp.push(event);
                Ok(())
            }
        })
    };

    let service_subscriber = EventSubscriber::new(
        "service".to_string(),
        vec!["service.request".to_string()],
        service_callback,
    );

    event_system.subscribe(service_subscriber).await.unwrap();

    // Plugin sends request to service
    let plugin = EventBus::new(
        event_system.clone(),
        "plugin".to_string(),
    );

    plugin
        .publish(
            "service.request".to_string(),
            serde_json::json!({"action": "get_data"}),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let resp = responses.lock().await;
    assert_eq!(resp.len(), 1);
    assert_eq!(resp[0].event_type, "service.request");
}

#[tokio::test]
async fn test_plugin_isolation_via_events() {
    let event_system = Arc::new(EventSystem::new(EventSystemConfig::default()));

    let plugin_a_received = Arc::new(Mutex::new(0));
    let plugin_b_received = Arc::new(Mutex::new(0));

    // Plugin A: Subscribes only to plugin A events
    let plugin_a = EventBus::new(
        event_system.clone(),
        "plugin_a".to_string(),
    );

    let callback_a = {
        let received = plugin_a_received.clone();
        Arc::new(move |event: Event| {
            let received = received.clone();
            async move {
                let mut count = received.lock().await;
                *count += 1;
                Ok(())
            }
        })
    };

    plugin_a
        .subscribe(vec!["plugin_a.*".to_string()], callback_a)
        .await
        .unwrap();

    // Plugin B: Subscribes only to plugin B events
    let plugin_b = EventBus::new(
        event_system.clone(),
        "plugin_b".to_string(),
    );

    let callback_b = {
        let received = plugin_b_received.clone();
        Arc::new(move |event: Event| {
            let received = received.clone();
            async move {
                let mut count = received.lock().await;
                *count += 1;
                Ok(())
            }
        })
    };

    plugin_b
        .subscribe(vec!["plugin_b.*".to_string()], callback_b)
        .await
        .unwrap();

    // Publish plugin A event
    let event_a = Event::new(
        "plugin_a.event".to_string(),
        "plugin_a".to_string(),
        serde_json::json!({}),
    );
    event_system.publish(event_a).await.unwrap();

    // Publish plugin B event
    let event_b = Event::new(
        "plugin_b.event".to_string(),
        "plugin_b".to_string(),
        serde_json::json!({}),
    );
    event_system.publish(event_b).await.unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let a_count = plugin_a_received.lock().await;
    let b_count = plugin_b_received.lock().await;

    assert_eq!(*a_count, 1); // Only received plugin A event
    assert_eq!(*b_count, 1); // Only received plugin B event
}
