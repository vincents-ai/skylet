Config Manager Plugin
=====================

Configuration loading and management service for Skylet autonomous marketplace.

Overview
--------

The Config Manager plugin provides centralized configuration management for Skylet, including:

- **Configuration Loading**: Load configurations from TOML, JSON, or YAML files
- **Service Registry**: Register ConfigService in the plugin system
- **Configuration Types**: Comprehensive type definitions for all system components
- **Validation**: Built-in configuration validation with detailed error messages
- **Export Formats**: Export configurations in multiple formats

Architecture
------------

The config-manager plugin follows the bootstrap plugin pattern and is essential for system initialization.

Key Components:

1. **Configuration Types**
   - AppConfig: Main configuration container
   - DatabaseConfig: Database and Raft cluster settings
   - TorConfig: Tor network settings
   - MoneroConfig: Monero wallet settings
   - DiscoveryConfig: Service discovery settings
   - AgentsConfig: Automation agent settings
   - EscrowConfig: Marketplace escrow settings
   - PaymentsConfig: Automated payment settings

2. **ConfigService**
   - In-memory configuration management
   - Thread-safe access via Arc<RwLock<>>
   - Individual configuration section accessors
   - Validation and export capabilities

3. **Plugin Interface**
   - C FFI exported functions for plugin ABI
   - JSON-based IPC for configuration operations
   - Initialization and shutdown hooks

Building the Plugin
-------------------

Build with default features:

```bash
cd skylet
cargo build -p config-manager
```

Build release binary (UPX compressed):

```bash
cd skylet
nix build .#packages.default
```

The compiled plugin will be at:
- Debug: `target/debug/libconfig_manager.so`
- Release: `result/lib/libconfig_manager.so`

Plugin Functions
----------------

The plugin exports the following C functions:

### Initialization

```c
int plugin_init(void)
```

Initializes the config-manager plugin and creates a ConfigService instance.

Returns:
- 0 on success
- Non-zero on failure

### Shutdown

```c
int plugin_shutdown(void)
```

Gracefully shuts down the plugin.

Returns:
- 0 on success
- Non-zero on failure

### Plugin Info

```c
const char* plugin_get_info(void)
```

Returns JSON string with plugin metadata:

```json
{
  "name": "config-manager",
  "version": "0.1.0",
  "description": "Configuration loading and management service",
  "capabilities": [
    "config_load",
    "config_get",
    "config_set",
    "config_validate",
    "config_export"
  ]
}
```

### Load Configuration

```c
const char* config_load(const char* path)
```

Load configuration from a file (TOML, JSON, or YAML).

Parameters:
- `path`: File path to configuration file

Returns JSON response:

Success:
```json
{
  "success": true,
  "message": "Configuration loaded from /path/to/config.toml"
}
```

Error:
```json
{
  "success": false,
  "error": "Failed to read config file: No such file or directory"
}
```

### Get Configuration

```c
const char* config_get(void)
```

Get the current configuration.

Returns JSON response with full configuration structure.

### Set Configuration

```c
const char* config_set(const char* config_json)
```

Set configuration from JSON string.

Parameters:
- `config_json`: JSON string containing AppConfig

Returns:
- Success or error response in JSON format

### Validate Configuration

```c
const char* config_validate(void)
```

Validate the current configuration against rules.

Returns:
- Success response with validation message
- Error response with validation failures

### Export Configuration

Export the configuration in different formats:

```c
const char* config_export_json(void)
const char* config_export_toml(void)
const char* config_export_yaml(void)
```

Each returns JSON response with exported configuration string.

Configuration File Format
-------------------------

### TOML Example

