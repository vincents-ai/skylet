# Secrets Manager Plugin - API Reference

## Plugin Overview

| Property | Value |
|----------|-------|
| **Plugin Name** | secrets-manager |
| **Version** | 0.1.0 |
| **Author** | Skylet |
| **License** | MIT OR Apache-2.0 |
| **ABI Version** | 1 |
| **Plugin Type** | Infrastructure |
| **Supports Async** | False |
| **Supports Hot Reload** | False |
| **Supports Streaming** | False |
| **Max Concurrency** | 20 |

### Key Capabilities
- Secure secret storage with automatic memory clearing via Zeroize crate
- Multiple backend support (in-memory, Vault, AWS Secrets Manager, filesystem)
- FFI Service API for C/C++ integration
- Audit-ready with comprehensive logging support
- Type-safe redacted debug output to prevent secret leaks
- Zero-copy secrets with automatic cleanup on drop
- Thread-safe concurrent access with Arc<Mutex<>>
- Hierarchical secret naming with prefix-based access

---

## FFI Functions

### plugin_init

Initializes the secrets manager plugin and registers the service.

**Signature:**
```c
PluginResult plugin_init(const PluginContext *context);
```

**Parameters:**
- `context` (const PluginContext*): Plugin execution context

**Return Value:**
- `PluginResult::Success`: Initialization successful
- `PluginResult::InvalidRequest`: Context is null

**Description:**
Initializes the secrets manager, creates the SecretsService with function pointers, and registers it in the plugin context. Prepares the plugin for secret operations.

**Example - C:**
```c
#include <skylet_abi.h>

PluginResult init_secrets(const PluginContext *ctx) {
    PluginResult result = plugin_init(ctx);
    if (result != PluginResult_Success) {
        fprintf(stderr, "Failed to initialize secrets manager\n");
        return result;
    }
    printf("Secrets Manager initialized\n");
    return PluginResult_Success;
}
```

**Example - Python:**
```python
import ctypes
from skylet_abi import PluginContext, PluginResult

def init_secrets_manager(context_ptr):
    """Initialize the secrets manager plugin"""
    result = lib.plugin_init(context_ptr)
    
    if result == PluginResult.Success:
        print("Secrets Manager initialized successfully")
        return True
    else:
        print(f"Plugin initialization failed: {result}")
        return False

# Usage
context = create_plugin_context()
if init_secrets_manager(context):
    # Plugin ready for secret operations
    pass
```

**Example - Rust:**
```rust
use skylet_abi::{PluginContext, PluginResult};

pub fn init_secrets_manager(context: *const PluginContext) -> PluginResult {
    if context.is_null() {
        eprintln!("Failed: context is null");
        return PluginResult::InvalidRequest;
    }
    
    unsafe {
        match plugin_init(context) {
            PluginResult::Success => {
                println!("Secrets Manager initialized");
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

Cleans up secrets manager resources and prepares for unload.

**Signature:**
```c
PluginResult plugin_shutdown(const PluginContext *context);
```

**Parameters:**
- `context` (const PluginContext*): Plugin execution context

**Return Value:**
- `PluginResult::Success`: Shutdown completed successfully

**Description:**
Releases any held resources, clears the cached plugin context, and shuts down the secrets manager. Ensures all secrets are cleared from memory.

---

### plugin_get_info

Retrieves plugin metadata and capabilities information.

**Signature:**
```c
const PluginInfo* plugin_get_info(void);
```

**Return Value:**
- Pointer to PluginInfo structure containing plugin metadata

---

## Service Interface

### SecretsService Structure

The secrets manager exposes a service interface for secure secret operations:

```c
typedef struct {
    SecretResult (*get_secret)(const char *path);
    SecretResult (*set_secret)(const char *path, const char *value);
    SecretResult (*delete_secret)(const char *path);
    SecretListResult (*list_secrets)(const char *prefix);
    void (*free_string)(char *ptr);
    void (*free_list)(SecretListResult *ptr);
} SecretsService;
```

### Return Structures

```c
typedef struct {
    int success;               // 1 for success, 0 for failure
    const char *value;         // Secret value (caller must free with free_string)
    const char *error_message; // Error message if failed
} SecretResult;

