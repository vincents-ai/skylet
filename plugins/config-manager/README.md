Config Manager Plugin
=====================

Configuration loading and management service for Skylet.

Overview
--------

The Config Manager plugin provides centralized configuration management for Skylet, including:

- **Configuration Loading**: Load configurations from TOML, JSON, or YAML files
- **Service Registry**: Register ConfigService in the plugin system
- **Configuration Types**: Type-safe definitions for system components
- **Validation**: Built-in configuration validation with detailed error messages
- **Export Formats**: Export configurations in TOML, JSON, or YAML

Architecture
------------

The config-manager plugin follows the bootstrap plugin pattern and is essential for system initialization.

Key Components:

1. **Configuration Types**
   - AppConfig: Main configuration container
   - DatabaseConfig: Database path, node ID, and data directory

2. **ConfigService**
   - In-memory configuration management
   - Thread-safe access via Arc<RwLock<>>
   - Individual configuration section accessors
   - Validation and export capabilities

3. **Plugin Interface (v2 ABI)**
   - C FFI exported functions for plugin ABI v2
   - JSON-based IPC for configuration operations
   - Initialization and shutdown hooks

Building the Plugin
-------------------

Build with default features:

```bash
cd skylet
cargo build -p config-manager
```

Build release binary:

```bash
cd skylet
nix build .#packages.default
```

The compiled plugin will be at:
- Debug: `target/debug/libconfig_manager.so`
- Release: `result/lib/libconfig_manager.so`

Configuration File Format
-------------------------

### TOML Example

```toml
[database]
path = "./data/skylet.db"
node_id = 1
data_dir = "./data"
```

### JSON Example

```json
{
  "database": {
    "path": "./data/skylet.db",
    "node_id": 1,
    "data_dir": "./data"
  }
}
```

Usage Examples
--------------

### Using ConfigService directly (Rust)

```rust
use config_manager::{ConfigService, DatabaseConfig};

// Load defaults
let service = ConfigService::new();

// Load from file
let service = ConfigService::load_from_toml("./config.toml")?;

// Auto-discover config (RFC-0006 compliant paths)
let service = ConfigService::load_auto()?;

// Get database config
let db_config = service.get_database_config()?;
println!("Database path: {:?}", db_config.path);
println!("Node ID: {}", db_config.node_id);

// Update database config
let mut db_config = service.get_database_config()?;
db_config.node_id = 2;
service.set_database_config(db_config)?;

// Validate
service.validate()?;

// Export
let toml_str = service.export_toml()?;
let json_str = service.export_json()?;
```

Integration with Skylet Core
-----------------------------

The config-manager plugin integrates with Skylet core through:

1. **Plugin ABI v2**: Exports standard plugin initialization/shutdown functions
2. **Service Registry**: ConfigService can be registered in the service registry
3. **Configuration Hierarchy**: Supports file-based defaults with RFC-0006 compliant config paths
4. **Bootstrap Sequence**: Should be loaded first in plugin initialization order

Configuration is discovered automatically using RFC-0006 compliant paths:
- Local: `./config/config-manager.toml`
- User: `~/.config/skylet/plugins/config-manager.toml`
- System: `/etc/skylet/plugins/config-manager.toml`

Environment variables use the `SKYLET_` prefix:

```bash
export SKYLET_DATABASE_PATH="./data/skylet.db"
export SKYLET_DATABASE_NODE_ID=2
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

Performance Notes
-----------------

- Configuration is stored in-memory using Arc<RwLock<>> for thread-safe access
- File I/O only happens during load/export operations
- Configuration updates are atomic writes using RwLock
- No blocking I/O operations after initialization

Security Considerations
-----------------------

1. **File Permissions**: Configuration files should have restricted permissions (600)
2. **Memory Security**: Consider using zeroize for secret values after use
3. **Environment Variables**: Be careful exposing secrets through environment variables

License
-------

This plugin is part of the Skylet project, dual-licensed under MIT OR Apache-2.0.

Contributing
------------

To contribute improvements to the config-manager plugin:

1. Add tests for new functionality
2. Update documentation for API changes
3. Ensure all tests pass: `cargo test -p config-manager`
4. Run clippy: `cargo clippy -p config-manager -- -D warnings`
