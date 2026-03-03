# Registry Plugin - API Reference

## Plugin Overview

| Property | Value |
|----------|-------|
| **Plugin Name** | registry |
| **Version** | 0.1.0 |
| **Author** | Skylet |
| **License** | MIT OR Apache-2.0 |
| **ABI Version** | 1 |
| **Plugin Type** | Infrastructure |
| **Supports Async** | True |
| **Supports Hot Reload** | False |
| **Supports Streaming** | False |
| **Max Concurrency** | 10 |

### Key Capabilities
- Federated plugin registry for managing multiple plugin sources
- Plugin discovery and search functionality across sources
- Plugin version resolution with semantic versioning support
- Registry source management (add/remove sources)
- Service-based architecture via plugin ABI
- Async I/O for non-blocking registry operations
- Caching support for improved performance
- Connection pooling for registry sources

---

## FFI Functions

### plugin_init

Initializes the registry plugin and sets up the registry service.

**Signature:**
```c
PluginResult plugin_init(const PluginContext *context);
```

**Parameters:**
- `context` (const PluginContext*): Plugin execution context

**Return Value:**
- `PluginResult::Success`: Initialization successful
- `PluginResult::Error`: Async runtime creation failed
- `PluginResult::InvalidRequest`: Context is null

**Description:**
Initializes the async runtime, creates the RegistryService with function pointers, and registers it in the plugin context. This is called once during plugin lifecycle startup.

**Example - C:**
```c
#include <skylet_abi.h>

PluginResult init_registry(const PluginContext *ctx) {
    PluginResult result = plugin_init(ctx);
    if (result != PluginResult_Success) {
        fprintf(stderr, "Failed to initialize registry plugin\n");
        return result;
    }
    printf("Registry plugin initialized\n");
    return PluginResult_Success;
}
```

**Example - Python:**
```python
import ctypes
from skylet_abi import PluginContext, PluginResult

def init_registry(context_ptr):
    """Initialize the registry plugin"""
    result = lib.plugin_init(context_ptr)
    
    if result == PluginResult.Success:
        print("Registry plugin initialized successfully")
    else:
        print(f"Plugin initialization failed: {result}")
    
    return result

# Usage
context = create_plugin_context()
status = init_registry(context)
```

**Example - Rust:**
```rust
use skylet_abi::{PluginContext, PluginResult};

pub fn init_registry(context: *const PluginContext) -> PluginResult {
    if context.is_null() {
        eprintln!("Failed: context is null");
        return PluginResult::InvalidRequest;
    }
    
    unsafe {
        match plugin_init(context) {
            PluginResult::Success => {
                println!("Registry plugin initialized");
                PluginResult::Success
            }
            err => {
                eprintln!("Initialization error: {:?}", err);
                err
            }
        }
    }
}
```

---

### plugin_shutdown

Cleans up registry resources and prepares for unload.

**Signature:**
```c
PluginResult plugin_shutdown(const PluginContext *context);
```

**Parameters:**
- `context` (const PluginContext*): Plugin execution context

**Return Value:**
- `PluginResult::Success`: Shutdown completed successfully

**Description:**
Releases any held resources, clears the cached plugin context, and shuts down the async runtime. Called during plugin lifecycle shutdown.

---

### plugin_get_info

Retrieves plugin metadata and capabilities information.

**Signature:**
```c
const PluginInfo* plugin_get_info(void);
```

**Return Value:**
- Pointer to PluginInfo structure containing:
  - name: "registry"
  - version: "0.1.0"
  - abi_version: "1"
  - description: "Federated plugin registry for discovery and management"
  - plugin_type: Infrastructure
  - max_concurrency: 10
  - supports_async: true

---

## Service Interface

### RegistryService Structure

The registry plugin exposes a service interface with the following function pointers:

