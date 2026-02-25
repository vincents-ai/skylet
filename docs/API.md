# API Reference

This document provides a reference for Skylet's core API, including types, FFI functions, and service interfaces.

## Core Types

### PluginContextV2

The main context object passed to plugin functions:

```c
struct PluginContextV2 {
    // Services
    ServiceRegistry* service_registry;
    Logger* logger;
    ConfigManager* config_manager;
    SecretsProvider* secrets_provider;
    RPCClient* rpc_client;
    JobQueue* job_queue;
    PermissionsManager* permissions_manager;
    
    // Metadata
    const char* plugin_id;
    const char* plugin_name;
    const char* plugin_version;
    const char* engine_version;
};
```

### PluginInfoV2

Plugin metadata structure:

```c
struct PluginInfoV2 {
    const char* name;
    const char* version;
    const char* description;
    const char* author;
    const char* license;
    const char* homepage;
    const char* skylet_version_min;
    const char* skylet_version_max;
    const char* abi_version;
    
    // Dependencies
    const PluginDependency* dependencies;
    size_t num_dependencies;
    
    // Services
    const ServiceInfo* provides_services;
    size_t num_provides_services;
    const ServiceInfo* requires_services;
    size_t num_requires_services;
    
    // Capabilities
    const Capability* capabilities;
    size_t num_capabilities;
    
    // Tags
    const char* const* tags;
    size_t num_tags;
    
    // Configuration
    PluginCategory category;
    bool supports_hot_reload;
    bool supports_async;
    bool supports_streaming;
    uint32_t max_concurrency;
    
    // Monetization
    MonetizationModel monetization_model;
    double price_usd;
};
```

### PluginResultV2

Return codes for plugin functions:

```rust
pub enum PluginResultV2 {
    Success = 0,              // Operation succeeded
    Error = -1,               // Generic error
    InvalidRequest = -2,      // Malformed request
    ServiceUnavailable = -3,   // Service not available
    PermissionDenied = -4,    // Insufficient permissions
    NotImplemented = -5,      // Feature not implemented
    Timeout = -6,             // Operation timed out
    ResourceExhausted = -7,   // Out of resources
}
```

### HealthStatus

Plugin health reporting:

```rust
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}
```

## FFI Functions

### Required Entry Points

#### plugin_get_info_v2

```c
const PluginInfoV2* plugin_get_info_v2(void);
```

Returns static plugin metadata. Called frequently - must be fast.

#### plugin_init_v2

```c
PluginResultV2 plugin_init_v2(const PluginContextV2* context);
```

Initialize plugin with provided context. Return `Success` on init.

#### plugin_shutdown_v2

```c
PluginResultV2 plugin_shutdown_v2(const PluginContextV2* context);
```

Clean up resources. Called during unload.

#### plugin_handle_request_v2

```c
PluginResultV2 plugin_handle_request_v2(
    const PluginContextV2* context,
    const RequestV2* request,
    ResponseV2* response
);
```

Handle service requests. Return `NotImplemented` if not handling requests.

### Optional Entry Points

#### plugin_prepare_hot_reload_v2

```c
PluginResultV2 plugin_prepare_hot_reload_v2(const PluginContextV2* context);
```

Prepare state for migration. Serialize state to be transferred.

#### plugin_init_from_state_v2

```c
PluginResultV2 plugin_init_from_state_v2(
    const PluginContextV2* context,
    const uint8_t* state_data,
    size_t state_len
);
```

Restore state after hot reload.

#### plugin_health_check_v2

```c
HealthStatus plugin_health_check_v2(const PluginContextV2* context);
```

Return current health status.

#### plugin_get_config_schema_json

```c
const char* plugin_get_config_schema_json(void);
```

Return JSON Schema for plugin configuration.

## Service Interfaces

### Logger

```c
struct Logger {
    void (*log)(const PluginContextV2*, PluginLogLevel, const char* message);
    void (*log_with_fields)(const PluginContextV2*, PluginLogLevel, 
                            const char* message, const LogField* fields, size_t num_fields);
};

enum PluginLogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Fatal = 5,
};
```

**Usage:**

```rust
unsafe {
    let logger = &(*context).logger;
    let msg = CString::new("Hello").unwrap();
    (logger.log)(context, PluginLogLevel::Info, msg.as_ptr());
}
```

### ConfigManager

