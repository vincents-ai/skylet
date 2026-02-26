# Quick Start Guide

Get up and running with Skylet in just a few minutes!

## Prerequisites

- Rust 1.70 or later
- Git
- Basic familiarity with Rust

## Installation

### 1. Clone the Repository

```bash
git clone https://github.com/vincents-ai/skylet.git
cd skylet
```

### 2. Build the Engine

```bash
# Default build (standalone mode)
cargo build --release

# With optional distributed tracing
cargo build --release --features opentelemetry
```

## Your First Plugin (5 Minutes)

### Step 1: Create a New Plugin Project

```bash
# Create a new library
cargo new --lib my-first-plugin
cd my-first-plugin
```

### Step 2: Configure Cargo.toml

Update `Cargo.toml`:

```toml
[package]
name = "my-first-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
skylet-abi = { git = "https://github.com/vincents-ai/skylet-abi.git", branch = "main" }
```

### Step 3: Implement the Plugin

Replace `src/lib.rs` with:

```rust
use skylet_abi::v2_spec::*;

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    static INFO: PluginInfoV2 = PluginInfoV2 {
        name: b"my-first-plugin\0" as *const u8 as *const i8,
        version: b"0.1.0\0" as *const u8 as *const i8,
        author: b"Your Name\0" as *const u8 as *const i8,
    };
    &INFO
}

#[no_mangle]
pub extern "C" fn plugin_init_v2(_context: *const PluginContextV2) -> PluginResultV2 {
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
    response: *mut ResponseV2,
) -> PluginResultV2 {
    unsafe {
        let resp = &mut *response;
        resp.status_code = 200;
        resp.content_type = b"text/plain\0".as_ptr() as *const std::ffi::c_char;
        
        let message = b"Hello from my first plugin!";
        resp.body = message.as_ptr() as *mut u8;
        resp.body_len = message.len();
    }
    
    PluginResultV2::Success
}

static PLUGIN_API: PluginApiV2 = PluginApiV2 {
    get_info: plugin_get_info_v2,
    init: plugin_init_v2,
    shutdown: plugin_shutdown_v2,
    handle_request: plugin_handle_request_v2,
    handle_event: None,
    prepare_hot_reload: None,
    health_check: None,
    get_metrics: None,
    query_capability: None,
    get_config_schema: None,
    get_billing_metrics: None,
    serialize_state: None,
    deserialize_state: None,
    free_state: None,
};

#[no_mangle]
pub extern "C" fn plugin_create_v2() -> *const PluginApiV2 {
    &PLUGIN_API
}
```

### Step 4: Build Your Plugin

```bash
cargo build --release
```

Your plugin is now built:
- **Linux**: `target/release/libmy_first_plugin.so`
- **macOS**: `target/release/libmy_first_plugin.dylib`
- **Windows**: `target/release/my_first_plugin.dll`

### Step 5: Test Your Plugin

```bash
cd ../plugin-test-harness
cargo build --release

# Test your plugin
./target/release/plugin-test-harness test \
  --plugin-path ../my-first-plugin/target/release/libmy_first_plugin.so
```

## Exploring Examples

The repository includes several example plugins to learn from:

### Hello Plugin
Minimal example - perfect for understanding the basics:

```bash
cd examples/hello-plugin
cargo build --release
```

### Echo Plugin
Demonstrates request handling:

```bash
cd examples/echo-plugin
cargo build --release
```

### Counter Plugin
Advanced example with state management:

```bash
cd examples/counter-plugin
cargo build --release
cargo test --test integration
```

See [Example Plugins](EXAMPLES.md) for detailed documentation.

## Next Steps

### Learn More

- **[Plugin Development Guide](PLUGIN_DEVELOPMENT.md)** - Comprehensive tutorial
- **[API Reference](API_REFERENCE.md)** - Complete API documentation
- **[Configuration Reference](CONFIG_REFERENCE.md)** - Config system details
- **[Security Guide](SECURITY.md)** - Security best practices

### Testing

- **[Testing Framework](../plugin-test-harness/README.md)** - How to write tests
- **[Example Plugins](EXAMPLES.md)** - Integration test examples

### Development

- **[Contributing Guidelines](../CONTRIBUTING.md)** - How to contribute
- **[Architecture](ARCHITECTURE.md)** - System design overview

## Common Commands

```bash
# Build the engine
cargo build --release

# Build specific feature
cargo build --release --features opentelemetry

# Run all tests
cargo test --all

# Run integration tests
cargo test --test integration

# Build documentation
cargo doc --no-deps --open

# Check code without building
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy
```

## Getting Help

- **Documentation**: Check the [docs/](./) directory
- **Issues**: [GitHub Issues](https://github.com/vincents-ai/skylet/issues)
- **Discussions**: [GitHub Discussions](https://github.com/vincents-ai/skylet/discussions)
- **Examples**: See [examples/](../examples/) directory

## Troubleshooting

### Build Errors

**Issue**: "error: linking with `cc` failed"

**Solution**: Install build tools:
- Ubuntu: `sudo apt install build-essential`
- macOS: `xcode-select --install`
- Windows: Install [Build Tools for Visual Studio](https://visualstudio.microsoft.com/downloads/)

### Plugin Not Found

**Issue**: "Failed to load plugin"

**Solution**: Ensure the plugin path is correct and the plugin is built with `--release`.

### ABI Mismatch

**Issue**: "ABI version mismatch"

**Solution**: Ensure both the engine and plugin are using compatible versions. Check [ABI Stability](ABI_STABILITY.md).

## What's Next?

Now that you have a working plugin, consider:

1. Adding configuration support
2. Implementing health checks
3. Adding metrics collection
4. Writing comprehensive tests
5. Exploring advanced features

See the [Plugin Development Guide](PLUGIN_DEVELOPMENT.md) for advanced topics!
