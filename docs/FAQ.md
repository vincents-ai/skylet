# Skylet FAQ

Frequently asked questions about Skylet.

## General

### What is Skylet?

Skylet is a secure, extensible plugin runtime for autonomous agents and microservices. It provides a stable ABI for plugins, sandboxed execution, and built-in observability.

### What license does Skylet use?

Skylet is dual-licensed under MIT OR Apache-2.0. All source files include the appropriate license headers.

### What platforms are supported?

- Linux (x86_64, aarch64)
- macOS (x86_64, aarch64)
- Windows (x86_64, experimental)

### What is the minimum Rust version?

Rust 1.70.0 or later. Rust 1.75.0+ is recommended.

## Plugin Development

### How do I create a plugin?

Use the `skylet_plugin_v2!` macro from `skylet-plugin-common`:

```rust
use skylet_plugin_common::skylet_plugin_v2;

skylet_plugin_v2! {
    name: "my-plugin",
    version: "0.1.0",
    description: "My plugin",
    author: "Author",
    license: "MIT",
    tagline: "Does something useful",
    category: skylet_abi::PluginCategory::Utility,
    max_concurrency: 10,
    supports_async: true,
    capabilities: ["my.capability"],
}
```

### What is the current ABI version?

ABI v2.0.0 is the current stable version. No breaking changes are planned until v3.0.0.

### Can I use async/await in plugins?

Yes. Set `supports_async: true` in the macro and use Tokio for async operations.

### How do I access configuration?

```rust
unsafe {
    let ctx = (*context);
    if let Some(config) = ctx.service_registry.get_service("config") {
        let value = config.get_value("my-plugin", "api_key");
    }
}
```

### How do I log messages?

```rust
unsafe {
    let ctx = (*context);
    if let Some(logger) = ctx.service_registry.get_service("logger") {
        logger.log("My message");
    }
}
```

## Deployment

### How do I run Skylet?

```bash
cargo build --release
./target/release/skylet --plugin ./path/to/plugin.so
```

### Can I hot-reload plugins?

Yes. Skylet supports hot-reload with state preservation. See [Enhanced Hot Reload](./ENHANCED_HOT_RELOAD.md).

### How do I configure plugins?

Create TOML configuration files in `~/.config/skylet/plugins/{plugin-name}.toml`:

```toml
[my-plugin]
enabled = true
api_key = "env://MY_API_KEY"
timeout = "30s"
```

### What environment variables are supported?

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Logging filter (e.g., `debug`) |
| `SKYLET_ENV` | Environment (development/production) |
| `Plugin-specific` | Override config values |

## Security

### How does Skylet secure plugins?

- **FFI Boundary Validation**: All pointers and data are validated
- **Capability System**: Plugins declare required permissions
- **Memory Safety**: RAII patterns and zeroization
- **Secret Management**: Vault, env vars, file-based secrets

### How do I handle secrets?

Use secret references in configuration:

```toml
api_key = "vault://secrets/my-plugin/api_key"
password = "env://MY_PLUGIN_PASSWORD"
```

### Can plugins access the filesystem?

Only if the plugin declares the `filesystem.read` or `filesystem.write` capabilities.

## Performance

### What is the FFI call overhead?

Approximately 200-500 nanoseconds per call. Minimize FFI calls by batching operations.

### How much memory does a plugin use?

Typically 5-20MB per plugin, depending on complexity.

### Can I profile plugins?

Yes. Use standard Rust profiling tools:

```bash
# Flamegraph
cargo flamegraph --bin my-plugin

# Perf (Linux)
perf record --call-graph=dwarf ./target/release/my-plugin
```

## Troubleshooting

### Plugin won't load

1. Check binary architecture matches host
2. Verify all dependencies are available
3. Check file permissions
4. Review engine logs for errors

```bash
# Check binary
file target/release/libmy_plugin.so

# Check dependencies (Linux)
ldd target/release/libmy_plugin.so
```

### Configuration not loading

1. Verify file path is correct
2. Check TOML syntax
3. Ensure plugin name matches filename

```bash
# Validate TOML
cargo install toml-cli
toml config.toml
```

### Plugin crashes

1. Enable debug logging: `RUST_LOG=debug`
2. Check for null pointer dereferences
3. Validate all FFI inputs
4. Run with sanitizers: `RUSTFLAGS="-Zsanitizer=address"`

### Hot reload fails

1. Verify plugin supports hot reload
2. Check state serialization is working
3. Review rollback history
4. Check for dependency issues

## Compatibility

### Can I use Skylet with other languages?

The ABI is C-compatible, so any language with FFI support can create plugins. However, Rust is the primary target.

### Will v2.0 plugins work with future versions?

Yes. ABI v2.x is stable and backward compatible. Plugins built for v2.0 will work with v2.1, v2.2, etc.

### When will v3.0 be released?

No date is set. Expect at least 2 years from v2.0 release (2026+).

## Contributing

### How do I contribute?

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Run `cargo test && cargo clippy`
5. Submit a pull request

### Where do I report bugs?

Open an issue at: https://github.com/vincents-ai/skylet/issues

### How do I report security issues?

Email: `shift+security@someone.section.me`

Do NOT create public issues for security vulnerabilities.

## More Information

- [Plugin Development Guide](./PLUGIN_DEVELOPMENT.md)
- [API Reference](./API_REFERENCE.md)
- [Configuration Reference](./CONFIG_REFERENCE.md)
- [Security Guide](./SECURITY.md)
- [Performance Guide](./PERFORMANCE.md)
