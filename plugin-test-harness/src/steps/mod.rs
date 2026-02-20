// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! BDD Step Definitions for Plugin Testing
//!
//! This module provides cucumber step definitions for testing Skylet plugins.
//! Steps are organized by category: plugin lifecycle, actions, assertions.

pub mod plugin_steps;

// Re-export all steps for convenience
#[allow(unused_imports)]
pub use plugin_steps::*;
