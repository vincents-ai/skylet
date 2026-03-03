# Troubleshooting Guide

Common issues and solutions for Skylet.

## Table of Contents

1. [Build Issues](#build-issues)
2. [Plugin Loading Issues](#plugin-loading-issues)
3. [Runtime Issues](#runtime-issues)
4. [Configuration Issues](#configuration-issues)
5. [Performance Issues](#performance-issues)
6. [Hot Reload Issues](#hot-reload-issues)
7. [Security Issues](#security-issues)
8. [Debug Tools](#debug-tools)

## Build Issues

### Compilation Errors

#### Missing Dependencies

**Symptom:**
```
error: could not find `skylet-abi` in registry
```

**Solution:**
```bash
# Ensure you're in the skylet directory
cd skylet

# Build dependencies
cargo build
```

#### Version Mismatch

**Symptom:**
```
error: package `skylet-abi v2.0.0` cannot be built
```

**Solution:**
```bash
# Update Rust
rustup update

# Clean and rebuild
cargo clean
cargo build
```

#### Linker Errors

**Symptom:**
```
error: linking with `cc` failed
```

**Solution:**
```bash
# Install build essentials (Linux)
sudo apt install build-essential

# Install Xcode tools (macOS)
xcode-select --install
```

### Plugin Build Issues

#### Crate Type Error

**Symptom:**
```
error: `cdylib` crate type requires `pub` functions
```

**Solution:**
Ensure `Cargo.toml` has:
```toml
[lib]
crate-type = ["cdylib"]
```

#### Missing Exports

**Symptom:**
```
error: plugin does not export required symbols
```

**Solution:**
Ensure all FFI functions have `#[no_mangle]` and `extern "C"`:
```rust
#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    // ...
}
```

## Plugin Loading Issues

### Plugin Not Found

**Symptom:**
```
Error: Plugin not found at path: ./libmy_plugin.so
```

**Solutions:**
```bash
# Check file exists
ls -la ./libmy_plugin.so

# Check path is correct
realpath ./libmy_plugin.so
```

### Architecture Mismatch

**Symptom:**
```
Error: Cannot load plugin: wrong ELF class
```

**Solution:**
```bash
# Check plugin architecture
file target/release/libmy_plugin.so

# Check host architecture
uname -m

# Rebuild for correct target
cargo build --release --target x86_64-unknown-linux-gnu
```

### Missing Dependencies

**Symptom:**
```
Error: libskylet_abi.so: cannot open shared object file
```

**Solution:**
```bash
# Check dependencies (Linux)
ldd target/release/libmy_plugin.so

# Check dependencies (macOS)
otool -L target/release/libmy_plugin.dylib

# Add library path
export LD_LIBRARY_PATH=/path/to/libs:$LD_LIBRARY_PATH
```

### Symbol Resolution Failed

**Symptom:**
```
Error: Cannot resolve symbol: plugin_init_v2
```

**Solution:**
```bash
# Verify symbols are exported
nm -D target/release/libmy_plugin.so | grep plugin

# Check for name mangling
nm target/release/libmy_plugin.so | grep plugin_init
```

## Runtime Issues

### Plugin Crashes

**Symptom:**
```
thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value'
```

**Solutions:**

1. Enable debug logging:
```bash
RUST_LOG=debug ./target/release/skylet
```

2. Check for null pointers:
```rust
if context.is_null() {
    return PluginResultV2::InvalidRequest;
}
```

3. Run with sanitizers:
```bash
RUSTFLAGS="-Zsanitizer=address" cargo run
```

### Memory Leaks

**Symptom:**
Memory usage grows over time.

**Solutions:**

1. Profile memory:
```bash
valgrind --leak-check=full ./target/release/skylet
```

2. Check for resource cleanup:
```rust
impl Drop for MyPlugin {
    fn drop(&mut self) {
        // Cleanup resources
    }
}
```

### Deadlocks

**Symptom:**
Plugin hangs indefinitely.

**Solutions:**

1. Enable deadlock detection:
```bash
RUST_LOG=tokio=trace ./target/release/skylet
```

2. Check lock ordering:
```rust
// Avoid: Lock A then B in one place, B then A in another
// Always lock in consistent order
```

## Configuration Issues

### Config File Not Found

**Symptom:**
```
Error: Configuration file not found: config.toml
```

**Solution:**
```bash
# Check standard locations
ls -la ~/.config/skylet/plugins/
ls -la /etc/skylet/plugins/
ls -la data/

# Create config file
mkdir -p ~/.config/skylet/plugins
touch ~/.config/skylet/plugins/my-plugin.toml
```

### Invalid TOML Syntax

**Symptom:**
```
Error: Failed to parse TOML: expected equals
```

**Solution:**
```bash
# Validate TOML syntax
cargo install toml-cli
toml config.toml

# Check for common issues:
# - Missing quotes around strings
# - Unclosed brackets
# - Incorrect indentation
```

### Environment Variables Not Working

**Symptom:**
Config shows `${env:VAR}` instead of value.

**Solution:**
```bash
# Verify variable is set
echo $MY_VAR

# Export the variable
export MY_VAR="value"

# Check in config
api_key = "${env:MY_VAR}"
```

### Validation Errors

**Symptom:**
```
Error: Validation failed: field 'port' exceeds maximum 65535
```

**Solution:**
Check configuration schema in `docs/CONFIG_REFERENCE.md` for valid ranges and types.

## Performance Issues

### Slow Plugin Loading

**Symptom:**
Plugin takes > 1 second to load.

**Solutions:**

1. Enable metadata caching:
```rust
// Use optimized loading path
let manager = PluginManager::new(config)
    .with_caching(true);
```

2. Check filesystem performance:
```bash
# Test disk speed
dd if=/dev/zero of=test bs=1M count=100
```

### High Memory Usage

**Symptom:**
Plugin uses excessive memory.

**Solutions:**

1. Profile memory allocation:
```bash
valgrind --tool=massif ./target/release/skylet
```

2. Check for large allocations:
```rust
// Avoid: vec![0u8; 1_000_000_000]  // 1GB
// Use: lazy loading or streaming
```

### High CPU Usage

**Symptom:**
CPU at 100%.

**Solutions:**

1. Profile CPU:
```bash
perf record -g ./target/release/skylet
perf report
```

2. Check for busy loops:
```rust
// Avoid: loop { }
// Use: async with proper waiting
tokio::time::sleep(Duration::from_millis(100)).await;
```

## Hot Reload Issues

### Reload Fails

**Symptom:**
```
Error: Hot reload failed for plugin my-plugin
```

**Solutions:**

1. Check plugin supports hot reload:
```rust
supports_hot_reload: true,  // In plugin info
```

2. Verify state serialization:
```rust
fn serialize_state(&self) -> Result<Vec<u8>> {
    serde_json::to_vec(&self.state)
}
```

3. Check rollback history:
```rust
let history = rollback_manager.get_history("my-plugin").await;
```

### State Loss After Reload

**Symptom:**
Plugin state is reset after reload.

**Solution:**
Ensure state is properly serialized/deserialized:
```rust
// Implement state preservation
fn serialize_state(&self) -> Result<Vec<u8>> { ... }
fn deserialize_state(&mut self, data: &[u8]) -> Result<()> { ... }
```

### Dependency Reload Loop

**Symptom:**
Plugins keep reloading in a loop.

**Solution:**
Check for circular dependencies:
```bash
# Enable debug logging
RUST_LOG=plugin_manager=debug ./target/release/skylet
```

## Security Issues

### Permission Denied

**Symptom:**
```
Error: Permission denied for operation: filesystem.read
```

**Solution:**
Declare required capabilities:
```rust
capabilities: ["filesystem.read", "filesystem.write"]
```

### Secret Access Failed

**Symptom:**
```
Error: Failed to resolve secret: vault://secrets/api_key
```

**Solutions:**

1. Check Vault is running:
```bash
vault status
```

2. Verify environment:
```bash
echo $VAULT_ADDR
echo $VAULT_TOKEN
```

3. Test secret access:
```bash
vault kv get secrets/my-plugin/api_key
```

### FFI Validation Errors

**Symptom:**
```
Error: Invalid FFI pointer: null context
```

**Solution:**
Always validate pointers:
```rust
if context.is_null() {
    return PluginResultV2::InvalidRequest;
}
```

## Debug Tools

### Enable Debug Logging

```bash
# All debug logs
RUST_LOG=debug ./target/release/skylet

# Specific module
RUST_LOG=plugin_manager=debug ./target/release/skylet

# Multiple modules
RUST_LOG=plugin_manager=debug,config=trace ./target/release/skylet
```

### Generate Flamegraph

```bash
# Install flamegraph
cargo install flamegraph

# Generate flamegraph
cargo flamegraph --bin skylet -o flamegraph.svg

# View in browser
open flamegraph.svg
```

### Memory Profiling

```bash
# Valgrind memory check
valgrind --leak-check=full --show-leak-kinds=all ./target/release/skylet

# Heap profiling
valgrind --tool=massif ./target/release/skylet
ms_print massif.out.<pid>
```

### Thread Sanitizers

```bash
# Thread sanitizer
RUSTFLAGS="-Zsanitizer=thread" cargo run --target x86_64-unknown-linux-gnu

# Address sanitizer
RUSTFLAGS="-Zsanitizer=address" cargo run --target x86_64-unknown-linux-gnu
```

### Core Dumps

```bash
# Enable core dumps
ulimit -c unlimited

# Analyze core dump
gdb ./target/release/skylet core.<pid>
(gdb) bt full
```

### Network Debugging

```bash
# Capture network traffic
tcpdump -i lo port 8080 -w capture.pcap

# Analyze with Wireshark
wireshark capture.pcap
```

## Getting Help

If you can't resolve an issue:

1. Search [GitHub Issues](https://github.com/vincents-ai/skylet/issues)
2. Ask in [GitHub Discussions](https://github.com/vincents-ai/skylet/discussions)
3. Create a new issue with:
   - Error message
   - Steps to reproduce
   - Environment (OS, Rust version)
   - Debug logs

## Related Documentation

- [Plugin Development Guide](./PLUGIN_DEVELOPMENT.md)
- [Configuration Reference](./CONFIG_REFERENCE.md)
- [Security Guide](./SECURITY.md)
- [Performance Guide](./PERFORMANCE.md)
- [FAQ](./FAQ.md)
