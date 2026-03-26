# Configuration Reference Guide

This guide documents the Skylet execution engine's configuration system for plugin developers.

## Overview

The configuration system provides:

- **Schema Definition**: Declare configuration fields with types, validation rules, and defaults
- **Type Safety**: Support for 14+ field types (string, integer, duration, path, email, etc.)
- **Validation**: Built-in validation rules (min/max, patterns, enums, custom)
- **Secret Management**: Reference secrets from vault, environment variables, or files
- **UI Generation**: Auto-generate UI components and JSON Schema from configuration definitions
- **Hot Reload**: Support for reloading plugin when configuration changes
- **Deprecation**: Mark fields as deprecated with migration guidance

## Quick Start

### 1. Define a Configuration Schema

```rust
use skylet_abi::config::{ConfigSchema, ConfigField, ConfigFieldType, ValidationRule};

let mut schema = ConfigSchema::new("my-plugin");

// Add a required string field
schema.add_field(ConfigField {
    name: "api_key".to_string(),
    label: Some("API Key".to_string()),
    description: Some("Your service API key".to_string()),
    field_type: ConfigFieldType::Secret,
    required: true,
    sensitive: true,
    default: None,
    validation: vec![
        ValidationRule::MinLength { value: 20 },
    ],
    ui_hints: None,
    secret_ref: Some(SecretReference {
        backend: SecretBackend::Vault,
        path: "secrets/my-plugin/api_key".to_string(),
    }),
    env_var: Some("MY_PLUGIN_API_KEY".to_string()),
    reload_on_change: false,
    deprecated: None,
});

// Add an optional integer field with range validation
schema.add_field(ConfigField {
    name: "max_retries".to_string(),
    label: Some("Max Retries".to_string()),
    description: Some("Maximum number of retry attempts".to_string()),
    field_type: ConfigFieldType::Integer,
    required: false,
    sensitive: false,
    default: Some(json!(3)),
    validation: vec![
        ValidationRule::Min { value: 1.0 },
        ValidationRule::Max { value: 10.0 },
    ],
    ui_hints: None,
    secret_ref: None,
    env_var: None,
    reload_on_change: false,
    deprecated: None,
});
```

### 2. Load Configuration

```rust
use skylet_abi::config::ConfigManager;
use std::path::Path;

let manager = ConfigManager::new();

// Register schema
manager.register_schema("my-plugin", schema);

// Load configuration from TOML file
manager.load_config("my-plugin", Path::new("config/my-plugin.toml"))?;

// Get configuration value
let api_key = manager.get_value("my-plugin", "api_key");

// Resolve secrets
manager.resolve_secrets()?;
```

### 3. Access Configuration in Plugin

```rust
use skylet_abi::v2_spec::{PluginContextV2, PluginResultV2};

// In your plugin_init_v2 function:
#[no_mangle]
pub extern "C" fn plugin_init_v2(context: *const PluginContextV2) -> PluginResultV2 {
    unsafe {
        let ctx = (*context);
        
        // Get configuration via service registry
        if let Some(config_service) = ctx.service_registry
            .get_service("config")
            .and_then(|s| s.downcast_ref::<ConfigService>()) {
            
            let config = config_service.get("my-plugin")?;
            let api_key = config.get_string("api_key")?;
            // Use api_key...
        }
    }
    PluginResultV2::Success
}
```

## Configuration Field Types

### Basic Types

#### String
Text value with optional validation rules.

```toml
[my-plugin]
name = "My Service"
description = "A longer description of the service"
```

```rust
ConfigFieldType::String
```

Validation rules: `MinLength`, `MaxLength`, `Pattern`

#### Integer
64-bit signed integer value.

```toml
[my-plugin]
port = 8080
max_connections = 100
```

```rust
ConfigFieldType::Integer
```

Validation rules: `Min`, `Max`

#### Float
64-bit floating point value.

```toml
[my-plugin]
timeout = 30.5
rate_limit = 0.95
```

```rust
ConfigFieldType::Float
```

Validation rules: `Min`, `Max`

#### Boolean
Boolean flag (true/false).

```toml
[my-plugin]
enabled = true
debug = false
ssl_verify = true
```

```rust
ConfigFieldType::Boolean
```

### Specialized Types

#### Secret
Sensitive value that references a secret backend.

```toml
[my-plugin]
api_key = "vault://secrets/my-plugin/api_key"
password = "env://DB_PASSWORD"
certificate = "file:///etc/ssl/cert.pem"
```

```rust
ConfigFieldType::Secret
```

Supported backends:
- `vault://path`: Read from Vault secret backend
- `env://VAR_NAME`: Read from environment variable
- `file:///path`: Read from file system

#### Duration
Human-readable duration format (e.g., "5s", "1h30m", "2.5h").

