# Architecture Guide

This document describes the high-level architecture of Skylet, the plugin system, ABI v2, plugin lifecycle, and security model.

## System Overview

Skylet is a plugin runtime that loads dynamic libraries (`.so`/`.dylib`/`.dll`) and communicates with them through a C FFI boundary. The architecture is designed for:

- **Security**: Strict FFI boundaries prevent malicious plugins from compromising the host
- **Stability**: ABI v2.0.0 is frozen with guaranteed backward compatibility
- **Performance**: Minimal overhead through efficient FFI and async/await patterns
- **Extensibility**: Service registry enables inter-plugin communication

```
┌─────────────────────────────────────────────────────────────┐
│                     Skylet Engine                           │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │  Plugin     │  │  Service    │  │  Configuration     │ │
│  │  Manager    │  │  Registry   │  │  Manager            │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │  Job Queue  │  │  Secrets    │  │  Permissions        │ │
│  │             │  │  Manager    │  │  Manager            │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                    FFI Boundary (ABI v2)                    │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │  Plugin A   │  │  Plugin B   │  │  Plugin C           │ │
│  │  (.so)      │  │  (.so)      │  │  (.so)              │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Plugin System Overview

### What is a Plugin?

A plugin is a compiled dynamic library that extends Skylet's functionality. Plugins:

- Are loaded at runtime from `.so` (Linux), `.dylib` (macOS), or `.dll` (Windows)
- Communicate with the engine via C FFI functions
- Can provide services to other plugins
- Can request capabilities for restricted operations

### Plugin Types

| Type | Description | Examples |
|------|-------------|----------|
| **Service** | Provides functionality to other plugins | Database, HTTP client |
| **Extension** | Adds features to the engine | New commands, UI |
| **Integration** | Connects to external systems | Kubernetes, GitHub |

### Plugin Structure

```
my-plugin/
├── Cargo.toml          # Build configuration
├── src/
│   └── lib.rs         # Plugin entry points (FFI functions)
└── target/release/
    └── libmy_plugin.so  # Compiled plugin
```

## ABI v2 Explanation

### Overview

ABI (Application Binary Interface) v2.0.0 defines the contract between plugins and the engine. Key characteristics:

- **C FFI**: Uses C calling convention for broad language support
- **Versioned**: Explicit version in function names (`_v2`)
- **Stable**: No breaking changes until v3.0.0
- **Minimal**: Only essential functions and types

### Core Types

```c
// Plugin context - passed to most functions
struct PluginContextV2 {
    ServiceRegistry* service_registry;    // Access to services
    Logger* logger;                       // Logging
    ConfigManager* config_manager;         // Configuration
    SecretsProvider* secrets_provider;     // Secrets
    RPCClient* rpc_client;                // Inter-plugin calls
    JobQueue* job_queue;                  // Background jobs
    PermissionsManager* permissions_manager;
    const char* plugin_id;
    const char* plugin_version;
};

// Plugin metadata
struct PluginInfoV2 {
    const char* name;
    const char* version;
    const char* description;
    const char* author;
    // ... more fields
};

// Result code
enum PluginResultV2 {
    Success = 0,
    Error = -1,
    InvalidRequest = -2,
    // ... more codes
};
```

### Required Entry Points

Every plugin must export these functions:

| Function | Purpose |
|----------|---------|
| `plugin_get_info_v2()` | Return plugin metadata |
| `plugin_init_v2()` | Initialize plugin |
| `plugin_shutdown_v2()` | Clean up on unload |
| `plugin_handle_request_v2()` | Handle requests |

### Optional Entry Points

| Function | Purpose |
|----------|---------|
| `plugin_prepare_hot_reload_v2()` | Prepare for reload |
| `plugin_init_from_state_v2()` | Restore state after reload |
| `plugin_get_config_schema_json()` | Define config schema |
| `plugin_health_check_v2()` | Health status |

## Plugin Lifecycle

### Load Sequence

```
1. Engine discovers plugin (config or directory scan)
2. Engine loads dynamic library (dlopen/LoadLibrary)
3. Engine calls plugin_get_info_v2() - caches metadata
4. Engine calls plugin_init_v2() - passes context
5. Plugin registers services, starts tasks
6. Plugin ready for requests
```

### Unload Sequence

```
1. Engine stops sending new requests
2. Engine waits for in-flight requests (with timeout)
3. Engine calls plugin_shutdown_v2()
4. Plugin cleans up resources
5. Engine unloads library (dlclose/FreeLibrary)
```

### Hot Reload Sequence

```
1. Engine calls plugin_prepare_hot_reload_v2()
2. Plugin serializes state to bytes
3. Engine unloads old plugin
4. Engine loads new plugin
5. Engine calls plugin_init_from_state_v2() with serialized state
6. New plugin continues with previous state
```

### State Management

Plugins can maintain state across hot reloads:

```rust
// Serialize state for hot reload
fn serialize_state() -> Vec<u8> {
    let state = get_plugin_state();
    serde_json::to_vec(&state).unwrap()
}

