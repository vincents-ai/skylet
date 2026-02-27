# Advanced Event System

## Overview

The Skylet Event System provides comprehensive event-driven plugin communication with:

- **Pattern-Based Routing**: Wildcard and regex pattern matching
- **Event Persistence**: Time-based event storage with replay capability
- **Advanced Filtering**: Content-based filters with transformation
- **Plugin Communication**: Request-response, broadcast, and multicast patterns
- **Event Statistics**: Real-time metrics on event flow

## Architecture

### Module Structure

```
src/plugin_manager/events/
├── mod.rs              # Main event system manager
├── types.rs            # Core event types and structures
├── router.rs           # Advanced event routing with patterns
├── storage.rs          # Event persistence and replay
├── filtering.rs        # Event filtering and rate limiting
└── plugin_comm.rs      # Plugin communication helpers
```

### Core Components

#### 1. Event System Manager (`mod.rs`)

The `EventSystem` provides the main interface for event management:

```rust
use plugin_manager::events::{EventSystem, EventSystemConfig};

// Create event system with configuration
let config = EventSystemConfig {
    enabled: true,
    max_event_size: 1024 * 1024,
    retention_period: Duration::from_secs(3600),
    enable_persistence: true,
    enable_replay: true,
    max_subscribers_per_event: 100,
    default_event_priority: EventPriority::Normal,
};

let event_system = EventSystem::new(config);

// Publish an event
let event = Event::new(
    "plugin.event".to_string(),
    "my_plugin".to_string(),
    serde_json::json!({"key": "value"}),
);

let result = event_system.publish(event).await?;

// Subscribe to events
let callback = Arc::new(|event: Event| async move {
    println!("Received event: {:?}", event.event_type);
    Ok(())
});

let subscriber = EventSubscriber::new(
    "my_plugin".to_string(),
    vec!["plugin.event".to_string()],
    callback,
);

event_system.subscribe(subscriber).await?;

// Replay events
event_system.replay_events(
    Some("plugin.event".to_string()),
    start_time,
    end_time,
    Arc::new(|event| {
        println!("Replayed: {}", event.event_type);
    }),
).await?;
```

#### 2. Event Types (`types.rs`)

Comprehensive event types for different use cases:

```rust
use plugin_manager::events::types::*;

// Create basic event
let event = Event::new(
    "test.event".to_string(),
    "test_plugin".to_string(),
    serde_json::json!({"data": "value"}),
);

// Add priority
let event = event.with_priority(EventPriority::Critical);

// Add headers
let event = event
    .with_header("X-Custom".to_string(), "value123".to_string())
    .with_header("X-Trace".to_string(), "trace-456".to_string());

// Add metadata
let event = event.with_metadata(
    EventMetadata::default()
        .with_correlation_id("abc123".to_string())
        .with_reply_to("response.event".to_string())
        .with_timeout(5000)
        .with_tag("important".to_string()),
);

// Get payload as typed data
let data: serde_json::Value = event.get_payload().unwrap();
```

#### 3. Event Router (`router.rs`)

Advanced routing with pattern matching:

```rust
use plugin_manager::events::router::EventRouter;

let router = EventRouter::new(storage.clone())
    .with_config(RoutingConfig {
        enable_pattern_matching: true,
        enable_wildcard_routing: true,
        max_routing_depth: 10,
        enable_dead_letter_queue: true,
        dead_letter_max_size: 1000,
    });

// Route event to subscribers
let subscriber_count = router.route_event(event).await?;

// Get subscriber count
let count = router.subscriber_count("plugin.event").await;
```

#### 4. Event Storage (`storage.rs`)

Event persistence with time-based retention:

```rust
use plugin_manager::events::storage::EventStorage;
use chrono::TimeDelta;

let storage = EventStorage::new(TimeDelta::hours(24));

// Store event
storage.store_event(event).await?;

// Get event by ID
let event = storage.get_event(&event_id).await;

// Query events by type
let events = storage.get_events_for_type("plugin.event").await?;

// Query events by time range
let events = storage.query_events(
    Some("plugin.event".to_string()),
    start_time,
    end_time,
).await?;

// Get cached events (fast path)
let recent = storage.get_cached_events("plugin.event", 10).await;

// Get dead letter events
let dead_letters = storage.get_dead_letters().await;

// Clear dead letters
storage.clear_dead_letters().await;
```

#### 5. Event Filtering (`filtering.rs`)

Advanced event filtering and rate limiting:

