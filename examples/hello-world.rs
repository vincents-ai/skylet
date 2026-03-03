// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Hello World Example Plugin
//!
//! A minimal plugin demonstrating basic functionality.

use std::ffi::CStr;

// These would be from skylet_abi in a real plugin
// For this example, we define minimal types locally
#[repr(C)]
pub struct PluginInfoV2 {
    pub name: *const i8,
    pub version: *const i8,
    pub author: *const i8,
}

#[repr(C)]
pub enum PluginResult {
    Success = 0,
    Error = 1,
    InvalidRequest = 2,
}

#[repr(C)]
pub struct PluginContextV2 {
    _private: *const (),
}

/// Initialize the plugin
///
/// This is called when the plugin is first loaded into memory.
#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResult {
    // Validate pointer
    if context.is_null() {
        return PluginResult::InvalidRequest;
    }

    // Log initialization (would use logger service in real implementation)
    eprintln!("Hello World plugin initialized");

    PluginResult::Success
}

/// Shutdown the plugin
///
/// This is called when the plugin is being unloaded.
#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(_context: *const PluginContextV2) -> PluginResult {
    eprintln!("Hello World plugin shutting down");
    PluginResult::Success
}

/// Get plugin metadata
///
/// Returns information about the plugin.
#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    static INFO: PluginInfoV2 = PluginInfoV2 {
        name: b"hello-world\0" as *const u8 as *const i8,
        version: b"0.1.0\0" as *const u8 as *const i8,
        author: b"Skylet Team\0" as *const u8 as *const i8,
    };
    &INFO
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_info() {
        let info = unsafe { plugin_get_info_v2() };
        assert!(!info.is_null());

        let info = unsafe { &*info };
        assert!(!info.name.is_null());

        let name = unsafe { CStr::from_ptr(info.name) };
        assert_eq!(name.to_string_lossy(), "hello-world");
    }

    #[test]
    fn test_init_with_null_context() {
        let result = plugin_init_v2(std::ptr::null());
        matches!(result, PluginResult::InvalidRequest);
    }
}