typedef struct {
    int success;               // 1 for success, 0 for failure
    const char **secrets;      // Array of secret keys (caller must free with free_list)
    size_t count;              // Number of secrets
    const char *error_message; // Error message if failed
} SecretListResult;
```

---

## Service Methods

### get_secret

Retrieve a secret by its path/key.

**Signature:**
```c
SecretResult get_secret(const char *path);
```

**Parameters:**
- `path` (const char*): Hierarchical secret path (e.g., "api/openai/key", "database/password")

**Return Structure:**
```c
typedef struct {
    int success;               // 1 if secret found, 0 otherwise
    const char *value;         // Secret value string (NULL if not found)
    const char *error_message; // Error description if failed
} SecretResult;
```

**Memory Management:**
- Returned `value` pointer must be freed using `free_string()` when no longer needed
- Do not modify the returned string

**Example - C:**
```c
SecretsService* service = get_secrets_service();

// Get an API key
SecretResult result = service->get_secret("api/openai/key");
if (result.success) {
    printf("API Key length: %zu\n", strlen(result.value));
    // Use the secret...
    service->free_string((char*)result.value);
} else {
    fprintf(stderr, "Secret not found: %s\n", result.error_message);
}

// Get database password
SecretResult db_result = service->get_secret("database/main/password");
if (db_result.success) {
    // Connect to database using password...
    service->free_string((char*)db_result.value);
}
```

**Example - Python:**
```python
def get_secret(service, path):
    """Retrieve a secret by path"""
    result = service.get_secret(path.encode('utf-8'))
    
    if result.success:
        secret_value = result.value.decode('utf-8')
        print(f"✓ Retrieved secret: {path}")
        return secret_value
    else:
        print(f"✗ Error: {result.error_message}")
        return None

# Get secrets
api_key = get_secret(service, "api/openai/key")
db_password = get_secret(service, "database/main/password")
jwt_secret = get_secret(service, "auth/jwt/secret")

if api_key:
    # Use API key
    pass
```

**Example - Rust:**
```rust
pub fn get_secret(service: &SecretsService, path: &str) -> Result<String> {
    let c_path = std::ffi::CString::new(path)?;
    
    let result = (service.get_secret)(c_path.as_ptr());
    
    if result.success != 0 {
        let secret = std::ffi::CStr::from_ptr(result.value)
            .to_string_lossy()
            .into_owned();
        
        // Clean up the returned pointer
        (service.free_string)(result.value as *mut i8);
        
        Ok(secret)
    } else {
        let error = std::ffi::CStr::from_ptr(result.error_message)
            .to_string_lossy()
            .into_owned();
        Err(format!("Secret not found: {}", error).into())
    }
}

// Usage
match get_secret(&service, "api/openai/key") {
    Ok(api_key) => println!("API key retrieved"),
    Err(e) => eprintln!("Error: {}", e),
}
```

---

### set_secret

Store or update a secret.

**Signature:**
```c
SecretResult set_secret(const char *path, const char *value);
```

**Parameters:**
- `path` (const char*): Hierarchical secret path
- `value` (const char*): Secret value to store

**Return Structure:**
```c
typedef struct {
    int success;               // 1 if stored successfully, 0 otherwise
    const char *value;         // NULL on set operations
    const char *error_message; // Error description if failed
} SecretResult;
```

**Example - C:**
```c
SecretsService* service = get_secrets_service();

// Store an API key
SecretResult result = service->set_secret(
    "api/openai/key",
    "sk-..."
);
if (result.success) {
    printf("✓ Secret stored\n");
} else {
    fprintf(stderr, "Error storing secret: %s\n", result.error_message);
}

// Store database credentials
service->set_secret("database/main/user", "dbuser");
service->set_secret("database/main/password", "secure_password");
```

**Example - Python:**
```python
def set_secret(service, path, value):
    """Store a secret"""
    result = service.set_secret(
        path.encode('utf-8'),
        value.encode('utf-8')
    )
    
    if result.success:
        print(f"✓ Secret stored: {path}")
        return True
    else:
        print(f"✗ Error: {result.error_message}")
        return False

# Store multiple secrets
set_secret(service, "api/openai/key", "sk-...")
set_secret(service, "database/main/password", "secure_pass")
set_secret(service, "auth/jwt/secret", "jwt_secret_key")
set_secret(service, "slack/webhook", "https://hooks.slack.com/...")
```

**Example - Rust:**
```rust
pub fn set_secret(
    service: &SecretsService,
    path: &str,
    value: &str,
) -> Result<()> {
    let c_path = std::ffi::CString::new(path)?;
    let c_value = std::ffi::CString::new(value)?;
    
    let result = (service.set_secret)(c_path.as_ptr(), c_value.as_ptr());
    
    if result.success != 0 {
        Ok(())
    } else {
        let error = std::ffi::CStr::from_ptr(result.error_message)
            .to_string_lossy()
            .into_owned();
        Err(format!("Failed to store secret: {}", error).into())
    }
}

