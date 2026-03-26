# Agent Instructions for Skylet

## Build Commands

This project uses Nix for the development environment. Use `nix develop --command` to run cargo commands:

```bash
# Build the main binary
nix develop --command cargo build --release --bin skylet

# Build the library
nix develop --command cargo build --release --lib

# Build all targets
nix develop --command cargo build --release
```

## Test Commands

```bash
# Run all tests
nix develop --command cargo test --release

# Run tests for a specific module
nix develop --command cargo test --release --lib module_name

# Run a specific test
nix develop --command cargo test --release test_name
```

## Plugin Deployment

Plugins must be deployed to BOTH directories:

```bash
# Plugin directories
/home/shift/code/skylet/skylet-dist/plugins/
/home/shift/code/skylet/dist/skylet-instance/plugins/
```

## Running the Server

The server requires OpenSSL library path to be set:

```bash
cd /home/shift/code/skylet/skylet-dist && \
LD_LIBRARY_PATH="/nix/store/qkfcr92mk15h8hmwzds3g5gkx0vm5l26-openssl-3.0.13/lib:$LD_LIBRARY_PATH" \
./bin/skylet server
```

## Key Files

- `src/main.rs` - Main server entry point
- `src/plugin_manager/manager.rs` - Plugin loading and management
- `src/plugin_manager/version_reload.rs` - Version-based hot reload (HR-009)
- `abi/src/v2_spec.rs` - ABI v2 specification with PluginInfoV2
- `abi/src/abi_loader.rs` - AbiV2PluginLoader for loading plugins

## Notes

- User uses "rusqlite" not "sqlite" for SQLite integration
- LLM provider config is at `~/.config/llm-provider/providers.toml` (contains API keys, do not read)
- When user asks about "OpenClaw", they mean the agentic AI assistant (openclaw.ai), NOT the Captain Claw game engine
