# Skylet - Execution Engine

<div align="center">
  <img src="logo.svg" alt="Skylet Logo" width="200" height="200">
</div>

A secure, extensible, open-source plugin runtime for autonomous agents and microservices.

[![License](https://img.shields.io/badge/License-Apache%202.0%20OR%20MIT-blue.svg)](LICENSE)
[![CI](https://github.com/vincents-ai/skylet/actions/workflows/test.yml/badge.svg)](https://github.com/vincents-ai/skylet/actions/workflows/test.yml)
[![Coverage](https://codecov.io/gh/vincents-ai/skylet/branch/main/graph/badge.svg)](https://codecov.io/gh/vincents-ai/skylet)
![Rust](https://img.shields.io/badge/Rust-1.70+-orange)
![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey)

## Overview

The Skylet execution engine is a beta-stage plugin runtime that enables:

- **Secure plugin execution** with strict FFI boundaries
- **Type-safe configuration** with schema validation
- **Hot reload support** for zero-downtime updates
- **Distributed tracing** with OpenTelemetry
- **Cryptographic operations** with industry-standard algorithms
- **Multi-instance deployments** with standalone mode

Perfect for building:
- Autonomous agent systems
- Microservice architectures
- Extensible applications
- Plugin-based platforms

## Quick Links

**Documentation** | Getting Started → [Plugin Development Guide](docs/PLUGIN_DEVELOPMENT.md)
--- | ---
**Configuration** | Learn → [Configuration Reference](docs/CONFIG_REFERENCE.md)
**Security** | Best Practices → [Security Guide](docs/SECURITY.md)
**Performance** | Optimize → [Performance Tuning](docs/PERFORMANCE.md)
**Specification** | Technical → [ABI Contract](docs/PLUGIN_CONTRACT.md)
**Migration** | Upgrade from V1 → [Migration Guide](docs/MIGRATION_GUIDE.md)
📊 **Stability** | Guarantees → [ABI Stability](docs/ABI_STABILITY.md)

## ✨ Features

### Core Features
- **Plugin ABI v2**: Stable C FFI interface (no breaking changes until v3.0)
- **Service Registry**: Unified service discovery and inter-plugin communication
- **Configuration System**: Type-safe schemas with 14+ field types and validation
- **Hot Reload**: Update plugins without downtime
- **Job Queue**: Background task processing and scheduling

### Security
- **Cryptographic Operations**: Ed25519 signatures, AES-GCM encryption, SHA-256
- **Secret Management**: Vault, environment variables, and file-based secrets
- **Input Validation**: Strict FFI boundary validation
- **Memory Safety**: RAII patterns and zeroization of sensitive data
- **Access Control**: Capability-based permission system

### Developer Experience
- **Comprehensive Documentation**: 2,500+ lines of guides and references
- **Plugin Templates**: Quick start code and example plugins
- **Error Handling**: Detailed error codes and diagnostics
- **Testing Support**: Unit and integration test examples
- **Performance Tools**: Profiling and benchmarking guidance

### Runtime Features
- **Async/Await**: Tokio-based async runtime
- **Distributed Tracing**: OpenTelemetry integration (optional)
- **Observability**: Structured logging with correlation IDs
- **Monitoring**: Built-in metrics collection
- **Platform Support**: Linux, macOS, Windows

## Quick Start

### Install Dependencies

```bash
# macOS
brew install rustup
rustup install 1.70

# Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup install 1.70

# Windows
# Visit https://rust-lang.org/install
```

### Create Your First Plugin

```bash
# Generate new plugin from template
cargo init --name my-plugin --lib
cd my-plugin

# Add dependencies
cargo add skylet-abi tokio serde serde_json
```

Create `src/lib.rs`:

```rust
use skylet_abi::{
    plugin_init_v2, plugin_shutdown_v2, PluginResult,
    PluginContextV2, PluginInfoV2,
};
use std::ffi::CString;

#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResult {
    unsafe {
        let ctx = (*context);
        if let Some(logger) = ctx.service_registry.get_service("logger") {
            logger.log("My plugin initialized!");
        }
    }
    PluginResult::Success
}

#[no_mangle]
pub extern "C" fn plugin_shutdown_v2(context: *const PluginContextV2) -> PluginResult {
    PluginResult::Success
}

#[no_mangle]
pub extern "C" fn plugin_get_info_v2() -> *const PluginInfoV2 {
    static INFO: PluginInfoV2 = PluginInfoV2 {
        name: b"my-plugin\0" as *const u8 as *const i8,
        version: b"1.0.0\0" as *const u8 as *const i8,
        author: b"Your Name\0" as *const u8 as *const i8,
    };
    &INFO
}
```

Build and test:

```bash
cargo build --release
# Plugin at: target/release/libmy_plugin.so (Linux)
#                         libmy_plugin.dylib (macOS)
#                         my_plugin.dll (Windows)
```

See [Plugin Development Guide](docs/PLUGIN_DEVELOPMENT.md) for complete tutorial.

### Build the Engine

```bash
# Default (standalone, no proprietary dependencies)
cargo build --release

# With optional distributed tracing
cargo build --release --features opentelemetry

# Full build (includes proprietary extensions)
cargo build --release --features proprietary
```

## Documentation

### For Plugin Developers
1. **[Plugin Development Guide](docs/PLUGIN_DEVELOPMENT.md)** - Getting started (629 lines)
   - Quick start tutorial
   - Project structure
   - Entry point implementation
   - Configuration handling
   - Error handling and testing

2. **[Configuration Reference](docs/CONFIG_REFERENCE.md)** - Config system (878 lines)
   - All field types with examples
   - Validation rules
   - Secret management
   - Environment variables
   - TOML file format

3. **[Security Best Practices](docs/SECURITY.md)** - Security guide (967 lines)
   - Input validation patterns
   - Memory safety
   - Cryptographic operations
   - Resource management
   - Access control

4. **[Performance Tuning](docs/PERFORMANCE.md)** - Optimization guide (555 lines)
   - FFI overhead reduction
   - Async patterns
   - Memory optimization
   - Profiling setup
   - Common bottlenecks

### Technical Reference
5. **[Plugin Contract](docs/PLUGIN_CONTRACT.md)** - FFI specification
   - Entry points
   - Context structure
   - Error codes
   - Lifecycle events

6. **[ABI Stability](docs/ABI_STABILITY.md)** - Versioning guarantees
   - Semantic versioning
   - Compatibility promises
   - Support timeline

### Migration
7. **[Migration Guide](docs/MIGRATION_GUIDE.md)** - Upgrade from V1
   - Step-by-step instructions
   - Common issues
   - Dual compatibility patterns

## 🏗️ Architecture

```
execution-engine/
├── abi/                      # Plugin ABI v2 (Rust bindings)
│   ├── src/
│   │   ├── lib.rs           # Main ABI exports
│   │   ├── v2_spec.rs       # FFI specifications
│   │   ├── config/          # Configuration system
│   │   ├── security_rfc/    # Security policies
│   │   ├── logging/         # Structured logging
│   │   └── ...
│   └── Cargo.toml
│
├── src/                      # Core engine implementation
│   ├── main.rs              # CLI entry point
│   ├── server.rs            # Server implementation
│   └── ...
│
├── core/                     # Test framework and utilities
├── plugins/                  # Built-in plugins
│   ├── logging/             # Logging service
│   ├── registry/            # Service registry
│   ├── config-manager/      # Configuration management
│   └── secrets-manager/     # Secret management
│
├── http-router/             # HTTP routing
├── job-queue/               # Background job queue
├── permissions/             # Permission system
├── plugin-packager/         # Plugin packaging utilities
│
├── docs/                    # Comprehensive documentation (2,500+ lines)
├── CHANGELOG.md             # Release notes
├── NOTICE                   # Third-party attributions
└── Cargo.toml
```

## 📊 Project Statistics

- **12 crates** with clear separation of concerns
- **125 source files** all with Apache 2.0 license headers
- **1,079 tests** with comprehensive coverage
- **Zero proprietary dependencies** in standalone mode
- **Feature-gated support** for advanced features
- **2,500+ lines of documentation**

## 🔒 Security

### Cryptographic Operations
- **Ed25519**: Digital signatures with ed25519-dalek
- **AES-GCM**: Authenticated encryption with aes-gcm
- **SHA-256**: Cryptographic hashing with sha2
- **Argon2**: Password hashing with argon2

### Secret Management
- **Vault Integration**: HashiCorp Vault support
- **Environment Variables**: Development support
- **File-based Secrets**: Local testing
- **Memory Zeroization**: Automatic cleanup with zeroize crate

### FFI Safety
- **Null Pointer Checking**: All pointers validated
- **Size Limits**: Input size enforcement
- **Memory Validation**: RAII patterns throughout
- **Error Propagation**: Context-rich error reporting

See [Security Best Practices](docs/SECURITY.md) for detailed guidelines.

## Performance

### Target Metrics
| Operation | Target | Notes |
|-----------|--------|-------|
| FFI call overhead | ~200-500ns | Unavoidable boundary cost |
| Plugin load | < 100ms | Typical plugin startup |
| Config validation | < 10ms | Complex schema |
| Request processing | < 50ms | P99 latency |
| Memory per plugin | 5-20MB | Typical usage |

See [Performance Tuning Guide](docs/PERFORMANCE.md) for optimization techniques.

## 🛠️ Development

### Build Variants

```bash
# Fast development build
cargo build

# Release build with optimizations
cargo build --release

# Check syntax without building
cargo check

# Run tests
cargo test

# Generate documentation
cargo doc --no-deps --open
```

### Feature Flags

```bash
# Standalone mode (default) - no proprietary dependencies
cargo build --features standalone

# With distributed tracing
cargo build --features opentelemetry

# Both
cargo build --features standalone,opentelemetry

# All features (requires proprietary dependencies)
cargo build --all-features
```

### Supported Platforms
- ✅ Linux (x86_64, aarch64)
- ✅ macOS (x86_64, aarch64)
- ✅ Windows (x86_64, experimental)

### Minimum Rust Version (MSRV)
- **1.70.0** or later
- **Recommended**: 1.75.0+

## 📦 Versioning

This project uses [Semantic Versioning](https://semver.org/):

### Current Version: **v0.5.0** (2024-02-20)

#### Stability Guarantees
- **Beta release** - API may change in v1.0.0
- **ABI v2.0** - Plugin ABI is stable, no breaking changes until v3.0.0
- **Forward compatibility** for v0.x releases
- **Deprecation grace period**: 1 release minimum

#### Support Timeline
- **v0.5.0**: Beta release (2024-02-20) - Gather feedback, fix issues
- **v1.0.0**: Stable release (TBD) - API stabilization
- **v2.0.0+**: Future major releases with ABI v2.0 stability

See [CHANGELOG.md](CHANGELOG.md) for detailed release notes.

## 🤝 Contributing

We welcome contributions! Please:

1. Read [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines
2. Read [Security Best Practices](docs/SECURITY.md)
3. Follow Rust naming conventions (snake_case functions, PascalCase types)
4. Add tests for new functionality
5. Update documentation
6. Sign commits with your GPG key

## 📄 License

This project is licensed under the **Apache License 2.0**.

- Full license text: See [LICENSE](LICENSE) file
- Third-party attributions: See [NOTICE](NOTICE) file
- All source files include Apache 2.0 header

### Summary
- ✅ Open source and free for commercial use
- ✅ Patent protection included
- ✅ Derivatives must be licensed under Apache 2.0
- ✅ No liability or warranty (use as-is)

## 📞 Support

### Getting Help
- **Documentation**: Start with [Plugin Development Guide](docs/PLUGIN_DEVELOPMENT.md)
- **Issues**: Report bugs on [GitHub Issues](https://github.com/vincents-ai/skylet/issues)
- **Discussions**: Ask questions in [GitHub Discussions](https://github.com/vincents-ai/skylet/discussions)
- **Security**: Report vulnerabilities to `shift+security@someone.section.me` (not public issues)

### Community
- Star ⭐ this repo if you find it useful
- Share your plugins and projects
- Contribute improvements

## Roadmap

### v2.1 (Q3 2024)
- WebAssembly (WASM) plugin support
- Enhanced metrics collection
- Additional example plugins

### v2.2 (Q4 2024)
- Distributed tracing defaults
- Plugin marketplace integration
- Performance optimizations

### v2.3 (Q1 2025)
- Peer-to-peer plugin distribution
- Cross-instance replication
- Advanced security policies

### v3.0 (2026+)
- Breaking changes allowed
- Next-generation ABI
- Enhanced clustering

See [CHANGELOG.md](CHANGELOG.md) for current status.

## 🙏 Acknowledgments

Special thanks to:
- **Rust Community**: Excellent ecosystem and tooling
- **Open Source Maintainers**: Libraries that make this possible
- **Contributors**: Everyone who helps improve the project

## 📊 Statistics

- **125 source files** - All with Apache 2.0 headers
- **1,079 tests** - Comprehensive coverage
- **2,500+ lines** of documentation
- **14+ configuration** field types
- **7+ security** best practice guides
- **Zero** proprietary dependencies (standalone mode)

---

**Made with ❤️ by Vincents AI**

[Repository](https://github.com/vincents-ai/skylet) | 
[Issues](https://github.com/vincents-ai/skylet/issues) | 
[Discussions](https://github.com/vincents-ai/skylet/discussions)
