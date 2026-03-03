// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! V2 ABI FFI Interface for Permissions Plugin
//!
//! This module implements RFC-0004 v2 ABI for permissions plugin.
//! Permissions provides authentication, authorization, and user context management.

use skylet_abi::v2_spec::*;
use skylet_abi::PluginLogLevel;
use std::ffi::{c_char, CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering, Ordering as AOrdering};
use std::sync::Arc;

use super::auth::{AuthProviderRegistry, LocalAuthProvider};
use super::authz::{AuthzAuditLog, PermissionChecker};

// ============================================================================
// Plugin Metadata Constants
// ============================================================================

const PLUGIN_NAME: &[u8] = b"permissions\0";
const PLUGIN_VERSION: &[u8] = b"0.2.0\0";
const PLUGIN_DESCRIPTION: &[u8] =
    b"User-Level Permissions and Context - authentication, authorization, and multi-tenancy\0";
const PLUGIN_AUTHOR: &[u8] = b"Skylet Team\0";
const PLUGIN_LICENSE: &[u8] = b"MIT OR Apache-2.0\0";
const PLUGIN_HOMEPAGE: &[u8] = b"https://github.com/vincents-ai/skylet\0";
const PLUGIN_ABI_VERSION: &[u8] = b"2.0\0";
const PLUGIN_SKYLET_MIN: &[u8] = b"1.0.0\0";
const PLUGIN_SKYLET_MAX: &[u8] = b"2.0.0\0";

// Plugin tags
const TAG_AUTH: &[u8] = b"auth\0";
const TAG_PERMISSIONS: &[u8] = b"permissions\0";
const TAG_SECURITY: &[u8] = b"security\0";
const TAG_RBAC: &[u8] = b"rbac\0";

// Service info
const SERVICE_NAME: &[u8] = b"PermissionsService\0";
const SERVICE_VERSION: &[u8] = b"2.0.0\0";
const SERVICE_DESC: &[u8] = b"Authentication and authorization service\0";
const SERVICE_SPEC: &[u8] = b"permissions-service-v2\0";

// ============================================================================
// Static Plugin Information
// ============================================================================

static PLUGIN_INFO: AtomicPtr<PluginInfoV2> = AtomicPtr::new(ptr::null_mut());
static CAPABILITIES_STORAGE: AtomicPtr<[CapabilityInfo; 8]> = AtomicPtr::new(ptr::null_mut());
static TAGS_STORAGE: AtomicPtr<[*const c_char; 4]> = AtomicPtr::new(ptr::null_mut());
static SERVICE_STORAGE: AtomicPtr<ServiceInfo> = AtomicPtr::new(ptr::null_mut());

// Thread-safe service storage using parking_lot::RwLock
static AUTH_REGISTRY: parking_lot::RwLock<Option<Arc<AuthProviderRegistry>>> =
    parking_lot::RwLock::new(None);
static PERMISSION_CHECKER: parking_lot::RwLock<Option<Arc<PermissionChecker>>> =
    parking_lot::RwLock::new(None);
static AUDIT_LOG: parking_lot::RwLock<Option<Arc<AuthzAuditLog>>> = parking_lot::RwLock::new(None);
static INITIALIZED: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Plugin Info Initialization
// ============================================================================