```rust
use plugin_manager::events::filtering::{EventFilter, FilterCondition, RateLimiter};

// Create filter
let filter = EventFilter::new(
    "filter-1".to_string(),
    "Allow only production events".to_string(),
)
.with_condition(FilterCondition::EventTypeEquals("production.*".to_string()))
.with_condition(FilterCondition::PriorityAtLeast(EventPriority::High))
.with_action(FilterAction::Allow);

// Check if event matches filter
if filter.matches(&event) {
    println!("Event passed filter");
}

// Apply filter to event
let result = filter.apply(&mut event.clone()).unwrap();
match result {
    FilterResult::Allowed => println!("Event allowed"),
    FilterResult::Blocked => println!("Event blocked"),
    FilterResult::Transformed => println!("Event transformed"),
    FilterResult::Filtered => println!("Event filtered"),
}

// Rate limiter
let limiter = RateLimiter::new(
    "limiter-1".to_string(),
    "high_frequency.event".to_string(),
    10.0, // 10 events per second
);

limiter.check(&event).await?;
```

#### 6. Plugin Communication (`plugin_comm.rs`)

High-level patterns for plugin communication:

```rust
use plugin_manager::events::plugin_comm::{EventBus, RequestResponseManager};

// Create event bus for plugin
let event_bus = EventBus::new(event_system.clone(), "my_plugin".to_string());

// Publish simple event
event_bus.publish(
    "my.event".to_string(),
    serde_json::json!({"data": "value"}),
).await?;

// Subscribe to events
event_bus.subscribe(
    vec!["target.event".to_string()],
    Arc::new(|event: Event| async move {
        Ok(())
    }),
).await?;

// Request-response pattern
let manager = RequestResponseManager::new(5000);

let response = manager.send_request(
    &event_system,
    "target_plugin".to_string(),
    Request::new(
        serde_json::json!({"key": "value"}),
        "my.response".to_string(),
    ),
).await?;

match response {
    Response::Success(data) => println!("Got response: {:?}", data),
    Response::Error(e) => println!("Error: {}", e),
    Response::Timeout(e) => println!("Timeout: {}", e),
}

// Broadcast to all subscribers
let broadcast_mgr = event_bus.broadcast_manager();
let count = broadcast_mgr.broadcast(
    "broadcast.event".to_string(),
    serde_json::json!({"msg": "hello"}),
).await?;

// Multicast to specific plugins
let results = broadcast_mgr.multicast(
    vec!["plugin1".to_string(), "plugin2".to_string()],
    "multicast.event".to_string(),
    serde_json::json!({"msg": "hello"}),
).await?;
```

## Event Patterns

### Exact Pattern

```rust
EventPattern::Exact("plugin.specific.event".to_string())
```

Matches only the exact event type.

### Wildcard Pattern

```rust
EventPattern::Wildcard("plugin.*.event".to_string())
```

Matches `plugin.any.event`, `plugin.test.event`, etc.

### Regex Pattern

```rust
EventPattern::Regex("regex:plugin\\.(success|error)\\.event".to_string())
```

Matches `plugin.success.event` or `plugin.error.event`.

## Event Priority

Priority levels for event ordering:

- `EventPriority::Critical` - Highest priority, process first
- `EventPriority::High` - High priority
- `EventPriority::Normal` - Default priority
- `EventPriority::Low` - Lowest priority

## Event Metadata

### Correlation ID

Link related events across multiple plugins:

```rust
let event = event.with_metadata(
    EventMetadata::default()
        .with_correlation_id(Uuid::new_v4().to_string())
);
```

### Request-Response

Enable request-response pattern:

```rust
let event = event.with_metadata(
    EventMetadata::default()
        .with_reply_to("plugin.response".to_string())
);
```

### Tags

Add custom tags for filtering:

```rust
let event = event.with_metadata(
    EventMetadata::default()
        .with_tags(vec!["important".to_string(), "audit".to_string()])
);
```

## Event Filtering

### Content-Based Filtering

Filter events based on content:

```rust
let filter = EventFilter::new("content-filter".to_string(), "Filter by content")
    .with_condition(FilterCondition::PayloadContains("important".to_string()))
    .with_condition(FilterCondition::SourceEquals("trusted-plugin".to_string()));
```

### Transformation

Transform events before processing:

```rust
let filter = EventFilter::new("transformer".to_string(), "Add timestamp")
    .with_action(FilterAction::Transform(Arc::new(|event: &mut Event| {
        event.headers.insert("processed_at".to_string(), Utc::now().to_rfc3339());
    })));
```

### Rate Limiting

Prevent event storms:

```rust
let limiter = RateLimiter::new(
    "rate-limiter".to_string(),
    "high.frequency.event".to_string(),
    100.0, // Max 100 events per second
);

if limiter.check(&event).await.is_err() {
    return Err("Rate limit exceeded".to_string());
}
```

## Dead Letter Queue

Failed events are automatically stored in the dead letter queue:

```rust
// Get dead letter events
let dead_letters = event_system.storage().get_dead_letters().await;

for dl in dead_letters {
    println!(
        "Event {} failed for {}: {}",
        dl.event.id,
        dl.subscriber,
        dl.error
    );
}

// Clear dead letters
event_system.storage().clear_dead_letters().await;
```

## Event Replay

Replay events for debugging and testing:

