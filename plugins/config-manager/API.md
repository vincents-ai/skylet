# Config Manager Plugin - API Reference

Complete API reference for the Config Manager Plugin FFI (Foreign Function Interface) and configuration handlers.

## Plugin Information

- **Name:** config-manager
- **Version:** 0.1.0
- **Type:** Infrastructure & Configuration Management
- **Capabilities:** configuration-management, file-loading, validation, export, environment-overrides
- **Services:** configuration-service, app-config, environment-management
- **Max Concurrency:** Unlimited (thread-safe via Arc<RwLock>)
- **Supports Hot Reload:** Yes (configuration can be reloaded at runtime)
- **Supports Async:** No (synchronous operations)
- **Supports Streaming:** No

## FFI Functions

### Plugin Lifecycle

#### `plugin_init(context: *const PluginContext) -> PluginResult`

Initializes the Config Manager plugin and creates a ConfigService instance.

**Parameters:**
- `context`: Plugin context pointer (PluginContext structure from marketplace ABI)

**Behavior:**
- Creates ConfigService instance with default configuration
- Initializes thread-safe configuration storage (Arc<RwLock<AppConfig>>)
- Sets up environment variable support
- Validates configuration structure
- Prepares system for configuration operations

**Returns:**
- `PluginResult::Success` - Plugin initialized successfully
- `PluginResult::Error` - Initialization failed

**Prerequisites:**
- No external dependencies required
- All configuration has sensible defaults
- Works offline without network connectivity

**Example:**
```rust
let result = plugin_init(&plugin_context);
match result {
    PluginResult::Success => println!("Config Manager plugin ready"),
    PluginResult::Error => eprintln!("Failed to initialize Config Manager"),
    _ => eprintln!("Unexpected result"),
}
```

#### `plugin_shutdown(context: *const PluginContext) -> PluginResult`

Gracefully shuts down the Config Manager plugin.

**Parameters:**
- `context`: Plugin context pointer

**Behavior:**
- Flushes any pending configuration writes
- Releases ConfigService resources
- Cleans up thread-local storage
- Logs shutdown completion

**Returns:**
- `PluginResult::Success` - Plugin shut down successfully
- `PluginResult::Error` - Shutdown failed

**Example:**
```rust
let result = plugin_shutdown(&plugin_context);
match result {
    PluginResult::Success => println!("Config Manager shut down cleanly"),
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
  "name": "config-manager",
  "version": "0.1.0",
  "description": "Configuration loading and management service"
}
```

**PluginInfo Structure:**
```c
{
  name: "config-manager",
  version: "0.1.0",
  description: "Configuration loading and management service for Skylet",
  author: "vincents-ai",
  license: "MIT OR Apache-2.0",
  homepage: "https://github.com/vincents-ai/skylet",
  plugin_type: Integration,
  capabilities: [
    "configuration-management",
    "file-loading",
    "validation",
    "export",
    "environment-overrides"
  ],
  supports_hot_reload: true,
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

## Configuration Functions

### Configuration Loading

#### `config_load(path: *const c_char) -> *const c_char`

Load configuration from a file (TOML, JSON, or YAML format).

**Parameters:**

- **path** (required)
  - Type: *const c_char
  - Description: Path to configuration file
  - Supported formats: `.toml`, `.json`, `.yaml`, `.yml`
  - Examples: `/etc/skylet/config.toml`, `./config.json`, `$HOME/.config/app.yaml`
  - Must be valid UTF-8 string
  - Can be absolute or relative path

**Behavior:**
- Auto-detects file format by extension
- Parses configuration file
- Validates configuration structure
- Replaces current ConfigService with loaded configuration
- Merges with environment variable overrides

**Returns:** JSON response as c_char pointer

**Success Response:**
```json
{
  "success": true,
  "message": "Configuration loaded from /etc/skylet/config.toml"
}
```

**Error Responses:**

```json
{
  "success": false,
  "error": "Path cannot be null"
}
```

```json
{
  "success": false,
  "error": "File not found: /etc/skylet/config.toml"
}
```

```json
{
  "success": false,
  "error": "Invalid TOML format: unexpected character at line 5"
}
```

**Supported Formats:**

**TOML Format:**
```toml
[database]
path = "./data/marketplace.db"
node_id = 1
raft_nodes = ["1 localhost:8100 localhost:8200"]
election_timeout_ms = 5000

