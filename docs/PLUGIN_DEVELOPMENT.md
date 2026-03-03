# Plugin Development Guide

Welcome! This guide will help you create plugins for the Skylet Execution Engine using the V2 ABI.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Project Setup](#project-setup)
3. [Using skylet-plugin-common](#using-skylet-plugin-common)
4. [The skylet_plugin_v2! Macro](#the-skylet_plugin_v2-macro)
5. [FFI Builders](#ffi-builders)
6. [Manual V2 ABI Implementation](#manual-v2-abi-implementation)
7. [Using Engine Services](#using-engine-services)
8. [Configuration Management](#configuration-management)
9. [Error Handling](#error-handling)
10. [Testing](#testing)
11. [Performance Optimization](#performance-optimization)
12. [Advanced Topics](#advanced-topics)

## Quick Start

The fastest way to create a plugin is using the `skylet_plugin_v2!` macro from `skylet-plugin-common`:

```bash
# Create project
cargo new --lib my-first-plugin
cd my-first-plugin
```

**Cargo.toml:**
```toml
[package]
name = "my-first-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-abi = { version = "2.0" }
skylet-plugin-common = { version = "0.5" }
```

**src/lib.rs:**
```rust
use skylet_plugin_common::skylet_plugin_v2;

skylet_plugin_v2! {
    name: "my-first-plugin",
    version: "0.1.0",
    description: "My awesome Skylet plugin",
    author: "Your Name",
    license: "MIT OR Apache-2.0",
    tagline: "Does something useful",
    category: skylet_abi::PluginCategory::Utility,
    max_concurrency: 10,
    supports_async: true,
    capabilities: ["my.read", "my.write"],
}
```

Build and you're done:
```bash
cargo build --release
# Output: target/release/libmy_first_plugin.so
```

The macro generates all required V2 ABI entry points:
- `plugin_get_info_v2()` - Returns plugin metadata
- `plugin_init_v2()` - Plugin initialization
- `plugin_shutdown_v2()` - Plugin shutdown
- `plugin_handle_request_v2()` - Request handler (returns NotImplemented by default)
- `plugin_health_check_v2()` - Health check (returns Healthy by default)
- `plugin_create_v2()` - Returns PluginApiV2 struct

## Project Setup

### Recommended Directory Structure

```
my-plugin/
├── Cargo.toml
├── Cargo.lock
├── src/
│   ├── lib.rs          # Entry points (macro invocation)
│   ├── handlers.rs     # Request handlers
│   └── services.rs     # Business logic
├── tests/
│   └── integration.rs
└── README.md
```

### Recommended Cargo.toml

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"
description = "A sample plugin for Skylet"
license = "MIT OR Apache-2.0"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-abi = { version = "2.0" }
skylet-plugin-common = { version = "0.5" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["rt-multi-thread"] }
thiserror = "1"

[profile.release]
opt-level = 3
lto = true
strip = true
```

## Using skylet-plugin-common

The `skylet-plugin-common` crate (v0.5.0+) provides utilities to eliminate boilerplate:

### Features

| Feature | Description |
|---------|-------------|
| `skylet_plugin_v2!` | Macro that generates all V2 ABI entry points |
| `CapabilityBuilder` | Fluent API for building capability arrays |
| `TagsBuilder` | Fluent API for building tag arrays |
| `ServiceInfoBuilder` | Fluent API for service metadata |
| `static_cstr!` | Create static null-terminated byte strings |
| `cstr_ptr!` | Convert static byte strings to C pointers |
| `PluginState<T>` | Thread-safe plugin state management |
| `ConfigManager` | Configuration loading and validation |
| `SecretsManager` | Secure secrets storage |
| `config_paths` | RFC-0006 compliant config path resolution |

### Import Pattern

```rust
use skylet_plugin_common::{
    skylet_plugin_v2,
    CapabilityBuilder,
    TagsBuilder,
    ServiceInfoBuilder,
    static_cstr,
    cstr_ptr,
};
```

## The skylet_plugin_v2! Macro

### Basic Usage

```rust
use skylet_plugin_common::skylet_plugin_v2;

skylet_plugin_v2! {
    name: "my-plugin",
    version: "0.1.0",
    description: "Plugin description",
    author: "Author Name",
    license: "MIT OR Apache-2.0",
    tagline: "Short tagline",
    category: skylet_abi::PluginCategory::Utility,
    max_concurrency: 10,
    supports_async: true,
    capabilities: ["cap.read", "cap.write"],
}
```

### With Tags

```rust
skylet_plugin_v2! {
    name: "my-plugin",
    version: "0.1.0",
    description: "Plugin description",
    author: "Author Name",
    license: "MIT OR Apache-2.0",
    tagline: "Short tagline",
    category: skylet_abi::PluginCategory::Security,
    max_concurrency: 10,
    supports_async: true,
    capabilities: ["secrets.read", "secrets.write"],
    tags: ["security", "encryption", "bootstrap"],
}
```

### With Lifecycle Hooks

```rust
use skylet_abi::v2_spec::{PluginContextV2, PluginResultV2, HealthStatus};

// Custom initialization
fn my_init(ctx: *const PluginContextV2) -> PluginResultV2 {
    // Initialize plugin state, connect to services, etc.
    println!("Plugin initializing...");
    PluginResultV2::Success
}

// Custom shutdown
fn my_shutdown(ctx: *const PluginContextV2) -> PluginResultV2 {
    // Cleanup resources
    println!("Plugin shutting down...");
    PluginResultV2::Success
}

// Custom health check
fn my_health_check(ctx: *const PluginContextV2) -> HealthStatus {
    // Check dependencies, connections, etc.
    HealthStatus::Healthy
}

skylet_plugin_v2! {
    name: "my-plugin",
    version: "0.1.0",
    description: "Plugin with custom hooks",
    author: "Author Name",
    license: "MIT OR Apache-2.0",
    tagline: "Customizable plugin",
    category: skylet_abi::PluginCategory::Utility,
    max_concurrency: 10,
    supports_async: true,
    capabilities: ["my.operation"],
    tags: ["custom"],
    on_init: my_init,
    on_shutdown: my_shutdown,
    health_check: my_health_check,
}
```

## FFI Builders

For plugins that need more control over their metadata (e.g., providing services), use the builders directly.

### CapabilityBuilder

```rust
use skylet_plugin_common::CapabilityBuilder;

let (capabilities_ptr, num_capabilities) = CapabilityBuilder::new()
    .add("secrets.get", "Get secret value by key", Some("secrets.read"))
    .add("secrets.set", "Set secret value by key", Some("secrets.write"))
    .add("secrets.delete", "Delete secret by key", Some("secrets.delete"))
    .add("secrets.list", "List all secrets", Some("secrets.list"))
    .build();

// Use in PluginInfoV2:
// capabilities: capabilities_ptr,
// num_capabilities,
```

### TagsBuilder

```rust
use skylet_plugin_common::TagsBuilder;

let (tags_ptr, num_tags) = TagsBuilder::new()
    .add("security")
    .add("encryption")
    .add("bootstrap")
    .build();

// Use in PluginInfoV2:
// tags: tags_ptr,
// num_tags,
```

### ServiceInfoBuilder

For plugins that provide services to other plugins:

```rust
use skylet_plugin_common::ServiceInfoBuilder;

let service_ptr = ServiceInfoBuilder::new("ConfigService", "2.0.0")
    .description("Centralized configuration management service")
    .interface_spec("config-service-v2")
    .build();

// Use in PluginInfoV2:
// provides_services: service_ptr,
// num_provides_services: 1,
```

### Static String Macros

For efficient static strings without heap allocation:

```rust
use skylet_plugin_common::{static_cstr, cstr_ptr};

// Create static null-terminated byte strings
const PLUGIN_NAME: &[u8] = static_cstr!("my-plugin");
const PLUGIN_VERSION: &[u8] = static_cstr!("1.0.0");

// Convert to C pointers
let name_ptr = cstr_ptr!(PLUGIN_NAME);
let version_ptr = cstr_ptr!(PLUGIN_VERSION);
```

## Manual V2 ABI Implementation

For maximum control, implement the V2 ABI manually. This is useful for bootstrap plugins or complex services.

### Example: Manual V2 Implementation

```rust
use skylet_abi::v2_spec::*;
use skylet_plugin_common::{CapabilityBuilder, TagsBuilder, static_cstr, cstr_ptr};
use std::ffi::CString;
use std::ptr;

const PLUGIN_NAME: &[u8] = static_cstr!("my-manual-plugin");
const PLUGIN_VERSION: &[u8] = static_cstr!("1.0.0");
const PLUGIN_DESCRIPTION: &[u8] = static_cstr!("A manually implemented V2 plugin");

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    static mut INFO: Option<PluginInfoV2> = None;
    
    unsafe {
        if INFO.is_none() {
            let (caps_ptr, num_caps) = CapabilityBuilder::new()
                .add("my.read", "Read data", Some("read"))
                .add("my.write", "Write data", Some("write"))
                .build();
            
            let (tags_ptr, num_tags) = TagsBuilder::new()
                .add("utility")
                .add("data")
                .build();
            
            INFO = Some(PluginInfoV2 {
                name: cstr_ptr!(PLUGIN_NAME),
                version: cstr_ptr!(PLUGIN_VERSION),
                description: cstr_ptr!(PLUGIN_DESCRIPTION),
                author: CString::new("Your Name").unwrap().into_raw(),
                license: CString::new("MIT").unwrap().into_raw(),
                homepage: ptr::null(),
                skylet_version_min: CString::new("0.1.0").unwrap().into_raw(),
                skylet_version_max: ptr::null(),
                abi_version: CString::new("2.0").unwrap().into_raw(),
                dependencies: ptr::null(),
                num_dependencies: 0,
                provides_services: ptr::null(),
                num_provides_services: 0,
                requires_services: ptr::null(),
                num_requires_services: 0,
                capabilities: caps_ptr,
                num_capabilities: num_caps,
                min_resources: ptr::null(),
                max_resources: ptr::null(),
                tags: tags_ptr,
                num_tags,
                category: PluginCategory::Utility,
                supports_hot_reload: false,
                supports_async: true,
                supports_streaming: false,
                max_concurrency: 10,
                tagline: ptr::null(),
                icon_url: ptr::null(),
                maturity_level: MaturityLevel::Stable,
                build_timestamp: ptr::null(),
                build_hash: ptr::null(),
                git_commit: ptr::null(),
                build_environment: ptr::null(),
                metadata: ptr::null(),
            });
        }
        INFO.as_ref().unwrap()
    }
}

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }
    PluginResultV2::Success
}

#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(_context: *const PluginContextV2) -> PluginResultV2 {
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
    HealthStatus::Healthy
}
```

## Using Engine Services

Plugins access engine services through the context pointer.

### Logging

```rust
use skylet_abi::PluginLogLevel;
use std::ffi::CString;

unsafe fn log_message(ctx: *const PluginContextV2, level: PluginLogLevel, msg: &str) {
    if let Some(context) = ctx.as_ref() {
        let logger = &(*context).logger;
        let c_msg = CString::new(msg).unwrap();
        (logger.log)(ctx, level, c_msg.as_ptr());
    }
}
```

### Configuration Access

```rust
unsafe fn get_config(ctx: *const PluginContextV2, key: &str) -> Option<String> {
    let context = ctx.as_ref()?;
    let cfg_mgr = &(*context).config_manager;
    let c_key = CString::new(key).ok()?;
    let value = (cfg_mgr.get_config_value)(ctx, c_key.as_ptr());
    if value.is_null() {
        return None;
    }
    Some(CStr::from_ptr(value).to_string_lossy().to_string())
}
```

## Configuration Management

### Using skylet-plugin-common Config Paths

```rust
use skylet_plugin_common::config_paths;

// Find config file (searches standard locations)
if let Some(path) = config_paths::find_config("my-plugin") {
    println!("Config found at: {:?}", path);
}

// Get standard config path
let standard_path = config_paths::get_standard_config_path("my-plugin");
// Returns: ~/.config/skylet/plugins/my-plugin.toml
```

### Config File Locations (RFC-0006)

1. `~/.config/skylet/plugins/{plugin_name}.toml` (primary)
2. `data/{plugin_name}.toml` (local project)
3. `/etc/skylet/plugins/{plugin_name}.toml` (system)

## Error Handling

Use appropriate V2 result codes:

```rust
use skylet_abi::v2_spec::PluginResultV2;

fn handle_operation() -> PluginResultV2 {
    // Input validation
    if invalid_input {
        return PluginResultV2::InvalidRequest;
    }
    
    // Permission check
    if !has_permission {
        return PluginResultV2::PermissionDenied;
    }
    
    // Service availability
    if service_down {
        return PluginResultV2::ServiceUnavailable;
    }
    
    // Timeout
    if operation_timeout {
        return PluginResultV2::Timeout;
    }
    
    // Resource limits
    if out_of_memory {
        return PluginResultV2::ResourceExhausted;
    }
    
    // Not implemented
    if unsupported_operation {
        return PluginResultV2::NotImplemented;
    }
    
    // Generic error
    if operation_failed {
        return PluginResultV2::Error;
    }
    
    PluginResultV2::Success
}
```

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plugin_info() {
        let info = unsafe { plugin_get_info_v2().as_ref() }.unwrap();
        assert!(!info.name.is_null());
        assert!(!info.version.is_null());
    }
    
    #[test]
    fn test_capability_builder() {
        let (ptr, count) = CapabilityBuilder::new()
            .add("test.cap", "Test capability", None)
            .build();
        assert_eq!(count, 1);
        assert!(!ptr.is_null());
    }
}
```

## Performance Optimization

### Use Static Strings

```rust
// Efficient: compile-time string, no allocation
const NAME: &[u8] = static_cstr!("my-plugin");
let ptr = cstr_ptr!(NAME);

// Less efficient: runtime allocation
let name = CString::new("my-plugin").unwrap();
let ptr = name.as_ptr();
```

### Batch Operations

Minimize FFI boundary crossings by batching operations.

### Thread-Safe State

```rust
use skylet_plugin_common::PluginState;

struct MyState {
    counter: u64,
    data: Vec<String>,
}

static STATE: PluginState<MyState> = PluginState::new(MyState {
    counter: 0,
    data: Vec::new(),
});

async fn increment_counter() {
    STATE.write(|state| {
        state.counter += 1;
    }).await;
}
```

## Advanced Topics

### Hot Reload State Transfer

Implement `serialize_state` and `deserialize_state` for epoch-based hot reload:

```rust
use skylet_abi::v2_spec::PluginApiV2;

// In your PluginApiV2 struct:
PluginApiV2 {
    // ... other fields ...
    serialize_state: Some(my_serialize_state),
    deserialize_state: Some(my_deserialize_state),
    free_state: Some(my_free_state),
}

extern "C" fn my_serialize_state(
    _ctx: *const PluginContextV2,
    out_len: *mut usize,
) -> *mut u8 {
    let state = get_current_state();
    let bytes = serde_json::to_vec(&state).unwrap();
    unsafe { *out_len = bytes.len(); }
    Box::leak(bytes.into_boxed_slice()).as_mut_ptr()
}
```

### Custom Request Handling

Override the default `plugin_handle_request_v2`:

```rust
#[no_mangle]
pub extern "C" fn plugin_handle_request_v2(
    context: *const PluginContextV2,
    request: *const RequestV2,
    response: *mut ResponseV2,
) -> PluginResultV2 {
    if request.is_null() || response.is_null() {
        return PluginResultV2::InvalidRequest;
    }
    
    unsafe {
        let method = CStr::from_ptr((*request).method).to_str().unwrap_or("");
        
        match method {
            "GET" => handle_get(request, response),
            "POST" => handle_post(request, response),
            _ => PluginResultV2::NotImplemented,
        }
    }
}
```

## See Also

- [Plugin Contract](./PLUGIN_CONTRACT.md) - ABI contract specification
- [ABI Stability](./ABI_STABILITY.md) - Version compatibility guarantees
- [Configuration Reference](./CONFIG_REFERENCE.md) - Config file formats
- [Security](./SECURITY.md) - Security best practices
- [Example Plugins](../plugins/) - Real plugin implementations
