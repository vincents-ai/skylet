# Advanced Metrics Collection and Monitoring

## Overview

The Skylet Metrics Collection System provides comprehensive plugin observability with:

- **Real-time Metrics Collection**: Performance metrics (latency, throughput, error rates)
- **Resource Usage Monitoring**: Memory, CPU, thread count, file handles
- **Custom Plugin Metrics**: Support for plugin-specific metrics
- **Time-series Storage**: Efficient storage with configurable retention
- **Multi-export Support**: Prometheus and OpenTelemetry export formats

## Architecture

### Module Structure

```
src/plugin_manager/metrics/
├── mod.rs              # Main metrics manager and configuration
├── types.rs            # Core metric types and structures
├── collector.rs        # System and plugin metrics collection
├── storage.rs          # Time-series metrics storage
├── export.rs           # Prometheus/OTel exporters
└── plugin_integration.rs # Plugin metrics integration helpers
```

### Core Components

#### 1. Metrics Manager (`mod.rs`)

The `MetricsManager` provides the main interface for metrics collection:

```rust
use plugin_manager::metrics::{MetricsManager, MetricsConfig};

// Create metrics manager with configuration
let config = MetricsConfig {
    enabled: true,
    collection_interval: Duration::from_secs(5),
    retention_period: Duration::from_secs(86400),
    enable_prometheus: true,
    enable_otel: false,
    prometheus_port: 9091,
    otel_endpoint: None,
    sample_rate: 1.0,
};

let manager = MetricsManager::new(config);

// Start metrics collection
manager.start().await?;

// Register a plugin for metrics collection
manager.register_plugin("my_plugin".to_string()).await;

// Record a metric
manager.record_metric(metric).await;

// Query metrics
let query = MetricQuery::new()
    .with_plugin("my_plugin".to_string())
    .with_limit(100);

let metrics = manager.query_metrics(query).await;
```

#### 2. Metric Types (`types.rs`)

Comprehensive metric types for different use cases:

```rust
use plugin_manager::metrics::types::*;

// Counter metric
let counter = Metric::counter("requests_total".to_string(), 42)
    .with_label("method".to_string(), "GET".to_string())
    .with_label("status".to_string(), "200".to_string());

// Gauge metric
let gauge = Metric::gauge("active_connections".to_string(), 100.0)
    .with_label("plugin".to_string(), "my_plugin".to_string());

// Histogram metric
let histogram = Metric::histogram("request_duration_ms".to_string(), vec![10.0, 20.0, 30.0]);

// Performance metrics
let mut perf = PerformanceMetrics::default();
perf.record_call(50.0, true);

// Resource metrics
let resource = ResourceMetrics {
    memory_mb: 256.0,
    cpu_percent: 45.5,
    thread_count: 8,
    file_handles: 42,
    timestamp: Utc::now(),
};

// Plugin metrics with health score
let mut plugin_metrics = PluginMetrics::new("my_plugin".to_string());
plugin_metrics.update_resource_metrics(resource);
let health_score = plugin_metrics.health_score();
```

#### 3. Metrics Collector (`collector.rs`)

Automatic collection of system and plugin metrics:

```rust
use plugin_manager::metrics::collector::MetricsCollector;

let collector = MetricsCollector::new(
    Duration::from_secs(5),
    1.0,
);

// Collect all metrics
let metrics = collector.collect_all().await?;

// Collect system metrics
let system_metrics = collector.collect_system_metrics().await?;

// Collect process metrics
let process_metrics = collector.collect_process_metrics().await?;

// Collect plugin metrics
let plugin_metrics = collector.collect_plugin_metrics(
    "my_plugin",
    &performance_metrics,
    &resource_metrics,
).await?;
```

#### 4. Metrics Storage (`storage.rs`)

Time-series storage with configurable retention:

```rust
use plugin_manager::metrics::storage::MetricsStorage;
use chrono::TimeDelta;

let storage = MetricsStorage::new(TimeDelta::hours(24));

// Store a single metric
storage.store(metric).await?;

// Store multiple metrics
storage.store_batch(metrics).await?;

// Query metrics
let query = MetricQuery::new()
    .with_metric("request_duration_ms".to_string())
    .with_time_range(start_time, end_time)
    .with_label("plugin".to_string(), "my_plugin".to_string())
    .with_limit(100);

let results = storage.query(query).await;

// Get latest metric
let latest = storage.get_latest("request_duration_ms").await;

// Get time range
let range = storage.get_range("request_duration_ms", start, end).await;

// Get storage stats
let stats = storage.get_stats().await;
println!("Total metrics: {}", stats.total_metrics);
```

#### 5. Metrics Exporters (`export.rs`)

Export metrics in different formats:

```rust
use plugin_manager::metrics::export::{PrometheusExporter, OpenTelemetryExporter};

// Prometheus exporter
let prom_exporter = PrometheusExporter::new();
let prometheus_format = prom_exporter.export().await?;

// OpenTelemetry exporter
let otel_exporter = OpenTelemetryExporter::new("http://localhost:4318".to_string());
let otel_format = otel_exporter.export().await?;

// Text exporter for debugging
let text_exporter = TextExporter::new();
let text_format = text_exporter.export().await?;
```

