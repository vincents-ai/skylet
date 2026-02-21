# Feature Flags - RFC-0100 (Phase 2.1)

The execution engine supports multiple feature configurations to allow for both standalone operation and integration with proprietary systems.

## Default Features

### `standalone` (default)

Enables standalone operation without proprietary dependencies:

- **Key Management**: Uses `DefaultKeyManagement` with ed25519-dalek for local key generation and signing
- **Instance Management**: Uses `StandaloneInstanceManager` for single-instance deployments
- **No external systems**: Works offline without connection to proprietary infrastructure

Build with standalone features:
```bash
cargo build --features standalone
```

## Optional Features

### `opentelemetry`

Enables OpenTelemetry-based observability and tracing:

- Distributed tracing support
- Metrics collection
- OTLP exporter integration
- Jaeger propagation support

Build with OpenTelemetry:
```bash
cargo build --features opentelemetry
```

### `proprietary`

Enables proprietary Skylet extension support:

- Integration with Skylet's AGE-based key hierarchy
- Multi-instance orchestration and zone management
- Peer-to-peer networking and discovery
- Hybrid billing and reputation systems

Build with proprietary features:
```bash
cargo build --features proprietary
```

Note: This feature requires additional proprietary crates that are not available in the open-source distribution.

## Feature Combinations

### Standard Deployment (Recommended for Most Users)

```bash
cargo build --features standalone
```

This provides:
- Full plugin execution capabilities
- Basic key management
- Single-instance operation
- No proprietary dependencies

### Development/Testing with Observability

```bash
cargo build --features "standalone,opentelemetry"
```

Adds OpenTelemetry-based observability for development and debugging.

### Proprietary Deployment (Skylet Internal)

```bash
cargo build --features "standalone,proprietary,opentelemetry"
```

Enables all features including proprietary Skylet extensions.

## Implementation Details

### Key Management

The trait-based `KeyManagement` interface allows for different implementations:

- **Standalone** (`DefaultKeyManagement`): Ed25519 signing using standard cryptographic libraries
- **Proprietary**: Would use Skylet's AGE-based key hierarchy with hardware security modules

### Instance Management

The trait-based `InstanceManager` interface allows for different implementations:

- **Standalone** (`StandaloneInstanceManager`): Single instance with no clustering
- **Proprietary**: Would use Skylet's zone-based instance hierarchy with P2P networking

## Choosing Features

### For Open-Source Users

Use the default `standalone` feature. This provides:
- ✅ Full plugin execution capability
- ✅ Key generation and signing
- ✅ No proprietary dependencies
- ✅ Reproducible builds

### For Skylet Customers

Use `standalone` + `proprietary`. This enables:
- ✅ All open-source features
- ✅ Multi-instance management
- ✅ Proprietary key hierarchy
- ✅ Advanced networking and billing

### For Production Deployments

Consider enabling `opentelemetry` for observability:
- ✅ Distributed tracing
- ✅ Metrics collection
- ✅ Integration with observability platforms

## Build Verification

After building, verify that only expected features are enabled:

```bash
# Check what was built
strings target/release/libskylet_abi.so | grep -i "feature"

# Or inspect with cargo tree
cargo tree --features standalone
```

## Future Enhancements

Additional features planned for future releases:

- `postgres`: PostgreSQL-specific extensions
- `s3`: S3-compatible storage extensions
- `k8s`: Kubernetes integration
- `vault`: HashiCorp Vault integration for secrets management
