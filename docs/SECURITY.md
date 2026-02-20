# Security Best Practices Guide

This guide covers security considerations for developing plugins on the Skylet execution engine.

## Overview

This guide addresses:

- **Input Validation**: Validating and sanitizing all external inputs
- **Memory Safety**: Safe handling of FFI boundaries and raw pointers
- **Cryptographic Operations**: Proper use of cryptographic primitives
- **Resource Management**: Preventing resource exhaustion attacks
- **Data Protection**: Securing sensitive data at rest and in transit
- **Access Control**: Implementing proper authorization checks
- **Dependency Security**: Managing and auditing third-party dependencies

## Input Validation

### Always Validate at FFI Boundaries

The FFI boundary between the Skylet engine and plugins is a critical security point. **Never trust data crossing this boundary.**

```rust
#[no_mangle]
pub extern "C" fn plugin_process_request(
    context: *const PluginContextV2,
    request: *const c_char,
    request_len: usize,
) -> PluginResult {
    unsafe {
        // VULNERABLE: No validation of pointers
        // let request_str = CStr::from_ptr(request).to_string_lossy();  // ❌
        
        // SECURE: Validate null pointers first
        if request.is_null() {
            return PluginResult::InvalidRequest;  // ✅
        }
        
        if request_len == 0 || request_len > MAX_REQUEST_SIZE {
            return PluginResult::InvalidRequest;  // ✅
        }
        
        // Create slice from raw pointer safely
        let request_bytes = std::slice::from_raw_parts(request as *const u8, request_len);  // ✅
        
        // Validate UTF-8
        match std::str::from_utf8(request_bytes) {
            Ok(request_str) => {
                // Process validated string
                process_request(request_str)
            }
            Err(_) => PluginResult::InvalidRequest,  // ✅
        }
    }
}
```

### Validate Data Types and Ranges

```rust
// VULNERABLE: No range checking
fn set_port(port: u16) {
    // Any u16 is valid, but we need 1-65535
    setup_listener(port);  // ❌
}

// SECURE: Explicit validation
fn set_port(port: u16) -> Result<()> {
    if port < 1 || port > 65535 {
        return Err(anyhow!("Invalid port number"));
    }
    setup_listener(port);
    Ok(())  // ✅
}
```

### Validate String Lengths

```rust
const MAX_USERNAME_LEN: usize = 255;
const MAX_API_KEY_LEN: usize = 256;

// VULNERABLE: No length checking
fn process_user(username: &str) {
    store_user(username);  // ❌ Could be 1GB string
}

// SECURE: Enforce length limits
fn process_user(username: &str) -> Result<()> {
    if username.is_empty() || username.len() > MAX_USERNAME_LEN {
        return Err(anyhow!("Invalid username length"));
    }
    store_user(username);
    Ok(())  // ✅
}
```

### Validate Configuration Schema

Always define and validate schema for configuration:

```rust
use skylet_abi::config::{ConfigSchema, ConfigField, ConfigFieldType, ValidationRule};

fn create_schema() -> ConfigSchema {
    let mut schema = ConfigSchema::new("my-plugin");
    
    schema.add_field(ConfigField {
        name: "api_endpoint".to_string(),
        field_type: ConfigFieldType::Url {
            schemes: vec!["https".to_string()],  // Only HTTPS ✅
        },
        validation: vec![
            ValidationRule::Pattern {
                regex: "^https://[a-zA-Z0-9.-]+$".to_string(),
            },
        ],
        required: true,
        ..Default::default()
    });
    
    schema.add_field(ConfigField {
        name: "timeout_seconds".to_string(),
        field_type: ConfigFieldType::Integer,
        validation: vec![
            ValidationRule::Min { value: 1.0 },
            ValidationRule::Max { value: 300.0 },  // Max 5 minutes ✅
        ],
        ..Default::default()
    });
    
    schema
}
```

## Memory Safety

### Manage Raw Pointers Carefully

```rust
// VULNERABLE: Memory could be freed while pointer is in use
unsafe {
    let ptr = malloc(100);
    spawn_task(ptr);  // Thread has pointer
    free(ptr);        // But we freed it! ❌
}

// SECURE: Use Arc for shared ownership
let data = Arc::new(data);
let data_clone = Arc::clone(&data);
spawn_task(data_clone);  // Thread owns reference
// data is dropped when all Arc instances are dropped ✅
```

### Avoid Buffer Overflows

