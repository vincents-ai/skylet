// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

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
pub use capabilities::{CapabilityInfo, CapabilityType, FilesystemAccess, FilesystemAccessMode, NetworkAccess, CommandExecution, CapabilityStatus, CapabilityCheckResult};
pub use filesystem::{FilesystemEnforcer, PathPermission, FilesystemAccessError};
pub use network::{NetworkEnforcer, HostPattern, NetworkAccessError, host_matches_pattern};
pub use policy::{SecurityPolicyEngine, CapabilityApproval, ApprovalStatus, RiskLevel, SecurityPolicy, PolicyError};
