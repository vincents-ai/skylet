//! Unit tests for metrics system

use super::*;
use crate::plugin_manager::metrics::*;
use std::time::Duration;

#[cfg(test)]
mod metrics_collector_tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collection() {
        let collector = MetricsCollector::new(MetricsConfig::default());

        collector.record_counter("test_counter", 1.0).await;
        collector.record_gauge("test_gauge", 42.0).await;
        collector.record_histogram("test_histogram", 10.5).await;

        let metrics = collector.get_metrics().await;

        assert!(metrics.contains_key("test_counter"));
        assert!(metrics.contains_key("test_gauge"));
        assert!(metrics.contains_key("test_histogram"));
    }

    #[tokio::test]
    async fn test_counter_increment() {
        let collector = MetricsCollector::new(MetricsConfig::default());

        collector.record_counter("test_counter", 5.0).await;
        collector.record_counter("test_counter", 3.0).await;

        let metrics = collector.get_metrics().await;
        let counter_value = metrics.get("test_counter").unwrap();

        assert_eq!(counter_value.value, 8.0);
    }

    #[tokio::test]
    async fn test_gauge_set() {
        let collector = MetricsCollector::new(MetricsConfig::default());

        collector.record_gauge("test_gauge", 10.0).await;
        collector.record_gauge("test_gauge", 20.0).await;

        let metrics = collector.get_metrics().await;
        let gauge_value = metrics.get("test_gauge").unwrap();

        assert_eq!(gauge_value.value, 20.0);
    }

    #[tokio::test]
    async fn test_histogram_statistics() {
        let collector = MetricsCollector::new(MetricsConfig::default());

        for value in [1.0, 2.0, 3.0, 4.0, 5.0] {
            collector.record_histogram("test_histogram", value).await;
        }

        let metrics = collector.get_metrics().await;
        let histogram = metrics.get("test_histogram").unwrap();

        assert_eq!(histogram.count, 5);
        assert_eq!(histogram.sum, 15.0);
        assert_eq!(histogram.avg, 3.0);
        assert_eq!(histogram.min, 1.0);
        assert_eq!(histogram.max, 5.0);
    }

    #[tokio::test]
    async fn test_metric_tags() {
        let collector = MetricsCollector::new(MetricsConfig::default());

        let mut tags = std::collections::HashMap::new();
        tags.insert("plugin".to_string(), "test_plugin".to_string());
        tags.insert("env".to_string(), "dev".to_string());

        collector
            .record_counter_with_tags("test_counter", 1.0, tags.clone())
            .await;

        let metrics = collector.get_metrics().await;
        let metric = metrics.get("test_counter").unwrap();

        assert!(metric.tags.contains_key("plugin"));
        assert!(metric.tags.contains_key("env"));
    }
}

#[cfg(test)]
mod system_metrics_tests {
    use super::*;

    #[tokio::test]
    async fn test_cpu_usage_collection() {
        let collector = SystemMetricsCollector::new();

        let cpu_usage = collector.collect_cpu_usage().await;

        assert!(cpu_usage >= 0.0 && cpu_usage <= 100.0);
    }

    #[tokio::test]
    async fn test_memory_usage_collection() {
        let collector = SystemMetricsCollector::new();

        let memory_usage = collector.collect_memory_usage().await;

        assert!(memory_usage.total > 0);
        assert!(memory_usage.used > 0);
        assert!(memory_usage.used <= memory_usage.total);
    }

    #[tokio::test]
    async fn test_disk_usage_collection() {
        let collector = SystemMetricsCollector::new();

        let disk_usage = collector.collect_disk_usage("/").await;

        assert!(disk_usage.total > 0);
        assert!(disk_usage.used > 0);
        assert!(disk_usage.used <= disk_usage.total);
    }

    #[tokio::test]
    async fn test_network_metrics_collection() {
        let collector = SystemMetricsCollector::new();

        let network_metrics = collector.collect_network_metrics().await;

        assert!(network_metrics.bytes_sent >= 0);
        assert!(network_metrics.bytes_received >= 0);
        assert!(network_metrics.packets_sent >= 0);
        assert!(network_metrics.packets_received >= 0);
    }

    #[tokio::test]
    async fn test_process_metrics_collection() {
        let collector = SystemMetricsCollector::new();

        let process_metrics = collector.collect_process_metrics().await;

        assert!(process_metrics.memory_usage > 0);
        assert!(process_metrics.cpu_usage >= 0.0);
        assert!(process_metrics.thread_count > 0);
    }
}

#[cfg(test)]
mod plugin_metrics_tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_metrics_tracking() {
        let plugin_metrics = PluginMetricsTracker::new("test_plugin".to_string());

        plugin_metrics.record_execution(Duration::from_millis(100)).await;
        plugin_metrics.record_success().await;
        plugin_metrics.record_error().await;

        let metrics = plugin_metrics.get_metrics().await;

