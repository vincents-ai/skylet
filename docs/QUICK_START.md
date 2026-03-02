# Skylet Quick Start Guide

Get up and running with Skylet in 5 minutes.

## Prerequisites

- Rust 1.70+ (recommended: 1.75+)
- Linux, macOS, or Windows

## Installation

```bash
# Clone the repository
git clone https://github.com/vincents-ai/skylet.git
cd skylet

# Build the engine
cargo build --release
```

## Your First Plugin

### 1. Create Plugin Project

```bash
cargo new --lib my-first-plugin
cd my-first-plugin
```

### 2. Configure Cargo.toml

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

### 3. Implement Plugin

Create `src/lib.rs`:

```rust
use skylet_plugin_common::skylet_plugin_v2;

skylet_plugin_v2! {
    name: "my-first-plugin",
    version: "0.1.0",
    description: "My first Skylet plugin",
    author: "Your Name",
    license: "MIT OR Apache-2.0",
    tagline: "Hello from Skylet!",
    category: skylet_abi::PluginCategory::Utility,
    max_concurrency: 10,
    supports_async: true,
    capabilities: ["my.read", "my.write"],
}
```

### 4. Build Plugin

```bash
cargo build --release
```

Output: `target/release/libmy_first_plugin.so` (Linux)

### 5. Run with Engine

```bash
# From skylet root directory
./target/release/skylet \
  --plugin ./my-first-plugin/target/release/libmy_first_plugin.so
```

## Common Commands

| Command | Description |
|---------|-------------|
| `cargo build --release` | Build optimized binary |
| `cargo test` | Run tests |
| `cargo doc --open` | View documentation |
| `cargo clippy` | Lint code |

## Next Steps

- [Plugin Development Guide](./PLUGIN_DEVELOPMENT.md) - Complete tutorial
- [Configuration Reference](./CONFIG_REFERENCE.md) - Configuration options
- [API Reference](./API_REFERENCE.md) - API documentation
- [Examples](../examples/) - Example plugins

## Getting Help

- [GitHub Issues](https://github.com/vincents-ai/skylet/issues)
- [GitHub Discussions](https://github.com/vincents-ai/skylet/discussions)
