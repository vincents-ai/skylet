# Skylet - Execution Engine Documentation

<div align="center">
  <img src="../logo.svg" alt="Skylet Logo" width="150">
</div>

## Overview

**Skylet** is a secure, extensible plugin runtime for autonomous agents and microservices. It provides a stable ABI (v2.0.0) that enables safe plugin execution with strict FFI boundaries, type-safe configuration, and hot reload support.

### Key Features

- **Secure Plugin Runtime**: Sandboxed execution with strict FFI boundaries
- **ABI v2.0.0**: Frozen ABI with backward compatibility until v3.0
- **Hot Reload**: Update plugins without downtime
- **Type-Safe Config**: Schema validation with 14+ field types
- **Cryptographic Operations**: Ed25519, AES-GCM, SHA-256 support
- **Comprehensive Testing**: 1,079+ tests, zero compiler warnings

### Quick Start

```bash
# Create a new plugin
cargo new --lib my-plugin
cd my-plugin

# Add dependencies
cargo add skylet-abi skylet-plugin-common

# Build
cargo build --release
```

See [Plugin Development Guide](PLUGIN_DEVELOPMENT.md) for the full tutorial.

---

## Documentation Structure

### Getting Started

| Guide | Description |
|-------|-------------|
| [Plugin Development](PLUGIN_DEVELOPMENT.md) | Create your first plugin |
| [API Reference](API.md) | Core types and FFI functions |
| [Configuration Reference](CONFIG_REFERENCE.md) | Configuration system |

### Core Concepts

| Document | Description |
|----------|-------------|
| [Architecture](ARCHITECTURE.md) | System design and plugin model |
| [Plugin Contract](PLUGIN_CONTRACT.md) | ABI v2.0 specification |
| [ABI Stability](ABI_STABILITY.md) | Versioning guarantees |

### Security & Operations

| Guide | Description |
|-------|-------------|
| [Security Best Practices](SECURITY.md) | Secure development guidelines |
| [Performance Tuning](PERFORMANCE.md) | Optimization techniques |

---

## Version Information

- **Release Version**: 0.5.0 (Beta)
- **ABI Version**: 2.0.0 (Stable)
- **License**: Apache 2.0

---

## Support

- **Issues**: [Report bugs](https://github.com/vincents-ai/skylet/issues)
- **Discussions**: [Ask questions](https://github.com/vincents-ai/skylet/discussions)
- **Security**: Report vulnerabilities to `shift+security@someone.section.me`

---

*Last Updated: February 2026*
