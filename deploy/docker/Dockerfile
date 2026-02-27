# Build stage
FROM rust:1.75-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY abi ./abi
COPY permissions ./permissions
COPY core ./core
COPY http-router ./http-router
COPY job-queue ./job-queue
COPY plugin-packager ./plugin-packager
COPY plugin-test-harness ./plugin-test-harness

# Build the project
RUN cargo build --release --bin skylet

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 1000 -s /bin/bash appuser

# Create directories
RUN mkdir -p /app/plugins /app/config /app/logs /app/data

# Copy binary from builder
COPY --from=builder /build/target/release/skylet /app/
COPY --from=builder /build/target/release/skylet-lifecycle-cli /app/

# Copy default configuration
COPY deploy/docker/default-config.toml /app/config/default.toml

# Set ownership
RUN chown -R appuser:appuser /app

# Switch to non-root user
USER appuser

# Environment variables
ENV PLUGIN_DIR=/app/plugins \
    CONFIG_DIR=/app/config \
    LOG_DIR=/app/logs \
    DATA_DIR=/app/data \
    RUST_LOG=info

# Expose ports
EXPOSE 8080 8081 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Entry point
ENTRYPOINT ["/app/skylet"]
CMD ["--config", "/app/config/default.toml"]
