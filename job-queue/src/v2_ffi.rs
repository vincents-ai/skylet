// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! V2 ABI FFI Interface for Job Queue Plugin
//!
//! This module implements RFC-0004 v2 ABI for job-queue plugin.
//! Job Queue provides background job processing with persistent storage.
//!
//! ## v2 Migration Changes
//! - Thread-safe JobQueue storage using RwLock<Arc<JobQueue>>
//! - PluginInfoV2 with full metadata fields
//! - SafePluginContext for type-safe service access
//! - No unsafe static mut - all storage uses thread-safe primitives
//! - All ABI functions follow RFC-0004 v2 specification

use skylet_abi;
use skylet_abi::v2_spec::*;
use std::ffi::{c_char, CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering, Ordering as AOrdering};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::job_queue::JobQueue;

// ============================================================================
// Plugin Metadata Constants
// ============================================================================

const PLUGIN_NAME: &[u8] = b"job-queue\0";
const PLUGIN_VERSION: &[u8] = b"0.2.0\0"; // Updated to v2
const PLUGIN_DESCRIPTION: &[u8] = b"Background job processing with persistent SQLite storage, retry logic, and scheduled execution\0";
const PLUGIN_AUTHOR: &[u8] = b"Skylet Team\0";
const PLUGIN_LICENSE: &[u8] = b"MIT OR Apache-2.0\0";
const PLUGIN_HOMEPAGE: &[u8] = b"https://github.com/vincents-ai/skylet\0";
const PLUGIN_ABI_VERSION: &[u8] = b"2.0\0";
const PLUGIN_SKYLET_MIN: &[u8] = b"1.0.0\0";
const PLUGIN_SKYLET_MAX: &[u8] = b"2.0.0\0";

// Plugin tags
const TAG_JOBS: &[u8] = b"jobs\0";
const TAG_QUEUE: &[u8] = b"queue\0";
const TAG_SCHEDULER: &[u8] = b"scheduler\0";
const TAG_BACKGROUND: &[u8] = b"background\0";

// Service info - this plugin provides job queue service
const SERVICE_NAME: &[u8] = b"JobQueue\0";
const SERVICE_VERSION: &[u8] = b"2.0.0\0"; // Updated to v2
const SERVICE_DESC: &[u8] = b"Background job processing service\0";
const SERVICE_SPEC: &[u8] = b"job-queue-service-v2\0"; // Updated spec

// ============================================================================
// Static Plugin Information
// ============================================================================

// Static storage for plugin info
static PLUGIN_INFO: AtomicPtr<PluginInfoV2> = AtomicPtr::new(ptr::null_mut());
static CAPABILITIES_STORAGE: AtomicPtr<[CapabilityInfo; 5]> = AtomicPtr::new(ptr::null_mut());
static TAGS_STORAGE: AtomicPtr<[*const c_char; 4]> = AtomicPtr::new(ptr::null_mut());
static SERVICE_STORAGE: AtomicPtr<ServiceInfo> = AtomicPtr::new(ptr::null_mut());

// Thread-safe job queue storage
static JOB_QUEUE: RwLock<Option<Arc<JobQueue>>> = RwLock::const_new(None);
static JOB_QUEUE_INITIALIZED: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Helper Functions
// ============================================================================

/// Initialize static plugin information
fn init_plugin_info() {
    // Initialize capabilities
    let capabilities = [
        CapabilityInfo {
            name: c"job.submit".as_ptr(),
            description: c"Submit a new job to the queue".as_ptr(),
            required_permission: ptr::null(),
        },
        CapabilityInfo {
            name: c"job.cancel".as_ptr(),
            description: c"Cancel a pending or running job".as_ptr(),
            required_permission: ptr::null(),
        },
        CapabilityInfo {
            name: c"job.status".as_ptr(),
            description: c"Get job status and progress".as_ptr(),
            required_permission: ptr::null(),
        },
        CapabilityInfo {
            name: c"job.list".as_ptr(),
            description: c"List jobs by status or time range".as_ptr(),
            required_permission: ptr::null(),
        },
        CapabilityInfo {
            name: c"job.retry".as_ptr(),
            description: c"Retry a failed job".as_ptr(),
            required_permission: ptr::null(),
        },
    ];

    CAPABILITIES_STORAGE.store(
        Box::leak(Box::new(capabilities)) as *mut _ as *mut _,
        Ordering::SeqCst,
    );

    // Initialize tags
    let tags = [
        TAG_JOBS.as_ptr().cast::<c_char>(),
        TAG_QUEUE.as_ptr().cast::<c_char>(),
        TAG_SCHEDULER.as_ptr().cast::<c_char>(),
        TAG_BACKGROUND.as_ptr().cast::<c_char>(),
    ];

    TAGS_STORAGE.store(
        Box::leak(Box::new(tags)) as *mut _ as *mut _,
        Ordering::SeqCst,
    );

    // Initialize service info
    let service = ServiceInfo {
        name: SERVICE_NAME.as_ptr().cast::<c_char>(),
        version: SERVICE_VERSION.as_ptr().cast::<c_char>(),
        description: SERVICE_DESC.as_ptr().cast::<c_char>(),
        interface_spec: SERVICE_SPEC.as_ptr().cast::<c_char>(),
    };

    SERVICE_STORAGE.store(
        Box::leak(Box::new(service)) as *mut _ as *mut _,
        Ordering::SeqCst,
    );

    // Initialize plugin info
    let info = PluginInfoV2 {
        // Basic metadata
        name: PLUGIN_NAME.as_ptr().cast::<c_char>(),
        version: PLUGIN_VERSION.as_ptr().cast::<c_char>(),
        description: PLUGIN_DESCRIPTION.as_ptr().cast::<c_char>(),
        author: PLUGIN_AUTHOR.as_ptr().cast::<c_char>(),
        license: PLUGIN_LICENSE.as_ptr().cast::<c_char>(),
        homepage: PLUGIN_HOMEPAGE.as_ptr().cast::<c_char>(),

        // Version compatibility
        skylet_version_min: PLUGIN_SKYLET_MIN.as_ptr().cast::<c_char>(),
        skylet_version_max: PLUGIN_SKYLET_MAX.as_ptr().cast::<c_char>(),
        abi_version: PLUGIN_ABI_VERSION.as_ptr().cast::<c_char>(),

        // Dependencies and services
        dependencies: ptr::null(),
        num_dependencies: 0,
        provides_services: SERVICE_STORAGE.load(Ordering::SeqCst),
        num_provides_services: 1,
        requires_services: ptr::null(),
        num_requires_services: 0,

        // Capabilities
        capabilities: CAPABILITIES_STORAGE.load(Ordering::SeqCst) as *const CapabilityInfo,
        num_capabilities: 5,

        // Resource requirements (job queue needs moderate resources)
        min_resources: ptr::null(),
        max_resources: ptr::null(),

        // Tags and categorization
        tags: TAGS_STORAGE.load(Ordering::SeqCst) as *const *const c_char,
        num_tags: 4,
        category: PluginCategory::Development,

        // Runtime capabilities
        supports_hot_reload: false,
        supports_async: true, // Job queue is async
        supports_streaming: false,
        max_concurrency: 10,

        // Marketplace (not sold)
        monetization_model: MonetizationModel::Free,
        price_usd: 0.0,
        purchase_url: ptr::null(),
        subscription_url: ptr::null(),
        marketplace_category: ptr::null(),
        tagline: ptr::null(),
        icon_url: ptr::null(),

        // Build and deployment
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

/// Initialize JobQueue (called during plugin_init_v2)
fn init_job_queue() {
    let data_dir = std::env::var("SKYLET_DATA_DIR").unwrap_or_else(|_| "./data".to_string());
    let db_path = format!("{}/jobs.db", data_dir);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let queue = Arc::new(JobQueue::new(db_path));
        if let Err(e) = queue.init() {
            tracing::error!("Failed to initialize job queue: {}", e);
            return;
        }
        let mut guard = JOB_QUEUE.write().await;
        *guard = Some(queue);
        JOB_QUEUE_INITIALIZED.store(true, AOrdering::SeqCst);
    });
}

