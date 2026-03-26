# Skylet V1 to V2 Migration Guide

This guide helps you migrate plugins from Skylet V1 ABI to V2 ABI.

## Overview

Skylet V2 introduces significant improvements to the plugin ABI:

- **Renamed entry points** with `_v2` suffix for versioning clarity
- **Service registry** replacing direct context field access
- **Structured error codes** instead of generic success/failure
- **Type-safe configuration** with schema validation
- **New lifecycle hooks** for config changes and metrics

## Quick Migration Checklist

- [ ] Rename `plugin_init` → `plugin_init_v2`
- [ ] Rename `plugin_shutdown` → `plugin_shutdown_v2`
- [ ] Rename `plugin_get_info` → `plugin_get_info_v2`
- [ ] Add `plugin_process_request` for request handling
- [ ] Add `plugin_on_config_change` (optional, for hot reload)
- [ ] Add `plugin_get_metrics` (optional, for monitoring)
- [ ] Update `PluginContext` usage to use service registry
- [ ] Update error handling to use `PluginResultV2`
- [ ] Update `Cargo.toml` dependencies

## Entry Point Changes

### V1 Entry Points (Deprecated)

```rust
#[no_mangle]
pub extern "C" fn plugin_init(context: *const PluginContext) -> PluginResult {
    // V1 initialization
}

#[no_mangle]
pub extern "C" fn plugin_process(request: *const c_char) -> *mut c_char {
    // V1 request handling
}

#[no_mangle]
pub extern "C" fn plugin_get_info() -> *const PluginInfo {
    // V1 plugin info
}

#[no_mangle]
pub extern "C" fn plugin_shutdown() -> PluginResult {
    // V1 cleanup
}
```

### V2 Entry Points (Current)

```rust
use skylet_abi::{PluginContextV2, PluginInfoV2, PluginResultV2};

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    // V2 initialization with service registry access
}

#[no_mangle]
pub extern "C" fn plugin_process_request(
    context: *const PluginContextV2,
    action: *const c_char,
    params: *const c_char,
) -> *mut c_char {
    // V2 request handling with action dispatch
}

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    // V2 plugin info with extended metadata
}

#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(context: *const PluginContextV2) -> PluginResultV2 {
    // V2 cleanup
}

// Optional: Hot reload support
#[no_mangle]
pub extern "C" fn plugin_on_config_change(
    context: *const PluginContextV2,
    config_json: *const c_char,
) -> PluginResultV2 {
    // Handle configuration changes at runtime
}

// Optional: Metrics collection
#[no_mangle]
pub extern "C" fn plugin_get_metrics(
    context: *const PluginContextV2,
) -> *mut c_char {
    // Return JSON metrics
}
```

## Context Changes

### V1 Context (Direct Field Access)

```rust
// V1: Direct field access
unsafe {
    let logger = (*context).logger;
    let config = (*context).config;
    // Direct access to fields
}
```

### V2 Context (Service Registry)

```rust
// V2: Service registry with capability-based access
unsafe {
    let registry = &(*context).service_registry;
    
    // Get logger service
    if let Some(logger) = registry.get_service("logger") {
        logger.log("Plugin initialized");
    }
    
    // Get config service
    if let Some(config) = registry.get_service("config") {
        let value = config.get("my_key");
    }
}
```

## Error Handling Changes

### V1 Errors (Generic)

```rust
pub enum PluginResult {
    Success = 0,
    Error = -1,
}
```

### V2 Errors (Structured)

```rust
pub enum PluginResultV2 {
    Success = 0,
    Error = -1,
    InvalidRequest = -2,
    ServiceUnavailable = -3,
    PermissionDenied = -4,
    NotImplemented = -5,
    Timeout = -6,
    ResourceExhausted = -7,
    Pending = -8,  // For async operations
}
```

Use specific error codes for better diagnostics:

```rust
fn handle_request(action: &str) -> PluginResultV2 {
    match action {
        "known_action" => PluginResultV2::Success,
        "unknown_action" => PluginResultV2::NotImplemented,
        "invalid_params" => PluginResultV2::InvalidRequest,
        _ => PluginResultV2::Error,
    }
}
```

