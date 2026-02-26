# Skylet - Execution Engine

<div align="center">
  <img src="logo.svg" alt="Skylet Logo" width="200" height="200">
</div>

A secure, extensible, open-source plugin runtime for autonomous agents and microservices.

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-1.70+-orange)
![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey)
![ABI Version](https://img.shields.io/badge/ABI-v2.0.0-green)
![Documentation](https://img.shields.io/badge/docs-2%2C500%2B%20lines-blue)

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

**Getting Started** | [Plugin Development Guide](docs/PLUGIN_DEVELOPMENT.md) | Create your first plugin
--- | --- | ---
**Configuration** | [Configuration Reference](docs/CONFIG_REFERENCE.md) | Understand config system
**Security** | [Security Best Practices](docs/SECURITY.md) | Secure your plugins
**Performance** | [Performance Tuning](docs/PERFORMANCE.md) | Optimize your code
**Technical** | [ABI Contract](docs/PLUGIN_CONTRACT.md) | FFI specification
**Stability** | [ABI Stability](docs/ABI_STABILITY.md) | Version guarantees
**Examples** | [Example Plugins](#example-plugins) | Learn by example

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

## Example Plugins

The repository includes several example plugins demonstrating different features:

### Hello Plugin
Basic plugin demonstrating:
- Simple request handling
- Response generation
- Health check implementation
- Plugin lifecycle management

```bash
cd examples/hello-plugin
cargo build --release
```

### Echo Plugin
Advanced plugin showing:
- Request body processing
- Event handling
- Metrics collection
- State management with atomic counters

```bash
cd examples/echo-plugin
cargo build --release
```

### More Examples Coming Soon
- Database plugin (SQLite integration)
- HTTP client plugin
- Cryptographic operations plugin
- Async task processing plugin

See [examples/](examples/) directory for full source code and integration tests.

### Testing Example Plugins

```bash
# Build all example plugins
cargo build --release

# Run integration tests using the test harness
cd plugin-test-harness
cargo test --test integration

# Test a specific plugin
./target/debug/plugin-test-harness test \
  --plugin-path ../target/release/libecho_plugin.so
```

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

### API Reference
7. **[API Reference](docs/API.md)** - Complete API documentation
    - Data structures
    - Function signatures
    - Type definitions
8. **[Architecture](docs/ARCHITECTURE.md)** - System design
    - Component overview
    - Data flow
    - Design decisions

## 🏗️ Architecture

```
skylet/
├── src/                     # Core engine implementation
│   ├── main.rs             # CLI entry point
│   ├── lib.rs              # Library exports
│   └── ...
│
├── plugins/                 # Plugin infrastructure
│   └── skylet-plugin-common/ # Common utilities for plugins
│       └── src/
│           └── lib.rs      # Plugin helpers and macros
│
├── examples/                # Example plugins
│   ├── hello-plugin/       # Basic plugin example
│   └── echo-plugin/        # Advanced plugin with events/metrics
│
├── plugin-test-harness/     # Testing framework
│   ├── src/
│   │   ├── lib.rs         # Test harness library
│   │   └── main.rs        # CLI tool
│   └── features/           # BDD/Gherkin tests
│
├── tools/                   # Development tools
│   ├── plugin-tester/      # Plugin testing utility
│   └── plugin-scaffold/    # Plugin project generator
│
├── docs/                    # Comprehensive documentation
│   ├── PLUGIN_DEVELOPMENT.md
│   ├── CONFIG_REFERENCE.md
│   ├── SECURITY.md
│   ├── PERFORMANCE.md
│   ├── PLUGIN_CONTRACT.md
│   ├── ABI_STABILITY.md
│   └── ...
│
├── README.md                # This file
├── CONTRIBUTING.md          # Contribution guidelines
├── CHANGELOG.md             # Release notes
├── LICENSE                  # Apache 2.0
└── NOTICE                   # Third-party attributions
```

## 📊 Project Statistics

- **Rust workspace** with 6 crates
- **Multiple example plugins** demonstrating key features
- **Comprehensive test framework** with BDD support
- **Zero proprietary dependencies** in standalone mode
- **Feature-gated support** for advanced features
- **2,500+ lines** of documentation
- **Integration tests** for all example plugins

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

## 🧪 Testing

### Test Framework

The project includes a comprehensive testing framework with:

- **Unit Tests**: Standard Rust unit tests
- **Integration Tests**: Full plugin lifecycle testing
- **BDD Tests**: Cucumber/Gherkin feature files
- **Mock Context**: Isolated testing without full engine

### Running Tests

```bash
# Run all tests
cargo test --all

# Run integration tests
cargo test --test integration

# Run BDD tests using plugin-test-harness
cd plugin-test-harness
cargo run -- bdd --feature-path ./features

# Test specific plugin
cargo run -- test --plugin-path ../target/release/libecho_plugin.so
```

### Test Coverage

Example plugins include comprehensive integration tests:
- **Hello Plugin**: Basic lifecycle and request handling
- **Echo Plugin**: Advanced features with events and metrics
- **Test Harness**: Full BDD suite with multiple scenarios

See [plugin-test-harness/README.md](plugin-test-harness/README.md) for testing documentation.

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

1. Read [Security Best Practices](docs/SECURITY.md)
2. Follow Rust naming conventions (snake_case functions, PascalCase types)
3. Add tests for new functionality
4. Update documentation
5. Sign commits with your GPG key

For detailed guidelines, see CONTRIBUTING.md (coming soon).

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

### v0.6.0 (Q1 2024)
- Additional example plugins with real-world use cases
- Integration tests for all example plugins
- Enhanced BDD test coverage
- Documentation improvements
- Plugin marketplace integration (planned)

### v0.7.0 (Q2 2024)
- WebAssembly (WASM) plugin support
- Enhanced metrics collection
- Performance profiling tools
- Plugin packaging utilities

### v0.8.0 (Q3 2024)
- Distributed tracing defaults
- Advanced security policies
- Plugin dependency management
- Hot reload improvements

### v1.0.0 (Q4 2024)
- API stabilization
- Full feature parity
- Production-ready guarantees
- Comprehensive plugin ecosystem

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