```toml
[my-plugin]
timeout = "30s"
cache_ttl = "5m"
session_timeout = "2h"
backoff_delay = "500ms"
```

```rust
ConfigFieldType::Duration
```

Supported formats:
- Milliseconds: `500ms`
- Seconds: `30s`
- Minutes: `5m`
- Hours: `1h`
- Combined: `1h30m`, `2h15m30s`

#### Port
Network port number (1-65535).

```toml
[my-plugin]
listen_port = 8080
http_port = 80
https_port = 443
```

```rust
ConfigFieldType::Port
```

Automatically validates range 1-65535.

#### Email
Email address with format validation.

```toml
[my-plugin]
admin_email = "admin@example.com"
support_email = "support@example.com"
```

```rust
ConfigFieldType::Email
```

Validates against RFC 5322 email format.

#### Hostname/IP Address
Hostname or IP address (IPv4 or IPv6).

```toml
[my-plugin]
database_host = "db.example.com"
redis_host = "localhost"
api_server = "192.168.1.10"
ipv6_address = "::1"
```

```rust
ConfigFieldType::Host
```

Accepts hostnames, IPv4 (192.168.1.1), and IPv6 (::1) addresses.

#### URL
Full URL with optional scheme restriction.

```toml
[my-plugin]
webhook_url = "https://webhook.example.com/notify"
api_endpoint = "https://api.service.com/v1"
```

```rust
ConfigFieldType::Url {
    schemes: vec!["https".to_string()],
}
```

Schemes: `http`, `https`, `ws`, `wss`, `ftp`, etc.

#### Path
File system path with existence checking.

```toml
[my-plugin]
data_dir = "/data/my-plugin"
config_file = "/etc/my-plugin/settings.json"
cert_path = "/ssl/cert.pem"
```

```rust
ConfigFieldType::Path {
    must_exist: false,
    is_dir: true,
}
```

Options:
- `must_exist`: Validate path exists
- `is_dir`: Validate it's a directory (vs file)

#### Array
Collection of values of the same type.

```toml
[my-plugin]
allowed_hosts = ["localhost", "127.0.0.1", "example.com"]
feature_flags = ["feature_a", "feature_b"]
backup_servers = ["backup1.example.com", "backup2.example.com"]
```

```rust
ConfigFieldType::Array(Box::new(ConfigFieldType::Host))
```

#### Object/Map
Key-value pairs (any JSON value).

```toml
[my-plugin.headers]
"Authorization" = "Bearer token"
"X-Custom-Header" = "value"

[my-plugin.routing]
"/path1" = "handler1"
"/path2" = "handler2"
```

```rust
ConfigFieldType::Object
```

#### Enum
Predefined set of allowed values.

```toml
[my-plugin]
log_level = "debug"
environment = "production"
mode = "strict"
```

```rust
ConfigFieldType::Enum {
    variants: vec![
        "debug".to_string(),
        "info".to_string(),
        "warn".to_string(),
        "error".to_string(),
    ],
}
```

## Validation Rules

### Min/Max
Validate numeric values are within range.

```rust
ValidationRule::Min { value: 1.0 }
ValidationRule::Max { value: 100.0 }
```

### MinLength/MaxLength
Validate string length.

```rust
ValidationRule::MinLength { value: 8 }
ValidationRule::MaxLength { value: 255 }
```

### Pattern
Validate string matches regex pattern.

```rust
ValidationRule::Pattern {
    regex: "^[a-zA-Z0-9_-]+$".to_string(),
}
```

### OneOf
Validate value is in allowed set.

```rust
ValidationRule::OneOf {
    values: vec![
        json!("debug"),
        json!("info"),
        json!("warn"),
    ],
}
```

### NotOneOf
Validate value is NOT in disallowed set.

```rust
ValidationRule::NotOneOf {
    values: vec![
        json!("localhost"),
        json!("127.0.0.1"),
    ],
}
```

### Custom
Custom validation with parameters.

```rust
ValidationRule::Custom {
    name: "is_valid_api_key".to_string(),
    params: json!({
        "min_entropy": 128,
        "allowed_chars": "alphanumeric+symbols",
    }).as_object().unwrap().clone(),
}
```

## Configuration File Format

Configuration files are TOML format with sections for each plugin.

### Basic Example

```toml
# /etc/skylet/plugins/my-plugin.toml

[my-plugin]
# Basic settings
enabled = true
name = "My Service"
debug = false

# Connection settings
api_key = "vault://secrets/my-plugin/api_key"
api_endpoint = "https://api.example.com"
timeout = "30s"

# Server settings
listen_port = 8080
max_connections = 100

# Features
features = ["feature_a", "feature_b"]

# Custom settings
[my-plugin.advanced]
cache_ttl = "5m"
retry_strategy = "exponential"
```

### Complete Example