[tor]
socks_port = 9050
control_port = 9051
hidden_service_port = 8080

[monero]
daemon_url = "http://localhost:18081"
wallet_rpc_port = 18083
network = "testnet"
```

**JSON Format:**
```json
{
  "database": {
    "path": "./data/marketplace.db",
    "node_id": 1,
    "raft_nodes": ["1 localhost:8100 localhost:8200"],
    "election_timeout_ms": 5000
  },
  "tor": {
    "socks_port": 9050,
    "control_port": 9051,
    "hidden_service_port": 8080
  }
}
```

**YAML Format:**
```yaml
database:
  path: ./data/marketplace.db
  node_id: 1
  raft_nodes:
    - "1 localhost:8100 localhost:8200"
  election_timeout_ms: 5000
tor:
  socks_port: 9050
  control_port: 9051
  hidden_service_port: 8080
```

**Error Codes:**

| Code | Description |
|------|-------------|
| `E001` | Path is null or invalid |
| `E002` | File not found at specified path |
| `E003` | Invalid file format (unsupported extension) |
| `E004` | Malformed configuration (parse error) |
| `E005` | Configuration validation failed |
| `E006` | File read permission denied |
| `E007` | File too large (> 10MB) |
| `E008` | Unsupported file encoding |

**Examples:**

**Bash - Load TOML configuration:**
```bash
#!/bin/bash
curl -X POST http://localhost:8080/plugin/config-manager/config_load \
  -H "Content-Type: application/json" \
  -d '{"path": "/etc/skylet/config.toml"}'
```

**Python - Load JSON configuration:**
```python
import ctypes
import json

# Load the plugin library
lib = ctypes.CDLL('./target/release/libconfig_manager.so')

# Define function signature
lib.config_load.argtypes = [ctypes.c_char_p]
lib.config_load.restype = ctypes.c_char_p

# Call function
path = b'/etc/skylet/config.json'
result = lib.config_load(path)
response = json.loads(result.decode('utf-8'))

print(f"Success: {response['success']}")
if response['success']:
    print(f"Message: {response['message']}")
else:
    print(f"Error: {response['error']}")
```

**Rust - Load YAML configuration:**
```rust
use std::ffi::{CString, CStr};

// Load plugin
let lib = libloading::Library::new("./target/release/libconfig_manager.so")?;

unsafe {
    let config_load: libloading::Symbol<unsafe extern "C" fn(*const std::ffi::c_char) -> *const std::ffi::c_char>
        = lib.get(b"config_load")?;

    let path = CString::new("/etc/skylet/config.yaml")?;
    let result_ptr = config_load(path.as_ptr());
    let result_str = CStr::from_ptr(result_ptr).to_string_lossy();
    
    let response: serde_json::Value = serde_json::from_str(&result_str)?;
    println!("Loaded: {}", response["message"]);
}
```

---

### Configuration Access

#### `config_get() -> *const c_char`

Retrieve the current configuration.

**Parameters:** None

**Behavior:**
- Returns complete current configuration
- Includes all configuration sections
- Includes environment variable overrides
- Returns configuration as JSON

**Returns:** JSON response containing configuration

**Success Response:**
```json
{
  "success": true,
  "data": {
    "database": {
      "path": "./data/marketplace.db",
      "node_id": 1,
      "raft_nodes": ["1 localhost:8100 localhost:8200"],
      "election_timeout_ms": 5000,
      "secret_raft": "MarketplaceRaftSecret1337",
      "secret_api": "MarketplaceApiSecret1337",
      "data_dir": "./data"
    },
    "tor": {
      "socks_port": 9050,
      "control_port": 9051,
      "hidden_service_port": 8080
    },
    "monero": {
      "daemon_url": "http://localhost:18081",
      "wallet_path": "./data/wallet",
      "wallet_rpc_port": 18083,
      "network": "testnet",
      "wallet_password": null,
      "auto_refresh": true,
      "refresh_interval": 30
    },
    "discovery": {
      "enabled": true,
      "dht": {
        "bootstrap_nodes": ["..."],
        "replication_factor": 20,
        "timeout_seconds": 30
      },
      "cache_ttl": 300,
      "announce_interval": 60
    },
    "agents": {
      "enabled": true,
      "max_concurrent": 10,
      "timeout_seconds": 300
    },
    "escrow": {
      "enabled": true,
      "timeout_blocks": 100,
      "min_confirmations": 6
    },
    "payments": {
      "enabled": true,
      "provider": "stripe",
      "api_version": "2023-10-16"
    }
  }
}
```

**Error Response:**
```json
{
  "success": false,
  "error": "ConfigService not initialized"
}
```

**Error Codes:**

| Code | Description |
|------|-------------|
| `E010` | ConfigService not initialized |
| `E011` | Configuration corrupted |
| `E012` | JSON serialization failed |

**Examples:**

**Bash - Get current configuration:**
```bash
#!/bin/bash
curl -X GET http://localhost:8080/plugin/config-manager/config_get
```

**Python - Retrieve and parse configuration:**
```python
import ctypes
import json