```c
typedef struct {
    RegistryListResult (*list_plugins)(const PluginContext *context);
    RegistryResult (*add_source)(const PluginContext *context, const char *url);
    RegistryResult (*remove_source)(const PluginContext *context, const char *url);
    RegistrySearchResult (*search)(const PluginContext *context, const char *query);
    RegistryGetResult (*get_plugin)(const PluginContext *context, const char *name);
} RegistryService;
```

---

## Service Methods

### list_plugins

Lists all registered plugins across all configured sources.

**Signature:**
```c
RegistryListResult list_plugins(const PluginContext *context);
```

**Return Structure:**
```c
typedef struct {
    PluginResult result;           // PluginResult::Success or error code
    const char** plugins;          // Array of plugin names
    size_t plugin_count;           // Number of plugins
    const char* error_message;     // Error message if failed
} RegistryListResult;
```

**Returns:**
- `plugins`: Dynamically allocated array of plugin name strings
- `plugin_count`: Total number of plugins found
- `error_message`: NULL on success, error description on failure

**Example - C:**
```c
RegistryListResult list_result = registry_service->list_plugins(context);
if (list_result.result == PluginResult_Success) {
    printf("Found %zu plugins:\n", list_result.plugin_count);
    for (size_t i = 0; i < list_result.plugin_count; i++) {
        printf("  - %s\n", list_result.plugins[i]);
    }
} else {
    fprintf(stderr, "Error: %s\n", list_result.error_message);
}
```

**Example - Python:**
```python
def list_plugins(registry_service, context):
    """List all registered plugins"""
    result = registry_service.list_plugins(context)
    
    if result.result == PluginResult.Success:
        plugins = []
        for i in range(result.plugin_count):
            plugins.append(result.plugins[i].decode('utf-8'))
        
        print(f"Found {len(plugins)} plugins:")
        for plugin in plugins:
            print(f"  - {plugin}")
        
        return plugins
    else:
        print(f"Error: {result.error_message}")
        return []

# Usage
plugins = list_plugins(service, context)
```

**Example - Rust:**
```rust
pub fn list_plugins(
    service: &RegistryService,
    context: *const PluginContext,
) -> Result<Vec<String>> {
    unsafe {
        let result = (service.list_plugins)(context);
        
        if result.result == PluginResult::Success {
            let mut plugins = Vec::new();
            let slice = std::slice::from_raw_parts(result.plugins, result.plugin_count);
            
            for &plugin_ptr in slice {
                let plugin_name = std::ffi::CStr::from_ptr(plugin_ptr)
                    .to_string_lossy()
                    .into_owned();
                plugins.push(plugin_name);
            }
            
            println!("Found {} plugins", plugins.len());
            Ok(plugins)
        } else {
            let error = std::ffi::CStr::from_ptr(result.error_message)
                .to_string_lossy()
                .into_owned();
            Err(format!("Failed to list plugins: {}", error).into())
        }
    }
}
```

---

### add_source

Add a new registry source to the federation.

**Signature:**
```c
RegistryResult add_source(const PluginContext *context, const char *url);
```

**Parameters:**
- `context` (const PluginContext*): Plugin context
- `url` (const char*): Registry source URL (e.g., "https://registry.example.com")

**Return Structure:**
```c
typedef struct {
    PluginResult result;           // Success or error code
    const char* error_message;     // Error message if failed
} RegistryResult;
```

**Example - C:**
```c
RegistryResult result = registry_service->add_source(
    context,
    "https://registry.example.com"
);
if (result.result != PluginResult_Success) {
    fprintf(stderr, "Error: %s\n", result.error_message);
}
```

**Example - Python:**
```python
def add_registry_source(registry_service, context, url):
    """Add a new registry source"""
    result = registry_service.add_source(context, url.encode('utf-8'))
    
    if result.result == PluginResult.Success:
        print(f"✓ Added registry source: {url}")
    else:
        print(f"✗ Error: {result.error_message}")
    
    return result.result == PluginResult.Success

# Usage
add_registry_source(service, context, "https://registry.example.com")
add_registry_source(service, context, "https://community-registry.example.com")
```

