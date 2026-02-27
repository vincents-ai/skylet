// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

pub mod config;
pub mod dependency_resolver;
pub mod discovery;
pub mod dynamic_reload;
pub mod epoch_guard;
pub mod events;
pub mod failover;
pub mod hot_reload;
pub mod lifecycle;
pub mod metrics;
/// Plugin Manager Module
///
/// Provides complete plugin lifecycle management with support for both
/// ABI v1 (legacy) and ABI v2 (RFC-0004) plugins.
///
/// # Modules
/// - `manager`: Core plugin loading and unloading (ABI v1/v2)
/// - `config`: Advanced configuration management (Phase 2) - schema validation, env vars, hot-reload, multi-env
/// - `metrics`: Advanced metrics collection (Phase 2) - performance, resource usage, Prometheus/OTel export
/// - `events`: Advanced event system (Phase 2) - pattern routing, persistence, filtering, plugin communication
/// - `lifecycle`: Full lifecycle automation (RFC-0002) - install, activate, deactivate, uninstall
/// - `failover`: Plugin failover and recovery
/// - `hot_reload`: Hot-reload service (RFC-0007) - file watching, state serialization, graceful reload
/// - `dynamic_reload`: Dynamic plugin reload with state preservation
/// - `dependency_resolver`: Plugin dependency ordering (CQ-004) - topological sort based on dependencies
/// - `discovery`: Dynamic plugin discovery (CQ-003) - filesystem-based plugin scanning
pub mod manager;
