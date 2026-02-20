# Enhanced Hot-Reload System

## Overview

The Skylet Enhanced Hot-Reload System provides advanced plugin reload capabilities with:

- **Dependency-Aware Reload**: Reload plugins in dependency order
- **Enhanced State Preservation**: Automatic state capture and restoration
- **Automatic Rollback**: Rollback on reload failure
- **Reload Monitoring**: Real-time monitoring and alerting
- **Batch Reload**: Efficient batch processing of multiple plugins

## Architecture

### Module Structure

```
src/plugin_manager/enhanced_hot_reload/
├── mod.rs              # Main enhanced hot-reload manager
├── types.rs            # Core types for enhanced hot-reload
├── state_manager.rs    # State preservation and management
└── rollback.rs         # Rollback functionality
```

### Core Components

#### 1. Enhanced Hot-Reload Manager (`mod.rs`)

The `EnhancedHotReloadManager` provides advanced reload capabilities:

```rust
use plugin_manager::enhanced_hot_reload::{EnhancedHotReloadManager, EnhancedHotReloadConfig};

// Create with configuration
let config = EnhancedHotReloadConfig {
    enabled: true,
    debounce_duration: Duration::from_millis(500),
    state_preservation_enabled: true,
    rollback_enabled: true,
    rollback_timeout: Duration::from_secs(30),
    monitoring_enabled: true,
    alert_on_failure: true,
    max_reload_attempts: 3,
    reload_batch_size: 5,
};

let manager = EnhancedHotReloadManager::new(
    config,
    dependency_resolver.clone(),
);

// Request single plugin reload
let request_id = manager.request_reload(
    "plugin1".to_string(),
    PathBuf::from("/path/to/plugin1.so"),
    ReloadReason::ManualRequest,
    "admin".to_string(),
).await?;

// Request batch reload
let plugins = vec![
    ("plugin1".to_string(), PathBuf::from("/path/to/plugin1.so"), ReloadReason::ManualRequest),
    ("plugin2".to_string(), PathBuf::from("/path/to/plugin2.so"), ReloadReason::ManualRequest),
];

let request_ids = manager.request_batch_reload(plugins, "admin".to_string()).await?;

// Process reload queue
let processed = manager.process_reload_queue().await?;

// Get reload status
let status = manager.get_reload_status("plugin1").await;

// Get monitoring data
let monitoring = manager.get_monitoring().await?;

println!("Processed {} plugins", processed);
println!("Total reloads: {}", monitoring.total_reloads);
println!("Successful: {}", monitoring.successful_reloads);
println!("Average time: {}ms", monitoring.average_reload_time_ms);

// Get alerts
let alerts = manager.get_alerts(10).await;
for alert in alerts {
    println!("ALERT: [{}] {} - {}", alert.alert_type, alert.plugin_id, alert.message);
}

// Rollback plugin
manager.rollback_plugin("plugin1").await?;

// Clear alerts
manager.clear_alerts().await?;
```

#### 2. Types (`types.rs`)

Core types for enhanced hot-reload:

```rust
use plugin_manager::enhanced_hot_reload::types::*;

// Plugin state snapshot
let snapshot = PluginStateSnapshot::new(
    "my_plugin".to_string(),
    state_data,
);

// State preservation configuration
let config = StatePreservationConfig::new();

// Reload reasons
let reason = ReloadReason::FileChange;
let manual = ReloadReason::ManualRequest;
let dependency = ReloadReason::DependencyUpdate;
```

#### 3. State Manager (`state_manager.rs`)

Plugin state preservation and management:

```rust
use plugin_manager::enhanced_hot_reload::state_manager::*;

let manager = StateManager::new(config);

// Save state before reload
let snapshot = manager.save_state("my_plugin".await?;

// Get latest snapshot
let latest = manager.get_latest_snapshot("my_plugin").await?;

// Restore state after failure
manager.restore_state("my_plugin", &snapshot).await?;

// Verify state integrity
let valid = manager.verify_state("my_plugin", &snapshot).await?;

// Get state metrics
let metrics = manager.get_metrics().await?;

// Export state to file
manager.export_state("my_plugin", PathBuf::from("/state.json")).await?;

// Import state from file
let imported = manager.import_state("my_plugin", PathBuf::from("/state.json")).await?;

// Delete old snapshots
let deleted = manager.delete_old_snapshots("my_plugin").await?;
```

