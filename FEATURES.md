# Feature Flags - RFC-0100 (Phase 2.1)

The execution engine supports multiple feature configurations for different deployment scenarios.

## Default Features

### `standalone` (default)

Enables standalone operation with no external dependencies:

- **Key Management**: Uses `DefaultKeyManagement` with ed25519-dalek for local key generation and signing
- **Instance Management**: Uses `StandaloneInstanceManager` for single-instance deployments
- **No external systems**: Works offline without any additional infrastructure

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

## Feature Combinations

### Standard Deployment (Recommended for Most Users)

```bash
cargo build --features standalone
```

This provides:
- Full plugin execution capabilities
- Basic key management
- Single-instance operation
- No external dependencies

### Development/Testing with Observability

```bash
cargo build --features "standalone,opentelemetry"
```

Adds OpenTelemetry-based observability for development and debugging.

## Implementation Details

### Key Management

The trait-based `KeyManagement` interface allows for different implementations:

- **Default** (`DefaultKeyManagement`): Ed25519 signing using standard cryptographic libraries
- **Custom**: Implement the `KeyManagement` trait to integrate with HSMs, cloud KMS, or other key backends

### Instance Management

The trait-based `InstanceManager` interface allows for different implementations:

- **Default** (`StandaloneInstanceManager`): Single instance with no clustering
- **Custom**: Implement the `InstanceManager` trait to integrate with cluster management systems

## Choosing Features

### For Most Users

Use the default `standalone` feature. This provides:
- Full plugin execution capability
- Key generation and signing
- No external dependencies
- Reproducible builds

### For Production Deployments

Consider enabling `opentelemetry` for observability:
- Distributed tracing
- Metrics collection
- Integration with observability platforms

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
