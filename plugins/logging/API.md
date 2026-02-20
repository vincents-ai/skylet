# Logging Plugin - API Reference

Complete API reference for the Logging Plugin FFI (Foreign Function Interface) and request handlers.

## Plugin Information

- **Name:** logging
- **Version:** 0.1.0
- **Type:** Infrastructure & Logging
- **Capabilities:** structured-logging, log-level-management, event-buffering, json-logging, rfc0018-compliance
- **Services:** logging-service, event-buffer-service
- **Max Concurrency:** Unlimited (thread-safe via Mutex)
- **Supports Hot Reload:** No
- **Supports Async:** No (synchronous operations)
- **Supports Streaming:** No

## FFI Functions

### Plugin Lifecycle

#### `plugin_init(context: *const PluginContext) -> PluginResult`

Initializes the Logging plugin and creates a LoggingService instance.

**Parameters:**
- `context`: Plugin context pointer (PluginContext structure from marketplace ABI)

**Behavior:**
- Initializes LoggingService with default INFO log level
- Sets up event buffer (capacity: 1000 events)
- Configures structured JSON logging (RFC-0018 compliant)
- Prepares for log event collection and retrieval
- Sets initialization flag

**Returns:**
- `PluginResult::Success` - Plugin initialized successfully
- `PluginResult::Error` - Initialization failed (Mutex lock failed)

**Prerequisites:**
- Skylet plugin system running
- No external dependencies
- Works offline

**Example:**
```rust
let result = plugin_init(&plugin_context);
match result {
    PluginResult::Success => println!("Logging plugin ready"),
    PluginResult::Error => eprintln!("Failed to initialize Logging plugin"),
    _ => eprintln!("Unexpected result"),
}
```

#### `plugin_shutdown(context: *const PluginContext) -> PluginResult`

Gracefully shuts down the Logging plugin.

**Parameters:**
- `context`: Plugin context pointer

**Behavior:**
- Clears event buffer
- Releases LoggingService resources
- Resets initialization flag
- Ensures clean shutdown

**Returns:**
- `PluginResult::Success` - Plugin shut down successfully
- `PluginResult::Error` - Shutdown failed

**Example:**
```rust
let result = plugin_shutdown(&plugin_context);
match result {
    PluginResult::Success => println!("Logging plugin shut down cleanly"),
    PluginResult::Error => eprintln!("Error during shutdown"),
    _ => eprintln!("Unexpected result"),
}
```

#### `plugin_get_info() -> *const c_char`

Returns plugin metadata and capabilities as JSON.

**Returns:** Pointer to const c_char containing JSON metadata

**Response Format:**
```json
{
  "name": "logging",
  "version": "0.1.0",
  "description": "Structured logging backend"
}
```

**PluginInfo Structure:**
```c
{
  name: "logging",
  version: "0.1.0",
  description: "Structured logging backend for Skylet with RFC-0018 compliance",
  author: "vincents-ai",
  license: "MIT OR Apache-2.0",
  homepage: "https://github.com/vincents-ai/skylet",
  plugin_type: Integration,
  capabilities: [
    "structured-logging",
    "log-level-management",
    "event-buffering",
    "json-logging",
    "rfc0018-compliance"
  ],
  supports_hot_reload: false,
  supports_async: false,
  supports_streaming: false,
  max_concurrency: unlimited
}
```

**Example:**
```rust
let info_ptr = plugin_get_info();
let info_str = unsafe { CStr::from_ptr(info_ptr).to_string_lossy() };
let info: serde_json::Value = serde_json::from_str(&info_str)?;
println!("Plugin: {}", info["name"]);
println!("Version: {}", info["version"]);
```

---

## Request Handlers (V2 API)

### Overview

The Logging Plugin provides HTTP-like request handlers for managing log levels and retrieving log events. Each handler:
- Accepts requests with method and path
- Returns JSON responses
- Supports error handling with detailed error messages
- Implements thread-safe operations via Mutex

### Handler 1: Get Current Log Level

**Endpoint:** `/log/level/get`
**Method:** `GET`

Get the currently configured log level.