// Deserialize state after reload
fn deserialize_state(data: &[u8]) {
    let state: PluginState = serde_json::from_slice(data).unwrap();
    set_plugin_state(state);
}
```

## Security Model

### FFI Boundary

The FFI boundary is the primary security boundary. All data crossing this boundary must be validated:

```rust
// Always validate pointers
fn process_request(context: *const PluginContextV2, 
                   request: *const c_char) -> PluginResultV2 {
    // Check for null
    if context.is_null() || request.is_null() {
        return PluginResultV2::InvalidRequest;
    }
    
    // Validate string is null-terminated
    if !is_valid_c_string(request) {
        return PluginResultV2::InvalidRequest;
    }
    
    // Now safe to use
    unsafe { /* process request */ }
}
```

### Capability System

Plugins declare required capabilities at build time:

```rust
skylet_plugin_v2! {
    name: "my-plugin",
    // Request specific capabilities
    capabilities: ["http.read", "secrets.get", "filesystem.read"],
}
```

Available capabilities:
- `http.read` / `http.write` - HTTP requests
- `secrets.get` / `secrets.set` - Secret management
- `filesystem.read` / `filesystem.write` - File access
- `database.query` - Database operations

### Memory Safety

Plugins must follow memory safety rules:

| Rule | Description |
|------|-------------|
| No use-after-free | Validate lifetime of all pointers |
| Bounds checking | Never access beyond buffer limits |
| Null checks | Always check pointers before dereference |
| Initialization | Initialize all memory before use |

### Secrets Handling

Sensitive data must be handled securely:

```rust
use zeroize::Zeroizing;

fn process_secret(secret: String) {
    let secret = Zeroizing::new(secret);
    // Use secret
    // Automatically zeroed when dropped
}
```

### Resource Limits

The engine enforces resource limits:

| Limit | Default | Description |
|-------|---------|-------------|
| Memory | 100MB | Max memory per plugin |
| CPU | 10% | CPU time limit |
| Requests/s | 100 | Rate limiting |
| Concurrent | 10 | Parallel requests |

## Service Registry

The service registry enables inter-plugin communication:

```rust
// Plugin registers a service
fn register_service(ctx: *const PluginContextV2) {
    unsafe {
        let registry = (*ctx).service_registry;
        registry.register("my-service", service_handler);
    }
}

// Another plugin calls the service
fn call_service(ctx: *const PluginContextV2) {
    unsafe {
        let registry = (*ctx).service_registry;
        if let Some(service) = registry.get_service("my-service") {
            service.call(request);
        }
    }
}
```

### Built-in Services

| Service | Description |
|---------|-------------|
| `logger` | Structured logging |
| `config` | Configuration management |
| `secrets` | Secret storage |
| `http` | HTTP client |
| `job-queue` | Background jobs |

## Configuration System

Plugins define configuration schemas:

```rust
let mut schema = ConfigSchema::new("my-plugin");
schema.add_field(ConfigField {
    name: "api_endpoint".to_string(),
    field_type: ConfigFieldType::Url {
        schemes: vec!["https".to_string()],
    },
    required: true,
    ..Default::default()
});
```

Configuration is validated at load time and can be changed at runtime.

## Performance Considerations

### FFI Overhead

Each FFI call has overhead (~200-500ns). Minimize crossings by:

- Batching operations
- Using bulk data transfer (pointers, not copies)
- Caching service lookups

### Async/Await

Plugins can use async for non-blocking operations:

```rust
async fn handle_request(request: Request) -> Response {
    let result = http_client.get(&request.url).await?;
    process_response(result).await
}
```

### Memory Usage

Typical plugin memory usage: 5-20MB. Optimize by:

- Using static allocation for metadata
- Releasing resources promptly
- Limiting concurrent allocations

## See Also

- [Plugin Development Guide](PLUGIN_DEVELOPMENT.md)
- [Plugin Contract](PLUGIN_CONTRACT.md)
- [ABI Stability](ABI_STABILITY.md)
- [Security Best Practices](SECURITY.md)