```toml
[database]
path = "./data/marketplace.db"
node_id = 1
raft_nodes = ["1 localhost:8100 localhost:8200"]
election_timeout_ms = 5000
secret_raft = "MarketplaceRaftSecret1337"
secret_api = "MarketplaceApiSecret1337"
data_dir = "./data"

[tor]
socks_port = 9050
control_port = 9051
hidden_service_port = 8080

[monero]
daemon_url = "http://localhost:18081"
wallet_path = "./data/wallet"
wallet_rpc_port = 18083
network = "testnet"
auto_refresh = true
refresh_interval = 30

[agents]
enabled = true

[agents.security]
enabled = true
scan_interval_seconds = 3600
auto_patch_threshold = 0.8
enable_vulnerability_scanning = true

[agents.maintenance]
enabled = true
maintenance_interval = 86400
test_environment = "production"
deployment_stages = ["pre", "staging", "production"]

[agents.security]
ops_interval = 3600

[discovery]
enabled = true
cache_ttl = 3600
announce_interval = 300

[discovery.dht]
bootstrap_nodes = [
  "/ip4/104.131.131.82/tcp/4001/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ"
]
replication_factor = 20
timeout_seconds = 30

[discovery.i2p]
enabled = false
sam_host = "127.0.0.1"
sam_port = 7656

[escrow]
default_release_days = 7
max_dispute_days = 30
marketplace_fee_percentage = 2.5
min_arbiter_confidence = 0.8
auto_arbitration_enabled = true

[payments]
enabled = false
reserve_months = 3
payment_buffer_days = 7
check_interval_hours = 24
max_payment_amount = 1000000000000
retry_attempts = 3
retry_delay_minutes = 60
large_payment_threshold = 500000000000
```

### JSON Example

```json
{
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
  "agents": {
    "enabled": true,
    "security": {
      "enabled": true,
      "scan_interval_seconds": 3600,
      "cve_database_url": null,
      "auto_patch_threshold": 0.8,
      "enable_vulnerability_scanning": true
    },
    "maintenance": {
      "enabled": true,
      "maintenance_interval": 86400,
      "test_environment": "production",
      "deployment_stages": ["pre", "staging", "production"]
    },
    "ops_interval": 3600
  },
  "discovery": {
    "enabled": true,
    "dht": {
      "bootstrap_nodes": ["/ip4/104.131.131.82/tcp/4001/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ"],
      "replication_factor": 20,
      "timeout_seconds": 30
    },
    "i2p": {
      "enabled": false,
      "sam_host": "127.0.0.1",
      "sam_port": 7656
    },
    "fallback_endpoints": ["marketplace.onion", "marketplace.i2p", "127.0.0.1:8080"],
    "cache_ttl": 3600,
    "announce_interval": 300,
    "peer_id": null,
    "private_key": null
  },
  "escrow": {
    "default_release_days": 7,
    "max_dispute_days": 30,
    "marketplace_fee_percentage": 2.5,
    "min_arbiter_confidence": 0.8,
    "auto_arbitration_enabled": true
  },
  "payments": {
    "enabled": false,
    "reserve_months": 3,
    "payment_buffer_days": 7,
    "check_interval_hours": 24,
    "max_payment_amount": 1000000000000,
    "retry_attempts": 3,
    "retry_delay_minutes": 60,
    "providers": [],
    "large_payment_threshold": 500000000000
  }
}
```

Usage Examples
--------------

### Load Configuration at Startup

```rust
use libloading::Library;
use std::ffi::CString;

unsafe {
    let lib = Library::new("./libconfig_manager.so")?;
    
    // Initialize
    let plugin_init: libloading::Symbol<unsafe extern "C" fn() -> i32> = 
        lib.get(b"plugin_init")?;
    plugin_init();
    
    // Load config
    let config_load: libloading::Symbol<unsafe extern "C" fn(*const c_char) -> *const c_char> = 
        lib.get(b"config_load")?;
    let path = CString::new("./config.toml")?;
    let result = config_load(path.as_ptr());
    
    // Parse result as JSON
    let result_str = CStr::from_ptr(result).to_str()?;
    let json: serde_json::Value = serde_json::from_str(result_str)?;
}
```