**Parameters:** None

**Behavior:**
- Retrieves current log level from LoggingService
- Returns level as string (TRACE, DEBUG, INFO, WARN, ERROR)
- Thread-safe via Mutex lock

**Returns:** JSON response

**Success Response:**
```json
{
  "level": "INFO",
  "status": "success"
}
```

**Error Response:**
```json
{
  "error": "Logging service lock failed",
  "status": "error"
}
```

**Possible Log Levels:**

| Level | Description | Use Case |
|-------|-------------|----------|
| TRACE | Most detailed logging | Development debugging |
| DEBUG | Debug information | Development and troubleshooting |
| INFO | Informational messages | Normal operations |
| WARN | Warning messages | Potential issues |
| ERROR | Error messages | Critical issues |

**Error Codes:**

| Code | Description |
|------|-------------|
| `E001` | Logging service lock failed |
| `E002` | Service not initialized |

**Examples:**

**Bash - Get log level:**
```bash
#!/bin/bash
curl -X GET http://localhost:8080/plugin/logging/log/level/get | jq .

# Response:
# {
#   "level": "INFO",
#   "status": "success"
# }
```

**Python - Retrieve log level:**
```python
import ctypes
import json
from ctypes import c_char_p, POINTER, Structure

# Define request structure (simplified)
class RequestV2(Structure):
    _fields_ = [
        ("method", c_char_p),
        ("path", c_char_p),
        ("body", POINTER(ctypes.c_byte)),
        ("body_len", ctypes.c_size_t),
    ]

lib = ctypes.CDLL('./target/release/liblogging.so')

# Create request
request = RequestV2()
request.method = b"GET"
request.path = b"/log/level/get"
request.body = None
request.body_len = 0

# Call handler
lib.plugin_handle_request_v2.restype = ctypes.c_void_p
response_ptr = lib.plugin_handle_request_v2(None, ctypes.byref(request))

# Parse response
if response_ptr:
    # Response parsing implementation
    print("Log level retrieved successfully")
```

**Rust - Check log level:**
```rust
use std::ffi::CStr;

unsafe {
    let lib = libloading::Library::new("./target/release/liblogging.so")?;
    
    let handle_request: libloading::Symbol<unsafe extern "C" fn(*const skylet_abi::PluginContextV2, *const skylet_abi::RequestV2) -> *mut skylet_abi::ResponseV2>
        = lib.get(b"plugin_handle_request_v2")?;

    // Create request for /log/level/get
    let method = CString::new("GET")?;
    let path = CString::new("/log/level/get")?;
    
    // Call handler (requires proper PluginContextV2 setup)
    // let response_ptr = handle_request(&context, &request);
    
    println!("Log level request prepared");
}
```

---

### Handler 2: Set Log Level

**Endpoint:** `/log/level/set`
**Method:** `POST`

Set the log level for the logging service.

**Parameters:**

Request body (JSON):
```json
{
  "level": "string (required, one of: TRACE, DEBUG, INFO, WARN, ERROR)"
}
```

**Parameter Details:**

- **level** (required)
  - Type: string
  - Valid values: `TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR` (case-insensitive)
  - Description: Desired log level
  - Examples: `"DEBUG"`, `"INFO"`, `"ERROR"`
  - Default: `INFO` (if invalid level provided)

**Behavior:**
- Parses JSON request body
- Validates log level string
- Updates LoggingService log level
- Returns new log level
- Thread-safe via Mutex lock

**Returns:** JSON response

**Success Response:**
```json
{
  "level": "DEBUG",
  "status": "success"
}
```

**Error Responses:**

```json
{
  "error": "Logging service lock failed",
  "status": "error"
}
```

**Error Codes:**

| Code | Description |
|------|-------------|
| `E010` | Logging service lock failed |
| `E011` | Invalid log level provided (defaults to INFO) |
| `E012` | Service not initialized |

**Examples:**

**Bash - Set log level to DEBUG:**
```bash
#!/bin/bash
curl -X POST http://localhost:8080/plugin/logging/log/level/set \
  -H "Content-Type: application/json" \
  -d '{"level": "DEBUG"}' | jq .

# Response:
# {
#   "level": "DEBUG",
#   "status": "success"
# }
```

