# Example Plugins

This directory contains example plugins demonstrating the Skylet Execution Engine plugin system.

## Getting Started

See the main [Plugin Development Guide](../docs/PLUGIN_DEVELOPMENT.md) for comprehensive tutorials.

## Examples

### 1. Hello World Plugin (Simple)
A minimal plugin that demonstrates basic functionality.

**Features:**
- Plugin initialization and shutdown
- Basic logging
- Plugin metadata

**Build:**
```bash
cd hello-world
cargo build --release
ls target/release/libhello_world.*
```

**Use:**
```toml
# config.toml
[hello-world]
enabled = true
```

### 2. Echo Plugin (Requests)
A plugin that echoes back requests with processing.

**Features:**
- Handle plugin_process_request
- Request validation
- Response formatting
- Error handling

**Build:**
```bash
cd echo-plugin
cargo build --release
```

**Use:**
```bash
# Send request to plugin
curl -X POST http://localhost:8080/echo \
  -d '{"message":"Hello"}'
```

### 3. Logger Plugin (Configuration)
A plugin demonstrating configuration system usage.

**Features:**
- Configuration schema definition
- Configuration validation
- Settings management
- Dynamic configuration handling

**Build:**
```bash
cd logger-plugin
cargo build --release
```

**Configure:**
```toml
# config.toml
[logger-plugin]
log_level = "debug"
output_format = "json"
max_size = 1000000
```

## Running Examples

### 1. Build a Plugin

```bash
cd examples/hello-world
cargo build --release

# Output: target/release/libhello_world.so (Linux)
#         target/release/libhello_world.dylib (macOS)
#         target/release/hello_world.dll (Windows)
```

### 2. Create Configuration

```toml
# config.toml
[hello-world]
enabled = true
```

### 3. Load in Engine

```bash
# From project root
cargo run --release -- \
  --plugin ./examples/hello-world/target/release/libhello_world.so \
  --config config.toml
```

## Example Plugin Structure

All example plugins follow this structure:

```
example-plugin/
├── Cargo.toml           # Dependencies and metadata
├── src/
│   └── lib.rs          # Plugin implementation
├── config.toml         # Example configuration
└── README.md           # Plugin-specific documentation
```

### Minimal Cargo.toml

```toml
[package]
name = "example-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]  # Important: Dynamic library

[dependencies]
skylet-abi = { path = "../../abi" }
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Minimal Plugin Code

```rust
use skylet_abi::{
    plugin_init_v2, plugin_shutdown_v2, plugin_get_info_v2,
    PluginResult, PluginContextV2, PluginInfoV2,
};
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

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    static INFO: PluginInfoV2 = PluginInfoV2 {
        name: b"example\0" as *const u8 as *const i8,
        version: b"0.1.0\0" as *const u8 as *const i8,
        author: b"Your Name\0" as *const u8 as *const i8,
    };
    &INFO
}
```

## Common Patterns

### Logging

```rust
unsafe {
    let ctx = (*context);
    if let Some(logger) = ctx.service_registry.get_service("logger") {
        logger.log("My message");
    }
}
```

### Configuration

```rust
#[no_mangle]
pub extern "C" fn plugin_get_config_schema() -> *const c_char {
    let schema = json!({
        "fields": [
            {
                "name": "api_key",
                "type": "secret",
                "required": true,
            }
        ]
    });
    // Return TOML or JSON representation
}
```

### Error Handling

```rust
#[no_mangle]
pub extern "C" fn plugin_process_request(
    context: *const PluginContextV2,
    request: *const c_char,
    request_len: usize,
) -> PluginResult {
    if request.is_null() {
        return PluginResult::InvalidRequest;
    }
    
    if request_len == 0 || request_len > 1024 * 1024 {
        return PluginResult::InvalidRequest;
    }
    
    PluginResult::Success
}
```

## Testing Examples

Each example includes tests:

```bash
cd examples/hello-world
cargo test
```

Example test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_info() {
        let info = unsafe { *plugin_get_info_v2() };
        assert!(!info.name.is_null());
    }
}
```

## Troubleshooting

### Build Fails
```bash
# Update dependencies
cargo update

# Clean and rebuild
cargo clean
cargo build --release
```

### Plugin Won't Load
```bash
# Check binary format
file target/release/libexample.so

# Check dependencies
ldd target/release/libexample.so  # Linux
otool -L target/release/libexample.dylib  # macOS
```

### Configuration Errors
```bash
# Validate TOML
cargo install toml-cli
toml config.toml

# Check schema
cat config.toml | jq .
```

## Creating Your Own Example

1. Copy an existing example:
   ```bash
   cp -r examples/hello-world examples/my-plugin
   ```

2. Update Cargo.toml:
   ```toml
   name = "my-plugin"
   ```

3. Implement your plugin in src/lib.rs

4. Build and test:
   ```bash
   cargo build --release
   cargo test
   ```

## Resources

- [Plugin Development Guide](../docs/PLUGIN_DEVELOPMENT.md) - Complete tutorial
- [Configuration Reference](../docs/CONFIG_REFERENCE.md) - Configuration system
- [Security Best Practices](../docs/SECURITY.md) - Security guidelines
- [Plugin Contract](../docs/PLUGIN_CONTRACT.md) - FFI specification
- [ABI Stability](../docs/ABI_STABILITY.md) - Version guarantees

## Contributing Examples

Want to contribute an example plugin?

1. Create a new directory in `examples/`
2. Follow the structure above
3. Add comprehensive README
4. Include tests
5. Update this file with description
6. Submit as pull request

See [CONTRIBUTING.md](../CONTRIBUTING.md) for guidelines.

## License

All example plugins are licensed under the same license as the main project:
Apache License 2.0

See [LICENSE](../LICENSE) for details.