**Example - Rust:**
```rust
pub fn add_registry_source(
    service: &RegistryService,
    context: *const PluginContext,
    url: &str,
) -> Result<()> {
    let c_url = std::ffi::CString::new(url)?;
    
    unsafe {
        let result = (service.add_source)(context, c_url.as_ptr());
        
        if result.result == PluginResult::Success {
            println!("✓ Added registry source: {}", url);
            Ok(())
        } else {
            let error = std::ffi::CStr::from_ptr(result.error_message)
                .to_string_lossy()
                .into_owned();
            Err(format!("Failed to add source: {}", error).into())
        }
    }
}
```

---

### remove_source

Remove a registry source from the federation.

**Signature:**
```c
RegistryResult remove_source(const PluginContext *context, const char *url);
```

**Parameters:**
- `context` (const PluginContext*): Plugin context
- `url` (const char*): Registry source URL to remove

**Example - C:**
```c
RegistryResult result = registry_service->remove_source(
    context,
    "https://old-registry.example.com"
);
```

**Example - Python:**
```python
def remove_registry_source(registry_service, context, url):
    """Remove a registry source"""
    result = registry_service.remove_source(context, url.encode('utf-8'))
    
    if result.result == PluginResult.Success:
        print(f"✓ Removed registry source: {url}")
    else:
        print(f"✗ Error: {result.error_message}")
    
    return result.result == PluginResult.Success

# Remove a source
remove_registry_source(service, context, "https://old-registry.example.com")
```

**Example - Rust:**
```rust
pub fn remove_registry_source(
    service: &RegistryService,
    context: *const PluginContext,
    url: &str,
) -> Result<()> {
    let c_url = std::ffi::CString::new(url)?;
    
    unsafe {
        let result = (service.remove_source)(context, c_url.as_ptr());
        
        if result.result == PluginResult::Success {
            println!("✓ Removed registry source: {}", url);
            Ok(())
        } else {
            let error = std::ffi::CStr::from_ptr(result.error_message)
                .to_string_lossy()
                .into_owned();
            Err(format!("Failed to remove source: {}", error).into())
        }
    }
}
```

---

### search

Search for plugins in the registry by query.

**Signature:**
```c
RegistrySearchResult search(const PluginContext *context, const char *query);
```

**Parameters:**
- `context` (const PluginContext*): Plugin context
- `query` (const char*): Search query (plugin name, keyword, description)

**Return Structure:**
```c
typedef struct {
    PluginResult result;           // Success or error code
    const char** results;          // Array of matching plugin names
    size_t result_count;           // Number of results
    const char* error_message;     // Error message if failed
} RegistrySearchResult;
```

**Example - C:**
```c
RegistrySearchResult search_result = registry_service->search(
    context,
    "database"
);
if (search_result.result == PluginResult_Success) {
    printf("Found %zu database-related plugins:\n", search_result.result_count);
    for (size_t i = 0; i < search_result.result_count; i++) {
        printf("  - %s\n", search_result.results[i]);
    }
}
```

**Example - Python:**
```python
def search_plugins(registry_service, context, query):
    """Search for plugins by keyword"""
    result = registry_service.search(context, query.encode('utf-8'))
    
    if result.result == PluginResult.Success:
        plugins = []
        for i in range(result.result_count):
            plugin_name = result.results[i].decode('utf-8')
            plugins.append(plugin_name)
        
        print(f"Found {len(plugins)} plugins matching '{query}':")
        for plugin in plugins:
            print(f"  - {plugin}")
        
        return plugins
    else:
        print(f"Error: {result.error_message}")
        return []

# Search for plugins
search_plugins(service, context, "database")
search_plugins(service, context, "http")
search_plugins(service, context, "networking")
```

