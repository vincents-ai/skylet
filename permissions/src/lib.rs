// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Permissions Plugin - RFC-0023 User-Level Permissions and Context
//!
//! This plugin provides authentication, authorization, and user context management:
//! - User identity with AGE keys and session tokens
//! - Authentication providers (local, OAuth2 ready)
//! - Authorization with RBAC and permission caching
//! - Multi-tenancy support
//! - Audit logging for auth/authz decisions
//!
//! ## V2 ABI Implementation
//!
//! This plugin implements RFC-0004 v2 ABI.
//!
//! ## HTTP Endpoints (GAP-003)
//!
//! The `http` module provides Axum handlers for:
//! - POST /auth/login - Authenticate user
//! - GET /auth/validate - Validate session token
//! - POST /auth/logout - Invalidate session
//! - POST /auth/register - Register new user

// Export v2 ABI implementation
mod v2_ffi;
pub use v2_ffi::*;

// Core types
pub mod types;
pub use types::*;

// Authentication
pub mod auth;
pub use auth::*;

// Authorization
pub mod authz;
pub use authz::*;

// HTTP handlers (GAP-003)
pub mod http;
pub use http::*;