## PluginInfo Changes

### V1 PluginInfo

```rust
pub struct PluginInfo {
    pub name: *const c_char,
    pub version: *const c_char,
}
```

### V2 PluginInfo

```rust
pub struct PluginInfoV2 {
    // Basic info
    pub name: *const c_char,
    pub version: *const c_char,
    pub description: *const c_char,
    pub author: *const c_char,
    
    // ABI version
    pub abi_version: *const c_char,  // "2.0.0"
    
    // Classification
    pub category: PluginCategory,
    pub maturity: MaturityLevel,
    
    // Services
    pub provided_services: *const ServiceInfo,
    pub required_services: *const DependencyInfo,
    
    // Capabilities
    pub supports_hot_reload: bool,
    pub supports_metrics: bool,
}
```

## Configuration Changes

### V1 Configuration (String-based)

```rust
// V1: Ad-hoc string parsing
let config_str = get_config_string();
let value = parse_config_value(config_str, "key");
```

### V2 Configuration (Schema-based)

```rust
use skylet_abi::config::{ConfigSchema, FieldType};

// Define schema
let schema = ConfigSchema::new("my-plugin")
    .field("timeout_ms", FieldType::Integer)
    .field("enabled", FieldType::Boolean)
    .field("api_key", FieldType::Secret);

// Validate and access
let config = schema.validate(config_json)?;
let timeout: i64 = config.get("timeout_ms")?;
let enabled: bool = config.get("enabled")?;
```

## Cargo.toml Updates

Update your `Cargo.toml`:

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-abi = { version = "0.1", features = ["v2"] }
tokio = { version = "1", features = ["rt-multi-thread"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

## Testing Your Migration

Use the plugin-test-harness to verify your migration:

```rust
use plugin_test_harness::{PluginTestHarness, PluginTestConfig};

#[tokio::test]
async fn test_migrated_plugin() {
    let config = PluginTestConfig {
        plugin_path: "./target/release/libmy_plugin.so".to_string(),
        ..Default::default()
    };
    
    let mut harness = PluginTestHarness::new(config);
    harness.load_plugin().await.unwrap();
    
    // Test V2 entry points
    let response = harness.execute_action("health", "{}").unwrap();
    assert!(response.contains("ok"));
}
```

## Common Migration Issues

### Issue: Missing V2 Entry Points

**Symptom**: Plugin fails to load with "Missing required symbol"

**Solution**: Ensure all required V2 entry points are exported:
- `plugin_init_v2`
- `plugin_shutdown_v2`
- `plugin_get_info_v2`
- `plugin_process_request`

### Issue: Null Context Pointer

**Symptom**: Segfault or access violation

**Solution**: Always check for null before dereferencing:

```rust
if context.is_null() {
    return PluginResultV2::InvalidRequest;
}
```

### Issue: String Memory Leaks

**Symptom**: Memory usage grows over time

**Solution**: Use `CString` properly and ensure callers free returned strings:

```rust
// Return string that caller must free
let result = CString::new(json_response).unwrap();
result.into_raw()  // Caller must call free_string()
```

## Deprecation Timeline

| Version | Status | Support |
|---------|--------|---------|
| V1 ABI | Deprecated | Removed in v3.0.0 |
| V2 ABI | Current | Stable until v3.0.0 |

**Recommendation**: Migrate all plugins to V2 ABI as soon as possible.

## Getting Help

- **Documentation**: [Plugin Development Guide](PLUGIN_DEVELOPMENT.md)
- **Examples**: See `examples/` directory for V2 plugin examples
- **Issues**: Report problems on [GitHub Issues](https://github.com/vincents-ai/skylet/issues)

## See Also

- [ABI Stability Guide](ABI_STABILITY.md)
- [Plugin Contract](PLUGIN_CONTRACT.md)
- [Configuration Reference](CONFIG_REFERENCE.md)