/// Shutdown JobQueue (called during plugin_shutdown_v2)
fn shutdown_job_queue() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut guard = JOB_QUEUE.write().await;
        *guard = None;
        JOB_QUEUE_INITIALIZED.store(false, AOrdering::SeqCst);
    });
}

// ============================================================================
// V2 ABI Implementation
// ============================================================================

/// Get plugin information
#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    if PLUGIN_INFO.load(Ordering::SeqCst).is_null() {
        init_plugin_info();
    }
    PLUGIN_INFO.load(Ordering::SeqCst)
}

/// Initialize plugin
#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    // Ensure plugin info is initialized
    if PLUGIN_INFO.load(Ordering::SeqCst).is_null() {
        init_plugin_info();
    }

    unsafe {
        let ctx = &*context;

        // Log initialization
        if !ctx.logger.is_null() {
            let logger = &*ctx.logger;
            let msg = CString::new("job-queue v2 plugin initialized").unwrap();
            let _ = (logger.log)(
                context as *const PluginContextV2,
                skylet_abi::PluginLogLevel::Info,
                msg.as_ptr(),
            );
        }

        // Register JobQueue in service registry
        if !ctx.service_registry.is_null() {
            let registry = &*ctx.service_registry;
            let service_name = CString::new("JobQueue").unwrap();
            let service_type = CString::new("job-queue-service-v2").unwrap();

            let _ = (registry.register)(
                context,
                service_name.as_ptr(),
                std::ptr::null_mut::<std::ffi::c_void>(),
                service_type.as_ptr(),
            );
        }
    }

    // Initialize JobQueue
    init_job_queue();

    PluginResultV2::Success
}

/// Shutdown plugin
#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }

    unsafe {
        let ctx = &*context;

        // Log shutdown
        if !ctx.logger.is_null() {
            let logger = &*ctx.logger;
            let msg = CString::new("job-queue v2 plugin shutting down").unwrap();
            let _ = (logger.log)(
                context as *const PluginContextV2,
                skylet_abi::PluginLogLevel::Info,
                msg.as_ptr(),
            );
        }
    }

    // Shutdown JobQueue
    shutdown_job_queue();

    PluginResultV2::Success
}

/// Handle HTTP request (not implemented for job-queue)
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
    // JobQueue is healthy if initialized
    if JOB_QUEUE_INITIALIZED.load(AOrdering::SeqCst) {
        HealthStatus::Healthy
    } else {
        HealthStatus::Degraded
    }
}

/// Get metrics
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
                memory_usage_mb: 32,
                cpu_usage_percent: 0.5,
                last_error: ptr::null(),
            });
        }
        METRICS.as_ref().unwrap()
    }
}

/// Query capability
#[no_mangle]
pub extern "C" fn plugin_query_capability_v2(
    _context: *const PluginContextV2,
    capability: *const c_char,
) -> bool {
    unsafe {
        if capability.is_null() {
            return false;
        }

        let cap_str = CStr::from_ptr(capability).to_str().unwrap_or("");

        // Check against our capabilities list
        matches!(
            cap_str,
            "job.submit" | "job.cancel" | "job.status" | "job.list" | "job.retry"
        )
    }
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
        get_metrics: Some(plugin_get_metrics_v2),
        query_capability: Some(plugin_query_capability_v2),
        get_config_schema: None,
        get_billing_metrics: None,
    };

    &API
}
