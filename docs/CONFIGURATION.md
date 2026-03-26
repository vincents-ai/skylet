# Advanced Configuration Management

## Overview

The Skylet Configuration Management system provides comprehensive plugin configuration capabilities with advanced features including:

- **Advanced Schema Validation**: JSON Schema-based validation with custom validators
- **Environment Variable Integration**: Seamlessly merge environment variables into configuration
- **Configuration Hot-Reload**: Automatic reloading when config files change
- **Multi-Environment Support**: Separate configurations for dev, staging, and production

## Architecture

### Module Structure

```
src/plugin_manager/config/
├── mod.rs           # Main configuration backend and types
├── schema.rs        # Schema validation system
├── env_integration.rs # Environment variable integration
├── hot_reload.rs    # Configuration hot-reload with file watching
└── multi_env.rs     # Multi-environment configuration management
```

### Core Components

#### 1. Advanced Config Backend (`mod.rs`)

The `AdvancedConfigBackend` provides the main configuration management interface:

```rust
use plugin_manager::config::{AdvancedConfigBackend, ConfigEnvironment};

// Create configuration backend
let config = AdvancedConfigBackend::new(PathBuf::from("./config"))
    .with_environment(ConfigEnvironment::Development)
    .with_env_prefix("SKYLET_".to_string());

// Load plugin configuration
let config_json = config.load_plugin_config("my_plugin").await?;

// Get configuration value
let value = config.get_config_value("my_plugin", "api_key").await?;

// Set configuration value (example value only - never use real secrets)
config.set_config_value("my_plugin", "api_key", "secret123").await?;
```

#### 2. Schema Validation (`schema.rs`)

The `SchemaValidator` provides comprehensive configuration validation:

```rust
use plugin_manager::config::schema::{SchemaValidator, SchemaProperty, SchemaType};

// Create validator from JSON schema
let schema_json = r#"{
    "properties": {
        "api_key": {
            "type": "string",
            "required": true,
            "minLength": 32,
            "maxLength": 256
        },
        "timeout": {
            "type": "integer",
            "minimum": 1,
            "maximum": 3600
        }
    },
    "required": ["api_key"]
}"#;

let validator = SchemaValidator::from_json(schema_json)?;
let result = validator.validate(config_json)?;

if !result.is_valid {
    for error in result.errors {
        eprintln!("Validation error at {}: {}", error.path, error.message);
    }
}
```

#### 3. Environment Variable Integration (`env_integration.rs`)

The `EnvVarIntegrator` provides seamless environment variable merging:

```rust
use plugin_manager::config::env_integration::{EnvVarConfig, EnvVarIntegrator};

// Create environment integrator
let env_config = EnvVarConfig::new("SKYLET_".to_string())
    .with_separator("_".to_string())
    .with_overwrite_files(true);

let integrator = EnvVarIntegrator::new(env_config);

// Merge environment variables into config
let merged_config = integrator.merge_into_config("my_plugin", config_json)?;

// Environment variable references in config
// ${env:API_URL:http://localhost:8080}
// ${env:API_TOKEN}
```

#### 4. Configuration Hot-Reload (`hot_reload.rs`)

The `ConfigHotReload` provides automatic configuration reloading:

```rust
use plugin_manager::config::hot_reload::{ConfigHotReload, ReloadConfig};

// Create hot-reload service
let reload_config = ReloadConfig {
    enabled: true,
    debounce_duration: Duration::from_millis(500),
    validate_after_reload: true,
    backup_on_failure: true,
    max_retries: 3,
};

let hot_reload = ConfigHotReload::new(config_dir, reload_config)?;

// Add reload callback
hot_reload.add_reload_callback(Arc::new(|event| {
    println!("Config reloaded: {:?}", event);
})).await;
```

#### 5. Multi-Environment Support (`multi_env.rs`)

The `MultiEnvConfigManager` provides environment-specific configuration:

```rust
use plugin_manager::config::multi_env::{MultiEnvConfigManager, EnvironmentConfig};

// Create multi-environment manager
let manager = MultiEnvConfigManager::new(PathBuf::from("./config"))
    .with_environment(ConfigEnvironment::Development)?;

// Load environment-specific config
let config = manager.load_plugin_config("my_plugin").await?;

// Switch environments
manager.switch_environment(ConfigEnvironment::Production)?;

// Compare configurations across environments
let comparison = manager.compare_configs_for_plugin("my_plugin");
for diff in comparison.differences {
    println!("{} differs: {}={:?} vs {}={:?}",
        diff.key, diff.env1, diff.value1, diff.env2, diff.value2);
}
```

## Configuration File Formats

### TOML Configuration

Plugin configurations are stored in TOML format:

```toml
# config/my_plugin.toml
name = "My Plugin"
version = "1.0.0"

[database]
host = "localhost"
port = 5432
username = "admin"

[database.credentials]
password = "${env:DB_PASSWORD}"

[cache]
enabled = true
ttl_seconds = 3600
```

### JSON Schema

Plugins can provide JSON Schema for configuration validation:

```json
{
    "$schema": "http://json-schema.org/draft-07/schema#",
    "title": "My Plugin Configuration",
    "type": "object",
    "properties": {
        "database": {
            "type": "object",
            "properties": {
                "host": {
                    "type": "string",
                    "default": "localhost"
                },
                "port": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 65535,
                    "default": 5432
                }
            },
            "required": ["host"]
        }
    }
}
```

## Environment Variable Integration

### Automatic Mapping

Environment variables are automatically mapped to configuration keys:

```bash
# Maps to my_plugin.api_key
export SKYLET_MYPLUGIN_API_KEY="secret123"

# Maps to my_plugin.database.host
export SKYLET_MYPLUGIN_DATABASE_HOST="localhost"
```

### Custom Mappings

Custom environment variable mappings can be configured:

```rust
env_config.add_mapping("api_key".to_string(), "MY_CUSTOM_API_KEY".to_string());
```

### Environment Variable References

Configuration files can reference environment variables:

```toml
# Using default value
api_url = "${env:API_URL:http://localhost:8080}"

# Required environment variable
api_token = "${env:API_TOKEN}"

# Multiple levels
[database]
password = "${env:DB_PASSWORD}"
```

## Hot-Reload Configuration

### Reload Events

When configuration files change, reload events are generated:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigReloadEvent {
    pub plugin_name: String,
    pub config_path: PathBuf,
    pub timestamp: DateTime<Utc>,
    pub reload_status: ReloadStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadStatus {
    Started,
    Success,
    Failed(String),
    Skipped,
}
```

### Debouncing

File system events are debounced to prevent excessive reloads:

```rust
let reload_config = ReloadConfig {
    debounce_duration: Duration::from_millis(500),
    // ... other config
};
```

## Multi-Environment Configuration

### Directory Structure

Environment-specific configurations are organized by environment:

```
config/
├── dev/
│   ├── plugin1.toml
│   └── plugin2.toml
├── staging/
│   ├── plugin1.toml
│   └── plugin2.toml
├── prod/
│   ├── plugin1.toml
│   └── plugin2.toml
└── common/
    └── plugin1.toml
```

### Configuration Hierarchy

Configuration is loaded in this priority order:

1. Environment-specific file (e.g., `dev/plugin1.toml`)
2. Common file (e.g., `common/plugin1.toml`)
3. Environment variables
4. Default values

### Environment Comparison

Compare configurations across environments:

```rust
let comparison = manager.compare_configs_for_plugin("my_plugin");

println!("Differences found: {}", comparison.differences.len());
for diff in comparison.differences {
    println!("  {}: {}={:?} vs {}={:?}",
        diff.key, diff.env1, diff.value1, diff.env2, diff.value2);
}
```

## Plugin Integration

### Loading Configuration

Plugins can load their configuration during initialization:

```rust
pub async fn init(context: *const PluginContextV2) -> PluginResultV2 {
    // Load configuration from backend
    let config_json = load_plugin_config_from_toml("my_plugin")?;

    // Validate against schema
    let validator = SchemaValidator::from_json(schema_json)?;
    let result = validator.validate(&serde_json::to_string(&config_json)?)?;

    if !result.is_valid {
        log_config_errors(&result.errors);
    }

    PluginResultV2::Success
}
```

### Configuration Schema

Plugins can export their configuration schema:

```c
const char* plugin_get_config_schema_v2() {
    return R"({
        "type": "object",
        "properties": {
            "enabled": {
                "type": "boolean",
                "default": true
            }
        }
    })";
}
```

## Testing

### Unit Tests

Each module includes comprehensive unit tests:

```bash
cargo test -p execution-engine --lib plugin_manager::config
```

### Integration Tests

Integration tests cover end-to-end scenarios:

```bash
cargo test -p execution-engine --test config_integration
```

## Performance Considerations

### Caching

Configuration values are cached in memory for fast access:

- Configurations are loaded once and cached
- Hot-reload updates the cache atomically
- Environment variables are resolved on first access

### Debouncing

File system events are debounced to reduce reload frequency:

- Default debounce: 500ms
- Configurable per-plugin
- Prevents cascading reloads

### Validation

Schema validation is optional and can be disabled:

```rust
let reload_config = ReloadConfig {
    validate_after_reload: false,
    // ...
};
```

## Security Considerations

### Secret Management

Sensitive values should use environment variables:

```toml
# ⚠️ WARNING: Never hardcode real secrets in config files!
# The example below shows what NOT to do:
api_key = "secret123"  # ❌ BAD: Example only - never commit real secrets