        assert_eq!(metrics.execution_count, 2);
        assert_eq!(metrics.success_count, 1);
        assert_eq!(metrics.error_count, 1);
        assert_eq!(metrics.total_execution_time_ms, 100);
    }

    #[tokio::test]
    async fn test_plugin_performance_metrics() {
        let plugin_metrics = PluginMetricsTracker::new("test_plugin".to_string());

        plugin_metrics.record_execution(Duration::from_millis(50)).await;
        plugin_metrics.record_execution(Duration::from_millis(100)).await;
        plugin_metrics.record_execution(Duration::from_millis(150)).await;

        let metrics = plugin_metrics.get_metrics().await;

        assert_eq!(metrics.execution_count, 3);
        assert_eq!(metrics.total_execution_time_ms, 300);
        assert_eq!(metrics.avg_execution_time_ms, 100.0);
        assert_eq!(metrics.min_execution_time_ms, 50);
        assert_eq!(metrics.max_execution_time_ms, 150);
    }

    #[tokio::test]
    async fn test_plugin_resource_usage() {
        let plugin_metrics = PluginMetricsTracker::new("test_plugin".to_string());

        plugin_metrics
            .update_resource_usage(ResourceUsage {
                memory_mb: 100.0,
                cpu_percent: 50.0,
                file_handles: 10,
                network_connections: 5,
            })
            .await;

        let metrics = plugin_metrics.get_metrics().await;

        assert_eq!(metrics.current_memory_mb, 100.0);
        assert_eq!(metrics.current_cpu_percent, 50.0);
        assert_eq!(metrics.file_handles, 10);
        assert_eq!(metrics.network_connections, 5);
    }
}

#[cfg(test)]
mod metrics_export_tests {
    use super::*;

    #[test]
    fn test_prometheus_export() {
        let metrics = std::collections::HashMap::from([
            ("test_counter".to_string(), MetricValue {
                metric_type: MetricType::Counter,
                value: 42.0,
                timestamp: chrono::Utc::now(),
                tags: std::collections::HashMap::new(),
                count: 0,
                sum: 0.0,
                avg: 0.0,
                min: 0.0,
                max: 0.0,
            }),
        ]);

        let exporter = PrometheusExporter::new();
        let output = exporter.export(&metrics);

        assert!(output.contains("test_counter"));
        assert!(output.contains("42"));
        assert!(output.contains("TYPE test_counter counter"));
    }

    #[test]
    fn test_json_export() {
        let metrics = std::collections::HashMap::from([
            ("test_metric".to_string(), MetricValue {
                metric_type: MetricType::Gauge,
                value: 100.0,
                timestamp: chrono::Utc::now(),
                tags: std::collections::HashMap::new(),
                count: 0,
                sum: 0.0,
                avg: 0.0,
                min: 0.0,
                max: 0.0,
            }),
        ]);

        let exporter = JsonExporter::new();
        let output = exporter.export(&metrics);

        assert!(output.contains("test_metric"));
        assert!(output.contains("100"));

        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(json["test_metric"].is_object());
        assert_eq!(json["test_metric"]["value"], 100.0);
    }

    #[test]
    fn test_csv_export() {
        let metrics = std::collections::HashMap::from([
            ("metric1".to_string(), MetricValue {
                metric_type: MetricType::Counter,
                value: 10.0,
                timestamp: chrono::Utc::now(),
                tags: std::collections::HashMap::new(),
                count: 0,
                sum: 0.0,
                avg: 0.0,
                min: 0.0,
                max: 0.0,
            }),
            ("metric2".to_string(), MetricValue {
                metric_type: MetricType::Gauge,
                value: 20.0,
                timestamp: chrono::Utc::now(),
                tags: std::collections::HashMap::new(),
                count: 0,
                sum: 0.0,
                avg: 0.0,
                min: 0.0,
                max: 0.0,
            }),
        ]);

        let exporter = CsvExporter::new();
        let output = exporter.export(&metrics);

        assert!(output.contains("metric1,10"));
        assert!(output.contains("metric2,20"));
        assert!(output.lines().count() >= 3); // Header + 2 data rows
    }
}

#[cfg(test)]
mod metrics_storage_tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_storage() {
        let storage = MetricsStorage::new(chrono::Duration::hours(1));

        let metric = MetricValue {
            metric_type: MetricType::Counter,
            value: 42.0,
            timestamp: chrono::Utc::now(),
            tags: std::collections::HashMap::new(),
            count: 0,
            sum: 0.0,
            avg: 0.0,
            min: 0.0,
            max: 0.0,
        };

        storage.store_metric("test_plugin", "test_metric", metric).await;

