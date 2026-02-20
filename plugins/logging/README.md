# Logging Plugin

Structured logging backend for Skylet with RFC-0018 compliance.

## Overview

The logging plugin provides comprehensive structured JSON logging capabilities for Skylet, enabling consistent, queryable log events across the entire system. This plugin is a bootstrap plugin required for basic Skylet operation.

## Features

- **Structured JSON Logging**: RFC-0018 compliant JSON output with timestamp, level, message, and contextual data
- **Log Level Management**: Dynamic log level adjustment via plugin API
- **Event Buffering**: In-memory buffering of log events with configurable size limits
- **Tracing Integration**: Native integration with Rust tracing ecosystem
- **Async Support**: Full async/await support for logging operations

## MCP Tools

### log_get_level

Get the current log level.

**Endpoint**: `GET /log/level/get`

**Response**:
```json
{
  "level": "INFO",
  "status": "success"
}
```

**Log Levels**: TRACE, DEBUG, INFO, WARN, ERROR

### log_set_level

Set the log level dynamically.

**Endpoint**: `POST /log/level/set`

**Request Body**:
```json
{
  "level": "DEBUG"
}
```

**Response**:
```json
{
  "level": "DEBUG",
  "status": "success"
}
```

### log_get_events

Retrieve buffered log events.

**Endpoint**: `GET /log/events`

**Response**:
```json
{
  "events": [
    "{...log event 1...}",
    "{...log event 2...}"
  ],
  "count": 2,
  "status": "success"
}
```

## Log Event Schema (RFC-0018)

Each log event is a JSON object with the following structure:

```json
{
  "timestamp": "2026-02-02T10:30:00Z",
  "level": "INFO",
  "message": "Operation completed successfully",
  "plugin_name": "example-plugin",
  "trace_id": "0xabc123",
  "span_id": "0x1",
  "data": {
    "operation_id": "op_12345",
    "duration_ms": 156
  }
}
```

### Top-Level Fields

- **timestamp** (string): RFC3339 formatted timestamp in UTC
- **level** (string): Log level (TRACE, DEBUG, INFO, WARN, ERROR)
- **message** (string): Human-readable log message
- **plugin_name** (string, optional): Name of the plugin that generated the log
- **trace_id** (string, optional): Distributed trace ID for correlation
- **span_id** (string, optional): Current span ID within the trace
- **data** (object, optional): Additional structured data fields

## Configuration

The logging plugin is automatically initialized when Skylet starts. No explicit configuration is required for basic operation.

### Environment Variables

- `RUST_LOG`: Standard Rust logging filter (e.g., `RUST_LOG=debug`)

## Building

```bash
cd plugins/logging
cargo build --release
```

The resulting binary will be available at `target/release/liblogging.so`.

## Testing

Run the test suite:

```bash
cargo test
```

## Plugin Lifecycle

### Initialization (plugin_init_v2)

When the plugin is loaded:
1. Verifies that the logger service is available
2. Creates a new LoggingService instance
3. Sets up structured JSON formatting
4. Initializes event buffering

### Shutdown (plugin_shutdown_v2)

When the plugin is unloaded:
1. Flushes any pending log events
2. Releases buffered event storage
3. Cleans up tracing subscriptions

## Usage in Other Plugins

Other plugins can interact with the logging plugin through the standard plugin API:

```rust
// Get current log level
let response = unsafe {
    ((*context).request_handler)(
        context,
        &RequestV2 {
            method: CString::new("GET").unwrap().as_ptr() as *mut c_char,
            path: CString::new("/log/level/get").unwrap().as_ptr() as *mut c_char,
            // ... other fields
        }
    )
};

// Set log level
let body = json!({"level": "DEBUG"}).to_string();
let response = unsafe {
    ((*context).request_handler)(
        context,
        &RequestV2 {
            method: CString::new("POST").unwrap().as_ptr() as *mut c_char,
            path: CString::new("/log/level/set").unwrap().as_ptr() as *mut c_char,
            body: body.as_bytes().as_ptr(),
            body_len: body.len(),
            // ... other fields
        }
    )
};
```

## Performance

- **Event Buffer**: Maximum 1000 events in memory with FIFO eviction
- **JSON Serialization**: Optimized for minimal overhead
- **Async/Await**: Non-blocking logging operations
- **Concurrency**: Supports up to 100 concurrent logging operations

## Compatibility

- **Skylet Version**: 0.1.0+
- **ABI Version**: 2.0
- **Rust Edition**: 2021
- **MSRV**: 1.70+

## Dependencies

- `skylet-abi`: Skylet plugin ABI definitions
- `tracing`: Structured logging framework
- `tracing-subscriber`: Tracing implementation with JSON formatting
- `serde_json`: JSON serialization
- `chrono`: Timestamp handling
- `tokio`: Async runtime support

## License

MIT

## Contributing

Contributions are welcome! Please ensure:
- All tests pass: `cargo test`
- Code is formatted: `cargo fmt`
- No clippy warnings: `cargo clippy -- -D warnings`
- Tests cover new functionality
- Documentation is updated

## See Also

- [RFC-0018: Structured Logging Schema](https://github.com/vincents-ai/skylet/blob/main/rfcs/0018-structured-logging.md)
- [Plugin Development Guide](https://github.com/vincents-ai/skylet/blob/main/docs/PLUGIN_DEVELOPMENT.md)
- [Core Refactoring Plan](https://github.com/vincents-ai/skylet/blob/main/docs/CORE_REFACTORING.md)