fn init_plugin_info() {
    let capabilities = [
        CapabilityInfo {
            name: c"auth.authenticate".as_ptr(),
            description: c"Authenticate user with credentials".as_ptr(),
            required_permission: ptr::null(),
        },
        CapabilityInfo {
            name: c"auth.validate".as_ptr(),
            description: c"Validate session token".as_ptr(),
            required_permission: ptr::null(),
        },
        CapabilityInfo {
            name: c"auth.revoke".as_ptr(),
            description: c"Revoke session token".as_ptr(),
            required_permission: ptr::null(),
        },
        CapabilityInfo {
            name: c"authz.check".as_ptr(),
            description: c"Check user permission".as_ptr(),
            required_permission: ptr::null(),
        },
        CapabilityInfo {
            name: c"authz.assign_role".as_ptr(),
            description: c"Assign role to user".as_ptr(),
            required_permission: ptr::null(),
        },
        CapabilityInfo {
            name: c"user.register".as_ptr(),
            description: c"Register new user".as_ptr(),
            required_permission: ptr::null(),
        },
        CapabilityInfo {
            name: c"user.context".as_ptr(),
            description: c"Get user context for plugin calls".as_ptr(),
            required_permission: ptr::null(),
        },
        CapabilityInfo {
            name: c"tenant.create".as_ptr(),
            description: c"Create new tenant".as_ptr(),
            required_permission: ptr::null(),
        },
    ];

    CAPABILITIES_STORAGE.store(
        Box::leak(Box::new(capabilities)) as *mut _ as *mut _,
        Ordering::SeqCst,
    );

    let tags = [
        TAG_AUTH.as_ptr() as *const c_char,
        TAG_PERMISSIONS.as_ptr() as *const c_char,
        TAG_SECURITY.as_ptr() as *const c_char,
        TAG_RBAC.as_ptr() as *const c_char,
    ];

    TAGS_STORAGE.store(
        Box::leak(Box::new(tags)) as *mut _ as *mut _,
        Ordering::SeqCst,
    );

    let service = ServiceInfo {
        name: SERVICE_NAME.as_ptr() as *const c_char,
        version: SERVICE_VERSION.as_ptr() as *const c_char,
        description: SERVICE_DESC.as_ptr() as *const c_char,
        interface_spec: SERVICE_SPEC.as_ptr() as *const c_char,
    };

    SERVICE_STORAGE.store(
        Box::leak(Box::new(service)) as *mut _ as *mut _,
        Ordering::SeqCst,
    );

    let info = PluginInfoV2 {
        name: PLUGIN_NAME.as_ptr() as *const c_char,
        version: PLUGIN_VERSION.as_ptr() as *const c_char,
        description: PLUGIN_DESCRIPTION.as_ptr() as *const c_char,
        author: PLUGIN_AUTHOR.as_ptr() as *const c_char,
        license: PLUGIN_LICENSE.as_ptr() as *const c_char,
        homepage: PLUGIN_HOMEPAGE.as_ptr() as *const c_char,

        skylet_version_min: PLUGIN_SKYLET_MIN.as_ptr() as *const c_char,
        skylet_version_max: PLUGIN_SKYLET_MAX.as_ptr() as *const c_char,
        abi_version: PLUGIN_ABI_VERSION.as_ptr() as *const c_char,

        dependencies: ptr::null(),
        num_dependencies: 0,
        provides_services: SERVICE_STORAGE.load(Ordering::SeqCst),
        num_provides_services: 1,
        requires_services: ptr::null(),
        num_requires_services: 0,

        capabilities: CAPABILITIES_STORAGE.load(Ordering::SeqCst) as *const CapabilityInfo,
        num_capabilities: 8,

        min_resources: ptr::null(),
        max_resources: ptr::null(),

        tags: TAGS_STORAGE.load(Ordering::SeqCst) as *const *const c_char,
        num_tags: 4,
        category: PluginCategory::Development,

        supports_hot_reload: false,
        supports_async: true,
        supports_streaming: false,
        max_concurrency: 100,

        // Plugin presentation
        tagline: ptr::null(),
        icon_url: ptr::null(),

        maturity_level: MaturityLevel::Alpha,
        build_timestamp: ptr::null(),
        build_hash: ptr::null(),
        git_commit: ptr::null(),
        build_environment: ptr::null(),
        metadata: ptr::null(),
    };

    PLUGIN_INFO.store(
        Box::leak(Box::new(info)) as *mut _ as *mut _,
        Ordering::SeqCst,
    );
}

fn init_services() {
    // Create auth provider registry
    let registry = Arc::new(AuthProviderRegistry::new());

    // Register local auth provider
    let local_provider = Arc::new(LocalAuthProvider::new(3600)); // 1 hour TTL
    registry.register("local", local_provider);

    *AUTH_REGISTRY.write() = Some(registry);

    // Create permission checker
    let checker = Arc::new(PermissionChecker::new());
    *PERMISSION_CHECKER.write() = Some(checker);

    // Create audit log
    let audit = Arc::new(AuthzAuditLog::new(10000));
    *AUDIT_LOG.write() = Some(audit);

    INITIALIZED.store(true, AOrdering::SeqCst);
}