lib = ctypes.CDLL('./target/release/libconfig_manager.so')
lib.config_get.restype = ctypes.c_char_p

result = lib.config_get()
response = json.loads(result.decode('utf-8'))

if response['success']:
    config = response['data']
    print(f"Database path: {config['database']['path']}")
    print(f"Monero network: {config['monero']['network']}")
    print(f"Discovery enabled: {config['discovery']['enabled']}")
else:
    print(f"Error: {response['error']}")
```

**Rust - Access specific configuration section:**
```rust
use std::ffi::CStr;

unsafe {
    let lib = libloading::Library::new("./target/release/libconfig_manager.so")?;
    let config_get: libloading::Symbol<unsafe extern "C" fn() -> *const std::ffi::c_char>
        = lib.get(b"config_get")?;

    let result_ptr = config_get();
    let result_str = CStr::from_ptr(result_ptr).to_string_lossy();
    
    let response: serde_json::Value = serde_json::from_str(&result_str)?;
    let db_path = &response["data"]["database"]["path"];
    println!("Database path: {}", db_path);
}
```

---

### Configuration Update

#### `config_set(config_json: *const c_char) -> *const c_char`

Update the configuration from a JSON string.

**Parameters:**

- **config_json** (required)
  - Type: *const c_char
  - Description: JSON string containing new configuration
  - Must be valid JSON matching AppConfig schema
  - Can be partial (only specified fields are updated) or complete
  - Max size: 10MB

**Behavior:**
- Parses JSON configuration string
- Validates new configuration structure
- Merges with existing configuration (preserving unspecified fields)
- Updates ConfigService in-memory store
- Does NOT persist to disk (use export functions for persistence)

**Returns:** JSON response

**Success Response:**
```json
{
  "success": true,
  "message": "Configuration updated successfully"
}
```

**Error Responses:**

```json
{
  "success": false,
  "error": "Config JSON cannot be null"
}
```

```json
{
  "success": false,
  "error": "Failed to parse config JSON: expected value at line 1 column 0"
}
```

```json
{
  "success": false,
  "error": "Configuration validation failed: database.election_timeout_ms must be > 0"
}
```

**Error Codes:**

| Code | Description |
|------|-------------|
| `E020` | Config JSON is null |
| `E021` | Invalid JSON format |
| `E022` | Configuration validation failed |
| `E023` | ConfigService not initialized |
| `E024` | Partial update caused invalid state |

**Examples:**

**Bash - Update database configuration:**
```bash
#!/bin/bash
curl -X POST http://localhost:8080/plugin/config-manager/config_set \
  -H "Content-Type: application/json" \
  -d '{
    "database": {
      "path": "./data/marketplace.db",
      "node_id": 2,
      "raft_nodes": ["1 localhost:8100 localhost:8200", "2 localhost:8101 localhost:8201"],
      "election_timeout_ms": 6000
    }
  }'
