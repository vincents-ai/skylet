# Plugin Development Guide

Welcome! This guide will help you create plugins for the Execution Engine ABI v2.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Project Setup](#project-setup)
3. [Basic Plugin Structure](#basic-plugin-structure)
4. [Implementing Entry Points](#implementing-entry-points)
5. [Using Engine Services](#using-engine-services)
6. [Configuration Management](#configuration-management)
7. [Error Handling](#error-handling)
8. [Testing](#testing)
9. [Performance Optimization](#performance-optimization)
10. [Advanced Topics](#advanced-topics)

## Quick Start

### Create a New Plugin

```bash
# Create project
cargo new --lib my-first-plugin
cd my-first-plugin

# Update Cargo.toml
cat > Cargo.toml << 'EOF'
[package]
name = "my-first-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-abi = "2.0"
libc = "0.2"
EOF

# Create plugin
mkdir src
cat > src/lib.rs << 'EOF'
use skylet_abi::{PluginContextV2, PluginInfoV2, PluginResult};
use std::ffi::CStr;

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResult {
    if context.is_null() {
        return PluginResult::InvalidRequest;
    }
    PluginResult::Success
}

#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(_context: *const PluginContextV2) -> PluginResult {
    PluginResult::Success
}

static PLUGIN_INFO: PluginInfoV2 = PluginInfoV2 {
    name: "my-first-plugin\0".as_ptr() as *const i8,
    version: "0.1.0\0".as_ptr() as *const i8,
    abi_version: "2.0\0".as_ptr() as *const i8,
    // ... other fields
};

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    &PLUGIN_INFO
}

#[no_mangle]
pub extern "C" fn plugin_handle_request_v2(
    request: *mut skylet_abi::PluginRequestV2,
    response: *mut skylet_abi::PluginResponseV2,
) -> PluginResult {
    if request.is_null() || response.is_null() {
        return PluginResult::InvalidRequest;
    }
    PluginResult::Success
}

#[no_mangle]
pub extern "C" fn plugin_prepare_hot_reload_v2(
    _context: *const PluginContextV2,
) -> PluginResult {
    PluginResult::Success
}

#[no_mangle]
pub extern "C" fn plugin_init_from_state_v2(
    _context: *const PluginContextV2,
    _state: *const i8,
) -> PluginResult {
    PluginResult::Success
}
EOF

# Build
cargo build --release
```

Your plugin is now ready: `target/release/libmy_first_plugin.so`

## Project Setup

### Recommended Directory Structure

```
my-plugin/
├── Cargo.toml
├── Cargo.lock
├── src/
│   ├── lib.rs          # Entry points
│   ├── mod.rs          # Module definitions
│   ├── services.rs     # Service implementations
│   ├── handlers.rs     # Request handlers
│   └── utils.rs        # Utilities
├── tests/
│   └── integration_test.rs  # Integration tests
├── examples/
│   └── usage.rs        # Usage examples
├── docs/
│   ├── API.md          # API documentation
│   ├── CONFIG.md       # Configuration guide
│   └── EXAMPLES.md     # Code examples
└── README.md
```

### Minimal Cargo.toml

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"
description = "A sample plugin for Execution Engine"
license = "MIT OR Apache-2.0"
repository = "https://github.com/yourorg/my-plugin"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-abi = "2.0"
# Add other dependencies as needed
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"

[profile.release]
opt-level = 3
lto = true
strip = true
```

## Basic Plugin Structure

### Essential Components

Every plugin needs:

1. **Static plugin info** describing the plugin
2. **Five required entry points** (init, shutdown, get_info, handle_request, hot_reload)
3. **Context storage** for plugin state
4. **Request handler logic** implementing actual functionality

### Template

```rust
use skylet_abi::{PluginContextV2, PluginInfoV2, PluginResult};
use std::sync::atomic::AtomicPtr;

// Global context storage (thread-safe)
static PLUGIN_CONTEXT: AtomicPtr<PluginContextV2> = AtomicPtr::new(std::ptr::null_mut());

// Plugin metadata
static PLUGIN_INFO: PluginInfoV2 = PluginInfoV2 {
    name: "my-plugin\0".as_ptr() as *const i8,
    version: "0.1.0\0".as_ptr() as *const i8,
    abi_version: "2.0\0".as_ptr() as *const i8,
    description: "My first plugin\0".as_ptr() as *const i8,
    author: "Your Name\0".as_ptr() as *const i8,
    license: "MIT\0".as_ptr() as *const i8,
    // ... other fields ...
};

// Initialize plugin
#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResult {
    if context.is_null() {
        return PluginResult::InvalidRequest;
    }
    
    // Store context for later use
    PLUGIN_CONTEXT.store(context as *mut _, std::sync::atomic::Ordering::SeqCst);
    
    // Initialize plugin state
    PluginResult::Success
}

// Shutdown plugin
#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(_context: *const PluginContextV2) -> PluginResult {
    // Clean up state
    PLUGIN_CONTEXT.store(std::ptr::null_mut(), std::sync::atomic::Ordering::SeqCst);
    PluginResult::Success
}

// Get plugin information
#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    &PLUGIN_INFO
}

// Handle incoming requests
#[no_mangle]
pub extern "C" fn plugin_handle_request_v2(
    request: *mut skylet_abi::PluginRequestV2,
    response: *mut skylet_abi::PluginResponseV2,
) -> PluginResult {
    if request.is_null() || response.is_null() {
        return PluginResult::InvalidRequest;
    }
    
    // Process request and fill response
    PluginResult::Success
}

// Prepare for hot reload
#[no_mangle]
pub extern "C" fn plugin_prepare_hot_reload_v2(
    _context: *const PluginContextV2,
) -> PluginResult {
    PluginResult::Success
}

// Restore from hot reload
#[no_mangle]
pub extern "C" fn plugin_init_from_state_v2(
    _context: *const PluginContextV2,
    _state: *const i8,
) -> PluginResult {
    PluginResult::Success
}
```

## Implementing Entry Points

### `plugin_init_v2`: Initialization

Called when plugin is loaded. Use this to:

- Validate the context
- Initialize plugin state
- Register services
- Start background tasks

**Best Practices:**

- Keep initialization fast (< 100ms)
- Return errors clearly if dependencies are missing
- Store context for later use
- Use thread-safe globals (Arc<Mutex<>>)

**Example:**

```rust
use std::sync::{Arc, Mutex};

struct PluginState {
    initialized: bool,
    request_count: u64,
}

static PLUGIN_STATE: Mutex<Option<Arc<PluginState>>> = Mutex::new(None);

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResult {
    if context.is_null() {
        return PluginResult::InvalidRequest;
    }
    
    match PLUGIN_STATE.lock() {
        Ok(mut state) => {
            *state = Some(Arc::new(PluginState {
                initialized: true,
                request_count: 0,
            }));
            PluginResult::Success
        }
        Err(_) => PluginResult::Error,
    }
}
```

### `plugin_handle_request_v2`: Request Processing

Main entry point for handling service calls. Structure:

```rust
#[no_mangle]
pub extern "C" fn plugin_handle_request_v2(
    request: *mut PluginRequestV2,
    response: *mut PluginResponseV2,
) -> PluginResult {
    // 1. Validate inputs
    if request.is_null() || response.is_null() {
        return PluginResult::InvalidRequest;
    }
    
    // 2. Parse request
    let method = unsafe {
        match CStr::from_ptr((*request).method) {
            Ok(m) => m.to_string_lossy(),
            Err(_) => return PluginResult::InvalidRequest,
        }
    };
    
    // 3. Route to handler
    match method.as_ref() {
        "get_status" => handle_get_status(request, response),
        "process_data" => handle_process_data(request, response),
        _ => PluginResult::NotImplemented,
    }
}

fn handle_get_status(
    _request: *mut PluginRequestV2,
    response: *mut PluginResponseV2,
) -> PluginResult {
    unsafe {
        // Build response
        (*response).status = PluginResult::Success as i32;
        // Set response body, headers, etc.
    }
    PluginResult::Success
}
```

## Using Engine Services

Plugins access engine services through the context pointer.

### Logging

```rust
use std::ffi::CString;

unsafe {
    if let Some(ctx) = PLUGIN_CONTEXT.load(std::sync::atomic::Ordering::SeqCst).as_ref() {
        let logger = (*ctx).logger;
        let msg = CString::new("Plugin initialized").unwrap();
        (logger.log)(ctx, skylet_abi::PluginLogLevel::Info, msg.as_ptr());
    }
}
```

### Configuration Access

```rust
unsafe {
    if let Some(ctx) = PLUGIN_CONTEXT.load(std::sync::atomic::Ordering::SeqCst).as_ref() {
        let cfg_mgr = (*ctx).config_manager;
        let plugin_name = CString::new("my-plugin").unwrap();
        let config = (cfg_mgr.get_config)(ctx, plugin_name.as_ptr());
    }
}
```

### Service Registry

Register a service:

```rust
unsafe {
    if let Some(ctx) = PLUGIN_CONTEXT.load(std::sync::atomic::Ordering::SeqCst).as_ref() {
        let registry = (*ctx).service_registry;
        let service_name = CString::new("my-service").unwrap();
        (registry.register)(ctx, service_name.as_ptr(), my_service_handler);
    }
}
```

## Configuration Management

### Define JSON Schema

```rust
#[no_mangle]
pub extern "C" fn plugin_get_config_schema_json() -> *const i8 {
    r#"{
        "type": "object",
        "properties": {
            "api_key": {
                "type": "string",
                "description": "API key for external service"
            },
            "timeout_seconds": {
                "type": "integer",
                "minimum": 1,
                "maximum": 300,
                "default": 30
            }
        },
        "required": ["api_key"]
    }"#.as_ptr() as *const i8
}
```

### Access Configuration

```rust
fn get_config_value(key: &str) -> Option<String> {
    unsafe {
        if let Some(ctx) = PLUGIN_CONTEXT.load(std::sync::atomic::Ordering::SeqCst).as_ref() {
            let cfg_mgr = (*ctx).config_manager;
            let plugin_name = CString::new("my-plugin").ok()?;
            let config_key = CString::new(key).ok()?;
            
            let value = (cfg_mgr.get_config_value)(ctx, plugin_name.as_ptr(), config_key.as_ptr());
            if !value.is_null() {
                return Some(CStr::from_ptr(value).to_string_lossy().to_string());
            }
        }
    }
    None
}
```

## Error Handling

Use appropriate error codes:

```rust
fn safe_operation() -> PluginResult {
    // Input validation
    if invalid_input {
        return PluginResult::InvalidRequest;
    }
    
    // Check permissions
    if !has_permission {
        return PluginResult::PermissionDenied;
    }
    
    // Check service availability
    if service_down {
        return PluginResult::ServiceUnavailable;
    }
    
    // Check timeouts
    if operation_timeout {
        return PluginResult::Timeout;
    }
    
    // Resource limits
    if out_of_memory {
        return PluginResult::ResourceExhausted;
    }
    
    // Success or generic error
    if operation_failed {
        return PluginResult::Error;
    }
    
    PluginResult::Success
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
        assert_eq!(info.name, "my-plugin\0".as_ptr() as *const i8);
        assert!(!info.version.is_null());
    }
}
```

### Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use skylet_abi_test_framework::*;
    use super::*;
    
    #[test]
    fn test_plugin_initialization() {
        let mut engine = TestEngine::new();
        engine.load_plugin("libmy_plugin.so").expect("Failed to load plugin");
        
        let info = engine.get_plugin_info().expect("Failed to get plugin info");
        assert_eq!(info.name, "my-plugin");
    }
    
    #[test]
    fn test_request_handling() {
        let mut engine = TestEngine::new();
        engine.load_plugin("libmy_plugin.so").unwrap();
        
        let request = TestRequest::new("get_status");
        let response = engine.handle_request(request).unwrap();
        assert_eq!(response.status, PluginResult::Success);
    }
}
```

## Performance Optimization

### Keep FFI Overhead Low

- Batch operations when possible
- Minimize pointer dereferences
- Use static storage instead of heap allocation in hot paths

### Efficient String Handling

```rust
// Inefficient: creates new CString on every call
fn bad_example(msg: &str) {
    unsafe {
        let cstring = CString::new(msg).unwrap();
        (logger.log)(ctx, PluginLogLevel::Info, cstring.as_ptr());
    }
}

// Efficient: reuse buffer
fn good_example(msg: &str) {
    static BUFFER: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    
    unsafe {
        if let Ok(mut buf) = BUFFER.lock() {
            buf.clear();
            buf.extend_from_slice(msg.as_bytes());
            buf.push(0); // Null terminator
            (logger.log)(ctx, PluginLogLevel::Info, buf.as_ptr() as *const i8);
        }
    }
}
```

### Use Async When Appropriate

Plugins can use Tokio for async operations:

```rust
#[tokio::main]
async fn async_operation() {
    let result = tokio::task::spawn_blocking(|| {
        // Blocking operation
    }).await;
}
```

## Advanced Topics

### Hot Reload

Implement state persistence for updates:

```rust
#[no_mangle]
pub extern "C" fn plugin_prepare_hot_reload_v2(
    context: *const PluginContextV2,
) -> PluginResult {
    // Serialize state
    let state = serde_json::to_string(&get_plugin_state()).unwrap();
    // Save to persistent storage
    PluginResult::Success
}

#[no_mangle]
pub extern "C" fn plugin_init_from_state_v2(
    context: *const PluginContextV2,
    state: *const i8,
) -> PluginResult {
    // Deserialize state
    let state_str = unsafe { CStr::from_ptr(state).to_string_lossy() };
    let plugin_state: PluginState = serde_json::from_str(&state_str).unwrap();
    // Restore state
    PluginResult::Success
}
```

### Multi-Threading

Use Arc and Mutex for shared state:

```rust
use std::sync::{Arc, Mutex};
use std::thread;

static SHARED_STATE: Mutex<Option<Arc<SharedData>>> = Mutex::new(None);

struct SharedData {
    data: Vec<u8>,
}

fn spawn_background_task() {
    if let Ok(state_lock) = SHARED_STATE.lock() {
        if let Some(shared) = state_lock.as_ref() {
            let shared_clone = Arc::clone(shared);
            thread::spawn(move || {
                // Access shared_clone
            });
        }
    }
}
```

## See Also

- [Plugin Contract](./PLUGIN_CONTRACT.md)
- [Migration Guide](./MIGRATION_GUIDE.md)
- [Configuration Reference](./CONFIG_REFERENCE.md)
- [Security Best Practices](./SECURITY.md)
- [Example Plugins](../plugins/)
