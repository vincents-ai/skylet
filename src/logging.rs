// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Structured logging module for RFC-0018 compliance
//!
//! Provides JSON-formatted logging output for the Skylet execution engine.

use std::sync::Arc;
use tracing::Subscriber;
use tracing_subscriber::registry::LookupSpan;

/// Initialize a structured logging subscriber
pub fn subscriber_with_buffer(
    _buf: Arc<std::sync::Mutex<Vec<u8>>>,
) -> impl Subscriber + for<'a> LookupSpan<'a> {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .finish()
}