```

**Python - Update multiple sections:**
```python
import ctypes
import json

lib = ctypes.CDLL('./target/release/libconfig_manager.so')
lib.config_set.argtypes = [ctypes.c_char_p]
lib.config_set.restype = ctypes.c_char_p

update = {
    "monero": {
        "daemon_url": "http://monero.example.com:18081",
        "network": "mainnet",
        "auto_refresh": True,
        "refresh_interval": 60
    },
    "tor": {
        "socks_port": 9050,
        "control_port": 9051,
        "hidden_service_port": 8080
    }
}

config_json = json.dumps(update).encode('utf-8')
result = lib.config_set(config_json)
response = json.loads(result.decode('utf-8'))

print(f"Success: {response['success']}")
if not response['success']:
    print(f"Error: {response['error']}")
```

**Rust - Update agent configuration:**
```rust
use std::ffi::CString;

unsafe {
    let lib = libloading::Library::new("./target/release/libconfig_manager.so")?;
    let config_set: libloading::Symbol<unsafe extern "C" fn(*const std::ffi::c_char) -> *const std::ffi::c_char>
        = lib.get(b"config_set")?;

    let new_config = serde_json::json!({
        "agents": {
            "enabled": true,
            "max_concurrent": 20,
            "timeout_seconds": 600
        }
    });

    let config_str = CString::new(new_config.to_string())?;
    let result_ptr = config_set(config_str.as_ptr());
    let result_str = CStr::from_ptr(result_ptr).to_string_lossy();
    
    let response: serde_json::Value = serde_json::from_str(&result_str)?;
    println!("Updated: {}", response["message"]);
}
```

---

### Configuration Validation

#### `config_validate() -> *const c_char`

Validate the current configuration.

**Parameters:** None

**Behavior:**
- Performs comprehensive validation of all configuration sections
- Checks for required fields
- Validates data types and ranges
- Verifies inter-dependencies
- Returns detailed validation results

**Validation Rules:**
- Database election_timeout_ms > 0
- Database node_id >= 1
- Tor ports in valid range (1-65535)
- Monero wallet path is valid
- Discovery timeout > 0
- Agents timeout > 0
- All URLs are valid format

**Returns:** JSON response

**Success Response:**
```json
{
  "success": true,
  "message": "Configuration is valid"
}
```

**Validation Error Response:**
```json
{
  "success": false,
  "error": "Configuration validation failed: database.election_timeout_ms must be > 0"
}
```

**Error Codes:**

| Code | Description |
|------|-------------|
| `E030` | ConfigService not initialized |
| `E031` | Validation failed - see error message |
| `E032` | Multiple validation errors (see error message) |

**Examples:**

**Bash - Validate current configuration:**
```bash
#!/bin/bash
curl -X GET http://localhost:8080/plugin/config-manager/config_validate
```

**Python - Validate and handle errors:**
```python
import ctypes
import json

lib = ctypes.CDLL('./target/release/libconfig_manager.so')
lib.config_validate.restype = ctypes.c_char_p

result = lib.config_validate()
response = json.loads(result.decode('utf-8'))

if response['success']:
    print("Configuration is valid and ready for use")
else:
    errors = response['error'].split(';')
    for error in errors:
        print(f"Validation error: {error.strip()}")
```

**Rust - Validate after configuration changes:**
```rust
use std::ffi::CStr;

