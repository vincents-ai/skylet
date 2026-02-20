Secrets Manager Plugin
======================

A secure, production-ready secrets management service for Skylet that provides safe handling, storage, and injection of sensitive credentials into workflow execution environments.

Features
--------

- Secure Secret Storage: In-memory storage with automatic clearing via the Zeroize crate
- Multiple Backend Support: Pluggable backend trait allows integration with Vault, AWS Secrets Manager, file systems, etc.
- FFI Service API: C-compatible interface for integration with Skylet core and other plugins
- Audit Ready: All secret operations can be logged for compliance and audit requirements
- Type Safety: Redacted debug output prevents accidental secret exposure in logs
- Zero-Copy Secrets: SecretValue type ensures secrets are cleared from memory when dropped

Architecture
------------

The plugin is organized into several layers:

1. Backend Layer (SecretBackend trait)
   - Abstract interface for different secret storage systems
   - InMemoryBackend provided as reference implementation
   - Extensible design for Vault, AWS, file, and other backends

2. Manager Layer (SecretsManager)
   - Coordinates secret operations across backends
   - Provides high-level get/set/delete/list operations
   - Thread-safe with Arc<Mutex<>> for concurrent access

3. Service Layer (SecretsService)
   - C FFI interface following the Skylet plugin ABI
   - Converts between C types and Rust types safely
   - Handles memory allocation and cleanup for callers

4. Plugin Layer (plugin_init, plugin_shutdown, plugin_get_info)
   - Standard Skylet plugin lifecycle management
   - Service registration in the plugin context
   - Plugin metadata and versioning

Building
--------

To build the secrets-manager plugin:

```bash
# Build in dev shell
nix develop -c -- cargo build -p secrets-manager

# Build release binary
nix develop -c -- cargo build -p secrets-manager --release

# Build with verbose output
nix develop -c -- cargo build -p secrets-manager --verbose
```

The compiled plugin will be available at:
- Debug: `target/debug/libsecrets_manager.so`
- Release: `target/release/libsecrets_manager.so`

Testing
-------

Run the unit tests:

```bash
# Run all tests
nix develop -c -- cargo test -p secrets-manager

# Run with output
nix develop -c -- cargo test -p secrets-manager -- --nocapture

# Run specific test
nix develop -c -- cargo test -p secrets-manager test_in_memory_backend_set_get
```

Test Coverage
~~~~~~~~~~~~~

The plugin includes comprehensive tests for:
- Secret value redaction in debug output
- In-memory backend operations (set, get, delete, list)
- SecretsManager interface
- Secret key validation
- FFI safety (null pointer handling, UTF-8 validation)

Usage from C/FFI
----------------

Once loaded by Skylet, other plugins can access the SecretsService:

```c
// Get the service from the registry
SecretsService* service = (SecretsService*)registry->get(
    context,
    "secrets-manager",
    "SecretsService"
);

// Get a secret
SecretResult result = service->get_secret("api/openai/key");
if (result.success) {
    const char* api_key = result.value;
    // Use the API key...
    service->free_string((char*)result.value);
}

// Set a secret
SecretResult set_result = service->set_secret("api/openai/key", "sk-...");
if (set_result.success) {
    // Secret stored successfully
}

// List secrets with prefix
SecretListResult list = service->list_secrets("api/");
if (list.success) {
    for (size_t i = 0; i < list.count; i++) {
        printf("%s\n", list.secrets[i]);
    }
    service->free_list(&list);
}

// Delete a secret
SecretResult del_result = service->delete_secret("api/openai/key");
```

Usage from Rust Plugins
-----------------------

Rust plugins can use the SecretsManager directly:

```rust
use secrets_manager::{SecretsManager, SecretsValue, InMemoryBackend};
use std::sync::Arc;

// Create a manager with in-memory backend
let manager = SecretsManager::with_in_memory();

// Set a secret
let secret = SecretValue::new("api_key_value".to_string());
manager.set_secret("llm/openai/key", secret)?;

// Get a secret
let secret = manager.get_secret("llm/openai/key")?;
println!("Secret length: {}", secret.as_str().len());

// List secrets
let secrets = manager.list_secrets("llm/")?;
println!("Found {} secrets", secrets.len());

// Delete a secret
manager.delete_secret("llm/openai/key")?;
```

