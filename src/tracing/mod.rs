// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

use skylet_abi::{PluginContext, PluginTracer, SpanHandle};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU64, Ordering};

static SPAN_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Lightweight tracing initialization for tests — uses tracing_subscriber default formatter.
pub fn init_tracing_for_tests() -> anyhow::Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    Ok(())
}

/// Create a minimal PluginTracer shim usable by plugins via the ABI. This does not
/// provide a full OpenTelemetry implementation — it's a stable testing shim that
/// records span handles and leaves integration to the host.
pub fn create_plugin_tracer() -> Box<PluginTracer> {
    Box::new(PluginTracer {
        start_span: start_span_ffi,
        end_span: end_span_ffi,
        add_event: add_event_ffi,
        set_attribute: set_attribute_ffi,
    })
}

extern "C" fn start_span_ffi(
    _context: *const (),
    name_ptr: *const c_char,
    _name_len: usize,
) -> SpanHandle {
    // Return a monotonically increasing handle; not a real span object.
    let _ = if !name_ptr.is_null() {
        unsafe { CStr::from_ptr(name_ptr).to_string_lossy() }
    };
    SPAN_COUNTER.fetch_add(1, Ordering::SeqCst)
}

extern "C" fn end_span_ffi(_context: *const (), _span_handle: SpanHandle) {
    // No-op shim for tests.
}

extern "C" fn add_event_ffi(_context: *const (), _name_ptr: *const c_char, _name_len: usize) {
    // No-op shim for tests.
}

extern "C" fn set_attribute_ffi(
    _context: *const (),
    _key_ptr: *const c_char,
    _key_len: usize,
    _value_ptr: *const c_char,
    _value_len: usize,
) {
    // No-op shim for tests.
}

/// Create a minimal PluginTracer shim usable by plugins via the ABI. This does not
/// provide a full OpenTelemetry implementation — it's a stable testing shim that
/// records span handles and leaves integration to the host.
pub fn create_plugin_tracer() -> Box<PluginTracer> {
    Box::new(PluginTracer {
        start_span: start_span_ffi,
        end_span: end_span_ffi,
        add_event: add_event_ffi,
        set_attribute: set_attribute_ffi,
    })
}

extern "C" fn start_span_ffi(
    _context: *const (),
    name_ptr: *const c_char,
    _name_len: usize,
) -> SpanHandle {
    // Return a monotonically increasing handle; not a real span object.
    let _ = if !name_ptr.is_null() {
        unsafe { CStr::from_ptr(name_ptr).to_string_lossy() }
    };
    SPAN_COUNTER.fetch_add(1, Ordering::SeqCst)
}

extern "C" fn end_span_ffi(_context: *const (), _span_handle: SpanHandle) {
    // No-op shim for tests.
}

extern "C" fn add_event_ffi(_context: *const (), _name_ptr: *const c_char, _name_len: usize) {
    // No-op shim for tests.
}

extern "C" fn set_attribute_ffi(
    _context: *const (),
    _key_ptr: *const c_char,
    _key_len: usize,
    _value_ptr: *const c_char,
    _value_len: usize,
) {
    // No-op shim for tests.
}