#### 6. Plugin Integration (`plugin_integration.rs`)

Easy integration for plugins to report metrics:

```rust
use plugin_manager::metrics::plugin_integration::PluginMetricsIntegration;

let integration = PluginMetricsIntegration::new(manager);

// Record operation timing
integration.record_call_start("my_plugin", "execute").await;

// ... perform operation ...

integration.record_call_end("my_plugin", "execute", true).await;

// Record custom metric
integration.record_custom_metric(
    "my_plugin",
    "custom_gauge".to_string(),
    Metric::gauge("custom_gauge".to_string(), 42.0),
).await;

// Increment counter
integration.increment_counter("my_plugin", "events_total".to_string(), 1).await;

// Set gauge
integration.set_gauge("my_plugin", "queue_size".to_string(), 10.0).await;

// Record histogram
integration.record_histogram(
    "my_plugin",
    "latency_ms".to_string(),
    vec![10.0, 20.0, 30.0],
).await;

// Get plugin summary
let summary = integration.get_plugin_summary("my_plugin").await;
println!("Health score: {}", summary.health_score);
```

## Built-in Metrics

### System Metrics

- `system_cpu_usage_percent` - Overall CPU usage
- `system_memory_mb` - System memory usage in MB
- `system_thread_count` - Active thread count
- `process_file_handles` - Open file handles

### Plugin Metrics

- `plugin_total_calls` - Total invocations
- `plugin_successful_calls` - Successful invocations
- `plugin_failed_calls` - Failed invocations
- `plugin_avg_latency_ms` - Average latency
- `plugin_p95_latency_ms` - 95th percentile latency
- `plugin_p99_latency_ms` - 99th percentile latency
- `plugin_error_rate` - Error rate
- `plugin_memory_mb` - Plugin memory usage
- `plugin_cpu_percent` - Plugin CPU usage

## Metric Types

### Counter

Counters are cumulative values that only increase:

```rust
let counter = Metric::counter("requests_total".to_string(), 42);
```

**Use cases**: Request counts, error counts, event totals

### Gauge

Gauges are point-in-time values that can go up or down:

```rust
let gauge = Metric::gauge("active_connections".to_string(), 100.0);
```

**Use cases**: Current queue size, active connections, memory usage

### Histogram

Histograms collect observations into configurable buckets:

```rust
let histogram = Metric::histogram("request_duration_ms".to_string(), vec![10.0, 20.0, 30.0]);
```

**Use cases**: Request latency, response sizes

## Labels

Labels provide dimensional data for metrics:

```rust
let metric = Metric::counter("requests_total".to_string(), 1)
    .with_label("plugin".to_string(), "my_plugin".to_string())
    .with_label("method".to_string(), "GET".to_string())
    .with_label("status".to_string(), "200".to_string());
```

**Best practices**:
- Keep label cardinality low (< 100 unique combinations)
- Use consistent label names across metrics
- Avoid high-cardinality labels like request IDs

## Querying Metrics

### Basic Query

```rust
let query = MetricQuery::new()
    .with_plugin("my_plugin".to_string())
    .with_metric("request_duration_ms".to_string());

let metrics = manager.query_metrics(query).await;
```

### Time Range Query

```rust
let start = Utc::now() - chrono::Duration::hours(1);
let end = Utc::now();

let query = MetricQuery::new()
    .with_time_range(start, end)
    .with_limit(1000);

let metrics = manager.query_metrics(query).await;
```

### Label Query

```rust
let query = MetricQuery::new()
    .with_label("environment".to_string(), "production".to_string())
    .with_label("version".to_string(), "1.0".to_string());

let metrics = manager.query_metrics(query).await;
```

## Prometheus Export

Metrics are exported in Prometheus text format:

```
# Skylet Plugin Metrics
# Generated by PrometheusExporter

plugin_requests_total{plugin="my_plugin",method="GET",status="200"} 1234567890
plugin_active_connections{plugin="my_plugin"} 1234567890
plugin_avg_latency_ms{plugin="my_plugin"} 42.5 1234567890
```

### Scrape Configuration

Configure Prometheus to scrape Skylet metrics:

```yaml
scrape_configs:
  - job_name: 'skylet'
    scrape_interval: 15s
    static_configs:
      - targets: ['localhost:9091']
```

### PromQL Examples

**Request rate per minute:**
```promql
rate(plugin_requests_total[1m])
```

**Error rate:**
```promql
rate(plugin_failed_calls[5m])
```

**P95 latency:**
```promql
histogram_quantile(0.95, rate(plugin_latency_ms_bucket[5m]))
```

**Health score per plugin:**
```promql
plugin_health_score
```

## OpenTelemetry Export

Metrics are exported in JSON format for OpenTelemetry collectors:

```json
{
  "name": "plugin_requests_total",
  "value": "42",
  "timestamp": 1234567890000,
  "labels": [
    ["plugin", "my_plugin"],
    ["method", "GET"]
  ],
  "type": "counter"
}
```

