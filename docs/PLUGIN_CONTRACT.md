# Plugin Contract - Execution Engine ABI v2.0.0

## Overview

This document defines the contract between plugins and the Execution Engine for ABI v2.0.0. It specifies:

- Required plugin entry points (FFI functions)
- Context structure and services
- Error codes and handling
- Lifecycle events
- Version compatibility

## ABI Version

- **Current Version**: v2.0.0
- **Stability**: Stable (no breaking changes planned)
- **Release Date**: 2024-02-20
- **License**: MIT OR Apache-2.0

## Required Entry Points

Every plugin built for Execution Engine ABI v2 must export the following FFI functions. All functions must use `#[no_mangle]` and `extern "C"` calling convention.

### 1. `plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2`

Called when the plugin is loaded by the execution engine.

**Responsibilities:**
- Initialize plugin state
- Validate the provided context
- Register services with the service registry
- Start background tasks if needed

**Parameters:**
- `context`: Pointer to the plugin context (valid for the plugin's lifetime)

**Returns:**
- `PluginResultV2::Success` (0) if initialization succeeded
- `PluginResultV2::Error` (-1) for initialization failures
- `PluginResultV2::InvalidRequest` (-2) if context is invalid

**Example:**
```rust
#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }
    
    unsafe {
        // Store context for later use
        PLUGIN_CONTEXT = Some(context);
        
        // Initialize plugin state
        if let Err(e) = initialize_plugin_state() {
            return PluginResultV2::Error;
        }
    }
    
    PluginResultV2::Success
}
```

### 2. `plugin_shutdown_v2(context: *const PluginContextV2) -> PluginResultV2`

Called when the plugin is unloaded or the engine is shutting down.

**Responsibilities:**
- Gracefully shut down background tasks
- Release resources
- Persist state if necessary
- Close connections

**Parameters:**
- `context`: Pointer to the plugin context

**Returns:**
- `PluginResultV2::Success` if shutdown succeeded
- `PluginResultV2::Error` if shutdown failed

**Example:**
```rust
#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(context: *const PluginContextV2) -> PluginResultV2 {
    unsafe {
        PLUGIN_CONTEXT = None;
    }
    
    // Gracefully shut down
    PluginResultV2::Success
}
```

### 3. `plugin_get_info_v2() -> *const PluginInfoV2`

Returns static information about the plugin.

**Responsibilities:**
- Provide plugin metadata
- Declare capabilities and permissions
- Specify version and dependencies
- Define configuration schema

**Returns:**
- Pointer to a static `PluginInfoV2` structure
- Must never return NULL

**Stability:** This function is called frequently and must be extremely fast (typically just returning a static pointer).

**Example:**
```rust
static PLUGIN_INFO: PluginInfoV2 = PluginInfoV2 {
    name: "my-plugin\0".as_ptr() as *const c_char,
    version: "1.0.0\0".as_ptr() as *const c_char,
    // ... other fields ...
};

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    &PLUGIN_INFO
}
```

### 4. `plugin_handle_request_v2(request: *mut PluginRequestV2, response: *mut PluginResponseV2) -> PluginResultV2`

Handles service requests from other plugins or the engine.

**Responsibilities:**
- Parse the request
- Invoke appropriate handler
- Populate the response
- Handle errors gracefully

**Parameters:**
- `request`: Mutable pointer to the request (plugin may modify for scratch space)
- `response`: Mutable pointer to the response (must be filled with results)

**Returns:**
- `PluginResultV2::Success` if request was handled
- `PluginResultV2::Error` for processing errors
- `PluginResultV2::InvalidRequest` for malformed requests
- `PluginResultV2::Timeout` if processing took too long

**Example:**
```rust
#[no_mangle]
pub extern "C" fn plugin_handle_request_v2(
    request: *mut PluginRequestV2,
    response: *mut PluginResponseV2,
) -> PluginResultV2 {
    if request.is_null() || response.is_null() {
        return PluginResultV2::InvalidRequest;
    }
    
    unsafe {
        // Handle request and populate response
        // ...
    }
    
    PluginResultV2::Success
}
```

### 5. `plugin_prepare_hot_reload_v2(context: *const PluginContextV2) -> PluginResultV2`

Called before the plugin is reloaded to allow state migration.

**Responsibilities:**
- Prepare state for migration to new version
- Serialize state to persistent storage if needed
- Validate that hot reload is safe

**Parameters:**
- `context`: Pointer to the plugin context

**Returns:**
- `PluginResultV2::Success` if hot reload is safe
- `PluginResultV2::Error` if hot reload cannot proceed
- `PluginResultV2::NotImplemented` if hot reload is not supported

**Example:**
```rust
#[no_mangle]
pub extern "C" fn plugin_prepare_hot_reload_v2(context: *const PluginContextV2) -> PluginResultV2 {
    // Save any persistent state before reload
    PluginResultV2::Success
}
```

### 6. `plugin_init_from_state_v2(context: *const PluginContextV2, state: *const c_char) -> PluginResultV2`

Called after hot reload to restore the plugin state.

**Responsibilities:**
- Restore plugin state from serialized data
- Validate restored state
- Resume operations

**Parameters:**
- `context`: Pointer to the plugin context
- `state`: C-string containing serialized state

**Returns:**
- `PluginResultV2::Success` if state was restored
- `PluginResultV2::Error` if state is corrupt or incompatible
- `PluginResultV2::NotImplemented` if hot reload is not supported

**Example:**
```rust
#[no_mangle]
pub extern "C" fn plugin_init_from_state_v2(
    context: *const PluginContextV2,
    state: *const c_char,
) -> PluginResultV2 {
    if state.is_null() {
        return PluginResultV2::InvalidRequest;
    }
    
    // Restore state
    PluginResultV2::Success
}
```

## Optional Entry Points

Plugins may implement these entry points for enhanced functionality:

### `plugin_get_config_schema_json() -> *const c_char`

Returns a JSON Schema describing the plugin's configuration.

**Returns:**
- Pointer to a C-string containing valid JSON Schema
- Must never return NULL (return empty schema `{}` if no config needed)

### `plugin_mcp_tools() -> *const MCPToolSchema`

Returns MCP (Model Context Protocol) tool definitions for AI agents.

**Returns:**
- Array of MCP tool schemas
- Each tool defines input/output parameters and capabilities

## Context Structure (PluginContextV2)

The `PluginContextV2` structure provides access to engine services:

```c
struct PluginContextV2 {
    // Service discovery
    ServiceRegistry* service_registry;
    
    // Logging
    Logger* logger;
    
    // Configuration
    ConfigManager* config_manager;
    
    // Secrets
    SecretsProvider* secrets_provider;
    
    // RPC/networking
    RPCClient* rpc_client;
    
    // Job queue
    JobQueue* job_queue;
    
    // Permissions
    PermissionsManager* permissions_manager;
    
    // Plugin ID and metadata
    const char* plugin_id;
    const char* plugin_name;
    const char* plugin_version;
};
```

### Using Services

**Logging Example:**
```rust
unsafe {
    if let Some(context) = PLUGIN_CONTEXT {
        let logger = (*context).logger;
        let msg = CString::new("Plugin initialized").unwrap();
        (logger.log)(context, PluginLogLevel::Info, msg.as_ptr());
    }
}
```

**Service Registry Example:**
```rust
unsafe {
    if let Some(context) = PLUGIN_CONTEXT {
        let registry = (*context).service_registry;
        let service_name = CString::new("my-service").unwrap();
        (registry.register)(context, service_name.as_ptr(), handler_fn);
    }
}
```

## Error Codes

Plugins should return appropriate `PluginResultV2` codes:

| Value | Name | Meaning |
|-------|------|---------|
| 0 | Success | Operation completed successfully |
| -1 | Error | Generic error (details in logs) |
| -2 | InvalidRequest | Request was malformed or invalid |
| -3 | ServiceUnavailable | Required service not available |
| -4 | PermissionDenied | Insufficient permissions for operation |
| -5 | NotImplemented | Feature not implemented by plugin |
| -6 | Timeout | Operation exceeded time limit |
| -7 | ResourceExhausted | Out of memory or other resources |

## Version Compatibility

### Forward Compatibility

Plugins built for ABI v2.0.0 will work with future v2.x releases that:

- Add new optional services to the context
- Add new optional entry points
- Maintain existing service signatures

### Backward Compatibility

Plugins built for older ABI versions:

- Will NOT work with ABI v2.x (breaking change requires major version bump)
- Must be recompiled for the target ABI version

### Version Checking

Plugins can check the ABI version at runtime:

```rust
unsafe {
    if let Some(context) = PLUGIN_CONTEXT {
        // Use version information to adjust behavior
        let version = CStr::from_ptr((*context).plugin_version);
    }
}
```

## Lifecycle Events

### Plugin Load Sequence

1. **Engine loads plugin binary** using dynamic linking
2. **Engine calls `plugin_init_v2()`** to initialize
3. **Engine caches result of `plugin_get_info_v2()`**
4. **Plugin is ready to handle requests**

### Plugin Unload Sequence

1. **Engine stops sending new requests**
2. **Engine waits for in-flight requests to complete** (with timeout)
3. **Engine calls `plugin_shutdown_v2()`**
4. **Engine unloads plugin binary**

### Hot Reload Sequence

1. **Engine calls `plugin_prepare_hot_reload_v2()`** on old version
2. **Engine unloads old version**
3. **Engine loads new version binary**
4. **Engine calls `plugin_init_from_state_v2()`** with saved state
5. **Engine switches requests to new version**

## Memory Management

### Ownership Rules

- **Strings from Engine**: Plugin must not free
- **Strings from Plugin**: Engine will free using the plugin's allocator
- **Structures**: Pass by pointer (ownership determined by convention)
- **Buffers**: Plugin is responsible for allocated buffers until returned to engine

### Allocation

Plugins should use standard C `malloc`/`free`:

```rust
extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
}
```

The engine will use the same allocator.

## Security Considerations

### Input Validation

All plugins MUST validate:

- Pointer validity before dereference
- String null-termination
- Buffer bounds
- Data type invariants

### Resource Limits

Plugins should respect:

- CPU usage limits (will be enforced by scheduler)
- Memory limits (will trigger OOM killer if exceeded)
- Network bandwidth limits
- Concurrent request limits

### Unsafe Code

FFI boundaries require `unsafe` blocks. Minimize unsafe code and document:

- Why the code is safe
- What invariants must hold
- Caller responsibilities

## Testing Your Plugin

### Minimal Test

```rust
#[test]
fn test_plugin_exports() {
    // Verify all required symbols are exported
    let lib = dlopen("target/release/my_plugin.so", RTLD_NOW).unwrap();
    
    let init: extern "C" fn(*const PluginContextV2) -> PluginResultV2 = 
        dlsym(&lib, "plugin_init_v2").unwrap().transmute();
    
    let info: extern "C" fn() -> *const PluginInfoV2 = 
        dlsym(&lib, "plugin_get_info_v2").unwrap().transmute();
    
    // ... more checks
}
```

### Integration Test

Use the provided test framework in the `core` crate to test your plugin with a live execution engine.

## See Also

- [Plugin Development Guide](./PLUGIN_DEVELOPMENT.md)
- [ABI Reference](./API_REFERENCE.md)
- [Security Model](./SECURITY.md)
- [Configuration Reference](./CONFIG_REFERENCE.md)
