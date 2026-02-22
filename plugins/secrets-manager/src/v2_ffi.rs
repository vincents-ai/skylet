// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! V2 ABI FFI Interface for Secrets Manager Plugin
//!
//! This module implements RFC-0004 v2 ABI for secrets-manager plugin.
//! Secrets Manager is Bootstrap Plugin #4 and provides encrypted secret storage.

use skylet_abi::v2_spec::*;
use skylet_plugin_common::{CapabilityBuilder, TagsBuilder};
use std::ffi::{c_char, CStr, CString};
use std::ptr;

// ============================================================================
// Plugin Metadata Constants
// ============================================================================

const PLUGIN_NAME: &str = "secrets-manager";
const VERSION: &str = "0.1.0";
const DESCRIPTION: &str = "Secure secrets management service with AES-256-GCM encryption, versioned storage, and rotation policies";
const AUTHOR: &str = "Skylet Team";
const LICENSE: &str = "MIT OR Apache-2.0";
const HOMEPAGE: &str = "https://github.com/vincents-ai/skylet";
const ABI_VERSION: &str = "2.0";
const SKYLET_VERSION_MIN: &str = "0.1.0";
const SKYLET_VERSION_MAX: &str = "2.0.0";

// ============================================================================
// V2 ABI Functions
// ============================================================================

/// Get plugin information
#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    static mut INFO: Option<PluginInfoV2> = None;

    unsafe {
        if INFO.is_none() {
            // Build tags using TagsBuilder
            let (tags_ptr, num_tags) = TagsBuilder::new()
                .add("security")
                .add("encryption")
                .add("secrets")
                .add("bootstrap")
                .build();

            // Build capabilities using CapabilityBuilder
            let (capabilities_ptr, num_capabilities) = CapabilityBuilder::new()
                .add(
                    "secrets.get",
                    "Get secret value by key",
                    Some("secrets.read"),
                )
                .add(
                    "secrets.set",
                    "Set secret value by key",
                    Some("secrets.write"),
                )
                .add(
                    "secrets.delete",
                    "Delete secret by key",
                    Some("secrets.delete"),
                )
                .add("secrets.list", "List all secrets", Some("secrets.list"))
                .add(
                    "secrets.rotate",
                    "Rotate secret value",
                    Some("secrets.rotate"),
                )
                .add(
                    "secrets.versioned",
                    "Versioned secret access",
                    Some("secrets.versioned"),
                )
                .build();

            INFO = Some(PluginInfoV2 {
                // Basic metadata
                name: CString::new(PLUGIN_NAME).unwrap().into_raw(),
                version: CString::new(VERSION).unwrap().into_raw(),
                description: CString::new(DESCRIPTION).unwrap().into_raw(),
                author: CString::new(AUTHOR).unwrap().into_raw(),
                license: CString::new(LICENSE).unwrap().into_raw(),
                homepage: CString::new(HOMEPAGE).unwrap().into_raw(),

                // Version compatibility
                skylet_version_min: CString::new(SKYLET_VERSION_MIN).unwrap().into_raw(),
                skylet_version_max: CString::new(SKYLET_VERSION_MAX).unwrap().into_raw(),
                abi_version: CString::new(ABI_VERSION).unwrap().into_raw(),

                // Dependencies and services
                dependencies: ptr::null(),
                num_dependencies: 0,
                provides_services: ptr::null(),
                num_provides_services: 0,
                requires_services: ptr::null(),
                num_requires_services: 0,

                // Capabilities
                capabilities: capabilities_ptr,
                num_capabilities,

                // Resources
                min_resources: ptr::null(),
                max_resources: ptr::null(),

                // Tags
                tags: tags_ptr,
                num_tags,
                category: PluginCategory::Security,

                // Runtime capabilities
                supports_hot_reload: false,
                supports_async: true,
                supports_streaming: false,
                max_concurrency: 10,

                // Marketplace (optional)
                monetization_model: MonetizationModel::Free,
                price_usd: 0.0,
                purchase_url: ptr::null(),
                subscription_url: ptr::null(),
                marketplace_category: ptr::null(),
                tagline: ptr::null(),
                icon_url: ptr::null(),

                // Build info
                maturity_level: MaturityLevel::Beta,
                build_timestamp: ptr::null(),
                build_hash: ptr::null(),
                git_commit: ptr::null(),
                build_environment: ptr::null(),

                // Metadata
                metadata: ptr::null(),
            });
        }

        INFO.as_ref().unwrap()
    }
}

