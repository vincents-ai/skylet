// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Job Queue Plugin - Background job processing system
//!
//! This plugin provides background job processing with:
//! - Persistent job storage (SQLite)
//! - Retry logic with exponential backoff
//! - Thread-safe concurrent job submission
//!
//! ## V2 ABI Implementation
//!
//! This plugin implements RFC-0004 v2 ABI.

#[cfg(feature = "plugin")]
mod v2_ffi;

pub mod job_queue;
pub use job_queue::*;