// Usage
set_secret(&service, "api/openai/key", "sk-...")?;
set_secret(&service, "database/password", "secure_pass")?;
```

---

### delete_secret

Delete a secret from storage.

**Signature:**
```c
SecretResult delete_secret(const char *path);
```

**Parameters:**
- `path` (const char*): Path of secret to delete

**Example - C:**
```c
SecretResult result = service->delete_secret("api/old/key");
if (result.success) {
    printf("✓ Secret deleted\n");
} else {
    fprintf(stderr, "Error deleting secret: %s\n", result.error_message);
}
```

**Example - Python:**
```python
def delete_secret(service, path):
    """Delete a secret"""
    result = service.delete_secret(path.encode('utf-8'))
    
    if result.success:
        print(f"✓ Secret deleted: {path}")
        return True
    else:
        print(f"✗ Error: {result.error_message}")
        return False

# Delete outdated secrets
delete_secret(service, "api/old/key")
delete_secret(service, "temp/auth/token")
```

**Example - Rust:**
```rust
pub fn delete_secret(service: &SecretsService, path: &str) -> Result<()> {
    let c_path = std::ffi::CString::new(path)?;
    let result = (service.delete_secret)(c_path.as_ptr());
    
    if result.success != 0 {
        Ok(())
    } else {
        let error = std::ffi::CStr::from_ptr(result.error_message)
            .to_string_lossy()
            .into_owned();
        Err(format!("Failed to delete secret: {}", error).into())
    }
}

// Usage
delete_secret(&service, "api/old/key")?;
```

---

### list_secrets

List all secrets with a specific prefix.

**Signature:**
```c
SecretListResult list_secrets(const char *prefix);
```

**Parameters:**
- `prefix` (const char*): Path prefix to search (e.g., "api/", "database/")

**Return Structure:**
```c
typedef struct {
    int success;               // 1 if query successful, 0 otherwise
    const char **secrets;      // Array of secret paths
    size_t count;              // Number of secrets found
    const char *error_message; // Error description if failed
} SecretListResult;
```

**Memory Management:**
- Returned array must be freed using `free_list()` when done
- Do not access array after calling `free_list()`

**Example - C:**
```c
SecretListResult list = service->list_secrets("api/");
if (list.success) {
    printf("Found %zu API secrets:\n", list.count);
    for (size_t i = 0; i < list.count; i++) {
        printf("  - %s\n", list.secrets[i]);
    }
    service->free_list(&list);
} else {
    fprintf(stderr, "Error listing secrets: %s\n", list.error_message);
}
```

**Example - Python:**
```python
def list_secrets(service, prefix):
    """List all secrets with a given prefix"""
    result = service.list_secrets(prefix.encode('utf-8'))
    
    if result.success:
        secrets = []
        for i in range(result.count):
            secret_path = result.secrets[i].decode('utf-8')
            secrets.append(secret_path)
        
        print(f"Found {len(secrets)} secrets under '{prefix}':")
        for secret in secrets:
            print(f"  - {secret}")
        
        service.free_list(result)
        return secrets
    else:
        print(f"Error: {result.error_message}")
        return []

# List all API secrets
api_secrets = list_secrets(service, "api/")

# List all database secrets
db_secrets = list_secrets(service, "database/")

# List all auth-related secrets
auth_secrets = list_secrets(service, "auth/")
```

**Example - Rust:**
```rust
pub fn list_secrets(service: &SecretsService, prefix: &str) -> Result<Vec<String>> {
    let c_prefix = std::ffi::CString::new(prefix)?;
    let result = (service.list_secrets)(c_prefix.as_ptr());
    
    if result.success != 0 {
        let mut secrets = Vec::new();
        let slice = unsafe {
            std::slice::from_raw_parts(result.secrets, result.count)
        };
        
        for &secret_ptr in slice {
            let secret_path = unsafe {
                std::ffi::CStr::from_ptr(secret_ptr)
                    .to_string_lossy()
                    .into_owned()
            };
            secrets.push(secret_path);
        }
        
        Ok(secrets)
    } else {
        let error = unsafe {
            std::ffi::CStr::from_ptr(result.error_message)
                .to_string_lossy()
                .into_owned()
        };
        Err(format!("Failed to list secrets: {}", error).into())
    }
}

// Usage
let api_secrets = list_secrets(&service, "api/")?;
println!("Found {} API secrets", api_secrets.len());
```

---

## Secret Path Conventions

Use hierarchical paths for organizing secrets:

```
api/
  openai/
    key
    organization
  anthropic/
    key
database/
  main/
    host
    port
    user
    password
  replica/
    password
auth/
  jwt/
    secret
    public_key
  oauth/
    client_id
    client_secret