fn shutdown_services() {
    *AUTH_REGISTRY.write() = None;
    *PERMISSION_CHECKER.write() = None;
    *AUDIT_LOG.write() = None;
    INITIALIZED.store(false, AOrdering::SeqCst);
}

// ============================================================================
// V2 ABI Implementation
// ============================================================================

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    if PLUGIN_INFO.load(Ordering::SeqCst).is_null() {
        init_plugin_info();
    }
    PLUGIN_INFO.load(Ordering::SeqCst)
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    if PLUGIN_INFO.load(Ordering::SeqCst).is_null() {
        init_plugin_info();
    }

    unsafe {
        let ctx = &*context;

        if !ctx.logger.is_null() {
            let logger = &*ctx.logger;
            let msg = CString::new("permissions v2 plugin initialized").unwrap();
            let _ = (logger.log)(context, PluginLogLevel::Info, msg.as_ptr());
        }

        if !ctx.service_registry.is_null() {
            let registry = &*ctx.service_registry;
            let service_name = CString::new("PermissionsService").unwrap();
            let service_type = CString::new("permissions-service-v2").unwrap();

            let _ = (registry.register)(
                context,
                service_name.as_ptr(),
                std::ptr::null_mut::<std::ffi::c_void>(),
                service_type.as_ptr(),
            );
        }
    }

    init_services();

    PluginResultV2::Success
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn plugin_shutdown_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    unsafe {
        let ctx = &*context;

        if !ctx.logger.is_null() {
            let logger = &*ctx.logger;
            let msg = CString::new("permissions v2 plugin shutting down").unwrap();
            let _ = (logger.log)(context, PluginLogLevel::Info, msg.as_ptr());
        }
    }

    shutdown_services();

    PluginResultV2::Success
}

#[no_mangle]
pub extern "C" fn plugin_handle_request_v2(
    _context: *const PluginContextV2,
    _request: *const RequestV2,
    _response: *mut ResponseV2,
) -> PluginResultV2 {
    PluginResultV2::NotImplemented
}

#[no_mangle]
pub extern "C" fn plugin_health_check_v2(_context: *const PluginContextV2) -> HealthStatus {
    if INITIALIZED.load(AOrdering::SeqCst) {
        HealthStatus::Healthy
    } else {
        HealthStatus::Degraded
    }
}

#[no_mangle]
#[allow(static_mut_refs)]
pub extern "C" fn plugin_get_metrics_v2(_context: *const PluginContextV2) -> *const PluginMetrics {
    use std::time::{SystemTime, UNIX_EPOCH};

    static mut METRICS: Option<PluginMetrics> = None;

    unsafe {
        if METRICS.is_none() {
            METRICS = Some(PluginMetrics {
                uptime_seconds: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                request_count: 0,
                error_count: 0,
                avg_response_time_ms: 0.0,
                memory_usage_mb: 64,
                cpu_usage_percent: 1.0,
                last_error: ptr::null(),
            });
        }
        METRICS.as_ref().unwrap()
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn plugin_query_capability_v2(
    _context: *const PluginContextV2,
    capability: *const c_char,
) -> bool {
    // SAFETY: Caller must ensure capability is a valid null-terminated C string
    // This is FFI boundary code where the caller is responsible for pointer validity
    unsafe {
        if capability.is_null() {
            return false;
        }

        let cap_str = CStr::from_ptr(capability).to_str().unwrap_or("");

        matches!(
            cap_str,
            "auth.authenticate"
                | "auth.validate"
                | "auth.revoke"
                | "authz.check"
                | "authz.assign_role"
                | "user.register"
                | "user.context"
                | "tenant.create"
        )
    }
}

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
        get_metrics: Some(plugin_get_metrics_v2),
        query_capability: Some(plugin_query_capability_v2),
        get_config_schema: None,
        serialize_state: None,
        deserialize_state: None,
        free_state: None,
    };

    &API
}
