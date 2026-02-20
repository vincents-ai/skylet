// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

use execution_engine_core::service_registry::{ServiceRegistry, ServiceRegistryHandle};
use skylet_abi::PluginResult;
use std::ffi::CString;
use std::sync::Arc;

#[test]
fn test_register_and_get_service() {
    let registry = Arc::new(ServiceRegistry::new());
    let _handle = Box::into_raw(Box::new(ServiceRegistryHandle::new(registry.clone())));

    // Simulate registering a service
    let name = CString::new("test.service").unwrap();
    let typ = CString::new("example.v1.Test").unwrap();
    let dummy_ptr = 0xdeadbeef as *mut std::ffi::c_void;

    let res = execution_engine_core::service_registry::core_service_register(
        std::ptr::null(),
        name.as_ptr(),
        dummy_ptr,
        typ.as_ptr(),
    );
    // We passed a null context so this should be InvalidRequest
    assert_eq!(res, PluginResult::InvalidRequest);

    // Use handle directly to register
    let r = registry.register("test.service", dummy_ptr, "example.v1.Test");
    assert_eq!(r, PluginResult::Success);

    let got = registry.get("test.service", Some("example.v1.Test"));
    assert_eq!(got, dummy_ptr);

    // Wrong type should return null
    let got2 = registry.get("test.service", Some("other.Type"));
    assert!(got2.is_null());

    // Unregister
    let u = registry.unregister("test.service");
    assert_eq!(u, PluginResult::Success);
}