```rust
// VULNERABLE: Unbounded copy
fn copy_user_input(input: &[u8]) {
    let mut buffer = [0u8; 256];
    std::ptr::copy_nonoverlapping(
        input.as_ptr(),
        buffer.as_mut_ptr(),
        input.len(),  // No bounds check! ❌
    );
}

// SECURE: Check bounds
fn copy_user_input(input: &[u8]) -> Result<Vec<u8>> {
    if input.len() > MAX_INPUT_SIZE {
        return Err(anyhow!("Input too large"));
    }
    let mut buffer = vec![0u8; input.len()];
    buffer.copy_from_slice(input);  // Safe copy ✅
    Ok(buffer)
}
```

### Initialize Memory Before Use

```rust
// VULNERABLE: Reading uninitialized memory
fn process_config() {
    let config: Config;  // Uninitialized
    read_config_file(&mut config);  // If read fails, config is still uninitialized
    process(&config);  // ❌
}

// SECURE: Use default initialization
fn process_config() -> Result<()> {
    let mut config = Config::default();  // Initialized ✅
    read_config_file(&mut config)?;
    process(&config);
    Ok(())
}
```

### Use Safe Wrapper Types

```rust
// VULNERABLE: Raw FFI pointers
unsafe {
    let ptr = context.service_registry;
    let registry = &*ptr;  // Could be null or invalid ❌
}

// SECURE: Use wrapper types
struct ServiceRegistry {
    ptr: *const ServiceRegistryC,
}

impl ServiceRegistry {
    fn new(ptr: *const ServiceRegistryC) -> Result<Self> {
        if ptr.is_null() {
            return Err(anyhow!("Null pointer"));  // ✅
        }
        Ok(ServiceRegistry { ptr })
    }
}
```

## Cryptographic Operations

### Use Approved Algorithms

```rust
// VULNERABLE: Custom or weak crypto
fn custom_hash(data: &[u8]) -> Vec<u8> {
    // Don't implement your own crypto! ❌
}

// SECURE: Use well-tested libraries
use sha2::{Sha256, Digest};

fn secure_hash(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()  // ✅
}
```

### Generate Cryptographically Secure Random Numbers

```rust
// VULNERABLE: Using weak RNG
fn generate_token() -> String {
    let mut rng = rand::thread_rng();
    format!("{:08x}", rng.gen::<u32>())  // Predictable! ❌
}

// SECURE: Use cryptographic RNG
use rand::prelude::*;
use rand::rngs::OsRng;

fn generate_token() -> String {
    let mut rng = OsRng;
    let random_bytes: [u8; 32] = rng.gen();
    hex::encode(&random_bytes)  // ✅
}
```

### Protect Secret Keys

```rust
// VULNERABLE: Storing key in plaintext variable
fn load_key() -> String {
    let key = read_secret();
    process_with_key(&key);
    // Key remains in memory until dropped ❌
    key
}

// SECURE: Use zeroize to clear sensitive data
use zeroize::Zeroizing;

fn load_key() -> Result<()> {
    let key = Zeroizing::new(read_secret());
    process_with_key(&key);
    // Key is automatically zeroed on drop ✅
    Ok(())
}
```

### Verify Signatures and Certificates

```rust
// VULNERABLE: Skipping signature verification
fn load_plugin(data: &[u8]) {
    let plugin = parse_plugin(data);  // Don't verify signature! ❌
}

// SECURE: Verify cryptographic signature
use ed25519_dalek::PublicKey;

fn load_plugin(data: &[u8], signature: &[u8], public_key: &PublicKey) -> Result<Plugin> {
    // Verify signature before processing
    public_key.verify(data, signature)?;  // ✅
    let plugin = parse_plugin(data);
    Ok(plugin)
}
```

## Resource Management

### Prevent Resource Exhaustion

```rust
const MAX_CONNECTIONS: usize = 1000;
const MAX_REQUEST_SIZE: usize = 10 * 1024 * 1024;  // 10 MB
const MAX_RESPONSE_TIME: Duration = Duration::from_secs(60);

// VULNERABLE: No limits
async fn handle_request(request: &Request) {
    let body = request.body().await;  // Could be gigabytes ❌
}

// SECURE: Enforce limits
async fn handle_request(request: &Request) -> Result<Response> {
    // Check connection count
    if active_connections() >= MAX_CONNECTIONS {
        return Err(anyhow!("Too many connections"));  // ✅
    }
    
    // Limit request size
    if request.content_length().unwrap_or(0) > MAX_REQUEST_SIZE {
        return Err(anyhow!("Request too large"));  // ✅
    }
    
    // Timeout for request processing
    tokio::time::timeout(
        MAX_RESPONSE_TIME,
        process_request(request),
    ).await??
}
```

### Manage File Handles

