# Config Manager Plugin - API Reference

Complete API reference for the Config Manager Plugin v2 ABI (RFC-0004) and configuration service.

## Plugin Information

- **Name:** config-manager
- **Version:** 0.2.0
- **ABI Version:** 2.0
- **Category:** Development
- **Maturity:** Stable
- **License:** MIT OR Apache-2.0
- **Capabilities:** `config.get`, `config.set`, `config.export`, `config.validate`
- **Services Provided:** `ConfigService` (interface: `config-service-v2`)
- **Tags:** config, settings, bootstrap, core
- **Max Concurrency:** 1
- **Supports Hot Reload:** No
- **Supports Async:** No
- **Supports Streaming:** No

## V2 ABI Entry Points

### `plugin_create_v2() -> *const PluginApiV2`

Returns a static `PluginApiV2` function table. This is the required entry point that the Skylet runtime uses to discover all plugin functions.

**Returns:** Pointer to a static `PluginApiV2` struct containing function pointers for all lifecycle and capability operations.

---

### `plugin_get_info_v2() -> *const PluginInfoV2`

Returns plugin metadata as a `PluginInfoV2` struct (40+ fields).

**Behavior:**
- Lazily initializes the static `PluginInfoV2` on first call
- Builds capability list, tag list, and service info using builder helpers from `skylet-plugin-common`
- Thread-safe (uses `AtomicPtr`)

**Key Fields:**
```
name:              "config-manager"
version:           "0.2.0"
description:       "Centralized configuration management with TOML/JSON support, environment overrides, and CLI parsing"
author:            "Skylet Team"
license:           "MIT OR Apache-2.0"
homepage:          "https://github.com/vincents-ai/skylet"
skylet_version_min: "1.0.0"
skylet_version_max: "2.0.0"
abi_version:       "2.0"
num_dependencies:  0
num_provides_services: 1
num_requires_services: 0
```

---

### `plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2`

Initializes the Config Manager plugin.

**Parameters:**
- `context`: Plugin context pointer (v2 ABI `PluginContextV2`)

**Behavior:**
1. Validates context is non-null
2. Ensures `PluginInfoV2` is initialized
3. Logs initialization via context logger
4. Registers `ConfigService` in the context service registry
5. Creates a new `ConfigService` with default configuration

**Returns:**
- `PluginResultV2::Success` - Initialized successfully
- `PluginResultV2::InvalidRequest` - Context was null

---

### `plugin_shutdown_v2(context: *const PluginContextV2) -> PluginResultV2`

Gracefully shuts down the Config Manager plugin.

**Behavior:**
1. Logs shutdown via context logger
2. Drops the `ConfigService` instance
3. Sets initialized flag to false

**Returns:**
- `PluginResultV2::Success` - Shut down successfully
- `PluginResultV2::InvalidRequest` - Context was null

---

### `plugin_health_check_v2(context: *const PluginContextV2) -> HealthStatus`

Returns the health status of the plugin.

**Returns:**
- `HealthStatus::Healthy` - ConfigService is initialized
- `HealthStatus::Degraded` - ConfigService is not initialized

---

### `plugin_get_metrics_v2(context: *const PluginContextV2) -> *const PluginMetrics`

Returns plugin runtime metrics.

**Returns:** Pointer to `PluginMetrics` struct:
```
uptime_seconds:      <seconds since UNIX epoch at init>
request_count:       0
error_count:         0
avg_response_time_ms: 0.0
memory_usage_mb:     16
cpu_usage_percent:   0.1
```

---

### `plugin_query_capability_v2(context: *const PluginContextV2, capability: *const c_char) -> bool`

Checks whether this plugin supports a given capability.

**Supported Capabilities:**
- `"config.get"` - Retrieve configuration values
- `"config.set"` - Update configuration values
- `"config.export"` - Export configuration to JSON/TOML/YAML
- `"config.validate"` - Validate current configuration

**Returns:** `true` if the capability is supported, `false` otherwise.

---

### `plugin_handle_request_v2(...) -> PluginResultV2`

Not implemented for config-manager. Always returns `PluginResultV2::NotImplemented`.

---

## ConfigService API

The `ConfigService` is the core Rust API for configuration management. It is used internally by the plugin and can be used directly in Rust code.

### Construction

#### `ConfigService::new() -> Self`

Creates a new service with default configuration.

```rust
let service = ConfigService::new();
```

#### `ConfigService::load_defaults() -> Result<Self>`

Creates a new service with default configuration (logs the action).

#### `ConfigService::load_auto() -> Result<Self>`

Loads configuration using RFC-0006 compliant config paths. Searches in order: local -> user -> system. Auto-detects file format by extension (`.toml`, `.json`, `.yaml`, `.yml`).

```rust
let service = ConfigService::load_auto()?;
```

#### `ConfigService::load_from_toml(path: &str) -> Result<Self>`

