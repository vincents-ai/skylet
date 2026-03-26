// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Security and Capabilities Module - RFC-0008
//!
//! This module provides capability-based sandboxing, network access control,
//! filesystem permissions, and security policy enforcement for plugins.
//!
//! # Architecture
//!
//! - `capabilities`: ABI-level capability declarations (FilesystemAccess, NetworkAccess, etc.)
//! - `network`: Network access control with whitelist-based enforcement
//! - `filesystem`: Filesystem permission enforcement
//! - `policy`: Security policy engine and approval workflow

pub mod capabilities;
pub mod filesystem;
pub mod network;
pub mod policy;

// Re-export main types for convenience
pub use capabilities::{
    CapabilityCheckResult, CapabilityInfo, CapabilityStatus, CapabilityType, CommandExecution,
    FilesystemAccess, FilesystemAccessMode, NetworkAccess,
};
pub use filesystem::{FilesystemAccessError, FilesystemEnforcer, PathPermission};
pub use network::{host_matches_pattern, HostPattern, NetworkAccessError, NetworkEnforcer};
pub use policy::{
    ApprovalStatus, CapabilityApproval, PolicyError, RiskLevel, SecurityPolicy,
    SecurityPolicyEngine,
};