```rust
// Replay events from a time range
let start = Utc::now() - Duration::hours(1);
let end = Utc::now();

let replayed = event_system.replay_events(
    Some("test.event".to_string()),
    start,
    end,
    Arc::new(|event: Event| {
        println!("Replaying: {}", event.event_type);
        Ok(())
    }),
).await?;

println!("Replayed {} events", replayed);
```

## Plugin Communication Patterns

### Pub-Sub

Simple publish-subscribe:

```rust
// Publisher
event_bus.publish("topic.event".to_string(), payload).await?;

// Subscriber
event_bus.subscribe(vec!["topic.event".to_string()], callback).await?;
```

### Request-Response

Request with response handling:

```rust
let manager = RequestResponseManager::new(5000);

// Send request
let response = manager.send_request(
    &event_system,
    "target.plugin".to_string(),
    Request::new(request_data, "my.response".to_string()),
).await?;

// Handle response
match response {
    Response::Success(data) => {
        println!("Success: {:?}", data);
    }
    Response::Error(e) => {
        println!("Error: {}", e);
    }
}
```

### Broadcast

Send to all subscribers of a topic:

```rust
let broadcast_mgr = event_bus.broadcast_manager();
let count = broadcast_mgr.broadcast("topic.event".to_string(), payload).await?;
println!("Broadcast to {} subscribers", count);
```

### Multicast

Send to specific plugins:

```rust
let results = broadcast_mgr.multicast(
    vec!["plugin1".to_string(), "plugin2".to_string()],
    "private.event".to_string(),
    payload,
).await?;
```

## Performance Considerations

### Event Size Limits

```rust
let config = EventSystemConfig {
    max_event_size: 1024 * 1024, // 1MB limit
    ..Default::default()
};
```

### Retention Period

Configure based on storage capacity:

```rust
let config = EventSystemConfig {
    retention_period: Duration::from_secs(3600), // 1 hour
    ..Default::default()
};
```

### Subscriber Limits

Prevent excessive subscribers:

```rust
let config = EventSystemConfig {
    max_subscribers_per_event: 100,
    ..Default::default()
};
```

## Event Statistics

Monitor event flow:

```rust
let stats = event_system.get_statistics().await;
println!("Total published: {}", stats.total_published);
println!("High priority: {}", stats.high_priority_published);

let per_event = event_system.get_statistics_for_event("test.event").await;
println!("Published: {}", per_event.published);
```

## Testing

### Unit Tests

Each module includes comprehensive unit tests:

```bash
cargo test -p execution-engine --lib plugin_manager::events
```

### Integration Tests

End-to-end event system tests:

```bash
cargo test -p execution-engine --test event_integration
```

## Troubleshooting

### Common Issues

#### Events Not Being Delivered

- Check if event system is enabled
- Verify subscriber registration
- Check routing patterns

#### High Memory Usage

- Reduce retention period
- Limit dead letter queue size
- Increase cleanup frequency

#### Slow Event Delivery

- Check subscriber callback performance
- Use async callbacks properly
- Consider event batching

### Debug Logging

Enable debug logging for events:

```bash
RUST_LOG=plugin_manager::events=debug cargo run
```

## Best Practices

### Event Naming

- Use dot-notation for hierarchy: `plugin.subsystem.event`
- Use descriptive names
- Keep names consistent

**Good:**
- `plugin.request.received`
- `plugin.data.processed`
- `plugin.error.timeout`

**Bad:**
- `event`
- `data`
- `e`

### Payload Structure

- Use structured data (JSON)
- Include version information
- Include correlation IDs for tracing

### Priority Usage

- Use `Critical` for system failures
- Use `High` for important business events
- Use `Normal` for regular operations
- Use `Low` for informational events

## API Reference

### Types

- `Event` - Core event structure
- `EventPriority` - Priority levels (Low, Normal, High, Critical)
- `EventMetadata` - Event correlation and metadata
- `EventSubscriber` - Subscription information
- `EventCallback` - Async callback trait
- `EventPattern` - Pattern types (Exact, Wildcard, Regex)
- `RoutingConfig` - Routing configuration
- `EventStatistics` - Event flow statistics

### Functions

See module documentation for detailed API references:

```rust
use plugin_manager::events;
```

## Future Enhancements

Planned features for the event system:

1. **Event Versioning**: Support multiple event schema versions
2. **Event Validation**: Schema validation for event payloads
3. **Event Aggregation**: Pre-computed aggregates
4. **Event Batching**: Batch multiple events for efficiency
5. **Event Compression**: Compress large payloads
6. **Event Encryption**: Encrypt sensitive event data
7. **Event Signing**: Verify event authenticity
8. **Event Replay Position**: Continue from last replay position
9. **Event Backpressure**: Flow control for high throughput
10. **Event Federation**: Cross-system event forwarding

## License

Event System Module is part of Skylet and licensed under MIT OR Apache-2.0 license.