```rust
// VULNERABLE: Resource leak
fn process_file(path: &Path) -> Result<Data> {
    let file = std::fs::File::open(path)?;
    let data = std::io::read_to_string(&file)?;
    // File not explicitly closed, relies on drop ❌
    Ok(data)
}

// SECURE: Use RAII or explicit close
fn process_file(path: &Path) -> Result<Data> {
    let file = std::fs::File::open(path)?;
    let data = {
        let mut reader = std::io::BufReader::new(file);
        let mut contents = String::new();
        reader.read_to_string(&mut contents)?;
        contents
    };  // file is dropped here ✅
    Ok(data)
}
```

### Limit Temporary Buffer Sizes

```rust
// VULNERABLE: Unbounded allocation
fn decompress(compressed: &[u8]) -> Result<Vec<u8>> {
    let size = compressed.len() * 100;  // User-controlled size ❌
    let mut buffer = vec![0u8; size];  // Could allocate GB
    decompress_to(&mut buffer)?;
    Ok(buffer)
}

// SECURE: Enforce maximum size
const MAX_DECOMPRESSED_SIZE: usize = 100 * 1024 * 1024;  // 100 MB

fn decompress(compressed: &[u8]) -> Result<Vec<u8>> {
    // Verify decompressed size first
    let decompressed_size = get_decompressed_size(compressed)?;
    if decompressed_size > MAX_DECOMPRESSED_SIZE {
        return Err(anyhow!("Decompressed size too large"));  // ✅
    }
    let mut buffer = vec![0u8; decompressed_size];
    decompress_to(&mut buffer)?;
    Ok(buffer)
}
```

## Data Protection

### Encrypt Sensitive Data

```rust
// VULNERABLE: Storing passwords in plaintext
fn store_password(username: &str, password: &str) {
    database.insert(username, password);  // ❌
}

// SECURE: Hash passwords
use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::SaltString;

fn store_password(username: &str, password: &str) -> Result<()> {
    let salt = SaltString::generate(rand::thread_rng());
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)?;
    database.insert(username, password_hash.to_string());  // ✅
    Ok(())
}
```

### Use TLS for Communication

```rust
// VULNERABLE: Unencrypted HTTP
async fn connect_to_service() -> Result<Connection> {
    let client = reqwest::Client::new();
    client.get("http://api.example.com/data").send().await?  // ❌
}

// SECURE: Enforce HTTPS
async fn connect_to_service() -> Result<Connection> {
    let client = reqwest::Client::builder()
        .https_only(true)  // ✅
        .danger_accept_invalid_certs(false)  // ✅
        .build()?;
    
    client.get("https://api.example.com/data").send().await?
}
```

### Sanitize Log Output

```rust
// VULNERABLE: Logging secrets
fn authenticate(username: &str, password: &str) {
    tracing::info!("Authenticating user: {} with password: {}", username, password);  // ❌
}

// SECURE: Redact secrets in logs
fn authenticate(username: &str, password: &str) -> Result<()> {
    tracing::info!("Authenticating user: {}", username);  // ✅
    // Password never logged
    verify_password(password)?;
    Ok(())
}
```

### Clear Sensitive Data on Drop

```rust
use zeroize::Zeroizing;

struct ApiKey(Zeroizing<String>);

impl ApiKey {
    fn new(key: String) -> Self {
        ApiKey(Zeroizing::new(key))
    }
}

// When ApiKey is dropped, the String's memory is zeroed ✅
```

## Access Control

### Validate Permissions

```rust
// VULNERABLE: No permission checks
fn delete_user(user_id: u64) -> Result<()> {
    database.delete_user(user_id)?;  // No auth check ❌
    Ok(())
}

// SECURE: Verify caller has permission
fn delete_user(caller_id: u64, user_id: u64) -> Result<()> {
    // Check caller is admin or deleting themselves
    if caller_id != user_id && !is_admin(caller_id)? {
        return Err(anyhow!("Permission denied"));  // ✅
    }
    database.delete_user(user_id)?;
    Ok(())
}
```

### Use Principle of Least Privilege

```rust
// VULNERABLE: Plugin runs with full privileges
fn init_plugin() {
    // Access everything
    access_filesystem();
    access_network();
    access_secrets();
}

// SECURE: Request only needed capabilities
fn init_plugin() -> Result<PluginCapabilities> {
    let capabilities = PluginCapabilities::builder()
        .with_config_read()  // ✅ Only config
        .with_http_client()  // ✅ Only outbound HTTP
        .build();
    
    Ok(capabilities)
}
```

## Error Handling

### Don't Leak Information in Error Messages

