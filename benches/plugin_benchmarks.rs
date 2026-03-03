// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Performance benchmarks for Skylet plugin execution engine

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// Benchmark plugin loading time
fn bench_plugin_loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("plugin_loading");

    for plugin_count in [1, 5, 10, 20, 50] {
        group.bench_with_input(
            BenchmarkId::from_parameter(plugin_count),
            plugin_count,
            |b, &count| {
                b.iter(|| {
                    // Simulate loading count plugins
                    black_box(load_mock_plugins(count));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark plugin execution time
fn bench_plugin_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("plugin_execution");

    for operation_type in ["simple", "complex", "io_heavy"] {
        group.bench_with_input(
            BenchmarkId::from_parameter(operation_type),
            operation_type,
            |b, op_type| {
                b.iter(|| {
                    // Simulate plugin execution
                    black_box(execute_plugin_mock(op_type));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark event publishing
fn bench_event_publishing(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_publishing");

    for event_count in [10, 100, 1000, 10000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(event_count),
            event_count,
            |b, &count| {
                b.iter(|| {
                    // Simulate publishing count events
                    black_box(publish_mock_events(count));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark metrics collection
fn bench_metrics_collection(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_collection");

    for metric_count in [10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(metric_count),
            metric_count,
            |b, &count| {
                b.iter(|| {
                    // Simulate collecting count metrics
                    black_box(collect_mock_metrics(count));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark configuration loading
fn bench_config_loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_loading");

    for plugin_count in [1, 5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::from_parameter(plugin_count),
            plugin_count,
            |b, &count| {
                b.iter(|| {
                    // Simulate loading configurations for count plugins
                    black_box(load_mock_configs(count));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark dependency resolution
fn bench_dependency_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("dependency_resolution");

    for dependency_complexity in ["linear", "tree", "complex"] {
        group.bench_with_input(
            BenchmarkId::from_parameter(dependency_complexity),
            dependency_complexity,
            |b, complexity| {
                b.iter(|| {
                    // Simulate resolving dependencies
                    black_box(resolve_mock_dependencies(complexity));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark hot reload
fn bench_hot_reload(c: &mut Criterion) {
    let mut group = c.benchmark_group("hot_reload");

    for plugin_count in [1, 5, 10] {
        group.bench_with_input(
            BenchmarkId::from_parameter(plugin_count),
            plugin_count,
            |b, &count| {
                b.iter(|| {
                    // Simulate hot reloading count plugins
                    black_box(reload_mock_plugins(count));
                });
            },
        );
    }

    group.finish();
}

// Mock helper functions for benchmarking

fn load_mock_plugins(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("plugin_{}", i)).collect()
}

fn execute_plugin_mock(operation_type: &str) -> String {
    match operation_type {
        "simple" => "simple result".to_string(),
        "complex" => {
            // Simulate complex calculation
            let mut result = 0;
            for i in 0..1000 {
                result += i * i;
            }
            format!("complex result: {}", result)
        }
        "io_heavy" => {
            // Simulate I/O operation
            std::thread::sleep(std::time::Duration::from_micros(10));
            "io heavy result".to_string()
        }
        _ => "unknown result".to_string(),
    }
}

fn publish_mock_events(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("event_{}", i)).collect()
}

fn collect_mock_metrics(count: usize) -> Vec<f64> {
    (0..count).map(|i| i as f64).collect()
}

fn load_mock_configs(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("config_{}", i)).collect()
}

fn resolve_mock_dependencies(complexity: &str) -> Vec<String> {
    match complexity {
        "linear" => vec!["plugin1".to_string(), "plugin2".to_string()],
        "tree" => vec![
            "plugin1".to_string(),
            "plugin2".to_string(),
            "plugin3".to_string(),
            "plugin4".to_string(),
        ],
        "complex" => {
            vec![
                "plugin1".to_string(),
                "plugin2".to_string(),
                "plugin3".to_string(),
                "plugin4".to_string(),
                "plugin5".to_string(),
            ]
        }
        _ => vec![],
    }
}

fn reload_mock_plugins(count: usize) -> Vec<String> {
    (0..count)
        .map(|i| format!("reloaded_plugin_{}", i))
        .collect()
}

criterion_group!(
    benches,
    bench_plugin_loading,
    bench_plugin_execution,
    bench_event_publishing,
    bench_metrics_collection,
    bench_config_loading,
    bench_dependency_resolution,
    bench_hot_reload
);
criterion_main!(benches);
