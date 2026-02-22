# API Reference

This is a guide to generating and understanding the Skylet Execution Engine API reference documentation.

## Generating Rust Documentation

### Build Full Documentation

```bash
# Generate with all dependencies visible
cargo doc --all --no-deps --open

# Generate with dependency documentation
cargo doc --all --open
```

This creates HTML documentation in `target/doc/` that you can browse locally.

### Generate Specific Crate Documentation

```bash
# Just the ABI crate
cargo doc -p skylet-abi --no-deps --open

# Just the core
cargo doc -p execution-engine-core --no-deps --open

# Specific plugin
cargo doc -p config-manager --no-deps --open
```

## Key Modules

### Core ABI (`skylet-abi`)

The primary module for plugin development. Key components:

#### `v2_spec` - Plugin ABI v2.0 Specification
- FFI entry point definitions
- Plugin metadata structures
- Result codes and error handling

**Main Types:**
- `PluginContextV2` - Execution context passed to plugins
- `PluginInfoV2` - Plugin metadata
- `PluginResultV2` - Result codes (Success, Error, InvalidRequest, etc.)
- `ServiceRegistry` - Access to available services

**Entry Points:**
- `plugin_init_v2()` - Initialize plugin
- `plugin_shutdown_v2()` - Shutdown plugin
- `plugin_get_info_v2()` - Get plugin metadata
- `plugin_process_request()` - Process requests
- `plugin_on_config_change()` - Handle config changes
- `plugin_get_metrics()` - Return metrics

#### `config` - Configuration System
- Schema definition and validation
- Configuration field types
- Secret reference resolution
- UI generation from schemas

**Main Types:**
- `ConfigSchema` - Plugin configuration schema
- `ConfigField` - Individual configuration field
- `ConfigFieldType` - Field type enumeration
- `ConfigValidator` - Configuration validator
- `ConfigManager` - Central config management
- `SecretResolver` - Secret reference resolution

**Common Usage:**
```rust
use skylet_abi::config::{ConfigSchema, ConfigField, ConfigFieldType};

let mut schema = ConfigSchema::new("my-plugin");
schema.add_field(ConfigField {
    name: "api_key".to_string(),
    field_type: ConfigFieldType::Secret,
    required: true,
    // ... other fields
});
```

#### `logging` - Structured Logging
- JSON schema for log events
- Correlation ID tracking
- Log level management
- Async logging support

**Main Types:**
- `LogEvent` - Individual log event
- `LogLevel` - Log severity level
- `Logger` - Logging service trait

**Common Usage:**
```rust
use skylet_abi::logging::{LogEvent, LogLevel};

let event = LogEvent {
    timestamp: SystemTime::now(),
    level: LogLevel::Info,
    message: "Processing request".to_string(),
    // ... other fields
};
```

#### `security_rfc` - Security Policies
- Capability definitions
- Permission policies
- Filesystem access controls
- Network policies

**Main Types:**
- `Capability` - Plugin capability
- `PermissionPolicy` - Permission rules
- `SecurityPolicy` - Overall security config

#### `key_management` - Cryptographic Operations
- KeyManagement trait abstraction
- Instance management traits
- Key generation and signing

**Main Types:**
- `KeyManagement` - Key operations trait
- `InstanceManager` - Instance management trait
- `DefaultKeyManagement` - Standard implementation

### Core Engine (`execution-engine`)

Main engine implementation with HTTP server and plugin management.

**Key Modules:**
- `plugin_loader` - Dynamic plugin loading
- `service_registry` - Service discovery
- `config_manager` - Configuration management
- `http_server` - HTTP API server
- `permissions` - Permission enforcement

### Supporting Crates

#### `http-router`
HTTP request routing and middleware support.

**Key Types:**
- `Router` - HTTP request router
- `Route` - Individual route definition
- `Middleware` - Request/response middleware

#### `job-queue`
Background job processing and scheduling.

**Key Types:**
- `JobQueue` - Job queue manager
- `Job` - Individual job definition
- `Schedule` - Job scheduling

#### `permissions`
Fine-grained permission system.

**Key Types:**
- `Permission` - Individual permission
- `Role` - Role definition
- `PolicyEngine` - Permission enforcement

## Common Patterns

### Accessing Services in Plugins

```rust
use skylet_abi::v2_spec::{PluginContextV2, PluginResultV2};

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    unsafe {
        let ctx = (*context);
        
        // Get logger service
        if let Some(logger) = ctx.service_registry.get_service("logger") {
            logger.log("Plugin initialized");
        }
        
        // Get config service
        if let Some(config) = ctx.service_registry.get_service("config") {
            let my_config = config.get("my-plugin")?;
        }
    }
    PluginResultV2::Success
}
```

### Configuration Usage Pattern

```rust
use skylet_abi::config::{ConfigManager, ConfigSchema, ConfigField, ConfigFieldType};

fn setup_config() {
    let manager = ConfigManager::new();
    
    // Create schema
    let mut schema = ConfigSchema::new("my-plugin");
    schema.add_field(ConfigField {
        name: "timeout".to_string(),
        field_type: ConfigFieldType::Integer,
        required: false,
        default: Some(json!(30)),
        // ... other fields
    });
    
    // Register and load
    manager.register_schema("my-plugin", schema);
    manager.load_config("my-plugin", Path::new("config.toml"))?;
    
    // Access values
    if let Some(timeout) = manager.get_value("my-plugin", "timeout") {
        println!("Timeout: {}", timeout);
    }
}
```