### Configuration

Configure OpenTelemetry collector endpoint:

```rust
let config = MetricsConfig {
    enable_otel: true,
    otel_endpoint: Some("http://localhost:4318".to_string()),
    ..Default::default()
};
```

## Performance Considerations

### Sampling

Use sampling to reduce overhead:

```rust
let config = MetricsConfig {
    sample_rate: 0.1, // Sample 10% of metrics
    ..Default::default()
};
```

### Retention

Configure retention based on storage capacity:

```rust
let config = MetricsConfig {
    retention_period: Duration::from_secs(86400), // 24 hours
    ..Default::default()
};
```

### Collection Interval

Balance freshness vs. overhead:

```rust
let config = MetricsConfig {
    collection_interval: Duration::from_secs(5), // Every 5 seconds
    ..Default::default()
};
```

## Plugin Integration

### Register Metrics Hooks

```rust
pub async fn init(context: *const PluginContextV2) -> PluginResultV2 {
    // Get metrics manager from context
    let manager = get_metrics_manager(context);

    // Register plugin
    manager.register_plugin("my_plugin".to_string()).await;

    PluginResultV2::Success
}
```

### Record Metrics in Operations

```rust
pub async fn execute(_context: *const PluginContextV2) -> PluginResultV2 {
    let integration = get_metrics_integration(context);

    // Record operation
    integration.record_call_start("my_plugin", "execute").await;

    // ... perform work ...

    let success = true;
    integration.record_call_end("my_plugin", "execute", success).await;

    PluginResultV2::Success
}
```

## Health Monitoring

### Plugin Health Score

Health scores range from 0.0 to 1.0:

- **1.0**: Perfect health (no errors, low latency)
- **0.7**: Good health (few errors, acceptable latency)
- **0.4**: Poor health (many errors, high latency)
- **0.0**: Critical health (all failing)

### Alerting

Set up alerts based on metrics:

```promql
# High error rate
alert: HighErrorRate
expr: rate(plugin_failed_calls[5m]) > 0.1

# High latency
alert: HighLatency
expr: plugin_p99_latency_ms > 1000

# Low health score
alert: LowHealthScore
expr: plugin_health_score < 0.7
```

## Troubleshooting

### Common Issues

#### Metrics Not Being Collected

- Check if metrics collection is enabled
- Verify collection interval is reasonable
- Check logs for collection errors

#### High Memory Usage

- Reduce retention period
- Lower sample rate
- Increase cleanup interval

#### Performance Impact

- Increase sample rate
- Reduce collection frequency
- Disable unnecessary metrics

### Debug Logging

Enable debug logging for metrics:

```bash
RUST_LOG=plugin_manager::metrics=debug cargo run
```

## Best Practices

### Metric Naming

- Use snake_case names
- Include units in name (e.g., `_ms`, `_bytes`, `_percent`)
- Be descriptive but concise

**Good:**
- `request_duration_ms`
- `connections_active`
- `errors_total`

**Bad:**
- `r` (too vague)
- `RequestDuration` (inconsistent case)
- `duration_without_unit` (missing unit)

### Metric Types

- Use counters for cumulative counts
- Use gauges for current values
- Use histograms for distributions

### Label Cardinality

Keep label cardinality low (< 100 combinations):

**Avoid:**
```rust
.with_label("request_id".to_string(), "abc123") // High cardinality!
```

**Prefer:**
```rust
.with_label("endpoint".to_string(), "/api/v1/users")
.with_label("method".to_string(), "GET")
```

## API Reference

### Types

- `Metric` - Single metric data point
- `MetricType` - Counter, Gauge, Histogram, Summary
- `MetricValue` - Counter, Gauge, Histogram, Summary variants
- `PerformanceMetrics` - Plugin performance metrics
- `ResourceMetrics` - System resource metrics
- `PluginMetrics` - Comprehensive plugin metrics
- `MetricQuery` - Metrics query builder
- `MetricsConfig` - Metrics system configuration
- `MetricsError` - Error types

### Functions

See module documentation for detailed API references:

```rust
use plugin_manager::metrics;
```

## Future Enhancements

Planned features for metrics collection:

1. **Metrics Aggregation**: Pre-computed aggregates (sum, avg, percentiles)
2. **Alert Management**: Built-in alert evaluation and notifications
3. **Metrics Dashboard**: Real-time visualization web UI
4. **Metrics Versioning**: Track metric schema changes over time
5. **Dynamic Buckets**: Configurable histogram buckets
5. **Metrics Export Scheduling**: Scheduled export to external systems
6. **Metrics Compression**: Efficient storage of historical data
7. **Metrics Federation**: Multi-node metrics aggregation
8. **Custom Aggregations**: User-defined aggregation functions
9. **Metrics Anomalies**: Automatic anomaly detection
10. **Metrics Rollup**: Time-based rollup aggregation

## License

Metrics Collection Module is part of Skylet and licensed under MIT OR Apache-2.0 license.
