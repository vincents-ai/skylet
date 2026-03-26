// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Skylet Plugin Execution Engine
//!
//! Skylet is a plugin-based execution engine where all functionality is delivered through
//! dynamically loaded shared libraries (plugins) that communicate over a shared, in-process
//! function bus.
//!
//! ## Architecture
//!
//! The platform provides:
//! - A **bootstrap sequence** that initializes foundation services
//! - A **plugin ABI contract** defining the interface between host and plugins
//! - A **service registry** for inter-plugin communication
//!
//! ## Core Modules
//!
//! - [`bootstrap`] - Plugin bootstrap and initialization sequence
//! - [`config`] - Application and plugin configuration management
//! - [`memory`] - Memory management utilities for plugin data
//! - [`observability`] - Logging, tracing, and metrics collection
//! - [`plugin_manager`] - Plugin lifecycle management (load, unload, hot-reload)
//! - [`security`] - Security policies, sandboxing, and credential management
//! - [`startup`] - Application startup and shutdown orchestration
//! - [`types`] - Common type definitions shared across modules
//!
//! ## Plugin Lifecycle
//!
//! Plugins move through defined states:
//! ```text
//! Discovered -> Downloaded -> Installed -> Loaded -> Initialized -> Running
//!                                                                     |
//!                                                          (optionally) Suspended
//!                                                                     |
//!                                                     Shutdown -> Uninstalled
//! ```
//!
//! ## The Shared Function Bus
//!
//! The bus has three complementary layers:
//! 1. **Service Registry** - Synchronous, named function pointers
//! 2. **RPC Layer** - Request/response over named methods
//! 3. **EventBus** - Asynchronous publish/subscribe
//!
//! ## Example
//!
//! ```rust,ignore
//! use skylet::startup::{StartupOptimizer, StartupConfig};
//!
//! let config = StartupConfig::default();
//! let optimizer = StartupOptimizer::new(config);
//! ```

pub mod bootstrap;
// pub mod cache;
pub mod config;
pub mod memory;
pub mod observability;
pub mod plugin_manager;
pub mod security;
pub mod startup;
// pub mod testing_comprehensive;
pub mod types;