unsafe {
    let lib = libloading::Library::new("./target/release/libconfig_manager.so")?;
    let config_validate: libloading::Symbol<unsafe extern "C" fn() -> *const std::ffi::c_char>
        = lib.get(b"config_validate")?;

    let result_ptr = config_validate();
    let result_str = CStr::from_ptr(result_ptr).to_string_lossy();
    
    let response: serde_json::Value = serde_json::from_str(&result_str)?;
    
    if response["success"].as_bool().unwrap_or(false) {
        println!("Configuration validated successfully");
        return Ok(());
    } else {
        eprintln!("Validation failed: {}", response["error"]);
        return Err(anyhow::anyhow!("Config validation"));
    }
}
```

---

### Configuration Export

#### `config_export_json() -> *const c_char`

Export current configuration as JSON.

**Parameters:** None

**Behavior:**
- Serializes current configuration to JSON format
- Includes all configuration sections
- Pretty-prints for readability
- Does not modify configuration state

**Returns:** JSON response with configuration data

**Success Response:**
```json
{
  "success": true,
  "data": "{\"database\":{\"path\":\"./data/marketplace.db\",\"node_id\":1,...},\"tor\":{...},...}"
}
```

**Error Response:**
```json
{
  "success": false,
  "error": "ConfigService not initialized"
}
```

**Error Codes:**

| Code | Description |
|------|-------------|
| `E040` | ConfigService not initialized |
| `E041` | JSON serialization failed |

**Examples:**

**Bash - Export as JSON:**
```bash
#!/bin/bash
curl -X GET http://localhost:8080/plugin/config-manager/config_export_json | jq '.data'
```

**Python - Export and save to file:**
```python
import ctypes
import json

lib = ctypes.CDLL('./target/release/libconfig_manager.so')
lib.config_export_json.restype = ctypes.c_char_p

result = lib.config_export_json()
response = json.loads(result.decode('utf-8'))

if response['success']:
    with open('config.json', 'w') as f:
        f.write(response['data'])
    print("Configuration exported to config.json")
```

**Rust - Export and pretty-print:**
```rust
use std::ffi::CStr;

unsafe {
    let lib = libloading::Library::new("./target/release/libconfig_manager.so")?;
    let config_export_json: libloading::Symbol<unsafe extern "C" fn() -> *const std::ffi::c_char>
        = lib.get(b"config_export_json")?;

    let result_ptr = config_export_json();
    let result_str = CStr::from_ptr(result_ptr).to_string_lossy();
    
    let response: serde_json::Value = serde_json::from_str(&result_str)?;
    if response["success"].as_bool().unwrap_or(false) {
        let config_json = &response["data"];
        println!("{}", serde_json::to_string_pretty(config_json)?);
    }
}
```

---

#### `config_export_toml() -> *const c_char`

Export current configuration as TOML.

**Parameters:** None

**Behavior:**
- Serializes current configuration to TOML format
- Maintains hierarchical structure
- Suitable for direct file writing
- Includes all configuration sections

**Returns:** JSON response with TOML data

**Success Response:**
```json
{
  "success": true,
  "data": "[database]\npath = \"./data/marketplace.db\"\nnode_id = 1\n..."
}
```

**Error Codes:**

| Code | Description |
|------|-------------|
| `E050` | ConfigService not initialized |
| `E051` | TOML serialization failed |

**Examples:**

**Bash - Export as TOML and save:**
```bash
#!/bin/bash
curl -X GET http://localhost:8080/plugin/config-manager/config_export_toml | \
  jq -r '.data' > config.toml
echo "Configuration saved to config.toml"
```

**Python - Export with formatting:**
```python
import ctypes
import json

lib = ctypes.CDLL('./target/release/libconfig_manager.so')
lib.config_export_toml.restype = ctypes.c_char_p

result = lib.config_export_toml()
response = json.loads(result.decode('utf-8'))

if response['success']:
    with open('config.toml', 'w') as f:
        f.write(response['data'])
    print("Exported to config.toml")
    print("\nFirst 50 lines:")
    for line in response['data'].split('\n')[:50]:
        print(line)
```

**Rust - Export and validate TOML:**
```rust
use std::ffi::CStr;