```toml
# /etc/skylet/plugins/data-processor.toml

[data-processor]
# Plugin identification
enabled = true
name = "Data Processor Service"
version = "1.0.0"

# Secrets (loaded from vault, env, or files)
api_key = "vault://secrets/data-processor/api_key"
database_password = "env://DATA_PROCESSOR_DB_PASSWORD"
ssl_certificate = "file:///etc/ssl/data-processor/cert.pem"

# Logging
log_level = "info"
log_format = "json"

# Server configuration
listen_host = "0.0.0.0"
listen_port = 9090
ssl_enabled = true
ssl_verify_client = false

# Connection settings
connection_timeout = "30s"
read_timeout = "60s"
write_timeout = "60s"
idle_timeout = "5m"

# Performance
max_connections = 1000
worker_threads = 8
buffer_size = 65536

# Features
enabled_features = ["compression", "caching", "metrics"]
experimental_features = ["async_processing"]

# Database
[data-processor.database]
host = "postgres.example.com"
port = 5432
name = "data_processor"
ssl = true
pool_size = 20

# Cache
[data-processor.cache]
backend = "redis"
host = "redis.example.com"
port = 6379
ttl = "1h"
max_size = "1gb"

# Monitoring
[data-processor.monitoring]
enabled = true
metrics_port = 9091
trace_sample_rate = 0.1
```

## Environment Variables

Override configuration values with environment variables.

### Naming Convention

`PLUGIN_NAME_FIELD_NAME` (uppercase with underscores)

```bash
# Set via environment
export MY_PLUGIN_API_KEY="sk-..."
export MY_PLUGIN_DEBUG="true"
export MY_PLUGIN_MAX_RETRIES="5"
export MY_PLUGIN_TIMEOUT="60s"

# Run plugin
./my-plugin
```

### In Configuration Schema

```rust
ConfigField {
    name: "api_key".to_string(),
    env_var: Some("MY_PLUGIN_API_KEY".to_string()),
    // ... other fields
}
```

Priority order (highest to lowest):
1. Environment variables
2. Configuration file values
3. Default values

## Secret Management

### Secret Backends

#### Vault
Retrieve secrets from HashiCorp Vault.

```toml
[my-plugin]
api_key = "vault://secrets/my-plugin/api_key"
db_password = "vault://database/credentials"
```

Configuration required:
```bash
export VAULT_ADDR="https://vault.example.com"
export VAULT_TOKEN="s.xxxxx"
export VAULT_NAMESPACE="my-namespace"  # Optional
```

#### Environment Variables
Read from environment variables.

```toml
[my-plugin]
api_key = "env://MY_API_KEY"
database_url = "env://DATABASE_URL"
```

The environment variable name follows the reference:
```bash
export MY_API_KEY="sk-..."
```

#### File System
Read from local files (use with caution).

```toml
[my-plugin]
certificate = "file:///etc/ssl/cert.pem"
private_key = "file:///etc/ssl/key.pem"
```

### Secret Resolution

Secrets are resolved when the plugin initializes:

```rust
// In plugin code
let manager = ConfigManager::new();
manager.resolve_secrets()?;  // Replaces secret refs with actual values
```

### Best Practices

1. **Never commit secrets** to version control
2. **Use Vault** for production deployments
3. **Use environment variables** for local development
4. **Mark sensitive fields** with `sensitive: true`
5. **Restrict file permissions** on secret files (600)
6. **Audit secret access** in logs

## Hot Reload

Mark fields that should trigger plugin reload when changed:

```rust
ConfigField {
    name: "log_level".to_string(),
    reload_on_change: true,  // Reload plugin when this changes
    // ... other fields
}
```

When `reload_on_change: true`:
1. Configuration change is detected
2. Plugin receives `on_config_change` event
3. Plugin can reload settings without restart
4. Old connections may continue with old config

Plugins should implement graceful reload:

```rust
use skylet_abi::v2_spec::{PluginContextV2, PluginResultV2};

#[no_mangle]
pub extern "C" fn plugin_on_config_change(
    context: *const PluginContextV2,
    config_json: *const c_char,
) -> PluginResultV2 {
    unsafe {
        let config_str = CStr::from_ptr(config_json)
            .to_string_lossy()
            .to_string();
        
        // Parse new configuration
        let new_config: MyConfig = serde_json::from_str(&config_str)
            .map_err(|_| PluginResultV2::InvalidRequest)?;
        
        // Update internal state
        update_plugin_config(new_config);
        
        PluginResultV2::Success
    }
}
```

## Validation Examples

### Example 1: Email Service Plugin

