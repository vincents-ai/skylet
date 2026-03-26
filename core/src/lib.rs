// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Skylet Core
//!
//! Core functionality for the Skylet plugin execution engine, providing service
//! registry, authentication, and testing utilities.
//!
//! ## Modules
//!
//! - [`auth`] - Authentication and authorization primitives
//! - [`framework`] - Test framework for plugin integration testing
//! - [`service_registry`] - Service discovery and registration for inter-plugin communication
//!
//! ## Service Registry
//!
//! The service registry is the heart of inter-plugin communication. Plugins register
//! named services and discover services from other plugins:
//!
//! ```rust,ignore
//! use skylet_core::service_registry::ServiceRegistry;
//!
//! let registry = ServiceRegistry::new();
//! registry.register("database", db_service);
//! let service = registry.get("database");
//! ```
//!
//! ## Testing Framework
//!
//! The test framework provides isolated environments for testing plugins:
//!
//! ```rust,ignore
//! use skylet_core::framework::TestFramework;
//!
//! let framework = TestFramework::new("my-test")?;
//! let env = framework.create_environment()?;
//! // Run tests in isolated environment
//! ```

pub mod auth;
pub mod framework;
pub mod service_registry;
pub mod tests;
pub use crate::service_registry::*;

pub use framework::TestFramework;