Loads configuration from a TOML file.

#### `ConfigService::load_from_json(path: &str) -> Result<Self>`

Loads configuration from a JSON file.

#### `ConfigService::load_from_yaml(path: &str) -> Result<Self>`

Loads configuration from a YAML file.

---

### Configuration Access

#### `get_config(&self) -> Result<AppConfig>`

Returns a clone of the current `AppConfig`.

#### `set_config(&self, config: AppConfig) -> Result<()>`

Replaces the current configuration. Thread-safe via `RwLock`.

#### `get_database_config(&self) -> Result<DatabaseConfig>`

Returns the current `DatabaseConfig`.

#### `set_database_config(&self, db_config: DatabaseConfig) -> Result<()>`

Updates the database section of the configuration.

---

### Validation

#### `validate(&self) -> Result<()>`

Validates the current configuration.

**Validation Rules:**
- `database.path` must not be empty
- `database.node_id` must be greater than 0

**Returns:**
- `Ok(())` - Configuration is valid
- `Err(...)` - Describes which validation rule failed

---

### Export

#### `export_json(&self) -> Result<String>`

Exports the current configuration as a pretty-printed JSON string.

#### `export_toml(&self) -> Result<String>`

Exports the current configuration as a TOML string.

#### `export_yaml(&self) -> Result<String>`

Exports the current configuration as a YAML string.

---

## Configuration Data Types

### AppConfig

The root configuration structure.

```rust
pub struct AppConfig {
    pub database: DatabaseConfig,
}
```

**Default:**
```json
{
  "database": {
    "path": "./data/skylet.db",
    "node_id": 1,
    "data_dir": "./data"
  }
}
```

### DatabaseConfig

```rust
pub struct DatabaseConfig {
    pub path: PathBuf,
    pub node_id: u64,
    pub data_dir: PathBuf,
}
```

| Field     | Type     | Default            | Description                 |
|-----------|----------|--------------------|-----------------------------|
| `path`    | PathBuf  | `./data/skylet.db` | Path to the database file   |
| `node_id` | u64     | `1`                | Node identifier (must be >0)|
| `data_dir`| PathBuf  | `./data`           | Data directory path         |

---

## Configuration File Formats

### TOML

```toml
[database]
path = "./data/skylet.db"
node_id = 1
data_dir = "./data"
```

### JSON

```json
{
  "database": {
    "path": "./data/skylet.db",
    "node_id": 1,
    "data_dir": "./data"
  }
}
```

### YAML

```yaml
database:
  path: ./data/skylet.db
  node_id: 1
  data_dir: ./data
```

---

## Environment Variable Overrides

Configuration values can be overridden via environment variables using the `SKYLET_` prefix (see RFC-0006). The plugin uses `skylet-plugin-common::config_paths` to resolve configuration file locations in this order:

1. `./skylet/plugins/config-manager/config.toml` (local)
2. `$XDG_CONFIG_HOME/skylet/plugins/config-manager/config.toml` (user)
3. `/etc/skylet/plugins/config-manager/config.toml` (system)

---

## Error Handling

All `ConfigService` methods return `anyhow::Result<T>`. Errors include:

| Scenario                  | Error Message                                  |
|---------------------------|------------------------------------------------|
| File not found            | `"Failed to read config file: {io_error}"`     |
| Invalid TOML              | `"Failed to parse TOML config: {parse_error}"` |
| Invalid JSON              | `"Failed to parse JSON config: {parse_error}"` |
| Invalid YAML              | `"Failed to parse YAML config: {parse_error}"` |
| RwLock poisoned (read)    | `"Failed to read config: {error}"`             |
| RwLock poisoned (write)   | `"Failed to write config: {error}"`            |
| Empty database path       | `"Database path cannot be empty"`              |
| Zero node_id              | `"Database node_id must be greater than 0"`    |
| TOML export failure       | `"Failed to export config as TOML: {error}"`   |
| JSON export failure       | `"Failed to export config as JSON: {error}"`   |
| YAML export failure       | `"Failed to export config as YAML: {error}"`   |

---

## Thread Safety

- `ConfigService` wraps configuration in `Arc<RwLock<AppConfig>>`
- Multiple concurrent readers are allowed
- Writers acquire exclusive access
- V2 ABI static storage uses `RwLock<Option<Arc<ConfigService>>>` and `AtomicBool` for the initialized flag
- Plugin info uses `AtomicPtr` for lock-free read access after initialization

## Performance

| Operation      | Complexity | Typical Latency |
|----------------|------------|-----------------|
| `get_config`   | O(1)       | < 1ms           |
| `set_config`   | O(1)       | < 1ms           |
| `validate`     | O(1)       | < 1ms           |
| `load_from_*`  | O(n)       | < 10ms          |
| `export_*`     | O(n)       | < 5ms           |

Where n = configuration size (currently very small with a single `database` section).
