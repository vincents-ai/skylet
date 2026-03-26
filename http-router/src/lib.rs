// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Skylet HTTP Router
//!
//! A lightweight HTTP router for plugin-provided API endpoints (RFC-0019).
//!
//! ## Features
//!
//! - Route registration with path parameters (e.g., `/users/{id}`)
//! - HTTP method routing (GET, POST, PUT, DELETE, PATCH)
//! - OpenAPI documentation generation
//! - Middleware support
//!
//! ## Usage
//!
//! ```rust,ignore
//! use http_router::{HttpRouter, HttpMethod, RouteConfig};
//! use skylet_abi::{HttpRequest, HttpResponse};
//!
//! let router = HttpRouter::new();
//!
//! router.register_route(RouteConfig {
//!     method: HttpMethod::Get,
//!     path: "/api/hello".to_string(),
//!     handler: Arc::new(|req: &HttpRequest, params: &HashMap<String, String>| {
//!         HttpResponse { status_code: 200, body: b"Hello!".to_vec(), ... }
//!     }),
//!     description: Some("Say hello".to_string()),
//! });
//!
//! // Route requests
//! let response = router.route(&request);
//! ```
//!
//! ## Path Parameters
//!
//! Path parameters are extracted using `{name}` syntax:
//!
//! ```text
//! /users/{id}       -> params["id"] = "123"
//! /posts/{id}/comments/{cid} -> params["id"] = "1", params["cid"] = "42"
//! ```

pub mod router;
pub use router::*;