# ✅ GOOD: Use environment variable references instead:
api_key = "${env:API_KEY}"
```

### Validation

Always validate configuration:

```rust
let result = validator.validate(config_json)?;
if !result.is_valid {
    return Err(anyhow!("Invalid configuration"));
}
```

### Environment Isolation

Use separate environments for different deployment stages:

```bash
# Development
SKYLET_ENV=development

# Production
SKYLET_ENV=production
```

## Migration Guide

### From Simple Config

Migrating from simple configuration:

1. **Add JSON Schema**: Create a schema for your plugin
2. **Enable Validation**: Update plugin to validate config
3. **Add Environment Variables**: Convert secrets to env vars
4. **Enable Hot-Reload**: Add reload callbacks if needed

### Example Migration

Before:

```rust
let config = load_toml("config.toml")?;
```

After:

```rust
use plugin_manager::config::{AdvancedConfigBackend, ConfigEnvironment};

let backend = AdvancedConfigBackend::new(PathBuf::from("./config"))
    .with_environment(ConfigEnvironment::Development);

let config = backend.load_plugin_config("my_plugin").await?;

let validator = SchemaValidator::from_json(schema_json)?;
let result = validator.validate(&serde_json::to_string(&config)?)?;

if !result.is_valid {
    handle_validation_errors(result.errors);
}
```

## Troubleshooting

### Common Issues

#### Configuration Not Reloading

- Check file permissions on config directory
- Verify hot-reload is enabled
- Check debounce duration

#### Environment Variables Not Working

- Verify environment variable prefix
- Check for typos in variable names
- Ensure env vars are exported, not just set

#### Validation Failing

- Check schema syntax
- Verify configuration structure
- Check data types match schema

### Debug Logging

Enable debug logging for configuration:

```bash
RUST_LOG=plugin_manager::config=debug cargo run
```

## Future Enhancements

Planned features for configuration management:

1. **Configuration Versioning**: Track configuration changes over time
2. **Rollback Support**: Revert to previous configuration versions
3. **Configuration Templates**: Support for template-based configs
4. **Configuration Encryption**: Encrypt sensitive configuration values
5. **Configuration UI**: Web UI for configuration management
6. **Remote Configuration**: Fetch configuration from remote services
7. **Configuration Diffing**: Visual diff of configuration changes
8. **Configuration Merging**: Advanced merging strategies for multiple sources

## API Reference

### Types

- `AdvancedConfigBackend`: Main configuration backend
- `SchemaValidator`: Configuration schema validator
- `EnvVarIntegrator`: Environment variable integration
- `ConfigHotReload`: Configuration hot-reload service
- `MultiEnvConfigManager`: Multi-environment configuration manager
- `ConfigEnvironment`: Environment enumeration
- `ValidationResult`: Schema validation result
- `ConfigReloadEvent`: Configuration reload event

### Functions

See module documentation for detailed API references:

```rust
use plugin_manager::config;
```

## License

Configuration Management Module is part of Skylet and licensed under the MIT OR Apache-2.0 license.