```rust
// VULNERABLE: Exposing internal details
fn authenticate(username: &str, password: &str) -> Result<User> {
    match database.find_user(username) {
        Ok(user) => {
            if verify_password(&user, password)? {
                Ok(user)
            } else {
                Err(anyhow!("Password incorrect for user: {}", username))  // ❌
            }
        }
        Err(_) => Err(anyhow!("User not found: {}", username)),  // ❌
    }
}

// SECURE: Generic error messages
fn authenticate(username: &str, password: &str) -> Result<User> {
    let user = database.find_user(username)
        .ok_or_else(|| anyhow!("Invalid credentials"))?;  // ✅
    
    if verify_password(&user, password)? {
        Ok(user)
    } else {
        Err(anyhow!("Invalid credentials"))  // ✅
    }
}
```

### Implement Structured Error Handling

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Invalid request")]
    InvalidRequest,
    
    #[error("Unauthorized")]
    Unauthorized,
    
    #[error("Internal server error")]
    InternalError,
}

// Return generic errors to clients, log details internally
fn process_request(request: &Request) -> Result<Response> {
    match handle_request(request) {
        Ok(response) => Ok(response),
        Err(e) => {
            tracing::error!("Request processing failed: {:?}", e);  // ✅ Detailed log
            Err(PluginError::InternalError)  // ✅ Generic response
        }
    }
}
```

## Dependency Security

### Audit Dependencies Regularly

```bash
# Check for known vulnerabilities
cargo audit

# Generate security report
cargo audit --json > security-report.json
```

### Use Dependency Pinning

```toml
# Cargo.toml

[dependencies]
# Pin to specific patch version
serde = "=1.0.197"
tokio = "=1.35.0"

# Or use conservative ranges
regex = "~1.10"  # >= 1.10.0, < 1.11.0
```

### Review Third-Party Code

```bash
# List all dependencies
cargo tree --depth 3

# Check license compatibility
cargo license --json | jq '.'
```

## Secure Configuration

### Never Commit Secrets

```bash
# Add to .gitignore
echo "*.key" >> .gitignore
echo "*.pem" >> .gitignore
echo ".env" >> .gitignore
echo "secrets/" >> .gitignore
```

### Use Secret References in Configuration

```toml
# Instead of hardcoding secrets:
# ❌ api_key = "sk-1234567890"

# Use secret references:
# ✅ api_key = "vault://secrets/my-plugin/api_key"
# ✅ api_key = "env://MY_PLUGIN_API_KEY"
```

### Set Restrictive File Permissions

```bash
# Configuration files with secrets
chmod 600 /etc/skynet/plugins/config.toml

# Private keys
chmod 600 /etc/ssl/my-plugin/key.pem

# Certificates can be readable
chmod 644 /etc/ssl/my-plugin/cert.pem
```

## Testing for Security

### Add Security Tests

```rust
#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn test_rejects_empty_input() {
        let result = process_input("");
        assert!(result.is_err());
    }

    #[test]
    fn test_rejects_oversized_input() {
        let huge = "x".repeat(MAX_INPUT_SIZE + 1);
        let result = process_input(&huge);
        assert!(result.is_err());
    }

    #[test]
    fn test_null_pointer_handling() {
        unsafe {
            let result = plugin_process_request(std::ptr::null(), std::ptr::null(), 0);
            assert_eq!(result, PluginResult::InvalidRequest);
        }
    }

    #[test]
    fn test_crypto_randomness() {
        let token1 = generate_token();
        let token2 = generate_token();
        assert_ne!(token1, token2);  // Tokens should be different
    }
}
```

### Use Fuzzing

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = process_input(data);
});
```

## Security Checklist

- [ ] All user inputs validated at FFI boundary
- [ ] No buffer overflows or unbounded allocations
- [ ] Cryptographic operations use approved algorithms
- [ ] Secrets never logged or stored in plaintext
- [ ] TLS/HTTPS used for all network communication
- [ ] File permissions restricted appropriately
- [ ] Dependencies audited for vulnerabilities
- [ ] Error messages don't leak sensitive information
- [ ] Resource limits enforced (connections, memory, time)
- [ ] Permission checks on sensitive operations
- [ ] No hardcoded secrets in source code
- [ ] Security tests included in test suite

## Reporting Security Issues

If you discover a security vulnerability in Skylet or a plugin:

1. **Do not** create a public issue
2. Email security details to: `shift+security@someone.section.me`
3. Include: vulnerability description, impact, reproduction steps
4. Allow 48 hours for initial response
5. Coordinate responsible disclosure timeline

## References

- [OWASP Top 10](https://owasp.org/Top10/)
- [CWE Top 25](https://cwe.mitre.org/top25/)
- [Rust Book - Unsafe Code](https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html)
- [NIST Cybersecurity Framework](https://www.nist.gov/cyberframework)

## See Also

- [Plugin Development Guide](./PLUGIN_DEVELOPMENT.md)
- [Configuration Reference](./CONFIG_REFERENCE.md)
- [ABI Specification](./PLUGIN_CONTRACT.md)