### Validate Configuration

```rust
let validate: libloading::Symbol<unsafe extern "C" fn() -> *const c_char> = 
    lib.get(b"config_validate")?;
let result = validate();
let result_str = CStr::from_ptr(result).to_str()?;
let json: serde_json::Value = serde_json::from_str(result_str)?;

if json["success"].as_bool().unwrap_or(false) {
    println!("Configuration is valid");
} else {
    eprintln!("Validation error: {}", json["error"]);
}
```

### Export Configuration

```rust
let export_json: libloading::Symbol<unsafe extern "C" fn() -> *const c_char> = 
    lib.get(b"config_export_json")?;
let result = export_json();
let result_str = CStr::from_ptr(result).to_str()?;
let json: serde_json::Value = serde_json::from_str(result_str)?;

if json["success"].as_bool().unwrap_or(false) {
    let config_str = json["data"].as_str().unwrap();
    println!("Exported configuration:\n{}", config_str);
}
```

Integration with Skylet Core
-----------------------------

The config-manager plugin integrates with Skylet core through:

1. **Plugin ABI**: Exports standard plugin initialization/shutdown functions
2. **Service Registry**: ConfigService can be registered in the service registry
3. **Configuration Hierarchy**: Supports file-based defaults, environment variable overrides, and CLI args
4. **Bootstrap Sequence**: Should be loaded first in plugin initialization order

Environment Variables
---------------------

Configuration can be overridden via environment variables with the AUTONOMOUS_ prefix:

```bash
export AUTONOMOUS_DATABASE_NODE_ID=2
export AUTONOMOUS_TOR_SOCKS_PORT=9999
export AUTONOMOUS_MONERO_NETWORK=mainnet
./skylet
```

Testing
-------

Run unit tests:

```bash
cargo test -p config-manager
```

Run specific test:

```bash
cargo test -p config-manager test_config_validation
```

Configuration Validation Rules
-------------------------------

The plugin enforces the following validation rules:

1. **Database**
   - path: Must not be empty
   - node_id: Must be > 0
   - raft_nodes: At least one node must be configured

2. **Monero**
   - daemon_url: Must not be empty
   - wallet_rpc_port: Must be > 0

3. **Discovery**
   - If enabled: DHT bootstrap nodes should be configured (warning if not)

4. **Escrow**
   - marketplace_fee_percentage: Must be between 0 and 100

Performance Notes
-----------------

- Configuration is stored in-memory using Arc<RwLock<>> for thread-safe access
- File I/O only happens during load/export operations
- Configuration updates are atomic writes using RwLock
- No blocking I/O operations after initialization

Security Considerations
-----------------------

1. **Secret Storage**: Secrets are stored in configuration files - consider encryption at rest
2. **File Permissions**: Configuration files should have restricted permissions (600)
3. **Memory Security**: Consider using zeroize for secret values after use
4. **Environment Variables**: Be careful exposing secrets through environment variables

Troubleshooting
---------------

### Plugin fails to load

Check that:
1. Plugin binary exists and is readable
2. All dependencies (libc, libgcc) are available
3. Plugin binary is correct architecture (x86_64, ARM, etc.)

### Configuration file not found

Check that:
1. File path is correct and absolute
2. File has correct extension (.toml, .json, .yaml)
3. File is readable by the plugin process

### Validation failures

Check the error message from `config_validate()` for specific issues:
- Invalid port numbers
- Missing required fields
- Invalid value ranges

License
-------

This plugin is part of the Skylet project and follows the same license terms.

Contributing
------------

To contribute improvements to the config-manager plugin:

1. Follow the Rust style guidelines in AGENTS.md
2. Add tests for new functionality
3. Update documentation for API changes
4. Ensure all tests pass: `cargo test -p config-manager`
5. Run clippy: `cargo clippy -p config-manager -- -D warnings`
