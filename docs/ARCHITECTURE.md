# Skylet Architecture Overview

## System Architecture

Skylet is a secure, extensible plugin runtime designed for autonomous agents and microservices. This document provides a high-level overview of the system architecture.

## Core Components

```
┌─────────────────────────────────────────────────────────────────┐
│                        Skylet Runtime                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │   Plugin    │  │   Service   │  │    Configuration        │ │
│  │   Manager   │  │  Registry   │  │     Manager             │ │
│  └──────┬──────┘  └──────┬──────┘  └───────────┬─────────────┘ │
│         │                │                      │               │
│  ┌──────┴────────────────┴──────────────────────┴─────────────┐ │
│  │                    Event Bus                                │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │   Metrics   │  │   Secrets   │  │      Job Queue          │ │
│  │  Collector  │  │  Provider   │  │                         │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Plugin Layer                              │
├─────────────────────────────────────────────────────────────────┤
│  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────────┐   │
│  │  logging  │ │  config-  │ │  secrets- │ │   registry    │   │
│  │           │ │  manager  │ │  manager  │ │               │   │
│  └───────────┘ └───────────┘ └───────────┘ └───────────────┘   │
│  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────────┐   │
│  │  custom   │ │  custom   │ │  custom   │ │    custom     │   │
│  │ plugin A  │ │ plugin B  │ │ plugin C  │ │    plugin D   │   │
│  └───────────┘ └───────────┘ └───────────┘ └───────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## Component Details

### Plugin Manager

The Plugin Manager is responsible for the complete lifecycle of plugins:

- **Loading**: Dynamic loading of plugin shared libraries (.so/.dylib/.dll)
- **Initialization**: Calling plugin entry points with context
- **Hot Reload**: Zero-downtime plugin updates with state preservation
- **Dependency Resolution**: Loading plugins in dependency order
- **Unloading**: Graceful shutdown and cleanup

**Key Files:**
- `src/plugin_manager/mod.rs` - Main manager
- `src/plugin_manager/manager.rs` - Core implementation
- `src/plugin_manager/enhanced_hot_reload/` - Hot reload system

### Service Registry

Central service discovery and inter-plugin communication:

- **Service Registration**: Plugins register provided services
- **Service Discovery**: Look up services by name or interface
- **Type-Safe Access**: Strongly typed service references

**Key Files:**
- `src/plugin_manager/service_registry.rs`

### Configuration Manager

Type-safe configuration with schema validation:

- **Schema Definition**: Declare configuration structure
- **Validation**: Automatic validation against schemas
- **Hot Reload**: Reload configuration without restart
- **Multi-Environment**: Dev/staging/production configs

**Key Files:**
- `src/plugin_manager/config/` - Configuration system
- `docs/CONFIG_REFERENCE.md` - User documentation

### Event Bus

Pub-sub event system for plugin communication:

- **Publish-Subscribe**: Decoupled event-driven communication
- **Pattern Matching**: Wildcard and regex routing
- **Event Persistence**: Replay capability for debugging
- **Rate Limiting**: Prevent event storms

**Key Files:**
- `src/plugin_manager/events/` - Event system
- `docs/EVENTS.md` - User documentation

### Metrics Collector

Observability and monitoring:

- **Collection**: Automatic metrics collection
- **Export**: Prometheus and OpenTelemetry formats
- **Health Scoring**: Plugin health assessment

**Key Files:**
- `src/plugin_manager/metrics/` - Metrics system
- `docs/METRICS.md` - User documentation

### Secrets Provider

Secure secret management:

- **Multiple Backends**: Vault, environment, files
- **Zeroization**: Automatic memory clearing
- **Rotation Support**: Secret rotation workflows

**Key Files:**
- `plugins/secrets-manager/` - Secrets plugin

## Plugin ABI

The ABI (Application Binary Interface) defines the contract between plugins and the runtime:

### Version: v2.0.0 (Stable)

**Entry Points:**
| Function | Purpose |
|----------|---------|
| `plugin_init_v2` | Initialize plugin |
| `plugin_shutdown_v2` | Cleanup and shutdown |
| `plugin_get_info_v2` | Return plugin metadata |
| `plugin_handle_request_v2` | Handle incoming requests |
| `plugin_health_check_v2` | Health status check |
| `plugin_prepare_hot_reload_v2` | Prepare for reload |
| `plugin_init_from_state_v2` | Restore from state |

**See Also:**
- [Plugin Contract](./PLUGIN_CONTRACT.md) - Full ABI specification
- [ABI Stability](./ABI_STABILITY.md) - Version guarantees

## Data Flow

### Request Processing

```
HTTP Request
     │
     ▼
┌─────────────┐
│ HTTP Router │
└──────┬──────┘
       │
       ▼
┌─────────────┐     ┌─────────────┐
│   Plugin    │────▶│   Service   │
│   Manager   │     │  Registry   │
└──────┬──────┘     └─────────────┘
       │
       ▼