#### 4. Rollback Manager (`rollback.rs`)

Automatic rollback functionality:

```rust
use plugin_manager::enhanced_hot_reload::rollback::*;

let manager = RollbackManager::new(true, Duration::from_secs(30));

// Perform rollback
let entry = manager.rollback("my_plugin").await?;

// Get rollback history
let history = manager.get_rollback_history("my_plugin", 10).await?;

for entry in history {
    println!("Rollback: {} -> {} ({})",
        entry.from_version, entry.to_version, entry.success);
}

// Clear rollback history
manager.clear_history("my_plugin").await?;

// Get rollback metrics
let metrics = manager.get_metrics().await?;

println!("Total rollbacks: {}", metrics.total_rollbacks);
println!("Successful: {}", metrics.successful_rollbacks);
println!("Average time: {}ms", metrics.avg_rollback_time_ms);
```

## Dependency-Aware Reload

### Reload Order Calculation

Plugins are reloaded in dependency order to avoid failures:

```rust
let plugins_to_reload = vec
!["plugin1".to_string(), "plugin2".to_string(), "plugin3".to_string()];

for plugin_id in plugins_to_reload {
    let dependencies = manager
        .dependency_resolver()
        .get_plugin_dependencies(&plugin_id)
        .await?;

    // Plugins with no dependencies are reloaded first
    if dependencies.is_empty() {
        manager.request_reload(plugin_id, path, ReloadReason::ManualRequest, "admin".to_string()).await?;
    }
}

// Reload order: plugin1, plugin2, plugin3
// plugin1 has no dependencies -> reloaded first
// plugin2 depends on plugin1 -> reloaded second
// plugin3 depends on plugin2 -> reloaded third
```

## State Preservation

### State Capture

State is automatically captured before reload:

```rust
let snapshot = manager.save_state("my_plugin").await?;

// Snapshot includes:
// - Plugin version
// - State data
// - Checksum for verification
// - Creation timestamp
// - Metadata (config hash, memory usage, uptime)
```

### State Restoration

State is restored after rollback:

```rust
manager.restore_state("my_plugin", &snapshot).await?;
```

### State Verification

Verify state integrity before restoration:

```rust
let valid = manager.verify_state("my_plugin", &snapshot).await?;

if !valid {
    println!("State checksum mismatch - data may be corrupted");
}
```

## Rollback

### Automatic Rollback

Rollback on reload failure:

```rust
// If reload fails, rollback is automatic
let config = EnhancedHotReloadConfig {
    rollback_enabled: true,
    rollback_timeout: Duration::from_secs(30),
    ..Default::default()
};
```

### Manual Rollback

Rollback to specific version:

```rust
let snapshot = manager.get_snapshot("my_plugin", "v2").await?;

manager.restore_state("my_plugin", &snapshot).await?;
```

### Rollback History

Track all rollback operations:

```rust
let history = manager.get_rollback_history("my_plugin", 100).await?;

for entry in history {
    println!("Rollback: {} -> {} ({}ms) - {}",
        entry.from_version,
        entry.to_version,
        entry.duration_ms,
        entry.success
    );
}
```

## Monitoring and Alerting

### Reload Metrics

Track reload operations:

```rust
let monitoring = manager.get_monitoring().await?;

println!("Total reloads: {}", monitoring.total_reloads);
println!("Successful: {}", monitoring.successful_reloads);
println!("Failed: {}", monitoring.failed_reloads);
println!("Rollbacks: {}", monitoring.rollback_count);
println!("Average time: {}ms", monitoring.average_reload_time_ms);
```

### Alert Types

