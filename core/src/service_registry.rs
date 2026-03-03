// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use skylet_abi::{PluginContext, PluginResult};

// Mock UserContext if not found in crate::auth
#[derive(Debug, Deserialize, Serialize)]
pub struct UserContext {
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

impl UserContext {
    pub fn is_admin(&self) -> bool {
        self.roles
            .iter()
            .any(|r| r == "admin" || r == "Administrator")
    }

    pub fn has_permission(&self, perm: &str) -> bool {
        self.permissions.iter().any(|p| p == perm)
    }
}

/// Simple service registry storing raw pointers and a service type string.
pub struct ServiceRegistry {
    inner: RwLock<HashMap<String, (*mut c_void, String)>>,
}

// SAFETY: ServiceRegistry uses RwLock for interior mutability and raw pointers
// are only accessed through safe methods. The registry is designed to be shared
// across threads via Arc.
unsafe impl Send for ServiceRegistry {}
unsafe impl Sync for ServiceRegistry {}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, name: &str, service: *mut c_void, service_type: &str) -> PluginResult {
        let mut map = self.inner.write().unwrap();
        if map.contains_key(name) {
            return PluginResult::Error;
        }
        map.insert(name.to_string(), (service, service_type.to_string()));
        PluginResult::Success
    }

    pub fn get(&self, name: &str, service_type: Option<&str>) -> *mut c_void {
        let map = self.inner.read().unwrap();
        if let Some((ptr, stored_type)) = map.get(name) {
            if let Some(req_type) = service_type {
                if req_type != stored_type.as_str() {
                    return std::ptr::null_mut();
                }
            }
            *ptr
        } else {
            std::ptr::null_mut()
        }
    }

    pub fn unregister(&self, name: &str) -> PluginResult {
        let mut map = self.inner.write().unwrap();
        if map.remove(name).is_some() {
            PluginResult::Success
        } else {
            PluginResult::Error
        }
    }
}

/// Thin handle stored in PluginContext.user_data so FFI functions can access the
/// shared registry instance.
pub struct ServiceRegistryHandle {
    pub registry: Arc<ServiceRegistry>,
}

impl ServiceRegistryHandle {
    pub fn new(registry: Arc<ServiceRegistry>) -> Self {
        Self { registry }
    }
}

// FFI functions used by plugins via `PluginServiceRegistry` callbacks.
// These functions expect the `PluginContext.user_data` to be a pointer to a
// `ServiceRegistryHandle` (heap allocated). They are intentionally simple and
// defensive about null pointers.

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn core_service_register(
    context: *const PluginContext,
    name: *const c_char,
    service: *mut c_void,
    service_type: *const c_char,
) -> PluginResult {
    if context.is_null() || name.is_null() {
        return PluginResult::InvalidRequest;
    }

    unsafe {
        let user = (*context).user_data as *mut ServiceRegistryHandle;
        if user.is_null() {
            return PluginResult::ServiceUnavailable;
        }

        let handle = &*user;

        let name = match CStr::from_ptr(name).to_str() {
            Ok(s) => s,
            Err(_) => return PluginResult::InvalidRequest,
        };

        let s_type = if service_type.is_null() {
            ""
        } else {
            match CStr::from_ptr(service_type).to_str() {
                Ok(s) => s,
                Err(_) => return PluginResult::InvalidRequest,
            }
        };

        handle.registry.register(name, service, s_type)
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn core_service_get(
    context: *const PluginContext,
    name: *const c_char,
    service_type: *const c_char,
) -> *mut c_void {
    if context.is_null() || name.is_null() {
        return std::ptr::null_mut();
    }

    unsafe {
        let user = (*context).user_data as *mut ServiceRegistryHandle;
        if user.is_null() {
            return std::ptr::null_mut();
        }
        let handle = &*user;

        let name = match CStr::from_ptr(name).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };

        let s_type = if service_type.is_null() {
            None
        } else {
            match CStr::from_ptr(service_type).to_str() {
                Ok(s) => Some(s),
                Err(_) => return std::ptr::null_mut(),
            }
        };

        // RBAC check: if plugin provided a user_context_json, deny access to services
        // unless user has 'service_discovery' permission or is admin. This is a basic
        // enforcement implemented in core for demonstration and tests.
        if !(*context).user_context_json.is_null() {
            let cstr = CStr::from_ptr((*context).user_context_json);
            if let Ok(json) = cstr.to_str() {
                if let Ok(uc) = serde_json::from_str::<UserContext>(json) {
                    if !uc.is_admin() && !uc.has_permission("service_discovery") {
                        return std::ptr::null_mut();
                    }
                }
            }
        }

        handle.registry.get(name, s_type)
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn core_service_unregister(
    context: *const PluginContext,
    name: *const c_char,
) -> PluginResult {
    if context.is_null() || name.is_null() {
        return PluginResult::InvalidRequest;
    }

    unsafe {
        let user = (*context).user_data as *mut ServiceRegistryHandle;
        if user.is_null() {
            return PluginResult::ServiceUnavailable;
        }
        let handle = &*user;

        let name = match CStr::from_ptr(name).to_str() {
            Ok(s) => s,
            Err(_) => return PluginResult::InvalidRequest,
        };

        // Require permission to unregister services
        if !(*context).user_context_json.is_null() {
            let cstr = CStr::from_ptr((*context).user_context_json);
            if let Ok(json) = cstr.to_str() {
                if let Ok(uc) = serde_json::from_str::<UserContext>(json) {
                    if !uc.is_admin() && !uc.has_permission("service_unregister") {
                        return PluginResult::PermissionDenied;
                    }
                }
            }
        }

        handle.registry.unregister(name)
    }
}