```toml
[email-service]
enabled = true

# SMTP configuration
smtp_host = "smtp.example.com"
smtp_port = 587
smtp_username = "sender@example.com"
smtp_password = "vault://secrets/email/smtp_password"

# Email settings
from_email = "noreply@example.com"
reply_to = "support@example.com"
max_recipients = 100

# Features
enable_html = true
enable_attachments = true
enable_tracking = false

# Validation rules applied:
# - smtp_host: MinLength(3), Pattern for domain
# - smtp_port: Min(1), Max(65535)
# - from_email, reply_to: Email format
# - max_recipients: Min(1), Max(1000)
```

### Example 2: Database Plugin

```toml
[database]
enabled = true

# Connection
host = "postgres.example.com"
port = 5432
database = "myapp"
username = "app_user"
password = "vault://secrets/database/password"
ssl_mode = "require"

# Connection pool
min_connections = 5
max_connections = 100
connection_timeout = "10s"
idle_timeout = "5m"

# Performance
statement_cache_size = 10
prepared_statement_cache = true

# Validation rules applied:
# - host: Host format
# - port: Port range (1-65535)
# - database: MinLength(1), MaxLength(63)
# - min_connections: Min(1), Max(1000)
# - max_connections: Min(1), Max(10000)
# - ssl_mode: OneOf(disable, allow, prefer, require, verify-ca, verify-full)
```

### Example 3: API Gateway Plugin

```toml
[api-gateway]
enabled = true

# Server
listen_host = "0.0.0.0"
listen_port = 8080
tls_enabled = true
tls_cert = "file:///etc/ssl/cert.pem"
tls_key = "file:///etc/ssl/key.pem"

# Rate limiting
rate_limit_enabled = true
rate_limit_requests = 1000
rate_limit_window = "1m"

# CORS
cors_enabled = true
cors_origins = ["https://example.com", "https://app.example.com"]
cors_methods = ["GET", "POST", "PUT", "DELETE"]
cors_headers = ["Content-Type", "Authorization"]

# Auth
auth_enabled = true
auth_type = "jwt"
jwt_secret = "vault://secrets/api-gateway/jwt_secret"
jwt_issuer = "https://auth.example.com"

# Routing
[api-gateway.routes.internal]
path = "/internal/*"
auth_required = true

[api-gateway.routes.public]
path = "/public/*"
auth_required = false
```

## Common Patterns

### Development vs Production

Use environment variables to switch configuration:

```rust
// Plugin code
let env = env::var("ENVIRONMENT").unwrap_or("development".to_string());
let config_file = if env == "production" {
    "/etc/skylet/plugins/prod.toml"
} else {
    "/etc/skylet/plugins/dev.toml"
};
```

### Optional Features

Declare feature flags in configuration:

```toml
[my-plugin]
features = ["experimental_caching", "advanced_metrics"]
```

Check in plugin:

```rust
let config = get_config("my-plugin")?;
let features: Vec<String> = config.get_array("features")?;

if features.contains(&"experimental_caching".to_string()) {
    enable_caching();
}
```

### Nested Configuration

Use TOML table sections for nested configs:

```toml
[my-plugin]
name = "Service"

[my-plugin.database]
host = "db.example.com"
port = 5432

[my-plugin.cache]
host = "redis.example.com"
ttl = "1h"

[my-plugin.logging]
level = "info"
format = "json"
```

Access in plugin:

```rust
let db_host = config.get("database.host")?;
let cache_ttl = config.get("cache.ttl")?;
let log_level = config.get("logging.level")?;
```

## Troubleshooting

### Configuration File Not Found

```
Error: Failed to read file '/etc/skylet/plugins/my-plugin.toml': No such file or directory
```

Solution:
- Verify file path is correct
- Check file permissions (must be readable)
- Create file if missing: `touch /etc/skylet/plugins/my-plugin.toml`

### Validation Error: Invalid Value

```
Error: Validation error for plugin 'my-plugin': Value '9000' exceeds maximum '65535' for field 'port'
```

Solution:
- Check field type and validation rules
- Ensure value matches expected type
- Review schema definition for constraints

### Secret Resolution Failed

```
Error: Secret resolution error: Vault token invalid
```

Solution:
- Verify Vault is running and accessible
- Check `VAULT_TOKEN` and `VAULT_ADDR` environment variables
- Verify secret path exists: `vault kv get secrets/my-plugin/api_key`

### Environment Variable Override Not Working

```
# Set via env, but config file value is used instead
export MY_PLUGIN_TIMEOUT="30s"
```

Solution:
- Check env variable name matches schema: `env_var: Some("MY_PLUGIN_TIMEOUT")`
- Verify environment variable is set: `echo $MY_PLUGIN_TIMEOUT`
- Check loading order: env vars override file values

## See Also

- [Plugin Development Guide](./PLUGIN_DEVELOPMENT.md) - Creating plugins
- [ABI Specification](./PLUGIN_CONTRACT.md) - FFI contract
- [Security Best Practices](./SECURITY.md) - Secure configuration handling