**Python - Change log level:**
```python
import requests
import json

# Set log level to ERROR
response = requests.post(
    'http://localhost:8080/plugin/logging/log/level/set',
    json={'level': 'ERROR'}
)

result = response.json()
print(f"Log level set to: {result['level']}")
print(f"Status: {result['status']}")
```

**Bash - Set different log levels:**
```bash
#!/bin/bash
# Set to TRACE for verbose debugging
curl -X POST http://localhost:8080/plugin/logging/log/level/set \
  -H "Content-Type: application/json" \
  -d '{"level": "TRACE"}'

# Set to WARN for production
curl -X POST http://localhost:8080/plugin/logging/log/level/set \
  -H "Content-Type: application/json" \
  -d '{"level": "WARN"}'

# Verify change
curl -X GET http://localhost:8080/plugin/logging/log/level/get | jq .
```

**Rust - Update log level dynamically:**
```rust
use std::ffi::{CString, CStr};
use serde_json::json;

unsafe {
    let lib = libloading::Library::new("./target/release/liblogging.so")?;
    
    // Prepare request body
    let body = json!({
        "level": "DEBUG"
    }).to_string();
    
    let method = CString::new("POST")?;
    let path = CString::new("/log/level/set")?;
    
    // Call handler to set log level
    println!("Setting log level to DEBUG");
    
    // Implementation requires proper RequestV2 structure
}
```

---

### Handler 3: Get Buffered Log Events

**Endpoint:** `/log/events`
**Method:** `GET`

Retrieve all buffered log events collected by the logging service.

**Parameters:** None