**Example - Rust:**
```rust
pub fn search_plugins(
    service: &RegistryService,
    context: *const PluginContext,
    query: &str,
) -> Result<Vec<String>> {
    let c_query = std::ffi::CString::new(query)?;
    
    unsafe {
        let result = (service.search)(context, c_query.as_ptr());
        
        if result.result == PluginResult::Success {
            let mut plugins = Vec::new();
            let slice = std::slice::from_raw_parts(result.results, result.result_count);
            
            for &plugin_ptr in slice {
                let plugin_name = std::ffi::CStr::from_ptr(plugin_ptr)
                    .to_string_lossy()
                    .into_owned();
                plugins.push(plugin_name);
            }
            
            println!("Found {} plugins matching '{}'", plugins.len(), query);
            Ok(plugins)
        } else {
            let error = std::ffi::CStr::from_ptr(result.error_message)
                .to_string_lossy()
                .into_owned();
            Err(format!("Search failed: {}", error).into())
        }
    }
}
```

---

### get_plugin

Get detailed information about a specific plugin.

**Signature:**
```c
RegistryGetResult get_plugin(const PluginContext *context, const char *name);
```

**Parameters:**
- `context` (const PluginContext*): Plugin context
- `name` (const char*): Plugin name or fully qualified ID (e.g., "vendor/plugin-name")

**Return Structure:**
```c
typedef struct {
    PluginResult result;           // Success or error code
    const char* plugin_json;       // Plugin metadata as JSON string
    const char* error_message;     // Error message if failed
} RegistryGetResult;
```

**Response JSON Format:**
```json
{
  "name": "my-plugin",
  "version": "1.0.0",
  "description": "Plugin description",
  "author": "Plugin Author",
  "license": "MIT OR Apache-2.0",
  "homepage": "https://example.com",
  "repository": {
    "type": "git",
    "url": "https://github.com/example/my-plugin.git"
  },
  "keywords": ["plugin", "example"],
  "downloads": 1234,
  "rating": 4.5
}
```

**Example - C:**
```c
RegistryGetResult get_result = registry_service->get_plugin(
    context,
    "skylet/database-plugin"
);
if (get_result.result == PluginResult_Success) {
    printf("Plugin info: %s\n", get_result.plugin_json);
} else {
    fprintf(stderr, "Error: %s\n", get_result.error_message);
}
```

**Example - Python:**
```python
import json

def get_plugin_info(registry_service, context, plugin_name):
    """Get detailed plugin information"""
    result = registry_service.get_plugin(context, plugin_name.encode('utf-8'))
    
    if result.result == PluginResult.Success:
        plugin_json = result.plugin_json.decode('utf-8')
        plugin_data = json.loads(plugin_json)
        
        print(f"Plugin: {plugin_data['name']}")
        print(f"Version: {plugin_data['version']}")
        print(f"Description: {plugin_data['description']}")
        print(f"Author: {plugin_data.get('author', 'Unknown')}")
        print(f"License: {plugin_data.get('license', 'Unknown')}")
        
        if 'keywords' in plugin_data:
            print(f"Keywords: {', '.join(plugin_data['keywords'])}")
        
        return plugin_data
    else:
        print(f"Error: {result.error_message}")
        return None

# Get plugin details
plugin = get_plugin_info(service, context, "skylet/database-plugin")
```

**Example - Rust:**
```rust
pub fn get_plugin_info(
    service: &RegistryService,
    context: *const PluginContext,
    name: &str,
) -> Result<serde_json::Value> {
    let c_name = std::ffi::CString::new(name)?;
    
    unsafe {
        let result = (service.get_plugin)(context, c_name.as_ptr());
        
        if result.result == PluginResult::Success {
            let json_str = std::ffi::CStr::from_ptr(result.plugin_json)
                .to_string_lossy()
                .into_owned();
            
            let plugin_data: serde_json::Value = serde_json::from_str(&json_str)?;
            
            if let Some(version) = plugin_data.get("version") {
                println!("Plugin: {} v{}", name, version);
            }
            
            Ok(plugin_data)
        } else {
            let error = std::ffi::CStr::from_ptr(result.error_message)
                .to_string_lossy()
                .into_owned();
            Err(format!("Failed to get plugin: {}", error).into())
        }
    }
}
```

