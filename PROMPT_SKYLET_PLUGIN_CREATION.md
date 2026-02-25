# Skylet Plugin Creation Guide

## Creating a New Plugin

### 1. Create Plugin Directory

```bash
mkdir -p plugins/<plugin-name>/src
```

### 2. Create Cargo.toml

```toml
[package]
name = "<plugin-name>"
version = "0.1.0"
edition = "2021"
description = "Plugin description"
license = "MIT OR Apache-2.0"
authors = ["Skylet Team"]

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
skylet-abi = { git = "https://github.com/vincents-ai/skylet.git", tag = "v0.2.0" }
serde = "1.0"
serde_json = "1.0"
```

### 3. Create lib.rs

```rust
// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Plugin Description

use skylet_abi::v2_spec::{PluginContextV2, PluginInfoV2, PluginResultV2};

static mut PLUGIN_STATE: Option<String> = None;

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    if context.is_null() {
        return PluginResultV2::InvalidRequest;
    }
    // Plugin initialization logic
    PluginResultV2::Success
}

#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(_context: *const PluginContextV2) -> PluginResultV2 {
    PluginResultV2::Success
}

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    std::ptr::null()
}
```

### 4. Add to Workspace

Edit `Cargo.toml` in repo root:

```toml
[workspace]
members = [
    "abi",
    "src",
    "plugins/<plugin-name>",
    # ... other plugins
]
```

### 5. Build

```bash
cargo build -p <plugin-name>
```

## Open Source Plugins (In skylet repo)

| Plugin | Description |
|--------|-------------|
| `platform-detect` | Bare metal, virtualized, container, secure boot, TPM detection |
| `security-classifier` | Device trust classification (trusted/high/moderate/low/minimal) |
| `metrics` | Prometheus-style metrics |
| `system-monitor` | CPU, memory, disk, network stats |

## Proprietary Plugins (Separate repos)

| Plugin | Repo |
|--------|------|
| `file-integrity` | vincents-ai/skylet-file-integrity |
| `network-monitor` | vincents-ai/skylet-network-monitor |
| `audit-logger` | vincents-ai/skylet-audit-logger |

## ABI Dependency

All plugins depend on `skylet-abi` from:
```
git = "https://github.com/vincents-ai/skylet.git"
branch = "main"
```

The `abi` crate contains:
- `v2_spec` module - PluginInfoV2, PluginContextV2, PluginResultV2
- FFI types for plugin communication
- Service interfaces