```c
struct ConfigManager {
    const char* (*get_config_value)(const PluginContextV2*, const char* key);
    int (*get_config_int)(const PluginContextV2*, const char* key, int default_value);
    bool (*get_config_bool)(const PluginContextV2*, const char* key, bool default_value);
    // ... more methods
};
```

### SecretsProvider

```c
struct SecretsProvider {
    const char* (*get_secret)(const PluginContextV2*, const char* key);
    void (*free_secret)(const char* secret);  // Use to free returned string
};
```

### ServiceRegistry

```c
struct ServiceRegistry {
    int (*register)(const PluginContextV2*, const char* service_name, 
                    ServiceHandler handler);
    ServiceHandler (*lookup)(const PluginContextV2*, const char* service_name);
    int (*unregister)(const PluginContextV2*, const char* service_name);
};
```

### JobQueue

```c
struct JobQueue {
    int (*enqueue)(const PluginContextV2*, const Job* job);
    int (*schedule)(const PluginContextV2*, const Job* job, 
                    uint64_t delay_ms);
    int (*cancel)(const PluginContextV2*, const char* job_id);
};
```

### RPCClient

```c
struct RPCClient {
    int (*call)(const PluginContextV2*, const char* service,
                const uint8_t* request, size_t request_len,
                uint8_t* response, size_t* response_len,
                uint32_t timeout_ms);
};
```

## Request/Response Types

### RequestV2

```c
struct RequestV2 {
    const char* method;
    const uint8_t* body;
    size_t body_len;
    const char* const* headers;
    size_t num_headers;
    void* user_data;
};
```

### ResponseV2

```c
struct ResponseV2 {
    uint16_t status_code;
    const uint8_t* body;
    size_t body_len;
    const char* const* headers;
    size_t num_headers;
    
    // For streaming
    bool is_streaming;
    void (*write_chunk)(ResponseV2*, const uint8_t*, size_t);
};
```

## Configuration Field Types

```rust
pub enum ConfigFieldType {
    String,
    Integer,
    Float,
    Boolean,
    Array(Box<ConfigFieldType>),
    Object,
    Secret,           // Masked input
    Enum { variants: Vec<String> },
    Path { must_exist: bool, is_dir: bool },
    Url { schemes: Vec<String> },
    Duration,
    Port,
    Email,
    Host,
}
```

## Log Level Values

```rust
pub enum LogLevel {
    Trace,   // Very detailed debug
    Debug,   // Debug information
    Info,    // General info
    Warn,    // Warning messages
    Error,   // Error messages
    Fatal,   // Fatal errors
}
```

## Plugin Categories

```rust
pub enum PluginCategory {
    Utility,
    Security,
    Database,
    Network,
    Storage,
    Monitoring,
    AI,
    Integration,
    Developer,
}
```

## Error Codes Reference

| Code | Value | Description |
|------|-------|-------------|
| Success | 0 | Operation completed |
| Error | -1 | Generic failure |
| InvalidRequest | -2 | Malformed input |
| ServiceUnavailable | -3 | Service down |
| PermissionDenied | -4 | Auth failed |
| NotImplemented | -5 | Not supported |
| Timeout | -6 | Took too long |
| ResourceExhausted | -7 | Out of memory |

## Example: Using the API

```rust
use skylet_abi::v2_spec::*;
use std::ffi::{CStr, CString};

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }
    
    unsafe {
        let ctx = &*context;
        
        // Log initialization
        let logger = &*ctx.logger;
        let msg = CString::new("Plugin initialized").unwrap();
        (logger.log)(context, PluginLogLevel::Info, msg.as_ptr());
        
        // Get configuration
        let config = &*ctx.config_manager;
        let api_key = CStr::from_ptr(config.get_config_value(
            context, 
            CString::new("api_key").unwrap().as_ptr()
        ));
    }
    
    PluginResultV2::Success
}

static PLUGIN_INFO: PluginInfoV2 = PluginInfoV2 {
    name: b"my-plugin\0" as *const u8 as *const i8,
    version: b"1.0.0\0" as *const u8 as *const i8,
    // ... fill in other fields
};

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    &PLUGIN_INFO
}
```

## See Also

- [Plugin Development Guide](PLUGIN_DEVELOPMENT.md)
- [Plugin Contract](PLUGIN_CONTRACT.md)
- [Configuration Reference](CONFIG_REFERENCE.md)
- [Security Best Practices](SECURITY.md)