---

## MCP Tools

The registry plugin provides the following MCP tools for command-line usage:

### registry_list

Lists all plugins in the configured registries.

**Command:**
```bash
mcp_call registry_list
```

**Example Output:**
```json
{
  "plugins": [
    "skylet/database-plugin",
    "skylet/http-server",
    "community/redis-integration",
    "vendor/custom-service"
  ],
  "total_count": 4
}
```

---

### registry_add_source

Add a new registry source.

**Command:**
```bash
mcp_call registry_add_source url="https://registry.example.com"
```

**Parameters:**
- `url` (string): Full URL to the registry source

---

### registry_remove_source

Remove a registry source.

**Command:**
```bash
mcp_call registry_remove_source url="https://old-registry.example.com"
```

---

### registry_search

Search for plugins by query.

**Command:**
```bash
mcp_call registry_search query="database"
```

**Parameters:**
- `query` (string): Search keyword or phrase

**Example Output:**
```json
{
  "results": [
    "skylet/postgres-plugin",
    "skylet/redis-plugin",
    "vendor/mongodb-integration"
  ],
  "result_count": 3
}
```

---

### registry_get_plugin

Get details about a specific plugin.

**Command:**
```bash
mcp_call registry_get_plugin name="skylet/postgres-plugin"
```

**Parameters:**
- `name` (string): Plugin name or fully qualified ID

---

## Configuration

### Registry Sources Configuration

Registry sources are configured via the Skylet configuration system:

```toml
[registry]
sources = [
    "https://registry.example.com",
    "https://community-registry.example.com",
    "https://vendor-registry.example.com"
]

# Optional: cache settings
cache_dir = "/var/cache/skylet/registry"
cache_ttl_secs = 3600
```

### Network Access

The plugin requires network access to:
- All configured registry source URLs
- Must support HTTPS for secure communication

---

## Error Handling

### Error Codes

| Code | Name | Description | Resolution |
|------|------|-------------|------------|
| 0 | Success | Operation completed successfully | N/A |
| -1 | Error | Generic error | Check error_message for details |
| -2 | InvalidRequest | Invalid input parameters | Verify parameter values and types |
| -3 | ServiceUnavailable | Registry service not available | Check plugin initialization |
| -4 | PermissionDenied | User lacks permission | Check user permissions |
| -5 | NotImplemented | Feature not implemented | Use supported features |
| -6 | Timeout | Operation timed out | Increase timeout or check network |
| -7 | ResourceExhausted | Resource limit reached | Reduce load or increase limits |

---

## Performance Characteristics

### Latency
- **List plugins**: 100ms - 2s (depends on number of sources)
- **Search plugins**: 100ms - 3s (depends on result set size)
- **Get plugin info**: 50ms - 500ms
- **Add/remove source**: 50ms (local operation)

### Throughput
- **Max concurrent requests**: 10
- **Registry queries**: Up to 50 queries/second with caching
- **Async operations**: Non-blocking I/O

### Caching
- **Cache TTL**: Configurable (default: 3600 seconds/1 hour)
- **Cache invalidation**: On source changes or manual refresh
- **Incremental result streaming**: For large result sets

---

## Version History

### v0.1.0 (Current)
- Initial release
- Federated registry support
- Plugin discovery and search
- Version resolution
- Registry source management
- Async I/O operations
- Caching support
- Connection pooling

### Planned Features (v0.2.0)
- Persistent storage backend (SQLite/PostgreSQL)
- Federated search across multiple sources
- Plugin dependency resolution
- Version constraint solving
- Plugin signature verification
- Local mirror support
- Analytics and usage tracking
- Plugin recommendation engine
