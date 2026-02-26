// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Capability Declaration Types - RFC-0008
//!
//! ABI-level capability declarations that plugins use to declare their
//! required permissions. These types are FFI-safe and used in PluginInfoV2.

use std::ffi::{c_char, c_void};
use std::fmt;

/// Filesystem access mode for capability declarations
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemAccessMode {
    /// Read-only access
    Read = 0,
    /// Read and write access
    ReadWrite = 1,
    /// Write-only access (e.g., for logs)
    WriteOnly = 2,
    /// Append-only access
    Append = 3,
}

/// Filesystem access capability declaration
///
/// Declares access to a specific file or directory path.
/// Uses VFS URI format for abstraction.
#[repr(C)]
pub struct FilesystemAccess {
    /// The VFS URI to the file or directory
    /// Examples: "shared-data://logs/", "plugin-data://cache/", "config://settings.json"
    pub uri: *const c_char,
    /// Access mode for this path
    pub mode: FilesystemAccessMode,
    /// Whether this access is required (failure if denied)
    pub required: bool,
}

/// Network access capability declaration
///
/// Declares allowed network connections by host pattern.
#[repr(C)]
pub struct NetworkAccess {
    /// The target host and port pattern
    /// Examples: "api.github.com:443", "*.google.com:*", "localhost:8080"
    pub host_pattern: *const c_char,
    /// TCP/UDP protocol restriction (null = any)
    pub protocol: *const c_char,
    /// Whether this access is required (failure if denied)
    pub required: bool,
}

/// Command execution capability declaration
///
/// Declares what external commands the plugin can execute.
#[repr(C)]
pub struct CommandExecution {
    /// True if the plugin needs to execute any arbitrary command
    pub allows_arbitrary: bool,
    /// List of specific commands allowed if allows_arbitrary is false
    pub allowed_commands: *const *const c_char,
    /// Number of allowed commands
    pub num_allowed_commands: usize,
    /// Whether command execution is required (failure if denied)
    pub required: bool,
}

/// Capability type discriminator for CapabilityInfo
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityType {
    /// Filesystem access capability
    Filesystem = 0,
    /// Network access capability
    Network = 1,
    /// Command execution capability
    Command = 2,
    /// Database access capability
    Database = 3,
    /// Secrets access capability
    Secrets = 4,
    /// Custom/extended capability
    Custom = 99,
}

/// Capability information for fine-grained permission system
///
/// This is the main capability declaration struct used in PluginInfoV2.
/// Each capability declares a specific permission requirement.
#[repr(C)]
pub struct CapabilityInfo {
    /// Type of capability being declared
    pub type_: CapabilityType,
    /// Pointer to the specific capability data based on type_
    /// - Filesystem: *const FilesystemAccess
    /// - Network: *const NetworkAccess
    /// - Command: *const CommandExecution
    pub data: *const c_void,
    /// Human-readable description of why this capability is needed
    pub description: *const c_char,
}

/// Capability status after approval workflow
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityStatus {
    /// Capability has not been reviewed yet
    Pending = 0,
    /// Capability approved by administrator
    Approved = 1,
    /// Capability denied by administrator
    Denied = 2,
    /// Capability automatically approved (within safe limits)
    AutoApproved = 3,
    /// Capability revoked after approval
    Revoked = 4,
}

/// Result of a capability check
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityCheckResult {
    /// Capability is granted
    Granted = 0,
    /// Capability is denied
    Denied = 1,
    /// Capability requires approval
    RequiresApproval = 2,
    /// Capability is not declared
    NotDeclared = 3,
    /// Invalid capability specification
    Invalid = 4,
}

/// Helper functions for working with capabilities
impl CapabilityInfo {
    /// Validate capability data pointer and size
    /// Phase 2 Issue #5: Type confusion prevention
    fn validate_data_pointer(&self) -> bool {
        if self.data.is_null() {
            return false;
        }

        // Validate pointer address (no null or obviously invalid addresses)
        let ptr_addr = self.data as usize;
        if ptr_addr < 0x1000 || ptr_addr == usize::MAX {
            return false;
        }

        true
    }

    /// Check if this capability is required
    /// Phase 2 Issue #5: Enhanced with type confusion prevention
    pub fn is_required(&self) -> bool {
        // RFC-0004-SEC-003: Validate pointer before access
        if !self.validate_data_pointer() {
            return false;
        }

        match self.type_ {
            CapabilityType::Filesystem => {
                // RFC-0004-SEC-003: Validate inner pointer
                unsafe {
                    let fs = self.data as *const FilesystemAccess;
                    // Check URI pointer if present
                    if !(*fs).uri.is_null() {
                        // Validate it looks like a reasonable pointer
                        let uri_addr = (*fs).uri as usize;
                        if uri_addr < 0x1000 || uri_addr == usize::MAX {
                            return false;
                        }
                    }
                    (*fs).required
                }
            }
            CapabilityType::Network => {
                // RFC-0004-SEC-003: Validate inner pointers
                unsafe {
                    let net = self.data as *const NetworkAccess;
                    // Check host_pattern pointer
                    if !(*net).host_pattern.is_null() {
                        let addr = (*net).host_pattern as usize;
                        if addr < 0x1000 || addr == usize::MAX {
                            return false;
                        }
                    }
                    // Check protocol pointer if present
                    if !(*net).protocol.is_null() {
                        let addr = (*net).protocol as usize;
                        if addr < 0x1000 || addr == usize::MAX {
                            return false;
                        }
                    }
                    (*net).required
                }
            }
            CapabilityType::Command => {
                // RFC-0004-SEC-003: Validate inner pointers and bounds
                unsafe {
                    let cmd = self.data as *const CommandExecution;
                    // Check allowed_commands pointer if present
                    if !(*cmd).allowed_commands.is_null() {
                        let addr = (*cmd).allowed_commands as usize;
                        if addr < 0x1000 || addr == usize::MAX {
                            return false;
                        }
                        // Validate command count is reasonable
                        if (*cmd).num_allowed_commands > 10_000 {
                            return false;
                        }
                    }
                    (*cmd).required
                }
            }
            _ => false,
        }
    }
}

impl fmt::Display for CapabilityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CapabilityType::Filesystem => write!(f, "filesystem"),
            CapabilityType::Network => write!(f, "network"),
            CapabilityType::Command => write!(f, "command"),
            CapabilityType::Database => write!(f, "database"),
            CapabilityType::Secrets => write!(f, "secrets"),
            CapabilityType::Custom => write!(f, "custom"),
        }
    }
}

impl fmt::Display for FilesystemAccessMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilesystemAccessMode::Read => write!(f, "read"),
            FilesystemAccessMode::ReadWrite => write!(f, "read-write"),
            FilesystemAccessMode::WriteOnly => write!(f, "write"),
            FilesystemAccessMode::Append => write!(f, "append"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_type_values() {
        assert_eq!(CapabilityType::Filesystem as i32, 0);
        assert_eq!(CapabilityType::Network as i32, 1);
        assert_eq!(CapabilityType::Command as i32, 2);
    }

    #[test]
    fn test_filesystem_access_mode_values() {
        assert_eq!(FilesystemAccessMode::Read as i32, 0);
        assert_eq!(FilesystemAccessMode::ReadWrite as i32, 1);
        assert_eq!(FilesystemAccessMode::WriteOnly as i32, 2);
    }

    #[test]
    fn test_capability_status_values() {
        assert_eq!(CapabilityStatus::Pending as i32, 0);
        assert_eq!(CapabilityStatus::Approved as i32, 1);
        assert_eq!(CapabilityStatus::Denied as i32, 2);
    }
}