/// Initialize plugin
#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    PluginResultV2::Success
}

/// Shutdown plugin
#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    // Call the cleanup function from lib.rs
    crate::cleanup_plugin();

    PluginResultV2::Success
}

/// Handle request
#[no_mangle]
pub extern "C" fn plugin_handle_request_v2(
    _context: *const PluginContextV2,
    _request: *const RequestV2,
    _response: *mut ResponseV2,
) -> PluginResultV2 {
    PluginResultV2::NotImplemented
}

/// Health check
#[no_mangle]
pub extern "C" fn plugin_health_check_v2(_context: *const PluginContextV2) -> HealthStatus {
    HealthStatus::Healthy
}

/// Query capability
#[no_mangle]
pub extern "C" fn plugin_query_capability_v2(
    _context: *const PluginContextV2,
    capability: *const c_char,
) -> bool {
    if capability.is_null() {
        return false;
    }

    unsafe {
        let cap_str = CStr::from_ptr(capability).to_str().unwrap_or("");
        matches!(
            cap_str,
            "secrets.get"
                | "secrets.set"
                | "secrets.delete"
                | "secrets.list"
                | "secrets.rotate"
                | "secrets.versioned"
        )
    }
}

// ============================================================================
// RFC-0006: Configuration Schema Export
// ============================================================================

/// JSON Schema for secrets-manager configuration
///
/// Configuration options:
/// - backend: Storage backend type ("memory", "encrypted")
/// - rotation_days: Default rotation interval in days
/// - max_versions: Maximum number of versions to keep per secret
/// - audit_enabled: Whether to enable audit logging
const CONFIG_SCHEMA: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "SecretsManagerConfig",
  "description": "Configuration for the secrets-manager plugin",
  "type": "object",
  "properties": {
    "backend": {
      "type": "string",
      "enum": ["memory", "encrypted"],
      "default": "encrypted",
      "description": "Storage backend type. Use 'encrypted' for production (AES-256-GCM), 'memory' for testing only."
    },
    "rotation_days": {
      "type": "integer",
      "minimum": 1,
      "maximum": 365,
      "default": 90,
      "description": "Default rotation interval in days for secrets without explicit rotation policy"
    },
    "max_versions": {
      "type": "integer",
      "minimum": 1,
      "maximum": 100,
      "default": 10,
      "description": "Maximum number of historical versions to retain per secret"
    },
    "audit_enabled": {
      "type": "boolean",
      "default": true,
      "description": "Enable audit logging for all secret access operations"
    },
    "encryption_key_path": {
      "type": "string",
      "description": "Path to the encryption key file (optional, auto-generated if not provided)"
    },
    "storage_path": {
      "type": "string",
      "description": "Path to persistent storage file (optional, in-memory if not provided)"
    }
  },
  "additionalProperties": false
}"#;

/// Get the plugin's configuration schema as JSON (RFC-0006)
///
/// Returns a pointer to a static JSON Schema string describing the plugin's
/// configuration structure. The schema follows JSON Schema draft-07.
#[no_mangle]
pub extern "C" fn plugin_get_config_schema_json() -> *const c_char {
    CONFIG_SCHEMA.as_ptr() as *const c_char
}

/// Create v2 plugin API - REQUIRED ENTRY POINT
#[no_mangle]
pub extern "C" fn plugin_create_v2() -> *const PluginApiV2 {
    static API: PluginApiV2 = PluginApiV2 {
        get_info: plugin_get_info_v2,
        init: plugin_init_v2,
        shutdown: plugin_shutdown_v2,
        handle_request: plugin_handle_request_v2,
        handle_event: None,
        prepare_hot_reload: None,
        health_check: Some(plugin_health_check_v2),
        get_metrics: None, // PluginMetrics contains raw pointers, not Sync-safe
        query_capability: Some(plugin_query_capability_v2),
        get_config_schema: Some(plugin_get_config_schema_json),
        get_billing_metrics: None,
        serialize_state: None,
        deserialize_state: None,
        free_state: None,
    };

    &API
}