Different alert types for different scenarios:

```rust
// Reload failed
AlertType::ReloadFailed

// Rollback required
AlertType::RollbackRequired

// State loss detected
AlertType::StateLoss

// Timeout
AlertType::Timeout

// Dependency error
AlertType::DependencyError
```

### Alert Management

Get and manage alerts:

```rust
// Get recent alerts
let alerts = manager.get_alerts(10).await?;

for alert in alerts {
    println!("ALERT [{}]: {} - {}",
        alert.alert_type,
        alert.plugin_id,
        alert.message
    );
}

// Clear alerts
manager.clear_alerts().await?;
```

## Performance Considerations

### Debouncing

Prevent excessive reloads:

```rust
let config = EnhancedHotReloadConfig {
    debounce_duration: Duration::from_millis(500),
    ..Default::default()
};
```

### Batch Processing

Process multiple reloads efficiently:

```rust
let config = EnhancedHotReloadConfig {
    reload_batch_size: 5,
    ..Default::default()
};

let manager = EnhancedHotReloadManager::new(config, dependency_resolver.clone());

let processed = manager.process_reload_queue().await?;
```

### State Size Limits

Limit state size to prevent issues:

```rust
let config = StatePreservationConfig {
    max_state_size_bytes: 10 * 1024 * 1024, // 10MB
    ..Default::default()
};
```

## Testing

### Unit Tests

Each module includes comprehensive unit tests:

```bash
cargo test -p execution-engine --lib plugin_manager::enhanced_hot_reload
```

## Troubleshooting

### Common Issues

#### Reload Failures

- Check plugin dependencies
- Verify plugin compatibility
- Review rollback history
- Check alert messages

#### State Loss

- Verify state capture before reload
- Check state checksums
- Review rollback history
- Check storage capacity

#### High Memory Usage

- Reduce state size limits
- Reduce snapshot retention
- Increase cleanup frequency

### Debug Logging

Enable debug logging:

```bash
RUST_LOG=plugin_manager::enhanced_hot_reload=debug cargo run
```

## Best Practices

### Dependency Management

- Declare all plugin dependencies
- Use semantic versioning
- Keep dependencies minimal

### State Management

- Keep state minimal and serializable
- Use version control for critical state
- Verify state integrity

### Rollback Strategy

- Always save state before reload
- Verify state after rollback
- Keep rollback history
- Rollback on timeout

### Monitoring

- Set up alerting for critical failures
- Monitor rollback success rates
- Track average reload times

## API Reference

### Types

- `EnhancedHotReloadConfig` - Configuration for enhanced hot-reload
- `ReloadRequest` - Reload request with dependencies
- `ReloadState` - Reload status tracking
- `ReloadReason` - Why reload was requested
- `ReloadResult` - Result of reload operation
- `PluginStateSnapshot` - Plugin state snapshot
- `StatePreservationConfig` - State preservation configuration
- `StateMetadata` - Additional state metadata
- `ReloadMonitoring` - Reload monitoring metrics
- `ReloadAlert` - Alert notifications
- `RollbackEntry` - Rollback operation record
- `RollbackMetrics` - Rollback statistics

### Functions

See module documentation for detailed API references:

```rust
use plugin_manager::enhanced_hot_reload;
```

## Future Enhancements

Planned features for enhanced hot-reload:

1. **Blue-Green Deployment**: Zero-downtime deployments
2. **Canary Rollouts**: Gradual deployment with monitoring
3. **Load Balancing**: Distribute load across instances
4. **Health Checks**: Pre-deployment health validation
5. **Automatic Rollback**: Rollback on health degradation
6. **Performance Monitoring**: Rollback impact tracking
7. **State Compression**: Advanced compression algorithms
8. **State Encryption**: Encrypted state storage
9. **Multi-Instance Sync**: State synchronization across instances
10. **Rollback Automation**: Intelligent rollback decisions

## License

Enhanced Hot-Reload System is part of Skylet and licensed under MIT OR Apache-2.0 license.
