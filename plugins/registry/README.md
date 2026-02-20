Registry Plugin
===============

A federated plugin registry service for Skylet that provides plugin discovery, registration, and management capabilities.

Features
--------

- Federated registry support for managing multiple plugin sources
- Plugin discovery and search functionality
- Plugin version resolution with semantic versioning
- Registry source management (add/remove sources)
- Service-based architecture via plugin ABI

Building
--------

Build the registry plugin as a shared library:

```bash
nix develop
cargo build -p registry --release
```

The compiled plugin will be located at:
```
target/release/libregistry.so
```

Usage
-----

Load the registry plugin in your Skylet configuration:

```rust
// In your Skylet core binary
let registry_plugin = load_plugin("libregistry.so")?;
let registry_service = registry_plugin.get_service::<RegistryService>()?;
```

API Reference
-------------

The registry plugin exposes the following service interface:

### RegistryService

#### list_plugins(context)
Lists all registered plugins in the registry.

**Returns:** `RegistryListResult`
- `result`: Status code
- `plugins`: Array of plugin names
- `plugin_count`: Number of plugins

**Example:**
```c
RegistryListResult list_result = registry_service->list_plugins(context);
if (list_result.result == PluginResult::Success) {
    for (size_t i = 0; i < list_result.plugin_count; i++) {
        printf("Plugin: %s\n", list_result.plugins[i]);
    }
}
```

#### add_source(context, url)
Add a new registry source.

**Parameters:**
- `context`: Plugin context
- `url`: Registry source URL

**Returns:** `RegistryResult`
- `result`: Status code
- `error_message`: Error message if failed

**Example:**
```c
RegistryResult result = registry_service->add_source(
    context,
    "https://registry.skylet.ai"
);
if (result.result != PluginResult::Success) {
    fprintf(stderr, "Error: %s\n", result.error_message);
}
```

#### remove_source(context, url)
Remove a registry source.

**Parameters:**
- `context`: Plugin context
- `url`: Registry source URL to remove

**Returns:** `RegistryResult`
- `result`: Status code
- `error_message`: Error message if failed

**Example:**
```c
RegistryResult result = registry_service->remove_source(
    context,
    "https://old-registry.skylet.ai"
);
```

#### search(context, query)
Search for plugins in the registry.

**Parameters:**
- `context`: Plugin context
- `query`: Search query string

**Returns:** `RegistrySearchResult`
- `result`: Status code
- `results`: Array of matching plugin names
- `result_count`: Number of results
- `error_message`: Error message if failed

**Example:**
```c
RegistrySearchResult search_result = registry_service->search(
    context,
    "database"
);
if (search_result.result == PluginResult::Success) {
    for (size_t i = 0; i < search_result.result_count; i++) {
        printf("Found: %s\n", search_result.results[i]);
    }
}
```

#### get_plugin(context, name)
Get details about a specific plugin.

**Parameters:**
- `context`: Plugin context
- `name`: Plugin name

**Returns:** `RegistryGetResult`
- `result`: Status code
- `plugin_json`: Plugin metadata as JSON string
- `error_message`: Error message if failed

**Example:**
```c
RegistryGetResult get_result = registry_service->get_plugin(
    context,
    "database/my-plugin"
);
if (get_result.result == PluginResult::Success) {
    printf("Plugin info: %s\n", get_result.plugin_json);
}
```

MCP Tools
---------

The registry plugin provides the following MCP tools:

### registry_list
Lists all plugins in the configured registries.

```bash
mcp_call registry_list
```

### registry_add_source
Add a new registry source.

```bash
mcp_call registry_add_source url="https://registry.example.com"
```

### registry_remove_source
Remove a registry source.

```bash
mcp_call registry_remove_source url="https://registry.example.com"
```

### registry_search
Search for plugins by query.

```bash
mcp_call registry_search query="database"
```

### registry_get_plugin
Get details about a specific plugin.

```bash
mcp_call registry_get_plugin name="vendor/plugin-name"
```

Data Flow
---------

The registry plugin follows this data flow:

1. **Plugin Initialization** (`plugin_init`)
   - Creates RegistryService with function pointers
   - Registers service in the global service registry
   - Initializes async runtime for async operations

2. **Registry Operations**
   - Service calls route through the C FFI boundary
   - Each operation validates inputs and converts C types
   - Results are returned as C structures with ownership semantics

3. **Plugin Shutdown** (`plugin_shutdown`)
   - Cleanup and resource release
   - Graceful shutdown of async runtime

Configuration
--------------

Registry sources are configured via the Skylet configuration system:

```toml
[registry]
sources = [
    "https://registry.skylet.ai",
    "https://community-registry.skylet.ai"
]

# Optional: cache settings
cache_dir = "/var/cache/skylet/registry"
cache_ttl_secs = 3600
```

Error Handling
--------------

The registry plugin returns standard `PluginResult` codes:

- `Success (0)`: Operation succeeded
- `Error (-1)`: Generic error
- `InvalidRequest (-2)`: Invalid input parameters
- `ServiceUnavailable (-3)`: Registry service not available
- `PermissionDenied (-4)`: User lacks permission
- `NotImplemented (-5)`: Feature not implemented
- `Timeout (-6)`: Operation timed out
- `ResourceExhausted (-7)`: Resource limit reached

Testing
-------

Run the test suite:

```bash
cargo test -p registry
```

Integration Testing
-------------------

Test the plugin integration:

```bash
# Build the plugin
cargo build -p registry --release

# Load in Skylet core
./skylet --plugin registry:target/release/libregistry.so
```

Performance Considerations
--------------------------

- Registry queries are cached to reduce network load
- Async I/O prevents blocking on network calls
- Connection pooling for registry sources
- Incremental result streaming for large result sets

Future Enhancements
-------------------

- [ ] Persistent storage backend (SQLite/PostgreSQL)
- [ ] Federated search across multiple sources
- [ ] Plugin dependency resolution
- [ ] Version constraint solving
- [ ] Plugin signature verification
- [ ] Local mirror support
- [ ] Analytics and usage tracking
- [ ] Plugin recommendation engine

Dependencies
------------

The registry plugin depends on:

- `skylet-abi`: Skylet plugin ABI
- `marketplace-registry`: Core registry implementation
- `tokio`: Async runtime
- `serde_json`: JSON serialization
- `anyhow`: Error handling

License
-------

MIT OR Apache-2.0 License - See LICENSE file for details

Contributing
------------

1. Create a feature branch: `git checkout -b feature/my-feature`
2. Commit your changes: `git commit -am 'Add feature'`
3. Push to the branch: `git push origin feature/my-feature`
4. Submit a pull request

Support
-------

For issues, questions, or contributions, please open an issue on GitHub or reach out to the Skylet team.