unsafe {
    let lib = libloading::Library::new("./target/release/libconfig_manager.so")?;
    let config_export_toml: libloading::Symbol<unsafe extern "C" fn() -> *const std::ffi::c_char>
        = lib.get(b"config_export_toml")?;

    let result_ptr = config_export_toml();
    let result_str = CStr::from_ptr(result_ptr).to_string_lossy();
    
    let response: serde_json::Value = serde_json::from_str(&result_str)?;
    if response["success"].as_bool().unwrap_or(false) {
        let toml_str = response["data"].as_str().unwrap();
        
        // Validate TOML can be parsed
        let _: toml::Value = toml::from_str(toml_str)?;
        println!("Exported TOML is valid");
    }
}
```

---

#### `config_export_yaml() -> *const c_char`

Export current configuration as YAML.

**Parameters:** None

**Behavior:**
- Serializes current configuration to YAML format
- Maintains hierarchical structure with indentation
- Human-readable format
- Suitable for Kubernetes and DevOps tools

**Returns:** JSON response with YAML data

**Success Response:**
```json
{
  "success": true,
  "data": "database:\n  path: ./data/marketplace.db\n  node_id: 1\n..."
}
```

**Error Codes:**

| Code | Description |
|------|-------------|
| `E060` | ConfigService not initialized |
| `E061` | YAML serialization failed |

**Examples:**

**Bash - Export as YAML:**
```bash
#!/bin/bash
curl -X GET http://localhost:8080/plugin/config-manager/config_export_yaml | \
  jq -r '.data' > config.yaml
echo "Configuration exported to config.yaml"
```

**Python - Export and apply to environment:**
```python
import ctypes
import json
import yaml
import os

lib = ctypes.CDLL('./target/release/libconfig_manager.so')
lib.config_export_yaml.restype = ctypes.c_char_p

result = lib.config_export_yaml()
response = json.loads(result.decode('utf-8'))

if response['success']:
    config = yaml.safe_load(response['data'])
    
    # Example: Set environment variables from YAML
    os.environ['DATABASE_PATH'] = config['database']['path']
    os.environ['MONERO_NETWORK'] = config['monero']['network']
    
    print("Environment variables updated from YAML config")
```

**Rust - Export and load as structured config:**
```rust
use std::ffi::CStr;