        let retrieved = storage
            .get_metrics("test_plugin", "test_metric")
            .await
            .unwrap();

        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].value, 42.0);
    }

    #[tokio::test]
    async fn test_metrics_query_by_time_range() {
        let storage = MetricsStorage::new(chrono::Duration::hours(1));

        let now = chrono::Utc::now();

        let metric = MetricValue {
            metric_type: MetricType::Gauge,
            value: 100.0,
            timestamp: now,
            tags: std::collections::HashMap::new(),
            count: 0,
            sum: 0.0,
            avg: 0.0,
            min: 0.0,
            max: 0.0,
        };

        storage.store_metric("test_plugin", "test_metric", metric).await;

        let retrieved = storage
            .get_metrics_by_time_range(
                "test_plugin",
                "test_metric",
                now - chrono::Duration::minutes(5),
                now + chrono::Duration::minutes(5),
            )
            .await
            .unwrap();

        assert_eq!(retrieved.len(), 1);
    }

    #[tokio::test]
    async fn test_metrics_retention() {
        let storage = MetricsStorage::new(chrono::Duration::milliseconds(100));

        let metric = MetricValue {
            metric_type: MetricType::Counter,
            value: 1.0,
            timestamp: chrono::Utc::now(),
            tags: std::collections::HashMap::new(),
            count: 0,
            sum: 0.0,
            avg: 0.0,
            min: 0.0,
            max: 0.0,
        };

        storage.store_metric("test_plugin", "test_metric", metric).await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        storage.cleanup_old_metrics().await;

        let retrieved = storage
            .get_metrics("test_plugin", "test_metric")
            .await;

        assert!(retrieved.is_err() || retrieved.unwrap().is_empty());
    }
}

#[cfg(test)]
mod metrics_aggregation_tests {
    use super::*;

    #[test]
    fn test_counter_aggregation() {
        let metrics = vec![
            MetricValue {
                metric_type: MetricType::Counter,
                value: 10.0,
                timestamp: chrono::Utc::now(),
                tags: std::collections::HashMap::new(),
                count: 0,
                sum: 0.0,
                avg: 0.0,
                min: 0.0,
                max: 0.0,
            },
            MetricValue {
                metric_type: MetricType::Counter,
                value: 20.0,
                timestamp: chrono::Utc::now(),
                tags: std::collections::HashMap::new(),
                count: 0,
                sum: 0.0,
                avg: 0.0,
                min: 0.0,
                max: 0.0,
            },
            MetricValue {
                metric_type: MetricType::Counter,
                value: 30.0,
                timestamp: chrono::Utc::now(),
                tags: std::collections::HashMap::new(),
                count: 0,
                sum: 0.0,
                avg: 0.0,
                min: 0.0,
                max: 0.0,
            },
        ];

        let aggregated = MetricsAggregator::aggregate(metrics);

        assert_eq!(aggregated.value, 60.0);
    }

    #[test]
    fn test_gauge_aggregation() {
        let metrics = vec![
            MetricValue {
                metric_type: MetricType::Gauge,
                value: 10.0,
                timestamp: chrono::Utc::now(),
                tags: std::collections::HashMap::new(),
                count: 0,
                sum: 0.0,
                avg: 0.0,
                min: 0.0,
                max: 0.0,
            },
            MetricValue {
                metric_type: MetricType::Gauge,
                value: 20.0,
                timestamp: chrono::Utc::now(),
                tags: std::collections::HashMap::new(),
                count: 0,
                sum: 0.0,
                avg: 0.0,
                min: 0.0,
                max: 0.0,
            },
        ];

        let aggregated = MetricsAggregator::aggregate(metrics);

        // Gauges use the latest value
        assert_eq!(aggregated.value, 20.0);
    }

    #[test]
    fn test_histogram_aggregation() {
        let metrics = vec![
            MetricValue {
                metric_type: MetricType::Histogram,
                value: 10.0,
                timestamp: chrono::Utc::now(),
                tags: std::collections::HashMap::new(),
                count: 0,
                sum: 0.0,
                avg: 0.0,
                min: 0.0,
                max: 0.0,
            },
            MetricValue {
                metric_type: MetricType::Histogram,
                value: 20.0,
                timestamp: chrono::Utc::now(),
                tags: std::collections::HashMap::new(),
                count: 0,
                sum: 0.0,
                avg: 0.0,
                min: 0.0,
                max: 0.0,
            },
            MetricValue {
                metric_type: MetricType::Histogram,
                value: 30.0,
                timestamp: chrono::Utc::now(),
                tags: std::collections::HashMap::new(),
                count: 0,
                sum: 0.0,
                avg: 0.0,
                min: 0.0,
                max: 0.0,
            },
        ];

        let aggregated = MetricsAggregator::aggregate(metrics);

        assert_eq!(aggregated.count, 3);
        assert_eq!(aggregated.sum, 60.0);
        assert_eq!(aggregated.avg, 20.0);
        assert_eq!(aggregated.min, 10.0);
        assert_eq!(aggregated.max, 30.0);
    }
}