**Behavior:**
- Retrieves all events from LoggingService event buffer
- Returns events array with count
- Thread-safe via Mutex lock
- Buffer maintains up to 1000 most recent events
- Returns copy of events (doesn't modify buffer)

**Returns:** JSON response

**Success Response:**
```json
{
  "events": [
    "{\"timestamp\":\"2024-02-03T14:25:30.123456Z\",\"level\":\"INFO\",\"message\":\"Service started\",\"plugin_name\":\"logging\"}",
    "{\"timestamp\":\"2024-02-03T14:25:31.456789Z\",\"level\":\"DEBUG\",\"message\":\"Event logged\",\"plugin_name\":\"logging\"}"
  ],
  "count": 2,
  "status": "success"
}
```

**Empty Buffer Response:**
```json
{
  "events": [],
  "count": 0,
  "status": "success"
}
```

**Error Response:**
```json
{
  "error": "Logging service lock failed",
  "status": "error"
}
```

**Event Format (RFC-0018 Compliant):**

Each event is a JSON string containing:
- **timestamp**: ISO 8601 timestamp (e.g., "2024-02-03T14:25:30.123456Z")
- **level**: Log level (TRACE, DEBUG, INFO, WARN, ERROR)
- **message**: Log message string
- **plugin_name** (optional): Name of plugin that generated log
- **trace_id** (optional): Distributed tracing trace ID
- **span_id** (optional): Distributed tracing span ID
- **data** (optional): Additional structured data

**Error Codes:**

| Code | Description |
|------|-------------|
| `E020` | Logging service lock failed |
| `E021` | Service not initialized |

**Buffer Characteristics:**

- **Capacity:** 1000 events maximum
- **FIFO removal:** Oldest events removed when limit exceeded
- **Data loss:** Events beyond capacity are discarded
- **Thread-safe:** Mutex-protected access

**Examples:**

**Bash - Retrieve all logged events:**
```bash
#!/bin/bash
curl -X GET http://localhost:8080/plugin/logging/log/events | jq .

# Response shows all buffered events
```

**Python - Parse and analyze log events:**
```python
import requests
import json
from datetime import datetime

# Get all log events
response = requests.get('http://localhost:8080/plugin/logging/log/events')
result = response.json()

if result['status'] == 'success':
    print(f"Total events: {result['count']}")
    
    # Parse and display events
    for event_str in result['events']:
        event = json.loads(event_str)
        timestamp = event.get('timestamp', 'N/A')
        level = event.get('level', 'UNKNOWN')
        message = event.get('message', '')
        
        print(f"[{timestamp}] {level}: {message}")
else:
    print(f"Error: {result.get('error')}")
```

**Bash - Count events by level:**
```bash
#!/bin/bash
# Get events and count by level
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events[] | fromjson | .level' | \
  sort | uniq -c | \
  awk '{print $2 ": " $1}'

# Output example:
# DEBUG: 15
# ERROR: 3
# INFO: 42
# WARN: 8
```

**Rust - Stream and filter log events:**
```rust
use std::ffi::CStr;

unsafe {
    let lib = libloading::Library::new("./target/release/liblogging.so")?;
    
    // Get events handler
    // ... implementation to call /log/events
    
    // Parse response
    let response: serde_json::Value = serde_json::from_str(&response_str)?;
    
    if response["status"] == "success" {
        let events = response["events"].as_array().unwrap_or(&vec![]);
        
        // Filter ERROR events
        for event_str in events {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(event_str.as_str().unwrap_or("")) {
                if event["level"] == "ERROR" {
                    println!("ERROR: {}", event["message"]);
                }
            }
        }
    }
}
```

**Bash - Export events to file:**
```bash
#!/bin/bash
# Export all events to JSON file
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events' > events_export.json

# Export with timestamps
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events | map(fromjson) | sort_by(.timestamp)' > events_sorted.json

# View summary
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '{total_count: .count, event_levels: (.events | map(fromjson) | group_by(.level) | map({level: .[0].level, count: length}))}'
```

---

## Health Check & Metrics

### `plugin_health_check_v2(context: *const PluginContextV2) -> HealthStatus`

Check the health status of the logging plugin.

**Parameters:**
- `context`: Plugin context pointer

**Returns:**

- `HealthStatus::Healthy` - Plugin is functioning normally
- `HealthStatus::Unhealthy` - Plugin initialization failed or service lock failed

**Behavior:**
- Attempts to acquire Mutex lock on LoggingService
- Returns Healthy if lock succeeds
- Returns Unhealthy if lock fails or context is null

**Example:**
```bash
#!/bin/bash
curl -X GET http://localhost:8080/plugin/logging/health | jq .

# Response:
# {
#   "status": "healthy",
#   "plugin": "logging",
#   "timestamp": "2024-02-03T14:30:00Z"
# }
```

### `plugin_get_metrics_v2(context: *const PluginContextV2) -> *const PluginMetrics`

Get performance metrics for the logging plugin.

**Parameters:**
- `context`: Plugin context pointer

**Returns:** Pointer to PluginMetrics structure

**Metrics Include:**
- Total request calls handled
- Buffer size and capacity
- Lock acquisition time
- Error counts

**Example:**
```bash
#!/bin/bash
curl -X GET http://localhost:8080/plugin/logging/metrics | jq .

# Response:
# {
#   "plugin": "logging",
#   "calls": 1248,
#   "errors": 0,
#   "buffer_size": 847,
#   "buffer_capacity": 1000,
#   "uptime_seconds": 3600
# }
```

---

## Log Event Format (RFC-0018)

### Structured JSON Format

All log events follow RFC-0018 structured logging format:

**Example Event:**
```json
{
  "timestamp": "2024-02-03T14:25:30.123456Z",
  "level": "INFO",
  "message": "Marketplace initialized",
  "plugin_name": "marketplace-core",
  "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736",
  "span_id": "00f067aa0ba902b7",
  "data": {
    "node_id": 1,
    "version": "0.1.0",
    "startup_time_ms": 152
  }
}
```

### Field Specifications

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| timestamp | string (ISO 8601) | Yes | Event timestamp in UTC |
| level | string | Yes | Log level (TRACE, DEBUG, INFO, WARN, ERROR) |
| message | string | Yes | Human-readable log message |
| plugin_name | string | No | Name of plugin generating log |
| trace_id | string | No | Distributed tracing trace ID |
| span_id | string | No | Distributed tracing span ID |
| data | object | No | Structured additional data |

### Timestamp Format

- Standard: ISO 8601 with microsecond precision
- Example: `2024-02-03T14:25:30.123456Z`
- Timezone: Always UTC (Z suffix)
- Format: `YYYY-MM-DDTHH:MM:SS.ffffffZ`

### Log Levels

| Level | Severity | Description |
|-------|----------|-------------|
| TRACE | Lowest | Very detailed information for debugging |
| DEBUG | Low | Debugging information |
| INFO | Medium | General informational messages |
| WARN | High | Warning messages about potential issues |
| ERROR | Highest | Error messages about failures |

---

## Event Buffer Management

### Buffer Behavior

- **Capacity:** 1000 events
- **Overflow:** FIFO removal of oldest events
- **Thread-safe:** Mutex-protected
- **Persistence:** In-memory only (cleared on shutdown)

### Buffer Operations

**Add Event:**
```json
{
  "timestamp": "2024-02-03T14:25:30.123456Z",
  "level": "INFO",
  "message": "New event added"
}
```

**Clear Buffer (on shutdown):**
- All events discarded
- Buffer reset to empty state

**Retrieve Events:**
- Returns copy of all buffered events
- Does not modify buffer
- Thread-safe operation

---

## Error Handling

### General Error Pattern

All request handlers return JSON responses with this structure:

**Success:**
```json
{
  "status": "success",
  "data": {}
}
```

**Failure:**
```json
{
  "status": "error",
  "error": "Detailed error message"
}
```

### Common Error Scenarios

**Service lock failed:**
```json
{
  "error": "Logging service lock failed",
  "status": "error"
}
```

**Service not initialized:**
```json
{
  "error": "Service not initialized",
  "status": "error"
}
```

**Invalid request:**
```json
{
  "error": "Not found",
  "path": "/log/invalid",
  "method": "GET"
}
```

---

## Integration Examples

### Create RFC-0018 Compliant Log Event

```rust
use crate::plugins::logging::create_log_event;

// Basic event
let event = create_log_event(
    "INFO",
    "User login successful",
    Some("auth-plugin"),
    None,
    None,
    None,
);

// Event with structured data
let mut data = serde_json::Map::new();
data.insert("user_id".to_string(), json!(12345));
data.insert("login_method".to_string(), json!("oauth2"));

let event = create_log_event(
    "INFO",
    "User login successful",
    Some("auth-plugin"),
    Some("trace-123"),
    Some("span-456"),
    Some(data),
);
```

### Monitoring Log Level Changes

```bash
#!/bin/bash
# Monitor log level for debugging issues

echo "Current log level:"
curl -X GET http://localhost:8080/plugin/logging/log/level/get | jq .level

# Increase verbosity for debugging
echo "Setting log level to DEBUG..."
curl -X POST http://localhost:8080/plugin/logging/log/level/set \
  -H "Content-Type: application/json" \
  -d '{"level": "DEBUG"}'

# ... perform operations that need debugging ...

# Retrieve debug events
echo "Debug events:"
curl -X GET http://localhost:8080/plugin/logging/log/events | \
  jq '.events | map(fromjson) | .[] | select(.level == "DEBUG")'

# Reset to INFO
echo "Resetting log level to INFO..."
curl -X POST http://localhost:8080/plugin/logging/log/level/set \
  -H "Content-Type: application/json" \
  -d '{"level": "INFO"}'
```

---

## Performance Characteristics

- **Log Level Get:** O(1) - Atomic read
- **Log Level Set:** O(1) - Atomic write
- **Get Events:** O(n) where n = buffer size
- **Thread Safety:** Mutex-based synchronization
- **Event Capacity:** 1000 events (5-10MB depending on event size)

**Typical Performance:**
- Get level: < 1μs
- Set level: < 1μs
- Get events (100 events): < 1ms
- Lock contention: < 100μs

**Memory Usage:**
- Base LoggingService: ~100 bytes
- Per event: ~200-500 bytes (depending on content)
- Max buffer: ~5-10MB (1000 events)