Integration with Skylet Workflows
----------------------------------

The secrets-manager plugin is designed to integrate with workflow execution systems:

1. Workflow Definition
   Define secret requirements in workflow metadata:
   ```yaml
   secrets:
     - key: OPENAI_API_KEY
       path: vault://llm/openai/key
       required: true
     - key: DEBUG_TOKEN
       path: vault://debug/token
       required: false
   ```

2. Secret Injection
   Before workflow execution, inject secrets:
   ```rust
   // Get secrets from manager
   let api_key = secrets_manager.get_secret("vault://llm/openai/key")?;
   
   // Create execution environment
   let mut env = ExecutionEnvironment::new("/tmp");
   env.add_secret("OPENAI_API_KEY", api_key.as_str());
   
   // Apply to subprocess
   let mut cmd = Command::new("workflow-executor");
   env.apply_to_command(&mut cmd);
   ```

3. Cleanup
   Secrets are automatically cleared when ExecutionEnvironment is dropped

Advanced Configuration
----------------------

Future versions will support:
- Multiple backends (Vault, AWS Secrets Manager, Kubernetes Secrets)
- Encryption at rest (AES-256-GCM)
- Secret rotation policies
- Audit trail with structured logging
- Rate limiting and access control

See CORE_REFACTORING.md and SECRET_INJECTION_SUMMARY.md for more details on the broader secrets infrastructure.

Security Considerations
-----------------------

1. Memory Safety
   - All secrets stored in SecretValue are zeroized on drop
   - No plaintext secrets in debug/display output
   - Uses Zeroize crate for explicit memory clearing

2. Access Control
   - Service registry pattern prevents unauthorized access
   - Future: RBAC integration for fine-grained control

3. Audit Trail
   - All operations logged via plugin logger
   - Future: Structured logging for compliance

4. Encryption
   - In-memory backend for development
   - Future: Encrypted storage at rest

Example: Complete Secret Workflow
---------------------------------

```rust
use secrets_manager::*;
use std::sync::Arc;

fn main() -> Result<()> {
    // Initialize
    let backend = Arc::new(InMemoryBackend::new());
    let secrets = SecretsManager::new(backend);
    
    // Store API credentials
    let openai_key = SecretValue::new("sk-...".to_string());
    secrets.set_secret("llm/openai/key", openai_key)?;
    
    let github_token = SecretValue::new("ghp_...".to_string());
    secrets.set_secret("vcs/github/token", github_token)?;
    
    // List all VCS secrets
    let vcs_secrets = secrets.list_secrets("vcs/")?;
    println!("VCS secrets: {:?}", vcs_secrets);
    
    // Use a secret
    let key = secrets.get_secret("llm/openai/key")?;
    println!("API key length: {}", key.as_str().len());
    
    // Cleanup (automatic via Drop)
    secrets.delete_secret("llm/openai/key")?;
    
    Ok(())
}
```

Dependencies
------------

Core Dependencies:
- `skylet-abi`: Skylet plugin ABI definitions
- `serde` & `serde_json`: Serialization
- `anyhow`: Error handling
- `tokio`: Async runtime
- `zeroize`: Secure memory clearing
- `aes-gcm`: Encryption (future use)
- `sha2`: Hashing (future use)
- `rand`: Random number generation (future use)

Development Dependencies:
- `tokio` (testing)

License
-------

MIT

See Also
--------

- skynet/CORE_REFACTORING.md - Overview of core refactoring and plugin structure
- skynet/SECRET_INJECTION_SUMMARY.md - Secret injection system details
- skynet/docs/SECRET_INJECTION.md - User guide for secret injection
- skynet/src/secret_injection.rs - Original secret injection implementation