### Error Handling Pattern

```rust
use std::ffi::CStr;
use skylet_abi::v2_spec::{PluginContextV2, PluginResultV2};

#[no_mangle]
pub extern "C" fn plugin_process_request(
    context: *const PluginContextV2,
    request: *const c_char,
    request_len: usize,
) -> PluginResultV2 {
    // Validate inputs
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }
    if request.is_null() || request_len == 0 {
        return PluginResultV2::InvalidRequest;
    }
    if request_len > MAX_REQUEST_SIZE {
        return PluginResultV2::InvalidRequest;
    }
    
    unsafe {
        // Safe to process now
        let request_str = CStr::from_ptr(request).to_string_lossy();
        // Process...
    }
    
    PluginResultV2::Success
}
```

## Type Reference

### PluginResultV2 Codes

```rust
pub enum PluginResultV2 {
    Success = 0,           // Operation successful
    Error = 1,             // Generic error
    InvalidRequest = 2,    // Invalid input
    Unauthorized = 3,      // Permission denied
    NotFound = 4,          // Resource not found
    Conflict = 5,          // Resource conflict
    ServiceUnavailable = 6, // Service not available
}
```

### ConfigFieldType Values

```rust
pub enum ConfigFieldType {
    String,
    Integer,
    Float,
    Boolean,
    Array(Box<ConfigFieldType>),
    Object,
    Secret,
    Enum { variants: Vec<String> },
    Path { must_exist: bool, is_dir: bool },
    Url { schemes: Vec<String> },
    Duration,
    Port,
    Email,
    Host,
}
```

### LogLevel Values

```rust
pub enum LogLevel {
    Trace,   // Very detailed debug info
    Debug,   // Debug information
    Info,    // General information
    Warn,    // Warning messages
    Error,   // Error messages
    Fatal,   // Fatal errors
}
```

## Trait Reference

### KeyManagement Trait

```rust
pub trait KeyManagement {
    fn generate_key(&self) -> Result<KeyInfo>;
    fn sign(&self, key_id: &str, data: &[u8]) -> Result<Vec<u8>>;
    fn verify(&self, key_id: &str, data: &[u8], signature: &[u8]) -> Result<()>;
    fn rotate_key(&self, key_id: &str) -> Result<()>;
    fn get_public_key(&self, key_id: &str) -> Result<Vec<u8>>;
}
```

### InstanceManager Trait

```rust
pub trait InstanceManager {
    fn get_instance_id(&self) -> String;
    fn get_instance_metadata(&self) -> Result<InstanceMetadata>;
    fn set_instance_metadata(&self, metadata: InstanceMetadata) -> Result<()>;
    fn get_role(&self) -> Result<InstanceRole>;
    fn discover_peers(&self) -> Result<Vec<PeerInfo>>;
}
```

## Building Custom Documentation

### Adding Doc Comments

```rust
/// Brief description of the function.
///
/// Longer description providing more details.
///
/// # Arguments
/// * `param1` - Description of param1
/// * `param2` - Description of param2
///
/// # Returns
/// Description of return value
///
/// # Errors
/// Describes what errors might be returned
///
/// # Examples
/// ```
/// let result = my_function(value);
/// assert!(result.is_ok());
/// ```
pub fn my_function(param1: &str, param2: usize) -> Result<String> {
    // Implementation
}
```

### Hiding Internal Items

```rust
/// Public API
pub fn public_function() {}

/// Internal helper (hidden from docs)
#[doc(hidden)]
pub fn internal_helper() {}

/// Deprecated - use new_function instead
#[deprecated(since = "2.1.0", note = "use new_function instead")]
pub fn old_function() {}
```

## Online Documentation

For a quick reference without building locally:

- **Docs.rs**: https://docs.rs/skylet-abi/ (when published)
- **GitHub**: See `docs/` directory for guides
- **Examples**: See `examples/` directory for code samples

## Search Tips

When using `cargo doc`:

1. **Local Search**: Use browser's Ctrl+F or Cmd+F
2. **Module Hierarchy**: Click module names to navigate
3. **Trait Implementations**: Look for "Implementations" section
4. **Examples**: Check "Examples" section below description

## Troubleshooting Documentation

### Documentation Not Showing

```bash
# Ensure doc comments are correct
cargo doc --document-private-items

# Check for compilation errors in doc tests
cargo test --doc
```

### Doc Tests Failing

```bash
# Run doc tests only
cargo test --doc

# Fix by adding `no_run` or `should_panic` attributes
/// ```no_run
/// my_function();  // Compile but don't run
/// ```

/// ```should_panic
/// my_function();  // Expected to panic
/// ```
```

## Contributing Documentation

When contributing:

1. Add doc comments to all public items
2. Include examples in doc comments
3. Run `cargo doc` to verify it builds
4. Test doc examples: `cargo test --doc`
5. Check links and references are correct

## References

- [Rust Book - Documentation](https://doc.rust-lang.org/book/ch14-04-installing-binaries.html)
- [Rustdoc Guide](https://doc.rust-lang.org/rustdoc/)
- [API Guidelines](https://rust-lang.github.io/api-guidelines/)

## See Also

- [Plugin Development Guide](../docs/PLUGIN_DEVELOPMENT.md)
- [Plugin Contract](../docs/PLUGIN_CONTRACT.md)
- [Configuration Reference](../docs/CONFIG_REFERENCE.md)