unsafe {
    let lib = libloading::Library::new("./target/release/libconfig_manager.so")?;
    let config_export_yaml: libloading::Symbol<unsafe extern "C" fn() -> *const std::ffi::c_char>
        = lib.get(b"config_export_yaml")?;

    let result_ptr = config_export_yaml();
    let result_str = CStr::from_ptr(result_ptr).to_string_lossy();
    
    let response: serde_json::Value = serde_json::from_str(&result_str)?;
    if response["success"].as_bool().unwrap_or(false) {
        let yaml_str = response["data"].as_str().unwrap();
        
        // Save to file for later loading
        std::fs::write("config.yaml", yaml_str)?;
        println!("Configuration exported to config.yaml");
    }
}
```

---

## Configuration Data Types

### AppConfig (Root Structure)

```json
{
  "database": DatabaseConfig,
  "tor": TorConfig,
  "monero": MoneroConfig,
  "discovery": DiscoveryConfig,
  "agents": AgentsConfig,
  "escrow": EscrowConfig,
  "payments": PaymentsConfig
}
```

### DatabaseConfig

```json
{
  "path": "string (path to database file)",
  "node_id": "number (Raft node identifier)",
  "raft_nodes": "string array (Raft cluster members)",
  "election_timeout_ms": "number (election timeout in milliseconds)",
  "secret_raft": "string (Raft cluster secret)",
  "secret_api": "string (API secret for authentication)",
  "data_dir": "string (data directory path)"
}
```

### TorConfig

```json
{
  "socks_port": "number (Tor SOCKS port, default 9050)",
  "control_port": "number (Tor control port, default 9051)",
  "hidden_service_port": "number (hidden service port, default 8080)"
}
```

### MoneroConfig

```json
{
  "daemon_url": "string (Monero daemon RPC URL)",
  "wallet_path": "string (wallet file path)",
  "wallet_rpc_port": "number (wallet RPC port)",
  "network": "string (mainnet, stagenet, testnet)",
  "wallet_password": "string | null (wallet password)",
  "auto_refresh": "boolean (auto-refresh wallet)",
  "refresh_interval": "number (refresh interval in seconds)"
}
```

### DiscoveryConfig

```json
{
  "enabled": "boolean (enable discovery)",
  "dht": {
    "bootstrap_nodes": "string array (DHT bootstrap nodes)",
    "replication_factor": "number (replication factor)",
    "timeout_seconds": "number (discovery timeout)"
  },
  "i2p": {
    "enabled": "boolean (enable I2P)",
    "sam_host": "string (SAM bridge host)",
    "sam_port": "number (SAM bridge port)"
  },
  "fallback_endpoints": "string array (fallback endpoints)",
  "cache_ttl": "number (cache TTL in seconds)",
  "announce_interval": "number (announce interval in seconds)",
  "peer_id": "string | null (peer identifier)",
  "private_key": "string | null (private key)"
}
```

### AgentsConfig

```json
{
  "enabled": "boolean (enable autonomous agents)",
  "max_concurrent": "number (max concurrent agents)",
  "timeout_seconds": "number (agent timeout in seconds)"
}
```

### EscrowConfig

```json
{
  "enabled": "boolean (enable escrow)",
  "timeout_blocks": "number (escrow timeout in blocks)",
  "min_confirmations": "number (min confirmations required)"
}
```

### PaymentsConfig

```json
{
  "enabled": "boolean (enable payments)",
  "provider": "string (payment provider: stripe, paypal, etc)",
  "api_version": "string (provider API version)"
}
```

---

## Error Handling

### General Error Pattern

All configuration functions return JSON responses with this structure:

**Success:**
```json
{
  "success": true,
  "message": "Operation successful",
  "data": {}
}
```

**Failure:**
```json
{
  "success": false,
  "error": "Detailed error message"
}
```

### Common Error Scenarios

**Configuration not initialized:**
```json
{
  "success": false,
  "error": "ConfigService not initialized"
}
```

**Invalid JSON:**
```json
{
  "success": false,
  "error": "Failed to parse config JSON: expected value at line 1 column 0"
}
```

**Validation failed:**
```json
{
  "success": false,
  "error": "Configuration validation failed: database.election_timeout_ms must be > 0"
}
```

**File not found:**
```json
{
  "success": false,
  "error": "File not found: /path/to/config.toml"
}
```

---

## Configuration Management Workflow

### Complete Workflow Example

```bash
#!/bin/bash
# Complete configuration management workflow

# 1. Load configuration from file
curl -X POST http://localhost:8080/plugin/config-manager/config_load \
  -H "Content-Type: application/json" \
  -d '{"path": "./config.toml"}' | jq .

# 2. Retrieve current configuration
curl -X GET http://localhost:8080/plugin/config-manager/config_get | jq '.data'

# 3. Update specific sections
curl -X POST http://localhost:8080/plugin/config-manager/config_set \
  -H "Content-Type: application/json" \
  -d '{
    "monero": {
      "network": "mainnet",
      "daemon_url": "http://monero.example.com:18081"
    }
  }' | jq .

# 4. Validate configuration
curl -X GET http://localhost:8080/plugin/config-manager/config_validate | jq .

# 5. Export as JSON for backup
curl -X GET http://localhost:8080/plugin/config-manager/config_export_json | \
  jq '.data' > config_backup.json

# 6. Export as YAML for Kubernetes
curl -X GET http://localhost:8080/plugin/config-manager/config_export_yaml | \
  jq '.data' > config.yaml
```

---

## Performance Characteristics

- **Configuration Load:** O(n) where n = file size
- **Configuration Get:** O(1) - returns in-memory copy
- **Configuration Set:** O(n) where n = configuration size
- **Validation:** O(n) - validates all sections
- **Export:** O(n) where n = configuration size

**Typical Performance:**
- Load: < 10ms for typical configurations
- Get: < 1ms
- Set: < 5ms
- Validate: < 5ms
- Export: < 10ms

**Thread Safety:**
- All operations are thread-safe via Arc<RwLock>
- Multiple simultaneous readers allowed
- Writers are exclusive
- No deadlock risk with standard configuration usage