┌─────────────┐
│   Target    │
│   Plugin    │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Response   │
└─────────────┘
```

### Hot Reload Sequence

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│ File Watch  │────▶│  Debounce   │────▶│  Validate   │
└─────────────┘     └─────────────┘     └──────┬──────┘
                                               │
                                               ▼
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Switch    │◀────│   Load      │◀────│  Serialize  │
│   Traffic   │     │   New       │     │   State     │
└─────────────┘     └─────────────┘     └─────────────┘
       │
       ▼
┌─────────────┐     ┌─────────────┐
│   Restore   │────▶│   Verify    │
│   State     │     │   Health    │
└─────────────┘     └─────────────┘
```

## Security Model

### FFI Boundary

All data crossing the FFI boundary is validated:

```
┌─────────────────────────────────────────────┐
│              Plugin Sandbox                 │
│  ┌───────────────────────────────────────┐  │
│  │           Plugin Code                 │  │
│  │                                       │  │
│  │  - Memory isolation                   │  │
│  │  - Capability-based permissions       │  │
│  │  - Input validation                   │  │
│  └───────────────────────────────────────┘  │
│                                             │
│  FFI Boundary (validated)                   │
│                                             │
│  ┌───────────────────────────────────────┐  │
│  │           Skylet Runtime              │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

### Capability System

Plugins declare required capabilities:

```rust
capabilities: ["config.read", "secrets.read", "http.client"]
```

Capabilities are enforced at runtime.

**See Also:** [Security Guide](./SECURITY.md)

## Performance Characteristics

| Component | Latency Target | Notes |
|-----------|---------------|-------|
| FFI Call | < 1µs | Boundary overhead |
| Plugin Load | < 100ms | Cold start |
| Config Reload | < 10ms | Hot reload |
| Event Delivery | < 100µs | Pub-sub latency |
| Metrics Collection | < 1ms | Per collection |

**See Also:** [Performance Guide](./PERFORMANCE.md)

## Deployment Topologies

### Standalone Mode

Single-process deployment with all plugins:

```
┌─────────────────────────────────┐
│        Single Process           │
│  ┌───────────────────────────┐  │
│  │    Skylet Runtime         │  │
│  │  ┌─────┐ ┌─────┐ ┌─────┐  │  │
│  │  │ P1  │ │ P2  │ │ P3  │  │  │
│  │  └─────┘ └─────┘ └─────┘  │  │
│  └───────────────────────────┘  │
└─────────────────────────────────┘
```

### Multi-Instance Mode

Distributed deployment with clustering:

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Instance 1 │◀───▶│  Instance 2 │◀───▶│  Instance 3 │
│  (Primary)  │     │ (Secondary) │     │ (Secondary) │
└─────────────┘     └─────────────┘     └─────────────┘
       │                   │                   │
       └───────────────────┴───────────────────┘
                           │
                    ┌──────┴──────┐
                    │   Shared    │
                    │   Storage   │
                    └─────────────┘
```

## Crate Structure

```
skylet/
├── abi/                    # Plugin ABI definitions
│   ├── v2_spec.rs         # ABI v2 specification
│   ├── config/            # Configuration types
│   └── logging/           # Logging types
│
├── src/                    # Core engine
│   ├── main.rs            # Entry point
│   ├── plugin_manager/    # Plugin management
│   └── server.rs          # HTTP server
│
├── http-router/           # HTTP routing
├── job-queue/             # Background jobs
├── permissions/           # Permission system
│
└── plugins/               # Built-in plugins
    ├── logging/           # Logging service
    ├── config-manager/    # Configuration
    ├── secrets-manager/   # Secrets
    └── registry/          # Service registry
```

## Extension Points

### Creating a Plugin

1. Implement ABI entry points
2. Define configuration schema
3. Register services (optional)
4. Handle requests

**See Also:** [Plugin Development Guide](./PLUGIN_DEVELOPMENT.md)

### Adding Services

1. Define service trait
2. Implement in plugin
3. Register with Service Registry
4. Document interface

### Custom Events

1. Define event type
2. Publish to Event Bus
3. Subscribers receive events

## Monitoring and Observability

### Logs

Structured JSON logs via logging plugin:

```json
{
  "timestamp": "2024-02-20T10:30:00Z",
  "level": "INFO",
  "message": "Plugin loaded",
  "plugin_name": "my-plugin"
}
```

### Metrics

Prometheus-compatible metrics:

```
plugin_requests_total{plugin="my-plugin"} 1234
plugin_latency_ms{plugin="my-plugin",quantile="0.99"} 42
```

### Tracing

OpenTelemetry distributed tracing (optional feature).

## Related Documentation

- [Plugin Development Guide](./PLUGIN_DEVELOPMENT.md) - Building plugins
- [API Reference](./API_REFERENCE.md) - API documentation
- [Configuration Reference](./CONFIG_REFERENCE.md) - Configuration options
- [Security Guide](./SECURITY.md) - Security best practices
- [Performance Guide](./PERFORMANCE.md) - Optimization techniques