third_party/
  stripe/
    api_key
  twilio/
    auth_token
```

---

## Integration Examples

### Complete Workflow: Application Secret Management

**Python Implementation:**
```python
#!/usr/bin/env python3

class SecretsManager:
    def __init__(self, service):
        self.service = service
    
    def store_api_credentials(self, provider, api_key, organization=None):
        """Store API credentials for a provider"""
        self.service.set_secret(f"api/{provider}/key", api_key)
        if organization:
            self.service.set_secret(f"api/{provider}/organization", organization)
        print(f"✓ Stored {provider} credentials")
    
    def get_api_key(self, provider):
        """Get API key for a provider"""
        path = f"api/{provider}/key"
        result = self.service.get_secret(path)
        if result.success:
            return result.value.decode('utf-8')
        return None
    
    def store_database_config(self, name, host, port, user, password):
        """Store database configuration"""
        prefix = f"database/{name}"
        self.service.set_secret(f"{prefix}/host", host)
        self.service.set_secret(f"{prefix}/port", str(port))
        self.service.set_secret(f"{prefix}/user", user)
        self.service.set_secret(f"{prefix}/password", password)
        print(f"✓ Stored {name} database configuration")
    
    def get_database_password(self, db_name):
        """Get database password"""
        path = f"database/{db_name}/password"
        result = self.service.get_secret(path)
        return result.value.decode('utf-8') if result.success else None
    
    def list_api_credentials(self):
        """List all API credentials"""
        result = self.service.list_secrets("api/")
        return [s.decode('utf-8') for s in result.secrets[:result.count]]

def main():
    manager = SecretsManager(service)
    
    # Store API credentials
    manager.store_api_credentials("openai", "sk-...", "org-123")
    manager.store_api_credentials("anthropic", "sk-ant-...")
    
    # Store database configs
    manager.store_database_config(
        "main",
        "db.example.com",
        5432,
        "app_user",
        "secure_password"
    )
    
    # Retrieve and use credentials
    openai_key = manager.get_api_key("openai")
    if openai_key:
        # Use OpenAI API...
        pass
    
    # List all credentials
    creds = manager.list_api_credentials()
    print(f"Stored {len(creds)} API credentials")

if __name__ == "__main__":
    main()
```

---

## Error Handling

### Common Errors

| Error | Cause | Resolution |
|-------|-------|------------|
| "Secret not found" | Path doesn't exist | Check secret path and prefix |
| "Empty secret key" | Path is empty string | Provide valid secret path |
| "Invalid characters in path" | Path contains invalid chars | Use alphanumeric, /, _ in paths |
| "Backend unavailable" | Storage backend not ready | Check backend configuration |
| "Permission denied" | User lacks permission | Check access control settings |

---

## Security Considerations

### Memory Safety
- All secret values are cleared from memory when dropped using Zeroize crate
- Debug output automatically redacts secret values to prevent leaks
- Secrets are never logged in plain text

### Best Practices
1. Use hierarchical paths for organization
2. Rotate secrets regularly
3. Use strong secret values
4. Limit secret access to necessary services only
5. Audit all secret access operations
6. Never commit secrets to version control

---

## Configuration

### Supported Backends

**In-Memory Backend (Default):**
```rust
let manager = SecretsManager::with_in_memory();
```

**Vault Backend:**
```rust
let manager = SecretsManager::with_vault("https://vault.example.com:8200")?;
```

**AWS Secrets Manager:**
```rust
let manager = SecretsManager::with_aws_secrets()?;
```

**Custom Backend:**
Implement the `SecretBackend` trait for custom storage.

---

## Performance Characteristics

### Latency
- **Get secret**: <1ms (in-memory), 50-200ms (remote backend)
- **Set secret**: <1ms (in-memory), 50-200ms (remote backend)
- **Delete secret**: <1ms (in-memory), 50-200ms (remote backend)
- **List secrets**: O(n) where n is number of secrets

### Throughput
- **Max concurrent operations**: 20
- **Operations per second**: 1,000+ (in-memory)
- **Thread-safe**: Yes (Arc<Mutex<>>)

### Memory Usage
- **Per secret**: ~100-500 bytes (depends on value size)
- **No leaks**: Automatic cleanup via Zeroize

---

## Version History

### v0.1.0 (Current)
- Initial release
- Secure secret storage with Zeroize
- Multiple backend support
- FFI Service API
- Type-safe debug output
- Thread-safe operations
- Hierarchical secret paths
- Prefix-based listing

### Planned Features (v0.2.0)
- Secret rotation policies
- Access control and permissions
- Audit logging
- Secret versioning
- Encryption at rest
- Hardware security module support
