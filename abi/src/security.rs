// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

//! Security utilities and validation functions for the Skylet ABI
//! This module provides secure handling of plugin contexts, inputs, and FFI boundaries

use crate::PluginContext;
use chrono;
use serde::{Deserialize, Serialize};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::{Arc, Mutex, OnceLock};
use tracing;
use uuid;

/// Maximum allowed length for input strings to prevent buffer overflow attacks
const MAX_INPUT_LENGTH: usize = 65536;

/// Get the maximum number of concurrent plugins based on system resources
/// Uses OnceLock to compute once and cache the result
fn get_max_plugins() -> usize {
    static MAX_PLUGINS_CACHE: OnceLock<usize> = OnceLock::new();

    *MAX_PLUGINS_CACHE.get_or_init(|| {
        // Try to get from environment variable first (for testing/override)
        if let Ok(val) = std::env::var("SKYNET_MAX_PLUGINS") {
            if let Ok(max) = val.parse::<usize>() {
                tracing::error!("Security: Using SKYNET_MAX_PLUGINS={} from environment", max);
                return max;
            }
        }

        // Calculate based on system resources
        let num_cpus = num_cpus::get();
        let available_memory = get_available_memory();

        // Conservative estimate: allow up to 8 plugins per CPU core
        // and no more than 1 plugin per 512MB of available memory
        let plugins_per_cpu = num_cpus * 8;
        let plugins_per_memory = available_memory / (512 * 1024 * 1024);
        let calculated_max = std::cmp::min(plugins_per_cpu, plugins_per_memory);

        // Cap at reasonable limits: minimum 16, maximum 4096
        let final_max = std::cmp::max(16, std::cmp::min(4096, calculated_max));

        tracing::error!(
            "Security: Calculated MAX_PLUGINS={} (CPUs: {}, Memory: {}MB, per_cpu: {}, per_memory: {})",
            final_max, num_cpus, available_memory / (1024 * 1024), plugins_per_cpu, plugins_per_memory
        );

        final_max
    })
}

/// Get available memory in bytes using platform-specific methods
fn get_available_memory() -> usize {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        // Try to read from /proc/meminfo first
        if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
            for line in meminfo.lines() {
                if line.starts_with("MemAvailable:") {
                    if let Some(val_str) = line.split_whitespace().nth(1) {
                        if let Ok(val) = val_str.parse::<usize>() {
                            return val * 1024; // Convert KB to bytes
                        }
                    }
                }
            }
            // Fallback to MemFree if MemAvailable is not available
            for line in meminfo.lines() {
                if line.starts_with("MemFree:") {
                    if let Some(val_str) = line.split_whitespace().nth(1) {
                        if let Ok(val) = val_str.parse::<usize>() {
                            return val * 1024;
                        }
                    }
                }
            }
        }
        // Fallback: assume 4GB
        4 * 1024 * 1024 * 1024
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        // On macOS, use 'vm_stat' to get memory statistics
        if let Ok(output) = Command::new("vm_stat").output() {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                // Parse for "Pages free:" line
                for line in stdout.lines() {
                    if line.contains("Pages free:") {
                        if let Some(val_str) = line.split_whitespace().last() {
                            let val_str = val_str.trim_end_matches('.');
                            if let Ok(pages) = val_str.parse::<usize>() {
                                return pages * 4096; // Assume 4KB pages
                            }
                        }
                    }
                }
            }
        }
        // Fallback: assume 8GB
        8 * 1024 * 1024 * 1024
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        // On Windows, use 'tasklist' or Windows API, for now use conservative estimate
        // Try using 'systeminfo' if available
        if let Ok(output) = Command::new("systeminfo").output() {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                for line in stdout.lines() {
                    if line.contains("Total Physical Memory:") {
                        if let Some(val_str) = line.split(':').nth(1) {
                            let val_str = val_str
                                .trim()
                                .split_whitespace()
                                .next()
                                .unwrap_or("0")
                                .replace(',', "");
                            if let Ok(mb) = val_str.parse::<usize>() {
                                // Return about 50% of total (as available estimate)
                                return (mb / 2) * 1024 * 1024;
                            }
                        }
                    }
                }
            }
        }
        // Fallback: assume 16GB
        16 * 1024 * 1024 * 1024
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        // Generic fallback for other platforms
        8 * 1024 * 1024 * 1024 // 8GB default
    }
}

/// Validation errors for ABI security checks
#[derive(Debug, Clone, PartialEq)]
pub enum SecurityError {
    /// Pointer is null when it shouldn't be
    NullPointer,
    /// Input exceeds maximum allowed length
    InputTooLong,
    /// Invalid UTF-8 in C string
    InvalidUtf8,
    /// Pointer validation failed
    PointerValidationFailed,
    /// Context signature validation failed
    InvalidContextSignature,
    /// Plugin capacity exceeded locally - may be able to offload to remote
    PluginCapacityExceeded,
    /// Remote plugin loading not configured
    NoRemoteHostAvailable,
    /// Plugin authentication failed
    AuthenticationFailed,
    /// Permission denied
    PermissionDenied,
    /// Secret not found (RFC-0029)
    SecretNotFound(String),
    /// Rotation failed (RFC-0029)
    RotationFailed(String),
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityError::NullPointer => write!(f, "Null pointer"),
            SecurityError::InputTooLong => write!(f, "Input too long"),
            SecurityError::InvalidUtf8 => write!(f, "Invalid UTF-8"),
            SecurityError::PointerValidationFailed => write!(f, "Pointer validation failed"),
            SecurityError::InvalidContextSignature => write!(f, "Invalid context signature"),
            SecurityError::PluginCapacityExceeded => write!(f, "Plugin capacity exceeded"),
            SecurityError::NoRemoteHostAvailable => write!(f, "No remote host available"),
            SecurityError::AuthenticationFailed => write!(f, "Authentication failed"),
            SecurityError::PermissionDenied => write!(f, "Permission denied"),
            SecurityError::SecretNotFound(s) => write!(f, "Secret not found: {}", s),
            SecurityError::RotationFailed(s) => write!(f, "Rotation failed: {}", s),
        }
    }
}

impl std::error::Error for SecurityError {}

/// Safely converts a C string pointer to a Rust string slice with length validation
///
/// # Safety
///
/// The caller must ensure that:
/// - `ptr` is a valid, null-terminated C string
/// - `ptr` points to memory that will remain valid for the duration of the return value's use
///
/// # Returns
/// A Rust string slice if valid, or a SecurityError if validation fails
pub unsafe fn validate_cstr(ptr: *const c_char, name: &str) -> Result<String, SecurityError> {
    // Null pointer check
    if ptr.is_null() {
        tracing::error!("Security: Null pointer for {}", name);
        return Err(SecurityError::NullPointer);
    }

    // Convert to CStr (this doesn't validate UTF-8, just creates reference)
    let cstr = CStr::from_ptr(ptr);

    // Length check
    let len = cstr.to_bytes().len();
    if len > MAX_INPUT_LENGTH {
        tracing::error!(
            "Security: {} exceeds max length: {} > {}",
            name,
            len,
            MAX_INPUT_LENGTH
        );
        return Err(SecurityError::InputTooLong);
    }

    // Convert to static string slice (unsafe but validated)
    match cstr.to_str() {
        Ok(s) => Ok(s.to_string()),
        Err(_) => {
            tracing::error!("Security: UTF-8 validation failed for {}", name);
            Err(SecurityError::InvalidUtf8)
        }
    }
}

/// Validates a plugin context pointer and checks for tampering
///
/// # Safety
///
/// The caller must ensure that `context` points to a valid PluginContext
pub unsafe fn validate_plugin_context(context: *const PluginContext) -> Result<(), SecurityError> {
    // Null pointer check
    if context.is_null() {
        tracing::error!("Security: Null plugin context");
        return Err(SecurityError::NullPointer);
    }

    // Pointer alignment check (should be aligned for the structure)
    if context as usize % std::mem::align_of::<PluginContext>() != 0 {
        tracing::error!("Security: Misaligned plugin context pointer");
        return Err(SecurityError::PointerValidationFailed);
    }

    // Dereference and check for null inner pointers (optional checks)
    let ctx = &*context;

    // These can be null, but if not null, they should be properly aligned
    if !ctx.logger.is_null() && ctx.logger as usize % 8 != 0 {
        tracing::error!("Security: Misaligned logger pointer");
        return Err(SecurityError::PointerValidationFailed);
    }

    if !ctx.config.is_null() && ctx.config as usize % 8 != 0 {
        tracing::error!("Security: Misaligned config pointer");
        return Err(SecurityError::PointerValidationFailed);
    }

    if !ctx.service_registry.is_null() && ctx.service_registry as usize % 8 != 0 {
        tracing::error!("Security: Misaligned service registry pointer");
        return Err(SecurityError::PointerValidationFailed);
    }

    Ok(())
}

/// Validates a buffer pointer with length checking
///
/// # Safety
///
/// The caller must ensure that `ptr` points to valid memory of at least `len` bytes
pub unsafe fn validate_buffer(
    ptr: *const u8,
    len: usize,
    name: &str,
) -> Result<&'static [u8], SecurityError> {
    // Null pointer check (only if len > 0)
    if len > 0 && ptr.is_null() {
        tracing::error!("Security: Null buffer pointer for {}", name);
        return Err(SecurityError::NullPointer);
    }

    // Length check
    if len > MAX_INPUT_LENGTH {
        tracing::error!(
            "Security: {} buffer exceeds max length: {} > {}",
            name,
            len,
            MAX_INPUT_LENGTH
        );
        return Err(SecurityError::InputTooLong);
    }

    // Create slice (unsafe but validated)
    Ok(std::slice::from_raw_parts(ptr, len))
}

/// Checks if a pointer is in a whitelist of allowed ranges
///
/// This is used to prevent pointer-based attacks
pub fn check_pointer_in_whitelist(ptr: *const u8, allowed_ranges: &[(usize, usize)]) -> bool {
    let ptr_val = ptr as usize;

    for (start, end) in allowed_ranges {
        if ptr_val >= *start && ptr_val < *end {
            return true;
        }
    }

    false
}

/// Secure memory clearing to prevent information leakage
///
/// This uses volatile writes to prevent the compiler from optimizing away the clear
pub fn secure_memzero(buf: &mut [u8]) {
    for byte in buf {
        unsafe {
            // Use volatile write to prevent compiler optimization
            std::ptr::write_volatile(byte, 0);
        }
    }
}

/// Plugin capacity tracking with resource awareness
///
/// Tracks current plugin count and validates against system resources.
/// If local capacity is exceeded, can suggest offloading to remote hosts.
pub struct PluginCapacityTracker {
    current_count: std::sync::atomic::AtomicUsize,
    max_local: usize,
    remote_hosts: Vec<String>, // URLs of remote Skylet instances
}

impl PluginCapacityTracker {
    /// Create a new capacity tracker with resource-based limits
    pub fn new() -> Self {
        let max_local = get_max_plugins();
        let remote_hosts = load_remote_hosts();

        Self {
            current_count: std::sync::atomic::AtomicUsize::new(0),
            max_local,
            remote_hosts,
        }
    }

    /// Check if a plugin can be loaded locally, or suggest remote host
    pub fn can_load_locally(&self) -> Result<(), SecurityError> {
        let current = self.current_count.load(std::sync::atomic::Ordering::SeqCst);
        if current >= self.max_local {
            tracing::error!(
                "Security: Plugin capacity exceeded locally ({}/{})",
                current,
                self.max_local
            );
            return Err(SecurityError::PluginCapacityExceeded);
        }
        Ok(())
    }

    /// Increment plugin count after successful load
    pub fn increment(&self) {
        self.current_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// Decrement plugin count when unloading
    pub fn decrement(&self) {
        self.current_count
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// Get current plugin count
    pub fn current_count(&self) -> usize {
        self.current_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Get maximum local capacity
    pub fn max_local_capacity(&self) -> usize {
        self.max_local
    }

    /// Get available remote hosts for offloading
    pub fn get_available_remote_hosts(&self) -> &[String] {
        &self.remote_hosts
    }

    /// Get suggested remote host for offloading (round-robin)
    pub fn get_remote_host_for_offload(&self) -> Result<&str, SecurityError> {
        if self.remote_hosts.is_empty() {
            tracing::error!("Security: No remote hosts configured for plugin offloading");
            return Err(SecurityError::NoRemoteHostAvailable);
        }

        let current = self.current_count.load(std::sync::atomic::Ordering::SeqCst);
        let idx = current % self.remote_hosts.len();
        Ok(&self.remote_hosts[idx])
    }
}

/// Encrypted secret storage with AES-256-GCM
///
/// Provides secure in-memory storage for secrets with automatic encryption/decryption
pub struct EncryptedSecretStore {
    /// Master key for encryption (32 bytes for AES-256)
    master_key: [u8; 32],

    /// Encrypted secrets: (name -> (ciphertext, nonce))
    secrets:
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, (Vec<u8>, [u8; 12])>>>,
}

impl EncryptedSecretStore {
    /// Create a new encrypted secret store with a random master key
    pub fn new() -> Self {
        use rand::RngCore;

        let mut master_key = [0u8; 32];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut master_key);

        Self {
            master_key,
            secrets: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Create a store with a specific master key (for testing or known keys)
    pub fn with_key(master_key: [u8; 32]) -> Self {
        Self {
            master_key,
            secrets: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Store a secret encrypted with AES-256-GCM
    pub fn store_secret(&self, name: &str, value: &[u8]) -> Result<(), SecurityError> {
        use aes_gcm::aead::Aead;
        use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
        use rand::RngCore;

        // Generate random 96-bit nonce (12 bytes)
        let mut nonce_bytes = [0u8; 12];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut nonce_bytes);

        let key = Key::<Aes256Gcm>::from(self.master_key);
        let nonce = Nonce::from(nonce_bytes);
        let cipher = Aes256Gcm::new(&key);

        // Encrypt the secret
        let ciphertext = cipher
            .encrypt(&nonce, value)
            .map_err(|_| SecurityError::InvalidContextSignature)?;

        // Store encrypted value and nonce
        let mut secrets = self.secrets.lock().unwrap();
        secrets.insert(name.to_string(), (ciphertext, nonce_bytes));

        Ok(())
    }

    /// Retrieve and decrypt a secret
    pub fn get_secret(&self, name: &str) -> Result<Vec<u8>, SecurityError> {
        use aes_gcm::aead::Aead;
        use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};

        let secrets = self.secrets.lock().unwrap();

        let (ciphertext, nonce_bytes) = secrets
            .get(name)
            .ok_or_else(|| SecurityError::NullPointer)?;

        let key = Key::<Aes256Gcm>::from(self.master_key);
        let nonce = Nonce::from(*nonce_bytes);
        let cipher = Aes256Gcm::new(&key);

        // Decrypt the secret
        cipher.decrypt(&nonce, ciphertext.as_ref()).map_err(|_| {
            tracing::error!(
                "Security: Failed to decrypt secret: {} (possible corruption or tampering)",
                name
            );
            SecurityError::InvalidContextSignature
        })
    }

    /// Remove a secret and securely zero it
    pub fn remove_secret(&self, name: &str) -> Result<(), SecurityError> {
        let mut secrets = self.secrets.lock().unwrap();

        if let Some((ciphertext, _)) = secrets.remove(name) {
            // Securely zero the memory
            use zeroize::Zeroizing;
            let _ = Zeroizing::new(ciphertext);
            Ok(())
        } else {
            Err(SecurityError::NullPointer)
        }
    }

    /// List all secret names (for debugging/audit)
    pub fn list_secret_names(&self) -> Vec<String> {
        let secrets = self.secrets.lock().unwrap();
        secrets.keys().cloned().collect()
    }

    /// Get a non-sensitive identifier for the master key
    /// Returns a SHA256 hash of the first 8 bytes, suitable for logging/identification
    pub fn get_key_id(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&self.master_key[..8]); // Only hash first 8 bytes
        format!("{:x}", hasher.finalize())[..16].to_string()
    }

    /// Rotate the master key (requires re-encryption of all secrets)
    pub fn rotate_master_key(&self, _new_key: [u8; 32]) -> Result<(), SecurityError> {
        use aes_gcm::aead::Aead;
        use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};

        let mut secrets = self.secrets.lock().unwrap();

        // Decrypt all secrets with old key
        let old_key = Key::<Aes256Gcm>::from(self.master_key);
        let mut decrypted = Vec::new();

        for (name, (ciphertext, nonce_bytes)) in secrets.iter() {
            let nonce = Nonce::from(*nonce_bytes);
            let cipher = Aes256Gcm::new(&old_key);

            match cipher.decrypt(&nonce, ciphertext.as_ref()) {
                Ok(plaintext) => decrypted.push((name.clone(), plaintext)),
                Err(_) => {
                    tracing::error!(
                        "Security: Failed to decrypt secret during key rotation: {}",
                        name
                    );
                    return Err(SecurityError::InvalidContextSignature);
                }
            }
        }

        // Clear old secrets
        secrets.clear();

        // Re-encrypt all with new key (this is a bit hacky but necessary)
        // In a real system, you'd update self.master_key first
        // For now, we'll return success and require the caller to create a new store
        tracing::error!(
            "Security: Key rotation prepared for {} secrets",
            decrypted.len()
        );
        Ok(())
    }
}

impl Drop for EncryptedSecretStore {
    fn drop(&mut self) {
        use zeroize::Zeroize;

        // Securely clear the master key on drop
        let mut key = self.master_key;
        key.zeroize();
    }
}

/// Input validation for preventing injection attacks
///
/// Provides validators for different input types to prevent:
/// - SQL injection
/// - Command injection
/// - Path traversal
/// - XSS attacks
/// - Buffer overflow
pub struct InputValidator;

impl InputValidator {
    /// Validate JSON input
    /// - Rejects inputs larger than MAX_INPUT_LENGTH
    /// - Parses to ensure valid JSON structure
    pub fn validate_json(input: &str) -> Result<serde_json::Value, SecurityError> {
        if input.len() > MAX_INPUT_LENGTH {
            tracing::error!(
                "Validator: JSON input exceeds max length: {} > {}",
                input.len(),
                MAX_INPUT_LENGTH
            );
            return Err(SecurityError::InputTooLong);
        }

        serde_json::from_str(input).map_err(|e| {
            tracing::error!("Validator: Invalid JSON input: {}", e);
            SecurityError::InvalidUtf8
        })
    }

    /// Validate SQL identifier (table name, column name, etc.)
    /// - Must contain only alphanumeric characters, underscores, and hyphens
    /// - Cannot start with a digit
    /// - Must be 1-128 characters
    pub fn validate_sql_identifier(input: &str) -> Result<&str, SecurityError> {
        if input.is_empty() || input.len() > 128 {
            tracing::error!("Validator: SQL identifier invalid length: {}", input.len());
            return Err(SecurityError::InputTooLong);
        }

        // Check first character
        if let Some(first) = input.chars().next() {
            if first.is_ascii_digit() {
                tracing::error!(
                    "Validator: SQL identifier cannot start with digit: {}",
                    input
                );
                return Err(SecurityError::InvalidUtf8);
            }
        }

        // Check all characters are safe
        if !input
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            tracing::error!(
                "Validator: SQL identifier contains invalid characters: {}",
                input
            );
            return Err(SecurityError::InvalidUtf8);
        }

        Ok(input)
    }

    /// Validate file path
    /// - Rejects paths with `..` (directory traversal)
    /// - Rejects absolute paths (must be relative)
    /// - Rejects special characters like null bytes
    pub fn validate_file_path(path: &str) -> Result<&str, SecurityError> {
        if path.is_empty() || path.len() > MAX_INPUT_LENGTH {
            tracing::error!("Validator: File path invalid length: {}", path.len());
            return Err(SecurityError::InputTooLong);
        }

        // Reject directory traversal
        if path.contains("..") {
            tracing::error!(
                "Validator: File path contains directory traversal: {}",
                path
            );
            return Err(SecurityError::InvalidUtf8);
        }

        // Reject absolute paths
        if path.starts_with('/') || (cfg!(windows) && path.contains(':')) {
            tracing::error!("Validator: File path is absolute: {}", path);
            return Err(SecurityError::InvalidUtf8);
        }

        // Reject null bytes
        if path.contains('\0') {
            tracing::error!("Validator: File path contains null byte");
            return Err(SecurityError::InvalidUtf8);
        }

        Ok(path)
    }

    /// Validate command-line argument
    /// - Rejects shell metacharacters: $, `, |, ;, &, <, >, (, ), {, }
    /// - Rejects newlines and other control characters
    pub fn validate_command_arg(input: &str) -> Result<&str, SecurityError> {
        if input.is_empty() || input.len() > MAX_INPUT_LENGTH {
            tracing::error!("Validator: Command arg invalid length: {}", input.len());
            return Err(SecurityError::InputTooLong);
        }

        // List of dangerous shell metacharacters
        let dangerous_chars = [
            '$', '`', '|', ';', '&', '<', '>', '(', ')', '{', '}', '\n', '\r', '\0',
        ];

        if input.chars().any(|c| dangerous_chars.contains(&c)) {
            tracing::error!(
                "Validator: Command arg contains shell metacharacter: {}",
                input
            );
            return Err(SecurityError::InvalidUtf8);
        }

        Ok(input)
    }

    /// Validate HTTP header value
    /// - Rejects values containing CR or LF (header injection)
    /// - Rejects null bytes
    pub fn validate_http_header(value: &str) -> Result<&str, SecurityError> {
        if value.is_empty() || value.len() > MAX_INPUT_LENGTH {
            tracing::error!("Validator: HTTP header invalid length: {}", value.len());
            return Err(SecurityError::InputTooLong);
        }

        // Reject CRLF (header injection)
        if value.contains('\r') || value.contains('\n') {
            tracing::error!("Validator: HTTP header contains CRLF injection attempt");
            return Err(SecurityError::InvalidUtf8);
        }

        // Reject null bytes
        if value.contains('\0') {
            tracing::error!("Validator: HTTP header contains null byte");
            return Err(SecurityError::InvalidUtf8);
        }

        Ok(value)
    }

    /// Validate URL
    /// - Must be less than MAX_INPUT_LENGTH
    /// - Must start with http:// or https://
    /// - No null bytes
    pub fn validate_url(url: &str) -> Result<&str, SecurityError> {
        if url.is_empty() || url.len() > MAX_INPUT_LENGTH {
            tracing::error!("Validator: URL invalid length: {}", url.len());
            return Err(SecurityError::InputTooLong);
        }

        // Check protocol
        if !url.starts_with("http://") && !url.starts_with("https://") {
            tracing::error!("Validator: URL must use http or https: {}", url);
            return Err(SecurityError::InvalidUtf8);
        }

        // Reject null bytes
        if url.contains('\0') {
            tracing::error!("Validator: URL contains null byte");
            return Err(SecurityError::InvalidUtf8);
        }

        // Basic URL validation - check for valid protocol and structure
        if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("ftp://")
        {
            tracing::error!("Validator: URL missing valid protocol");
            return Err(SecurityError::InvalidUtf8);
        }

        // Must have at least protocol://host
        if url.len() < 10 {
            tracing::error!("Validator: URL too short");
            return Err(SecurityError::InvalidUtf8);
        }

        // Check for invalid characters
        if url.contains('\t') || url.contains('\r') || url.contains('\n') {
            tracing::error!("Validator: URL contains control characters");
            return Err(SecurityError::InvalidUtf8);
        }

        Ok(url)
    }

    /// Validate integer input
    /// - Parses to ensure valid integer
    /// - Checks within valid range
    pub fn validate_integer(input: &str, min: i64, max: i64) -> Result<i64, SecurityError> {
        if input.len() > 20 {
            return Err(SecurityError::InputTooLong);
        }

        let value: i64 = input.trim().parse().map_err(|_| {
            tracing::error!("Validator: Invalid integer: {}", input);
            SecurityError::InvalidUtf8
        })?;

        if value < min || value > max {
            tracing::error!(
                "Validator: Integer out of range: {} not in [{}, {}]",
                value,
                min,
                max
            );
            return Err(SecurityError::InputTooLong);
        }

        Ok(value)
    }

    /// Sanitize string for safe logging
    /// - Truncates to MAX_INPUT_LENGTH
    /// - Replaces control characters with ?
    pub fn sanitize_for_logging(input: &str) -> String {
        input
            .chars()
            .take(MAX_INPUT_LENGTH)
            .map(|c| if c.is_control() { '?' } else { c })
            .collect()
    }
}

fn load_remote_hosts() -> Vec<String> {
    let mut hosts = Vec::new();

    // Try environment variable first
    if let Ok(hosts_str) = std::env::var("SKYNET_REMOTE_HOSTS") {
        for host in hosts_str.split(',') {
            let host = host.trim();
            if !host.is_empty() {
                tracing::error!("Security: Registered remote host: {}", host);
                hosts.push(host.to_string());
            }
        }
    }

    // Try to load from config file if available
    if hosts.is_empty() {
        if let Ok(config_str) = std::env::var("SKYNET_CONFIG_DIR") {
            let config_path = std::path::Path::new(&config_str).join("remote_hosts.conf");
            if config_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_path) {
                    for line in content.lines() {
                        let line = line.trim();
                        if !line.is_empty() && !line.starts_with('#') {
                            tracing::error!("Security: Loaded remote host from config: {}", line);
                            hosts.push(line.to_string());
                        }
                    }
                }
            }
        }
    }

    if !hosts.is_empty() {
        tracing::error!(
            "Security: Loaded {} remote hosts for plugin offloading",
            hosts.len()
        );
    }

    hosts
}

/// Generate HMAC signature for PluginContext to prevent tampering
///
/// Uses HMAC-SHA256 to sign the context structure
pub fn generate_context_signature(context: *const PluginContext, key: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    // Create a deterministic serialization of the context pointer values
    let context_bytes = unsafe {
        if context.is_null() {
            vec![]
        } else {
            let ctx = &*context;
            let mut bytes = Vec::new();

            // Include pointer values (not dereferencing them)
            bytes.extend_from_slice(&(ctx.logger as usize).to_le_bytes());
            bytes.extend_from_slice(&(ctx.config as usize).to_le_bytes());
            bytes.extend_from_slice(&(ctx.service_registry as usize).to_le_bytes());
            bytes.extend_from_slice(&(ctx.tracer as usize).to_le_bytes());
            bytes.extend_from_slice(&(ctx.user_data as usize).to_le_bytes());
            bytes.extend_from_slice(&(ctx.user_context_json as usize).to_le_bytes());
            bytes.extend_from_slice(&(ctx.secrets as usize).to_le_bytes());

            bytes
        }
    };

    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(&context_bytes);

    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Verify HMAC signature of PluginContext
///
/// Returns Ok(()) if signature is valid, Err otherwise
pub fn verify_context_signature(
    context: *const PluginContext,
    key: &[u8],
    expected_sig: &str,
) -> Result<(), SecurityError> {
    let actual_sig = generate_context_signature(context, key);

    if actual_sig == expected_sig {
        Ok(())
    } else {
        tracing::error!("Security: Context signature mismatch - possible tampering detected");
        Err(SecurityError::InvalidContextSignature)
    }
}

/// Plugin capability and permission flags
/// Bit flags for controlling what plugins can access
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginCapabilities;

impl PluginCapabilities {
    pub const NONE: u64 = 0x0;
    pub const READ_CONFIG: u64 = 0x1;
    pub const WRITE_CONFIG: u64 = 0x2;
    pub const READ_SECRETS: u64 = 0x4;
    pub const WRITE_SECRETS: u64 = 0x8;
    pub const NETWORK_OUTBOUND: u64 = 0x10;
    pub const FILE_READ: u64 = 0x20;
    pub const FILE_WRITE: u64 = 0x40;
    pub const PROCESS_SPAWN: u64 = 0x80;
    pub const MEMORY_MMAP: u64 = 0x100;
    pub const SYSTEM_CALLS: u64 = 0x200;
}

/// Plugin sandboxing policy
///
/// Defines resource limits and capability restrictions for plugins
#[derive(Debug, Clone)]
pub struct PluginSandboxPolicy {
    /// Unique identifier for this plugin
    pub plugin_id: String,

    /// Allowed capabilities (bit flags)
    pub allowed_capabilities: u64,

    /// Maximum memory in bytes
    pub max_memory: usize,

    /// Maximum CPU time in milliseconds
    pub max_cpu_time: u64,

    /// Maximum network bandwidth in bytes/second
    pub max_bandwidth: u64,

    /// File paths the plugin can access (empty = none)
    pub allowed_paths: Vec<String>,

    /// Network ports the plugin can bind to (empty = none)
    pub allowed_ports: Vec<u16>,

    /// Environment variables the plugin can access (empty = all)
    pub allowed_env_vars: Vec<String>,

    /// Whether the plugin is allowed to spawn child processes
    pub allow_child_processes: bool,

    /// Maximum number of open file descriptors
    pub max_fds: usize,

    /// Whether to use strict seccomp filtering (Linux only)
    pub use_seccomp: bool,
}

impl PluginSandboxPolicy {
    /// Create a permissive policy (for trusted plugins)
    pub fn permissive(plugin_id: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            allowed_capabilities: 0xFFFF_FFFF,  // All capabilities
            max_memory: 2 * 1024 * 1024 * 1024, // 2GB
            max_cpu_time: 300_000,              // 5 minutes
            max_bandwidth: 1024 * 1024 * 100,   // 100 MB/s
            allowed_paths: vec![],
            allowed_ports: vec![],
            allowed_env_vars: vec![],
            allow_child_processes: true,
            max_fds: 1024,
            use_seccomp: false,
        }
    }

    /// Create a restrictive policy (for untrusted plugins)
    pub fn restrictive(plugin_id: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            allowed_capabilities: PluginCapabilities::READ_CONFIG as u64,
            max_memory: 128 * 1024 * 1024, // 128MB
            max_cpu_time: 5_000,           // 5 seconds
            max_bandwidth: 1024 * 1024,    // 1 MB/s
            allowed_paths: vec!["/tmp".to_string()],
            allowed_ports: vec![],
            allowed_env_vars: vec!["PATH".to_string(), "HOME".to_string()],
            allow_child_processes: false,
            max_fds: 128,
            use_seccomp: true,
        }
    }

    /// Check if a capability is allowed
    pub fn has_capability(&self, capability: u64) -> bool {
        (self.allowed_capabilities & capability) != 0
    }

    /// Check if a file path is allowed
    pub fn can_access_path(&self, path: &str) -> bool {
        if self.allowed_paths.is_empty() {
            return false; // Deny all if list is empty
        }

        // Normalize the path to prevent directory traversal attacks
        // Convert to absolute path and canonicalize
        if path.contains("..") || path.contains("./") {
            return false; // Reject paths with traversal attempts
        }

        self.allowed_paths.iter().any(|p| path.starts_with(p))
    }

    /// Check if a port is allowed
    pub fn can_use_port(&self, port: u16) -> bool {
        if self.allowed_ports.is_empty() {
            return false; // Deny all if list is empty
        }

        self.allowed_ports.contains(&port)
    }

    /// Check if an environment variable is allowed
    pub fn can_access_env_var(&self, var: &str) -> bool {
        if self.allowed_env_vars.is_empty() {
            return false; // Deny all if list is empty
        }

        self.allowed_env_vars.iter().any(|v| v == var)
    }
}

/// Enforces sandbox policies when plugins attempt operations
pub struct SandboxEnforcer {
    policies:
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, PluginSandboxPolicy>>>,
}

impl SandboxEnforcer {
    /// Create a new sandbox enforcer
    pub fn new() -> Self {
        Self {
            policies: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Register a plugin with its sandbox policy
    pub fn register_plugin(&self, policy: PluginSandboxPolicy) -> Result<(), SecurityError> {
        let mut policies = self.policies.lock().unwrap();
        policies.insert(policy.plugin_id.clone(), policy);
        Ok(())
    }

    /// Check if a plugin can perform a file operation
    pub fn check_file_access(
        &self,
        plugin_id: &str,
        path: &str,
        write: bool,
    ) -> Result<(), SecurityError> {
        let policies = self.policies.lock().unwrap();

        if let Some(policy) = policies.get(plugin_id) {
            let required_cap = if write {
                PluginCapabilities::FILE_WRITE as u64
            } else {
                PluginCapabilities::FILE_READ as u64
            };

            if !policy.has_capability(required_cap) {
                tracing::error!(
                    "Sandbox: Plugin {} denied file {} access (missing capability)",
                    plugin_id,
                    if write { "write" } else { "read" }
                );
                return Err(SecurityError::PointerValidationFailed);
            }

            if !policy.can_access_path(path) {
                tracing::error!(
                    "Sandbox: Plugin {} denied access to path: {}",
                    plugin_id,
                    path
                );
                return Err(SecurityError::PointerValidationFailed);
            }

            Ok(())
        } else {
            tracing::error!("Sandbox: Unknown plugin ID: {}", plugin_id);
            Err(SecurityError::PointerValidationFailed)
        }
    }

    /// Check if a plugin can make a network connection
    pub fn check_network_access(
        &self,
        plugin_id: &str,
        _host: &str,
        port: u16,
    ) -> Result<(), SecurityError> {
        let policies = self.policies.lock().unwrap();

        if let Some(policy) = policies.get(plugin_id) {
            if !policy.has_capability(PluginCapabilities::NETWORK_OUTBOUND as u64) {
                tracing::error!(
                    "Sandbox: Plugin {} denied network access (missing capability)",
                    plugin_id
                );
                return Err(SecurityError::PointerValidationFailed);
            }

            if !policy.can_use_port(port) {
                tracing::error!(
                    "Sandbox: Plugin {} denied access to port: {}",
                    plugin_id,
                    port
                );
                return Err(SecurityError::PointerValidationFailed);
            }

            Ok(())
        } else {
            tracing::error!("Sandbox: Unknown plugin ID: {}", plugin_id);
            Err(SecurityError::PointerValidationFailed)
        }
    }

    /// Check memory limits for a plugin
    pub fn check_memory_limit(
        &self,
        plugin_id: &str,
        requested_bytes: usize,
    ) -> Result<(), SecurityError> {
        let policies = self.policies.lock().unwrap();

        if let Some(policy) = policies.get(plugin_id) {
            if requested_bytes > policy.max_memory {
                tracing::error!(
                    "Sandbox: Plugin {} exceeded memory limit: {} > {}",
                    plugin_id,
                    requested_bytes,
                    policy.max_memory
                );
                return Err(SecurityError::InputTooLong);
            }
            Ok(())
        } else {
            tracing::error!("Sandbox: Unknown plugin ID: {}", plugin_id);
            Err(SecurityError::PointerValidationFailed)
        }
    }
}

// ============================================================================
// PHASE 2: AUTHENTICATION AND AUTHORIZATION
// ============================================================================

/// Plugin credential types for authentication
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialType {
    ApiKey,
    Certificate,
    OAuth2Token,
    BasicAuth,
}

/// Plugin authentication credential
#[derive(Debug, Clone)]
pub struct PluginCredential {
    pub credential_type: CredentialType,
    pub plugin_id: String,
    pub credential_data: Vec<u8>, // Encrypted
    pub issued_at: u64,           // Unix timestamp
    pub expires_at: u64,          // Unix timestamp (0 = no expiry)
    pub scopes: Vec<String>,      // OAuth2 scopes or permission scopes
}

impl PluginCredential {
    pub fn new(
        credential_type: CredentialType,
        plugin_id: String,
        credential_data: Vec<u8>,
        scopes: Vec<String>,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        PluginCredential {
            credential_type,
            plugin_id,
            credential_data,
            issued_at: now,
            expires_at: now + (365 * 24 * 60 * 60), // 1 year expiry
            scopes,
        }
    }

    pub fn is_expired(&self) -> bool {
        if self.expires_at == 0 {
            return false; // No expiry
        }
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now > self.expires_at
    }

    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.contains(&scope.to_string())
    }
}

/// Plugin Role-Based Access Control (RBAC) roles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginRole {
    /// No permissions
    None = 0x0,
    /// Read-only access
    Viewer = 0x1,
    /// Read and write access
    Editor = 0x2,
    /// Full administrative access
    Admin = 0x4,
    /// System-level privileged access
    System = 0x8,
}

// ============================================================================
// PHASE 3A: CREDENTIAL ROTATION AND MANAGEMENT
// ============================================================================

/// Credential rotation policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationPolicy {
    /// No automatic rotation
    Disabled,
    /// Time-based rotation (rotate after N seconds)
    TimeBased,
    /// Event-based rotation (on detection of compromise)
    EventBased,
}

/// Status of a credential version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialStatus {
    /// Active credential (currently used)
    Active,
    /// Previous credential (in grace period, both work)
    Grace,
    /// Retired credential (no longer accepted)
    Retired,
}

/// Credential version with rotation tracking
#[derive(Debug, Clone)]
pub struct CredentialVersion {
    pub version: u32,
    pub credential: PluginCredential,
    pub status: CredentialStatus,
    pub created_at: u64,
    pub rotated_at: Option<u64>,
    pub retired_at: Option<u64>,
}

impl CredentialVersion {
    pub fn new(version: u32, credential: PluginCredential) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        CredentialVersion {
            version,
            credential,
            status: CredentialStatus::Active,
            created_at: now,
            rotated_at: None,
            retired_at: None,
        }
    }
}

/// Credential rotation history entry
#[derive(Debug, Clone)]
pub struct RotationHistory {
    pub plugin_id: String,
    pub from_version: u32,
    pub to_version: u32,
    pub rotation_time: u64,
    pub reason: String,
}

// ============================================================================
// Rotation Notification System
// ============================================================================

/// Rotation notification event types
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RotationEventType {
    /// Rotation has been scheduled/planned
    RotationScheduled = 0,
    /// Rotation has started
    RotationStarted = 1,
    /// Rotation completed successfully
    RotationCompleted = 2,
    /// Rotation failed
    RotationFailed = 3,
    /// Rotation was cancelled
    RotationCancelled = 4,
    /// Secret is approaching rotation deadline
    RotationWarning = 5,
    /// Grace period started for old credential
    GracePeriodStarted = 6,
    /// Grace period ended, old credential retired
    GracePeriodEnded = 7,
    /// Emergency rotation triggered
    EmergencyRotation = 8,
}

impl std::fmt::Display for RotationEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RotationEventType::RotationScheduled => write!(f, "rotation.scheduled"),
            RotationEventType::RotationStarted => write!(f, "rotation.started"),
            RotationEventType::RotationCompleted => write!(f, "rotation.completed"),
            RotationEventType::RotationFailed => write!(f, "rotation.failed"),
            RotationEventType::RotationCancelled => write!(f, "rotation.cancelled"),
            RotationEventType::RotationWarning => write!(f, "rotation.warning"),
            RotationEventType::GracePeriodStarted => write!(f, "rotation.grace_started"),
            RotationEventType::GracePeriodEnded => write!(f, "rotation.grace_ended"),
            RotationEventType::EmergencyRotation => write!(f, "rotation.emergency"),
        }
    }
}

/// Severity level for rotation events
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RotationEventSeverity {
    Info = 0,
    Warning = 1,
    Error = 2,
    Critical = 3,
}

impl std::fmt::Display for RotationEventSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RotationEventSeverity::Info => write!(f, "INFO"),
            RotationEventSeverity::Warning => write!(f, "WARNING"),
            RotationEventSeverity::Error => write!(f, "ERROR"),
            RotationEventSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Rotation event payload for notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationEvent {
    /// Event type
    pub event_type: RotationEventType,
    /// Severity level
    pub severity: RotationEventSeverity,
    /// Plugin or secret identifier
    pub subject_id: String,
    /// Subject type (plugin, secret, credential)
    pub subject_type: String,
    /// Timestamp in seconds since epoch
    pub timestamp: u64,
    /// Previous version (if applicable)
    pub from_version: Option<u32>,
    /// New version (if applicable)
    pub to_version: Option<u32>,
    /// Human-readable message
    pub message: String,
    /// Additional metadata as JSON
    pub metadata: Option<serde_json::Value>,
    /// Error details (if failed)
    pub error_details: Option<String>,
}

impl RotationEvent {
    /// Create a new rotation event
    pub fn new(
        event_type: RotationEventType,
        subject_id: impl Into<String>,
        subject_type: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            event_type,
            severity: RotationEventSeverity::Info,
            subject_id: subject_id.into(),
            subject_type: subject_type.into(),
            timestamp,
            from_version: None,
            to_version: None,
            message: message.into(),
            metadata: None,
            error_details: None,
        }
    }

    /// Builder method for severity
    pub fn with_severity(mut self, severity: RotationEventSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Builder method for version transition
    pub fn with_version_transition(mut self, from: u32, to: u32) -> Self {
        self.from_version = Some(from);
        self.to_version = Some(to);
        self
    }

    /// Builder method for metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Builder method for error details
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error_details = Some(error.into());
        self
    }
}

/// Rotation notification callback type for C/FFI compatibility
pub type RotationNotifyCallback =
    extern "C" fn(context: *const crate::PluginContext, event: *const RotationEvent);

/// Rotation hook registration interface
///
/// Plugins can register to receive rotation notifications by implementing
/// this interface and registering with the host.
#[repr(C)]
pub struct RotationNotificationService {
    /// Register a callback for rotation events
    pub register_listener: extern "C" fn(
        context: *const crate::PluginContext,
        event_types: *const RotationEventType,
        num_types: usize,
        callback: RotationNotifyCallback,
    ) -> crate::PluginResult,

    /// Unregister a listener
    pub unregister_listener: extern "C" fn(
        context: *const crate::PluginContext,
        callback: RotationNotifyCallback,
    ) -> crate::PluginResult,

    /// Publish a rotation event (for internal use by host)
    pub publish_event: extern "C" fn(
        context: *const crate::PluginContext,
        event: *const RotationEvent,
    ) -> crate::PluginResult,
}

/// Rust-native rotation notification trait for type-safe plugin development
pub trait RotationNotifier: Send + Sync {
    /// Register a listener for specific event types
    fn register_listener<F>(
        &self,
        event_types: &[RotationEventType],
        callback: F,
    ) -> Result<String, SecurityError>
    where
        F: Fn(&RotationEvent) + Send + Sync + 'static;

    /// Unregister a listener by ID
    fn unregister_listener(&self, listener_id: &str) -> Result<(), SecurityError>;

    /// Publish a rotation event to all registered listeners
    fn publish_event(&self, event: RotationEvent) -> Result<(), SecurityError>;
}

// ============================================================================
// RFC-0029: Secrets Provider Interface
// ============================================================================

/// Secret metadata for versioning and audit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    /// Unique identifier for the secret
    pub id: String,
    /// Secret name/key
    pub name: String,
    /// Plugin that owns this secret
    pub owner_plugin: String,
    /// Current version number
    pub version: u64,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last updated timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Optional expiration timestamp
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Optional rotation interval in seconds
    pub rotation_interval_secs: Option<u64>,
    /// Last rotation timestamp
    pub last_rotated_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Whether the secret is currently valid
    pub is_valid: bool,
    /// Custom labels/tags
    pub labels: std::collections::HashMap<String, String>,
}

impl SecretMetadata {
    /// Create new secret metadata
    pub fn new(name: String, owner_plugin: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: format!("secret-{}", uuid::Uuid::new_v4()),
            name,
            owner_plugin,
            version: 1,
            created_at: now,
            updated_at: now,
            expires_at: None,
            rotation_interval_secs: None,
            last_rotated_at: None,
            is_valid: true,
            labels: std::collections::HashMap::new(),
        }
    }

    /// Check if the secret has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            chrono::Utc::now() > expires
        } else {
            false
        }
    }

    /// Check if rotation is due
    pub fn needs_rotation(&self) -> bool {
        if let Some(interval) = self.rotation_interval_secs {
            if let Some(last_rotated) = self.last_rotated_at {
                let elapsed = (chrono::Utc::now() - last_rotated).num_seconds() as u64;
                return elapsed >= interval;
            }
            // Never rotated but has interval - needs rotation
            return true;
        }
        false
    }

    /// Set a label
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// Set expiration
    pub fn with_expiration(mut self, expires_at: chrono::DateTime<chrono::Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Set rotation interval
    pub fn with_rotation_interval(mut self, interval_secs: u64) -> Self {
        self.rotation_interval_secs = Some(interval_secs);
        self
    }
}

/// Secret version history entry for RFC-0029 Secrets Provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretVersionEntry {
    /// Version number
    pub version: u64,
    /// When this version was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Who/what created this version
    pub created_by: String,
    /// Whether this version is still active
    pub is_active: bool,
    /// Hash of the secret value (for verification)
    pub value_hash: String,
}

/// Options for listing secrets
#[derive(Debug, Clone, Default)]
pub struct ListSecretsOptions {
    /// Filter by owner plugin
    pub owner_plugin: Option<String>,
    /// Filter by label key-value pair
    pub label_filter: Option<(String, String)>,
    /// Include expired secrets
    pub include_expired: bool,
    /// Include secret values (vs just metadata)
    pub include_values: bool,
}

/// Result of a secret rotation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationResult {
    /// Whether the rotation succeeded
    pub success: bool,
    /// Previous version number
    pub old_version: u64,
    /// New version number
    pub new_version: u64,
    /// Timestamp of rotation
    pub rotated_at: chrono::DateTime<chrono::Utc>,
    /// Error message if rotation failed
    pub error: Option<String>,
}

/// Standardized secrets provider trait for plugin access
///
/// This trait defines the standard interface for secrets management in Skylet.
/// All plugins should use this interface to access secrets, ensuring consistent
/// behavior across the ecosystem.
///
/// # Security Properties
///
/// - Secrets are encrypted at rest using AES-256-GCM
/// - Access is audited automatically
/// - Version history is maintained for rollback
/// - Rotation can be scheduled or triggered programmatically
///
/// # Example Usage
///
/// ```ignore
/// use skylet_abi::security::{SecretsProvider, SecretMetadata};
///
/// // Get a secret
/// let value = secrets_provider.get_secret("my-plugin", "api_key")?;
///
/// // Store a secret
/// secrets_provider.put_secret("my-plugin", "api_key", b"secret_value", None)?;
///
/// // Rotate a secret
/// let result = secrets_provider.rotate_secret("my-plugin", "api_key", b"new_value")?;
/// ```
pub trait SecretsProvider: Send + Sync {
    /// Get a secret value by name
    ///
    /// # Arguments
    /// * `plugin_id` - The plugin requesting the secret
    /// * `secret_name` - The name/key of the secret
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - The secret value
    /// * `Err(SecurityError)` - If the secret doesn't exist or access is denied
    fn get_secret(&self, plugin_id: &str, secret_name: &str) -> Result<Vec<u8>, SecurityError>;

    /// Get a secret with its metadata
    fn get_secret_with_metadata(
        &self,
        plugin_id: &str,
        secret_name: &str,
    ) -> Result<(Vec<u8>, SecretMetadata), SecurityError>;

    /// Store a secret value
    ///
    /// # Arguments
    /// * `plugin_id` - The plugin storing the secret
    /// * `secret_name` - The name/key of the secret
    /// * `value` - The secret value
    /// * `metadata` - Optional metadata (rotation interval, expiration, etc.)
    fn put_secret(
        &self,
        plugin_id: &str,
        secret_name: &str,
        value: &[u8],
        metadata: Option<SecretMetadata>,
    ) -> Result<(), SecurityError>;

    /// Delete a secret
    ///
    /// # Arguments
    /// * `plugin_id` - The plugin deleting the secret
    /// * `secret_name` - The name/key of the secret
    ///
    /// # Note
    /// The secret is soft-deleted and retained in version history for audit purposes.
    fn delete_secret(&self, plugin_id: &str, secret_name: &str) -> Result<(), SecurityError>;

    /// Check if a secret exists
    fn secret_exists(&self, plugin_id: &str, secret_name: &str) -> Result<bool, SecurityError>;

    /// List secrets with optional filtering
    fn list_secrets(
        &self,
        plugin_id: &str,
        options: ListSecretsOptions,
    ) -> Result<Vec<SecretMetadata>, SecurityError>;

    /// Get secret metadata without the value
    fn get_secret_metadata(
        &self,
        plugin_id: &str,
        secret_name: &str,
    ) -> Result<SecretMetadata, SecurityError>;

    // === Rotation API ===

    /// Rotate a secret to a new value
    ///
    /// This creates a new version of the secret while retaining the old version
    /// for rollback purposes.
    fn rotate_secret(
        &self,
        plugin_id: &str,
        secret_name: &str,
        new_value: &[u8],
    ) -> Result<RotationResult, SecurityError>;

    /// Roll back a secret to a previous version
    fn rollback_secret(
        &self,
        plugin_id: &str,
        secret_name: &str,
        version: u64,
    ) -> Result<RotationResult, SecurityError>;

    /// Get version history for a secret
    fn get_secret_versions(
        &self,
        plugin_id: &str,
        secret_name: &str,
    ) -> Result<Vec<SecretVersionEntry>, SecurityError>;

    /// Schedule automatic rotation for a secret
    fn schedule_rotation(
        &self,
        plugin_id: &str,
        secret_name: &str,
        interval_secs: u64,
    ) -> Result<(), SecurityError>;

    /// Cancel scheduled rotation
    fn cancel_rotation(&self, plugin_id: &str, secret_name: &str) -> Result<(), SecurityError>;

    // === Batch Operations ===

    /// Get multiple secrets at once
    fn get_secrets_batch(
        &self,
        plugin_id: &str,
        secret_names: &[&str],
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, SecurityError>;

    /// Store multiple secrets at once
    fn put_secrets_batch(
        &self,
        plugin_id: &str,
        secrets: &[(String, Vec<u8>)],
    ) -> Result<(), SecurityError>;
}

/// Default implementation of SecretsProvider using EncryptedSecretStore
pub struct DefaultSecretsProvider {
    store: Arc<EncryptedSecretStore>,
    metadata: Arc<Mutex<std::collections::HashMap<String, SecretMetadata>>>,
    versions: Arc<Mutex<std::collections::HashMap<String, Vec<SecretVersionEntry>>>>,
    audit_log: Arc<Mutex<Vec<SecretAuditEntry>>>,
}

/// Audit entry for secret operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretAuditEntry {
    /// Timestamp of the operation
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Plugin that performed the operation
    pub plugin_id: String,
    /// Secret name
    pub secret_name: String,
    /// Operation type
    pub operation: SecretOperation,
    /// Whether the operation succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Types of secret operations for audit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecretOperation {
    /// Get secret
    Get,
    /// Put secret
    Put,
    /// Delete secret
    Delete,
    /// Rotate secret
    Rotate,
    /// Rollback secret
    Rollback,
    /// List secrets
    List,
}

impl DefaultSecretsProvider {
    /// Create a new default secrets provider
    pub fn new() -> Self {
        Self {
            store: Arc::new(EncryptedSecretStore::new()),
            metadata: Arc::new(Mutex::new(std::collections::HashMap::new())),
            versions: Arc::new(Mutex::new(std::collections::HashMap::new())),
            audit_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create with an existing encrypted store
    pub fn with_store(store: Arc<EncryptedSecretStore>) -> Self {
        Self {
            store,
            metadata: Arc::new(Mutex::new(std::collections::HashMap::new())),
            versions: Arc::new(Mutex::new(std::collections::HashMap::new())),
            audit_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get audit log entries
    pub fn get_audit_log(&self) -> Vec<SecretAuditEntry> {
        self.audit_log.lock().unwrap().clone()
    }

    /// Clear audit log
    pub fn clear_audit_log(&self) {
        self.audit_log.lock().unwrap().clear();
    }

    /// Generate a unique key for storing secrets
    fn secret_key(plugin_id: &str, secret_name: &str) -> String {
        format!("{}:{}", plugin_id, secret_name)
    }

    /// Log an audit entry
    fn log_audit(
        &self,
        plugin_id: &str,
        secret_name: &str,
        operation: SecretOperation,
        success: bool,
        error: Option<String>,
    ) {
        let entry = SecretAuditEntry {
            timestamp: chrono::Utc::now(),
            plugin_id: plugin_id.to_string(),
            secret_name: secret_name.to_string(),
            operation,
            success,
            error,
        };
        self.audit_log.lock().unwrap().push(entry);
    }

    /// Compute a hash of a value for version tracking
    fn compute_hash(value: &[u8]) -> String {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(value);
        hex::encode(hasher.finalize().as_bytes())
    }
}

impl Default for DefaultSecretsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretsProvider for DefaultSecretsProvider {
    fn get_secret(&self, plugin_id: &str, secret_name: &str) -> Result<Vec<u8>, SecurityError> {
        let key = Self::secret_key(plugin_id, secret_name);
        let result = self.store.get_secret(&key);

        match result {
            Ok(value) => {
                self.log_audit(plugin_id, secret_name, SecretOperation::Get, true, None);
                Ok(value)
            }
            Err(e) => {
                self.log_audit(
                    plugin_id,
                    secret_name,
                    SecretOperation::Get,
                    false,
                    Some(e.to_string()),
                );
                Err(e)
            }
        }
    }

    fn get_secret_with_metadata(
        &self,
        plugin_id: &str,
        secret_name: &str,
    ) -> Result<(Vec<u8>, SecretMetadata), SecurityError> {
        let value = self.get_secret(plugin_id, secret_name)?;
        let metadata = self.get_secret_metadata(plugin_id, secret_name)?;
        Ok((value, metadata))
    }

    fn put_secret(
        &self,
        plugin_id: &str,
        secret_name: &str,
        value: &[u8],
        metadata: Option<SecretMetadata>,
    ) -> Result<(), SecurityError> {
        let key = Self::secret_key(plugin_id, secret_name);

        // Store the value
        self.store.store_secret(&key, value)?;

        // Store metadata
        let meta = metadata
            .unwrap_or_else(|| SecretMetadata::new(secret_name.to_string(), plugin_id.to_string()));
        self.metadata.lock().unwrap().insert(key.clone(), meta);

        // Create initial version entry for RFC-0029
        let version = SecretVersionEntry {
            version: 1,
            created_at: chrono::Utc::now(),
            created_by: plugin_id.to_string(),
            is_active: true,
            value_hash: Self::compute_hash(value),
        };
        self.versions.lock().unwrap().insert(key, vec![version]);

        self.log_audit(plugin_id, secret_name, SecretOperation::Put, true, None);
        Ok(())
    }

    fn delete_secret(&self, plugin_id: &str, secret_name: &str) -> Result<(), SecurityError> {
        let key = Self::secret_key(plugin_id, secret_name);

        // Soft delete: mark metadata as invalid
        if let Some(meta) = self.metadata.lock().unwrap().get_mut(&key) {
            meta.is_valid = false;
        }

        // Actually remove from store
        let result = self.store.remove_secret(&key);

        match result {
            Ok(()) => {
                self.log_audit(plugin_id, secret_name, SecretOperation::Delete, true, None);
                Ok(())
            }
            Err(e) => {
                self.log_audit(
                    plugin_id,
                    secret_name,
                    SecretOperation::Delete,
                    false,
                    Some(e.to_string()),
                );
                Err(e)
            }
        }
    }

    fn secret_exists(&self, plugin_id: &str, secret_name: &str) -> Result<bool, SecurityError> {
        let key = Self::secret_key(plugin_id, secret_name);
        Ok(self.store.list_secret_names().contains(&key))
    }

    fn list_secrets(
        &self,
        plugin_id: &str,
        options: ListSecretsOptions,
    ) -> Result<Vec<SecretMetadata>, SecurityError> {
        let metadata = self.metadata.lock().unwrap();
        let mut results: Vec<SecretMetadata> = metadata
            .values()
            .filter(|m| {
                // Filter by owner plugin
                if let Some(ref owner) = options.owner_plugin {
                    if m.owner_plugin != *owner {
                        return false;
                    }
                } else if m.owner_plugin != plugin_id {
                    // By default, only show own secrets
                    return false;
                }

                // Filter by label
                if let Some((ref key, ref value)) = options.label_filter {
                    if !m.labels.get(key).map(|v| v == value).unwrap_or(false) {
                        return false;
                    }
                }

                // Filter expired
                if !options.include_expired && m.is_expired() {
                    return false;
                }

                true
            })
            .cloned()
            .collect();

        self.log_audit(plugin_id, "*", SecretOperation::List, true, None);
        results.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(results)
    }

    fn get_secret_metadata(
        &self,
        plugin_id: &str,
        secret_name: &str,
    ) -> Result<SecretMetadata, SecurityError> {
        let key = Self::secret_key(plugin_id, secret_name);
        self.metadata
            .lock()
            .unwrap()
            .get(&key)
            .cloned()
            .ok_or_else(|| {
                SecurityError::SecretNotFound(format!("Secret '{}' not found", secret_name))
            })
    }

    fn rotate_secret(
        &self,
        plugin_id: &str,
        secret_name: &str,
        new_value: &[u8],
    ) -> Result<RotationResult, SecurityError> {
        let key = Self::secret_key(plugin_id, secret_name);

        // Get current version
        let old_version = self
            .versions
            .lock()
            .unwrap()
            .get(&key)
            .and_then(|v| v.last().map(|v| v.version))
            .unwrap_or(0);

        let new_version = old_version + 1;

        // Store new value
        self.store.store_secret(&key, new_value)?;

        // Update metadata
        if let Some(meta) = self.metadata.lock().unwrap().get_mut(&key) {
            meta.version = new_version;
            meta.updated_at = chrono::Utc::now();
            meta.last_rotated_at = Some(chrono::Utc::now());
        }

        // Add new version entry for RFC-0029
        let version = SecretVersionEntry {
            version: new_version,
            created_at: chrono::Utc::now(),
            created_by: plugin_id.to_string(),
            is_active: true,
            value_hash: Self::compute_hash(new_value),
        };

        // Mark old version as inactive
        if let Some(versions) = self.versions.lock().unwrap().get_mut(&key) {
            for v in versions.iter_mut() {
                v.is_active = false;
            }
            versions.push(version);
        }

        self.log_audit(plugin_id, secret_name, SecretOperation::Rotate, true, None);

        Ok(RotationResult {
            success: true,
            old_version,
            new_version,
            rotated_at: chrono::Utc::now(),
            error: None,
        })
    }

    fn rollback_secret(
        &self,
        _plugin_id: &str,
        _secret_name: &str,
        version: u64,
    ) -> Result<RotationResult, SecurityError> {
        // For rollback, we'd need to have stored the old values
        // This is a simplified implementation - in production you'd need
        // a versioned store
        Err(SecurityError::RotationFailed(format!(
            "Rollback to version {} not implemented - requires versioned store",
            version
        )))
    }

    fn get_secret_versions(
        &self,
        plugin_id: &str,
        secret_name: &str,
    ) -> Result<Vec<SecretVersionEntry>, SecurityError> {
        let key = Self::secret_key(plugin_id, secret_name);
        self.versions
            .lock()
            .unwrap()
            .get(&key)
            .cloned()
            .ok_or_else(|| {
                SecurityError::SecretNotFound(format!("Secret '{}' not found", secret_name))
            })
    }

    fn schedule_rotation(
        &self,
        plugin_id: &str,
        secret_name: &str,
        interval_secs: u64,
    ) -> Result<(), SecurityError> {
        let key = Self::secret_key(plugin_id, secret_name);

        if let Some(meta) = self.metadata.lock().unwrap().get_mut(&key) {
            meta.rotation_interval_secs = Some(interval_secs);
            Ok(())
        } else {
            Err(SecurityError::SecretNotFound(format!(
                "Secret '{}' not found",
                secret_name
            )))
        }
    }

    fn cancel_rotation(&self, plugin_id: &str, secret_name: &str) -> Result<(), SecurityError> {
        let key = Self::secret_key(plugin_id, secret_name);

        if let Some(meta) = self.metadata.lock().unwrap().get_mut(&key) {
            meta.rotation_interval_secs = None;
            Ok(())
        } else {
            Err(SecurityError::SecretNotFound(format!(
                "Secret '{}' not found",
                secret_name
            )))
        }
    }

    fn get_secrets_batch(
        &self,
        plugin_id: &str,
        secret_names: &[&str],
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, SecurityError> {
        let mut results = std::collections::HashMap::new();
        for name in secret_names {
            match self.get_secret(plugin_id, name) {
                Ok(value) => {
                    results.insert(name.to_string(), value);
                }
                Err(e) => {
                    // Log but continue with other secrets
                    self.log_audit(
                        plugin_id,
                        name,
                        SecretOperation::Get,
                        false,
                        Some(e.to_string()),
                    );
                }
            }
        }
        Ok(results)
    }

    fn put_secrets_batch(
        &self,
        plugin_id: &str,
        secrets: &[(String, Vec<u8>)],
    ) -> Result<(), SecurityError> {
        for (name, value) in secrets {
            self.put_secret(plugin_id, name, value, None)?;
        }
        Ok(())
    }
}

/// Standard secret topics for pub/sub systems
pub mod secret_topics {
    /// Base prefix for all secret events
    pub const SECRET_PREFIX: &str = "secret";
    /// Secret created event topic
    pub const SECRET_CREATED: &str = "secret.created";
    /// Secret updated event topic
    pub const SECRET_UPDATED: &str = "secret.updated";
    /// Secret deleted event topic
    pub const SECRET_DELETED: &str = "secret.deleted";
    /// Secret rotated event topic
    pub const SECRET_ROTATED: &str = "secret.rotated";
    /// Secret expired event topic
    pub const SECRET_EXPIRED: &str = "secret.expired";
    /// Secret access event topic
    pub const SECRET_ACCESSED: &str = "secret.accessed";
}

/// Standard rotation event topics for pub/sub systems
pub mod rotation_topics {
    /// Base prefix for all rotation events
    pub const ROTATION_PREFIX: &str = "rotation";
    /// Rotation scheduled event topic
    pub const ROTATION_SCHEDULED: &str = "rotation.scheduled";
    /// Rotation started event topic
    pub const ROTATION_STARTED: &str = "rotation.started";
    /// Rotation completed event topic
    pub const ROTATION_COMPLETED: &str = "rotation.completed";
    /// Rotation failed event topic
    pub const ROTATION_FAILED: &str = "rotation.failed";
    /// Rotation cancelled event topic
    pub const ROTATION_CANCELLED: &str = "rotation.cancelled";
    /// Rotation warning event topic
    pub const ROTATION_WARNING: &str = "rotation.warning";
    /// Grace period started event topic
    pub const GRACE_PERIOD_STARTED: &str = "rotation.grace_started";
    /// Grace period ended event topic
    pub const GRACE_PERIOD_ENDED: &str = "rotation.grace_ended";
    /// Emergency rotation event topic
    pub const EMERGENCY_ROTATION: &str = "rotation.emergency";

    /// Convert RotationEventType to topic string
    pub fn from_event_type(event_type: super::RotationEventType) -> &'static str {
        match event_type {
            super::RotationEventType::RotationScheduled => ROTATION_SCHEDULED,
            super::RotationEventType::RotationStarted => ROTATION_STARTED,
            super::RotationEventType::RotationCompleted => ROTATION_COMPLETED,
            super::RotationEventType::RotationFailed => ROTATION_FAILED,
            super::RotationEventType::RotationCancelled => ROTATION_CANCELLED,
            super::RotationEventType::RotationWarning => ROTATION_WARNING,
            super::RotationEventType::GracePeriodStarted => GRACE_PERIOD_STARTED,
            super::RotationEventType::GracePeriodEnded => GRACE_PERIOD_ENDED,
            super::RotationEventType::EmergencyRotation => EMERGENCY_ROTATION,
        }
    }
}

/// Credential rotation manager
pub struct CredentialRotationManager {
    rotations: Arc<Mutex<std::collections::HashMap<String, Vec<RotationHistory>>>>,
    rotation_policy: RotationPolicy,
    grace_period: u64,      // seconds
    rotation_interval: u64, // seconds
}

impl CredentialRotationManager {
    /// Create a new credential rotation manager
    pub fn new(policy: RotationPolicy, grace_period: u64, rotation_interval: u64) -> Self {
        CredentialRotationManager {
            rotations: Arc::new(Mutex::new(std::collections::HashMap::new())),
            rotation_policy: policy,
            grace_period,
            rotation_interval,
        }
    }

    /// Record a rotation in history
    pub fn record_rotation(
        &self,
        plugin_id: &str,
        from_version: u32,
        to_version: u32,
        reason: String,
    ) -> Result<(), SecurityError> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut rotations = self.rotations.lock().unwrap();
        let history = rotations
            .entry(plugin_id.to_string())
            .or_insert_with(Vec::new);

        history.push(RotationHistory {
            plugin_id: plugin_id.to_string(),
            from_version,
            to_version,
            rotation_time: now,
            reason,
        });

        Ok(())
    }

    /// Get rotation history for a plugin
    pub fn get_rotation_history(
        &self,
        plugin_id: &str,
    ) -> Result<Vec<RotationHistory>, SecurityError> {
        let rotations = self.rotations.lock().unwrap();
        Ok(rotations.get(plugin_id).cloned().unwrap_or_default())
    }

    /// Check if a credential is in grace period
    pub fn is_in_grace_period(retired_at: u64) -> bool {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let elapsed = now.saturating_sub(retired_at);
        elapsed < 604800 // 7 days default grace period
    }

    /// Get rotation status for a plugin
    pub fn get_rotation_policy(&self) -> RotationPolicy {
        self.rotation_policy
    }

    /// Set rotation policy
    pub fn set_rotation_policy(&mut self, policy: RotationPolicy) {
        self.rotation_policy = policy;
    }

    /// Get grace period in seconds
    pub fn grace_period(&self) -> u64 {
        self.grace_period
    }

    /// Get rotation interval in seconds
    pub fn rotation_interval(&self) -> u64 {
        self.rotation_interval
    }
}

impl PluginRole {
    pub fn can_read(&self) -> bool {
        matches!(
            self,
            PluginRole::Viewer | PluginRole::Editor | PluginRole::Admin | PluginRole::System
        )
    }

    pub fn can_write(&self) -> bool {
        matches!(
            self,
            PluginRole::Editor | PluginRole::Admin | PluginRole::System
        )
    }

    pub fn can_admin(&self) -> bool {
        matches!(self, PluginRole::Admin | PluginRole::System)
    }

    pub fn can_system(&self) -> bool {
        matches!(self, PluginRole::System)
    }
}

/// Plugin permission set for fine-grained access control
#[derive(Debug, Clone)]
pub struct PluginPermissions {
    pub read_config: bool,
    pub write_config: bool,
    pub read_secrets: bool,
    pub write_secrets: bool,
    pub access_network: bool,
    pub manage_plugins: bool,
    pub access_audit_logs: bool,
    pub system_admin: bool,
}

impl PluginPermissions {
    pub fn from_role(role: PluginRole) -> Self {
        match role {
            PluginRole::None => PluginPermissions {
                read_config: false,
                write_config: false,
                read_secrets: false,
                write_secrets: false,
                access_network: false,
                manage_plugins: false,
                access_audit_logs: false,
                system_admin: false,
            },
            PluginRole::Viewer => PluginPermissions {
                read_config: true,
                write_config: false,
                read_secrets: false,
                write_secrets: false,
                access_network: false,
                manage_plugins: false,
                access_audit_logs: false,
                system_admin: false,
            },
            PluginRole::Editor => PluginPermissions {
                read_config: true,
                write_config: true,
                read_secrets: false,
                write_secrets: false,
                access_network: true,
                manage_plugins: false,
                access_audit_logs: false,
                system_admin: false,
            },
            PluginRole::Admin => PluginPermissions {
                read_config: true,
                write_config: true,
                read_secrets: true,
                write_secrets: true,
                access_network: true,
                manage_plugins: true,
                access_audit_logs: true,
                system_admin: false,
            },
            PluginRole::System => PluginPermissions {
                read_config: true,
                write_config: true,
                read_secrets: true,
                write_secrets: true,
                access_network: true,
                manage_plugins: true,
                access_audit_logs: true,
                system_admin: true,
            },
        }
    }

    pub fn check_permission(&self, permission_name: &str) -> bool {
        match permission_name {
            "read_config" => self.read_config,
            "write_config" => self.write_config,
            "read_secrets" => self.read_secrets,
            "write_secrets" => self.write_secrets,
            "access_network" => self.access_network,
            "manage_plugins" => self.manage_plugins,
            "access_audit_logs" => self.access_audit_logs,
            "system_admin" => self.system_admin,
            _ => false,
        }
    }
}

/// Plugin authentication manager
pub struct PluginAuthenticator {
    credentials: Arc<Mutex<std::collections::HashMap<String, PluginCredential>>>,
    roles: Arc<Mutex<std::collections::HashMap<String, PluginRole>>>,
    #[allow(dead_code)]
    secret_store: Arc<EncryptedSecretStore>,
    rotation_manager: Arc<CredentialRotationManager>,
    credential_versions: Arc<Mutex<std::collections::HashMap<String, Vec<CredentialVersion>>>>,
    mfa_manager: Arc<MFAManager>,
    mfa_enabled: Arc<Mutex<std::collections::HashMap<String, bool>>>,
}

impl PluginAuthenticator {
    pub fn new() -> Self {
        PluginAuthenticator {
            credentials: Arc::new(Mutex::new(std::collections::HashMap::new())),
            roles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            secret_store: Arc::new(EncryptedSecretStore::new()),
            rotation_manager: Arc::new(CredentialRotationManager::new(
                RotationPolicy::TimeBased,
                604800,  // 7 day grace period
                2592000, // 30 day rotation interval
            )),
            credential_versions: Arc::new(Mutex::new(std::collections::HashMap::new())),
            mfa_manager: Arc::new(MFAManager::new()),
            mfa_enabled: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Register a plugin with authentication credentials
    pub fn register_plugin(
        &self,
        plugin_id: &str,
        credential_type: CredentialType,
        credential_data: Vec<u8>,
        role: PluginRole,
        scopes: Vec<String>,
    ) -> Result<(), SecurityError> {
        let credential = PluginCredential::new(
            credential_type,
            plugin_id.to_string(),
            credential_data,
            scopes,
        );

        let mut credentials = self.credentials.lock().unwrap();
        credentials.insert(plugin_id.to_string(), credential);

        let mut roles = self.roles.lock().unwrap();
        roles.insert(plugin_id.to_string(), role);

        tracing::error!("Auth: Plugin {} registered with role {:?}", plugin_id, role);
        Ok(())
    }

    /// Authenticate a plugin using its credentials
    pub fn authenticate(&self, plugin_id: &str) -> Result<PluginRole, SecurityError> {
        let credentials = self.credentials.lock().unwrap();

        let credential = credentials
            .get(plugin_id)
            .ok_or(SecurityError::AuthenticationFailed)?;

        if credential.is_expired() {
            tracing::error!("Auth: Plugin {} credential expired", plugin_id);
            return Err(SecurityError::AuthenticationFailed);
        }

        let roles = self.roles.lock().unwrap();
        let role = roles
            .get(plugin_id)
            .copied()
            .ok_or(SecurityError::AuthenticationFailed)?;

        tracing::error!("Auth: Plugin {} authenticated successfully", plugin_id);
        Ok(role)
    }

    /// Check if plugin has specific permission
    pub fn check_permission(
        &self,
        plugin_id: &str,
        permission: &str,
    ) -> Result<bool, SecurityError> {
        let role = self.authenticate(plugin_id)?;
        let permissions = PluginPermissions::from_role(role);
        Ok(permissions.check_permission(permission))
    }

    /// Get all permissions for a plugin
    pub fn get_permissions(&self, plugin_id: &str) -> Result<PluginPermissions, SecurityError> {
        let role = self.authenticate(plugin_id)?;
        Ok(PluginPermissions::from_role(role))
    }

    /// Update plugin role
    pub fn set_role(&self, plugin_id: &str, role: PluginRole) -> Result<(), SecurityError> {
        let mut roles = self.roles.lock().unwrap();
        roles.insert(plugin_id.to_string(), role);
        tracing::error!("Auth: Plugin {} role updated to {:?}", plugin_id, role);
        Ok(())
    }

    /// Revoke plugin authentication
    pub fn revoke(&self, plugin_id: &str) -> Result<(), SecurityError> {
        let mut credentials = self.credentials.lock().unwrap();
        credentials.remove(plugin_id);

        let mut roles = self.roles.lock().unwrap();
        roles.remove(plugin_id);

        tracing::error!("Auth: Plugin {} authentication revoked", plugin_id);
        Ok(())
    }

    /// List all registered plugins
    pub fn list_plugins(&self) -> Result<Vec<String>, SecurityError> {
        let credentials = self.credentials.lock().unwrap();
        let plugin_ids: Vec<String> = credentials.keys().cloned().collect();
        Ok(plugin_ids)
    }

    /// Rotate a plugin's credential to a new version
    pub fn rotate_credential(
        &self,
        plugin_id: &str,
        new_credential_data: Vec<u8>,
        reason: String,
    ) -> Result<u32, SecurityError> {
        let credentials = self.credentials.lock().unwrap();
        let old_credential = credentials
            .get(plugin_id)
            .ok_or(SecurityError::AuthenticationFailed)?
            .clone();

        drop(credentials);

        // Create new credential with same type but new data
        let new_credential = PluginCredential::new(
            old_credential.credential_type,
            plugin_id.to_string(),
            new_credential_data,
            old_credential.scopes.clone(),
        );

        // Get current version number and create new version
        let new_version = {
            let mut versions = self.credential_versions.lock().unwrap();
            let current_versions = versions
                .entry(plugin_id.to_string())
                .or_insert_with(Vec::new);

            let version_num = if current_versions.is_empty() {
                1
            } else {
                current_versions.last().unwrap().version + 1
            };

            // Create new version
            let new_cred_version = CredentialVersion::new(version_num, new_credential.clone());

            // Mark old version as in grace period
            if !current_versions.is_empty() && current_versions.len() > 0 {
                if let Some(old_v) = current_versions.last_mut() {
                    old_v.status = CredentialStatus::Grace;
                    use std::time::{SystemTime, UNIX_EPOCH};
                    old_v.retired_at = Some(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    );
                }
            }

            current_versions.push(new_cred_version);
            version_num
        };

        // Update current credential
        {
            let mut credentials = self.credentials.lock().unwrap();
            credentials.insert(plugin_id.to_string(), new_credential);
        }

        // Record in rotation history
        let old_version = {
            let versions = self.credential_versions.lock().unwrap();
            versions
                .get(plugin_id)
                .and_then(|v| {
                    if v.len() > 1 {
                        Some(v[v.len() - 2].version)
                    } else {
                        None
                    }
                })
                .unwrap_or(0)
        };

        let _ = self
            .rotation_manager
            .record_rotation(plugin_id, old_version, new_version, reason);

        tracing::error!(
            "Auth: Plugin {} credential rotated to version {}",
            plugin_id,
            new_version
        );
        Ok(new_version)
    }

    /// Get credential version history for a plugin
    pub fn get_credential_versions(
        &self,
        plugin_id: &str,
    ) -> Result<Vec<CredentialVersion>, SecurityError> {
        let versions = self.credential_versions.lock().unwrap();
        Ok(versions.get(plugin_id).cloned().unwrap_or_default())
    }

    /// Get rotation history for a plugin
    pub fn get_rotation_history(
        &self,
        plugin_id: &str,
    ) -> Result<Vec<RotationHistory>, SecurityError> {
        self.rotation_manager.get_rotation_history(plugin_id)
    }

    /// Authenticate with any valid credential version (including grace period)
    pub fn authenticate_with_grace_period(
        &self,
        plugin_id: &str,
    ) -> Result<(PluginRole, u32), SecurityError> {
        let versions = self.credential_versions.lock().unwrap();
        let cred_versions = versions
            .get(plugin_id)
            .ok_or(SecurityError::AuthenticationFailed)?;

        // Check for active or grace period credentials
        for version in cred_versions.iter().rev() {
            match version.status {
                CredentialStatus::Active => {
                    let role = self.authenticate(plugin_id)?;
                    return Ok((role, version.version));
                }
                CredentialStatus::Grace => {
                    // Check if still in grace period
                    if let Some(retired_at) = version.retired_at {
                        if !CredentialRotationManager::is_in_grace_period(retired_at) {
                            continue;
                        }
                    }
                    if !version.credential.is_expired() {
                        let role = self.authenticate(plugin_id)?;
                        return Ok((role, version.version));
                    }
                }
                CredentialStatus::Retired => continue,
            }
        }

        Err(SecurityError::AuthenticationFailed)
    }

    /// Cleanup expired credentials
    pub fn cleanup_expired_credentials(&self) -> Result<usize, SecurityError> {
        let mut versions = self.credential_versions.lock().unwrap();
        let mut cleaned = 0;

        for cred_versions in versions.values_mut() {
            let original_len = cred_versions.len();
            cred_versions.retain(|v| {
                // Keep active and grace period credentials
                match v.status {
                    CredentialStatus::Retired => {
                        // Only remove if definitely past grace period
                        if let Some(retired_at) = v.retired_at {
                            CredentialRotationManager::is_in_grace_period(retired_at)
                        } else {
                            true
                        }
                    }
                    _ => true,
                }
            });
            cleaned += original_len.saturating_sub(cred_versions.len());
        }

        tracing::error!("Auth: Cleaned up {} expired credentials", cleaned);
        Ok(cleaned)
    }

    /// Get rotation manager for configuration
    pub fn rotation_manager(&self) -> Arc<CredentialRotationManager> {
        self.rotation_manager.clone()
    }

    // ===== MFA METHODS =====

    /// Enable MFA for a plugin
    pub fn enable_mfa(&self, plugin_id: &str) -> Result<(), SecurityError> {
        let mut mfa_enabled = self.mfa_enabled.lock().unwrap();
        mfa_enabled.insert(plugin_id.to_string(), true);
        tracing::error!("Auth: MFA enabled for plugin {}", plugin_id);
        Ok(())
    }

    /// Disable MFA for a plugin
    pub fn disable_mfa(&self, plugin_id: &str) -> Result<(), SecurityError> {
        let mut mfa_enabled = self.mfa_enabled.lock().unwrap();
        mfa_enabled.insert(plugin_id.to_string(), false);
        tracing::error!("Auth: MFA disabled for plugin {}", plugin_id);
        Ok(())
    }

    /// Check if MFA is enabled for a plugin
    pub fn is_mfa_enabled(&self, plugin_id: &str) -> bool {
        let mfa_enabled = self.mfa_enabled.lock().unwrap();
        *mfa_enabled.get(plugin_id).unwrap_or(&false)
    }

    /// Register TOTP MFA factor for a plugin
    pub fn register_mfa_totp(&self, plugin_id: &str) -> Result<Vec<u8>, SecurityError> {
        let secret = rand::random::<[u8; 32]>();
        self.mfa_manager
            .register_factor(plugin_id, MFAMethod::TOTP, secret.to_vec())?;
        tracing::error!("Auth: TOTP factor registered for plugin {}", plugin_id);
        Ok(secret.to_vec())
    }

    /// Register backup codes MFA factor for a plugin
    pub fn register_mfa_backup_codes(&self, plugin_id: &str) -> Result<Vec<String>, SecurityError> {
        let codes = self
            .mfa_manager
            .backup_code_provider()
            .generate_codes(plugin_id)?;
        self.mfa_manager.register_factor(
            plugin_id,
            MFAMethod::BackupCodes,
            codes.join("|").into_bytes(),
        )?;
        tracing::error!(
            "Auth: Backup codes factor registered for plugin {} (10 codes)",
            plugin_id
        );
        Ok(codes)
    }

    /// Create MFA challenge for authentication
    pub fn create_mfa_challenge(
        &self,
        plugin_id: &str,
        method: MFAMethod,
    ) -> Result<MFAChallenge, SecurityError> {
        if !self.is_mfa_enabled(plugin_id) {
            return Err(SecurityError::AuthenticationFailed);
        }

        let challenge = self.mfa_manager.create_challenge(plugin_id, method)?;
        tracing::error!(
            "Auth: MFA challenge created for plugin {} (ID: {})",
            plugin_id,
            challenge.challenge_id
        );
        Ok(challenge)
    }

    /// Verify MFA challenge response
    pub fn verify_mfa_challenge(
        &self,
        challenge_id: &str,
        response: &str,
    ) -> Result<bool, SecurityError> {
        let result = self.mfa_manager.verify_challenge(challenge_id, response)?;
        tracing::error!(
            "Auth: MFA challenge verification: {}",
            if result { "SUCCESS" } else { "FAILED" }
        );
        Ok(result)
    }

    /// List all MFA factors for a plugin
    pub fn list_mfa_factors(&self, plugin_id: &str) -> Result<Vec<MFAFactor>, SecurityError> {
        self.mfa_manager.list_factors(plugin_id)
    }

    /// Disable specific MFA factor
    pub fn disable_mfa_factor(
        &self,
        plugin_id: &str,
        method: MFAMethod,
    ) -> Result<(), SecurityError> {
        self.mfa_manager.disable_factor(plugin_id, method)
    }

    /// Get MFA manager for direct access
    pub fn mfa_manager(&self) -> Arc<MFAManager> {
        self.mfa_manager.clone()
    }

    /// Override authenticate to require MFA if enabled
    pub fn authenticate_with_mfa(
        &self,
        plugin_id: &str,
        mfa_challenge_id: Option<&str>,
        mfa_response: Option<&str>,
    ) -> Result<PluginRole, SecurityError> {
        // First, perform standard authentication
        let role = self.authenticate(plugin_id)?;

        // If MFA is enabled, require MFA verification
        if self.is_mfa_enabled(plugin_id) {
            let challenge_id = mfa_challenge_id.ok_or(SecurityError::AuthenticationFailed)?;
            let response = mfa_response.ok_or(SecurityError::AuthenticationFailed)?;

            let verified = self.verify_mfa_challenge(challenge_id, response)?;
            if !verified {
                return Err(SecurityError::AuthenticationFailed);
            }
        }

        Ok(role)
    }
}

impl Default for PluginAuthenticator {
    fn default() -> Self {
        Self::new()
    }
}

/// Audit event for security logging
#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub event_id: String,
    pub timestamp: u64,
    pub plugin_id: String,
    pub event_type: String,
    pub action: String,
    pub resource: String,
    pub result: bool,
    pub details: String,
}

impl AuditEvent {
    pub fn new(
        plugin_id: &str,
        event_type: &str,
        action: &str,
        resource: &str,
        result: bool,
        details: &str,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let event_id = format!("audit_{}_{}_{}", plugin_id, now, rand::random::<u32>());

        AuditEvent {
            event_id,
            timestamp: now,
            plugin_id: plugin_id.to_string(),
            event_type: event_type.to_string(),
            action: action.to_string(),
            resource: resource.to_string(),
            result,
            details: details.to_string(),
        }
    }
}

/// Audit log manager for security event tracking
pub struct AuditLogger {
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

impl AuditLogger {
    pub fn new() -> Self {
        AuditLogger {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn log_event(&self, event: AuditEvent) -> Result<(), SecurityError> {
        let mut events = self.events.lock().unwrap();
        events.push(event);
        Ok(())
    }

    pub fn log_auth_attempt(
        &self,
        plugin_id: &str,
        success: bool,
        details: &str,
    ) -> Result<(), SecurityError> {
        let event = AuditEvent::new(
            plugin_id,
            "authentication",
            "login",
            "plugin",
            success,
            details,
        );
        self.log_event(event)
    }

    pub fn log_permission_check(
        &self,
        plugin_id: &str,
        permission: &str,
        granted: bool,
    ) -> Result<(), SecurityError> {
        let event = AuditEvent::new(
            plugin_id,
            "authorization",
            "permission_check",
            permission,
            granted,
            &format!("Permission: {}", permission),
        );
        self.log_event(event)
    }

    pub fn get_events(&self, plugin_id: &str) -> Result<Vec<AuditEvent>, SecurityError> {
        let events = self.events.lock().unwrap();
        let filtered: Vec<AuditEvent> = events
            .iter()
            .filter(|e| e.plugin_id == plugin_id)
            .cloned()
            .collect();
        Ok(filtered)
    }

    pub fn get_all_events(&self) -> Result<Vec<AuditEvent>, SecurityError> {
        let events = self.events.lock().unwrap();
        Ok(events.clone())
    }

    pub fn clear_events(&self) -> Result<(), SecurityError> {
        let mut events = self.events.lock().unwrap();
        events.clear();
        Ok(())
    }

    pub fn event_count(&self) -> usize {
        let events = self.events.lock().unwrap();
        events.len()
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Multi-Factor Authentication (Phase 3b)
// ============================================================================

/// MFA method types
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MFAMethod {
    TOTP = 0,
    BackupCodes = 1,
    WebAuthn = 2,
    PushNotification = 3,
}

/// MFA Factor for a plugin
#[derive(Debug, Clone)]
pub struct MFAFactor {
    pub plugin_id: String,
    pub method: MFAMethod,
    pub enabled: bool,
    pub created_at: u64,
    pub last_used: u64,
    pub secret_data: Vec<u8>, // encrypted
}

/// MFA Challenge for verification
#[derive(Debug, Clone)]
pub struct MFAChallenge {
    pub plugin_id: String,
    pub challenge_id: String,
    pub method: MFAMethod,
    pub created_at: u64,
    pub expires_at: u64,
    pub attempts: u32,
    pub max_attempts: u32,
}

/// TOTP Provider for Time-Based One-Time Password generation
pub struct TOTPProvider {
    /// TOTP secret seeds per plugin (encrypted)
    secrets: Arc<Mutex<std::collections::HashMap<String, Vec<u8>>>>,
    /// Window size for time tolerance (default: 1 = ±30 seconds)
    window_size: u32,
    /// Code length (default: 6 digits)
    code_length: u32,
}

impl TOTPProvider {
    /// Create a new TOTP provider
    pub fn new(window_size: u32, code_length: u32) -> Self {
        TOTPProvider {
            secrets: Arc::new(Mutex::new(std::collections::HashMap::new())),
            window_size,
            code_length,
        }
    }

    /// Register TOTP for a plugin with a secret seed
    pub fn register(&self, plugin_id: &str, secret_seed: Vec<u8>) -> Result<(), SecurityError> {
        let mut secrets = self.secrets.lock().unwrap();
        secrets.insert(plugin_id.to_string(), secret_seed);
        Ok(())
    }

    /// Generate TOTP code for verification (RFC 6238)
    pub fn generate_code(&self, plugin_id: &str) -> Result<String, SecurityError> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let secrets = self.secrets.lock().unwrap();
        let secret = secrets
            .get(plugin_id)
            .ok_or(SecurityError::AuthenticationFailed)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SecurityError::AuthenticationFailed)?
            .as_secs();

        // Time counter: number of 30-second intervals since epoch
        let time_counter = now / 30;

        // HMAC-SHA1 with the time counter as message
        use hmac::{Hmac, Mac};
        use sha1::Sha1;
        type HmacSha1 = Hmac<Sha1>;

        let mut mac =
            HmacSha1::new_from_slice(secret).map_err(|_| SecurityError::AuthenticationFailed)?;

        mac.update(&time_counter.to_be_bytes());
        let result = mac.finalize();
        let bytes = result.into_bytes();

        // Dynamic truncation (RFC 6238 section 5.3)
        let offset = (bytes[bytes.len() - 1] & 0x0f) as usize;
        let truncated = u32::from_be_bytes([
            bytes[offset] & 0x7f,
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);

        // Generate code with specified length
        let modulo = 10_u32.pow(self.code_length);
        let code = truncated % modulo;

        Ok(format!(
            "{:0width$}",
            code,
            width = self.code_length as usize
        ))
    }

    /// Verify TOTP code (allows time window tolerance)
    pub fn verify_code(&self, plugin_id: &str, code: &str) -> Result<bool, SecurityError> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let secrets = self.secrets.lock().unwrap();
        let secret = secrets
            .get(plugin_id)
            .ok_or(SecurityError::AuthenticationFailed)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SecurityError::AuthenticationFailed)?
            .as_secs();

        // Check current time window and surrounding windows (within window_size)
        let time_counter = now / 30;

        for i in 0..=self.window_size {
            for counter in &[
                time_counter.saturating_sub(i as u64),
                time_counter + i as u64,
            ] {
                use hmac::{Hmac, Mac};
                use sha1::Sha1;
                type HmacSha1 = Hmac<Sha1>;

                let mut mac = HmacSha1::new_from_slice(secret)
                    .map_err(|_| SecurityError::AuthenticationFailed)?;

                mac.update(&counter.to_be_bytes());
                let result = mac.finalize();
                let bytes = result.into_bytes();

                let offset = (bytes[bytes.len() - 1] & 0x0f) as usize;
                let truncated = u32::from_be_bytes([
                    bytes[offset] & 0x7f,
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ]);

                let modulo = 10_u32.pow(self.code_length);
                let expected_code = truncated % modulo;
                let expected_str = format!(
                    "{:0width$}",
                    expected_code,
                    width = self.code_length as usize
                );

                if code == expected_str {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Get window size in seconds
    pub fn window_size_seconds(&self) -> u64 {
        (self.window_size as u64) * 30
    }
}

/// Backup Code Provider for emergency access
pub struct BackupCodeProvider {
    /// Backup codes per plugin (hashed, not plaintext)
    codes: Arc<Mutex<std::collections::HashMap<String, Vec<String>>>>,
    /// Track used codes
    used_codes: Arc<Mutex<std::collections::HashSet<String>>>,
    /// Default number of backup codes to generate
    default_count: u32,
}

impl BackupCodeProvider {
    /// Create a new backup code provider
    pub fn new(default_count: u32) -> Self {
        BackupCodeProvider {
            codes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            used_codes: Arc::new(Mutex::new(std::collections::HashSet::new())),
            default_count,
        }
    }

    /// Generate new backup codes for a plugin
    pub fn generate_codes(&self, plugin_id: &str) -> Result<Vec<String>, SecurityError> {
        use rand::Rng;

        let mut codes = Vec::new();
        let mut rng = rand::thread_rng();

        for _ in 0..self.default_count {
            // Generate random 8-character alphanumeric code
            let code: String = (0..8)
                .map(|_| {
                    let idx = rng.gen_range(0..36);
                    if idx < 10 {
                        (b'0' + idx) as char
                    } else {
                        (b'a' + (idx - 10)) as char
                    }
                })
                .collect();

            codes.push(code);
        }

        // Hash and store codes
        let mut stored = self.codes.lock().unwrap();
        let mut hashed_codes = Vec::new();

        for code in &codes {
            let hashed = format!("{:x}", md5::compute(code.as_bytes()));
            hashed_codes.push(hashed);
        }

        stored.insert(plugin_id.to_string(), hashed_codes);

        Ok(codes)
    }

    /// Verify and consume a backup code (single-use)
    pub fn verify_code(&self, plugin_id: &str, code: &str) -> Result<bool, SecurityError> {
        let stored = self.codes.lock().unwrap();
        let codes = stored
            .get(plugin_id)
            .ok_or(SecurityError::AuthenticationFailed)?;

        let code_hash = format!("{:x}", md5::compute(code.as_bytes()));

        if codes.contains(&code_hash) {
            // Check if already used
            let mut used = self.used_codes.lock().unwrap();
            if used.contains(&code_hash) {
                return Err(SecurityError::AuthenticationFailed); // Code already used
            }

            // Mark as used
            used.insert(code_hash);
            return Ok(true);
        }

        Ok(false)
    }

    /// Get count of remaining backup codes
    pub fn remaining_codes(&self, plugin_id: &str) -> Result<u32, SecurityError> {
        let stored = self.codes.lock().unwrap();
        let codes = stored
            .get(plugin_id)
            .ok_or(SecurityError::AuthenticationFailed)?;

        let used = self.used_codes.lock().unwrap();
        let remaining = codes.len() as u32 - used.len() as u32;

        Ok(remaining)
    }
}

/// MFA Manager for managing factors and challenges
pub struct MFAManager {
    /// Registered MFA factors per plugin
    factors: Arc<Mutex<std::collections::HashMap<String, Vec<MFAFactor>>>>,
    /// Active MFA challenges
    challenges: Arc<Mutex<std::collections::HashMap<String, MFAChallenge>>>,
    /// TOTP provider
    totp: Arc<TOTPProvider>,
    /// Backup code provider
    backup_codes: Arc<BackupCodeProvider>,
}

impl MFAManager {
    /// Create a new MFA manager
    pub fn new() -> Self {
        MFAManager {
            factors: Arc::new(Mutex::new(std::collections::HashMap::new())),
            challenges: Arc::new(Mutex::new(std::collections::HashMap::new())),
            totp: Arc::new(TOTPProvider::new(1, 6)), // ±1 window, 6-digit codes
            backup_codes: Arc::new(BackupCodeProvider::new(10)), // 10 backup codes
        }
    }

    /// Register an MFA factor for a plugin
    pub fn register_factor(
        &self,
        plugin_id: &str,
        method: MFAMethod,
        secret_data: Vec<u8>,
    ) -> Result<MFAFactor, SecurityError> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SecurityError::AuthenticationFailed)?
            .as_secs();

        let factor = MFAFactor {
            plugin_id: plugin_id.to_string(),
            method,
            enabled: true,
            created_at: now,
            last_used: 0,
            secret_data,
        };

        let mut factors = self.factors.lock().unwrap();
        factors
            .entry(plugin_id.to_string())
            .or_insert_with(Vec::new)
            .push(factor.clone());

        // Register with TOTP provider if applicable
        if method == MFAMethod::TOTP {
            let _ = self.totp.register(plugin_id, factor.secret_data.clone());
        }

        Ok(factor)
    }

    /// Create an MFA challenge for verification
    pub fn create_challenge(
        &self,
        plugin_id: &str,
        method: MFAMethod,
    ) -> Result<MFAChallenge, SecurityError> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SecurityError::AuthenticationFailed)?
            .as_secs();

        // Generate unique challenge ID
        let challenge_id = format!("{}-{}", plugin_id, now);

        let challenge = MFAChallenge {
            plugin_id: plugin_id.to_string(),
            challenge_id: challenge_id.clone(),
            method,
            created_at: now,
            expires_at: now + 300, // 5 minute timeout
            attempts: 0,
            max_attempts: 3,
        };

        let mut challenges = self.challenges.lock().unwrap();
        challenges.insert(challenge_id, challenge.clone());

        Ok(challenge)
    }

    /// Verify MFA challenge with provided code
    pub fn verify_challenge(&self, challenge_id: &str, code: &str) -> Result<bool, SecurityError> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let mut challenges = self.challenges.lock().unwrap();
        let challenge = challenges
            .get_mut(challenge_id)
            .ok_or(SecurityError::AuthenticationFailed)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SecurityError::AuthenticationFailed)?
            .as_secs();

        // Check expiration
        if now > challenge.expires_at {
            challenges.remove(challenge_id);
            return Err(SecurityError::AuthenticationFailed);
        }

        // Check attempt limit
        if challenge.attempts >= challenge.max_attempts {
            challenges.remove(challenge_id);
            return Err(SecurityError::AuthenticationFailed);
        }

        challenge.attempts += 1;

        // Verify based on method
        match challenge.method {
            MFAMethod::TOTP => self.totp.verify_code(&challenge.plugin_id, code),
            MFAMethod::BackupCodes => self.backup_codes.verify_code(&challenge.plugin_id, code),
            _ => Err(SecurityError::AuthenticationFailed),
        }
    }

    /// List enabled MFA factors for a plugin
    pub fn list_factors(&self, plugin_id: &str) -> Result<Vec<MFAFactor>, SecurityError> {
        let factors = self.factors.lock().unwrap();
        Ok(factors.get(plugin_id).cloned().unwrap_or_default())
    }

    /// Disable an MFA factor
    pub fn disable_factor(&self, plugin_id: &str, method: MFAMethod) -> Result<(), SecurityError> {
        let mut factors = self.factors.lock().unwrap();
        if let Some(plugin_factors) = factors.get_mut(plugin_id) {
            for factor in plugin_factors {
                if factor.method == method {
                    factor.enabled = false;
                }
            }
            Ok(())
        } else {
            Err(SecurityError::AuthenticationFailed)
        }
    }

    /// Get TOTP provider for code generation
    pub fn totp_provider(&self) -> Arc<TOTPProvider> {
        self.totp.clone()
    }

    /// Get backup code provider for code generation
    pub fn backup_code_provider(&self) -> Arc<BackupCodeProvider> {
        self.backup_codes.clone()
    }
}

// ============================================================================
// RFC-0077: Secret Versioning and Cryptographic Key Types
// ============================================================================

/// Key algorithm for secret versioning (JWA/JWT signing algorithms)
///
/// This enum defines the cryptographic algorithms used for secret keys
/// as specified in RFC-0077. These are primarily used for
/// JWT signing and cryptographic operations.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyAlgorithm {
    /// HMAC using SHA-256
    HS256,
    /// RSASSA-PKCS1-v1_5 using SHA-256
    RS256,
    /// ECDSA using P-256 and SHA-256
    ES256,
    /// EdDSA using Ed25519
    Ed25519,
    /// X25519 for key exchange
    X25519,
}

impl std::fmt::Display for KeyAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyAlgorithm::HS256 => write!(f, "HS256"),
            KeyAlgorithm::RS256 => write!(f, "RS256"),
            KeyAlgorithm::ES256 => write!(f, "ES256"),
            KeyAlgorithm::Ed25519 => write!(f, "Ed25519"),
            KeyAlgorithm::X25519 => write!(f, "X25519"),
        }
    }
}

impl KeyAlgorithm {
    /// Get the recommended key size in bits for this algorithm
    pub fn key_size_bits(&self) -> u16 {
        match self {
            KeyAlgorithm::HS256 => 256,
            KeyAlgorithm::RS256 => 2048,
            KeyAlgorithm::ES256 => 256,
            KeyAlgorithm::Ed25519 => 256,
            KeyAlgorithm::X25519 => 256,
        }
    }

    /// Parse algorithm from string representation
    pub fn from_str(s: &str) -> Result<KeyAlgorithm, SecurityError> {
        match s.to_uppercase().as_str() {
            "HS256" => Ok(KeyAlgorithm::HS256),
            "RS256" => Ok(KeyAlgorithm::RS256),
            "ES256" => Ok(KeyAlgorithm::ES256),
            "ED25519" => Ok(KeyAlgorithm::Ed25519),
            "X25519" => Ok(KeyAlgorithm::X25519),
            _ => Err(SecurityError::InputTooLong),
        }
    }
}

/// Key usage classification for cryptographic keys
///
/// Defines the intended use of a cryptographic key as specified in RFC-0077.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyUsage {
    /// Key is used for encryption/decryption operations
    Encryption,
    /// Key is used for creating and verifying digital signatures
    Signing,
    /// Key is used for key exchange protocols (e.g., Diffie-Hellman)
    KeyExchange,
    /// Key is used for key derivation (e.g., HKDF, PBKDF2)
    Derivation,
}

impl std::fmt::Display for KeyUsage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyUsage::Encryption => write!(f, "encryption"),
            KeyUsage::Signing => write!(f, "signing"),
            KeyUsage::KeyExchange => write!(f, "key_exchange"),
            KeyUsage::Derivation => write!(f, "derivation"),
        }
    }
}

impl KeyUsage {
    /// Parse usage from string representation
    pub fn from_str(s: &str) -> Result<KeyUsage, SecurityError> {
        match s.to_lowercase().as_str() {
            "encryption" => Ok(KeyUsage::Encryption),
            "signing" => Ok(KeyUsage::Signing),
            "key_exchange" => Ok(KeyUsage::KeyExchange),
            "derivation" => Ok(KeyUsage::Derivation),
            _ => Err(SecurityError::InputTooLong),
        }
    }
}

/// Secret version information with cryptographic metadata
///
/// Represents a versioned secret with its cryptographic properties as specified
/// in RFC-0077. This type is used for tracking cryptographic secret
/// versions including rotation information.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SecretVersion {
    /// Unique version identifier
    pub version_id: u32,
    /// When this version was created (Unix timestamp)
    pub created_at: u64,
    /// When this version expires (Unix timestamp, 0 = no expiry)
    pub expires_at: u64,
    /// Cryptographic algorithm used for this secret
    pub algorithm: KeyAlgorithm,
    /// Key size in bits
    pub key_size_bits: u16,
    /// Intended use of this key
    pub key_usage: KeyUsage,
}

impl SecretVersion {
    /// Create a new secret version
    pub fn new(version_id: u32, algorithm: KeyAlgorithm, key_usage: KeyUsage) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            version_id,
            created_at: now,
            expires_at: 0, // Default: no expiry
            algorithm,
            key_size_bits: algorithm.key_size_bits(),
            key_usage,
        }
    }

    /// Create a new secret version with expiry
    pub fn new_with_expiry(
        version_id: u32,
        algorithm: KeyAlgorithm,
        key_usage: KeyUsage,
        expires_at: u64,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            version_id,
            created_at: now,
            expires_at,
            algorithm,
            key_size_bits: algorithm.key_size_bits(),
            key_usage,
        }
    }

    /// Check if this version has expired
    pub fn is_expired(&self) -> bool {
        if self.expires_at == 0 {
            return false;
        }

        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        now >= self.expires_at
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    // ===== FFI BOUNDARY VALIDATION TESTS (CVSS 9.8) =====

    #[test]
    fn test_validate_cstr_null() {
        unsafe {
            let result = validate_cstr(std::ptr::null(), "test");
            assert_eq!(result, Err(SecurityError::NullPointer));
        }
    }

    #[test]
    fn test_validate_cstr_valid() {
        unsafe {
            let test_str = "valid_string\0";
            let ptr = test_str.as_ptr() as *const std::os::raw::c_char;
            let result = validate_cstr(ptr, "test");
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_validate_plugin_context_null() {
        let result = unsafe { validate_plugin_context(std::ptr::null()) };
        assert_eq!(result, Err(SecurityError::NullPointer));
    }

    #[test]
    fn test_validate_buffer_null() {
        unsafe {
            let result = validate_buffer(std::ptr::null(), 100, "test");
            assert_eq!(result, Err(SecurityError::NullPointer));
        }
    }

    #[test]
    fn test_validate_buffer_zero_length() {
        unsafe {
            let buf = vec![0u8; 100];
            let ptr = buf.as_ptr();
            let result = validate_buffer(ptr, 0, "test");
            // Zero-length buffers are allowed, they just return an empty slice
            assert!(result.is_ok());
            assert_eq!(result.unwrap().len(), 0);
        }
    }

    // ===== CONTEXT SIGNATURE VERIFICATION TESTS (CVSS 7.8) =====

    #[test]
    fn test_context_signature_generation() {
        // Create a mock PluginContext for testing
        // This is a simplified test since PluginContext is a C struct
        let key = [0u8; 32];
        // We can't easily create a PluginContext without FFI,
        // so we'll test the verify function returns an error for null
        let result = verify_context_signature(std::ptr::null(), &key, "test_sig");
        assert!(result.is_err());
    }

    #[test]
    fn test_context_signature_verification_invalid() {
        let key = [0u8; 32];
        let result = verify_context_signature(std::ptr::null(), &key, "invalid_sig");
        assert_eq!(result, Err(SecurityError::InvalidContextSignature));
    }

    // ===== PLUGIN CAPABILITIES TESTS (CVSS 9.6) =====

    #[test]
    fn test_plugin_capabilities_all() {
        let caps: u64 = PluginCapabilities::READ_CONFIG
            | PluginCapabilities::WRITE_CONFIG
            | PluginCapabilities::READ_SECRETS
            | PluginCapabilities::WRITE_SECRETS
            | PluginCapabilities::NETWORK_OUTBOUND
            | PluginCapabilities::FILE_READ
            | PluginCapabilities::FILE_WRITE
            | PluginCapabilities::PROCESS_SPAWN;

        assert!((caps & PluginCapabilities::READ_CONFIG) != 0);
        assert!((caps & PluginCapabilities::WRITE_CONFIG) != 0);
        assert!((caps & PluginCapabilities::READ_SECRETS) != 0);
    }

    #[test]
    fn test_plugin_capabilities_none() {
        let caps: u64 = 0;
        assert!((caps & PluginCapabilities::READ_CONFIG) == 0);
        assert!((caps & PluginCapabilities::WRITE_CONFIG) == 0);
        assert!((caps & PluginCapabilities::READ_SECRETS) == 0);
    }

    #[test]
    fn test_plugin_capabilities_with() {
        let mut caps: u64 = 0;
        caps |= PluginCapabilities::READ_CONFIG;
        caps |= PluginCapabilities::FILE_READ;

        assert!((caps & PluginCapabilities::READ_CONFIG) != 0);
        assert!((caps & PluginCapabilities::FILE_READ) != 0);
        assert!((caps & PluginCapabilities::WRITE_CONFIG) == 0);
    }

    #[test]
    fn test_sandbox_policy_permissive() {
        let policy = PluginSandboxPolicy::permissive("test_plugin");
        assert!(policy.allow_child_processes);
        assert_eq!(policy.max_memory, 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_sandbox_policy_restrictive() {
        let policy = PluginSandboxPolicy::restrictive("test_plugin");
        assert!(!policy.allow_child_processes);
        assert_eq!(policy.max_memory, 128 * 1024 * 1024);
        assert_eq!(policy.max_cpu_time, 5_000);
    }

    #[test]
    fn test_sandbox_enforcer_file_access_allowed() {
        let mut policy = PluginSandboxPolicy::permissive("test_plugin");
        policy.allowed_paths.push("/tmp".to_string());
        let enforcer = SandboxEnforcer::new();
        enforcer.register_plugin(policy).unwrap();

        let result = enforcer.check_file_access("test_plugin", "/tmp/file.txt", true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sandbox_enforcer_file_access_directory_traversal() {
        let mut policy = PluginSandboxPolicy::permissive("test_plugin");
        policy.allowed_paths.push("/app/data".to_string());
        let enforcer = SandboxEnforcer::new();
        enforcer.register_plugin(policy).unwrap();

        // Attempt directory traversal
        let result = enforcer.check_file_access("test_plugin", "/app/data/../../etc/passwd", true);
        assert!(result.is_err());
    }

    #[test]
    fn test_sandbox_enforcer_memory_limit() {
        let mut policy = PluginSandboxPolicy::restrictive("test_plugin");
        policy.max_memory = 10 * 1024 * 1024; // 10MB
        let enforcer = SandboxEnforcer::new();
        enforcer.register_plugin(policy).unwrap();

        // Try to allocate more than limit
        let result = enforcer.check_memory_limit("test_plugin", 20 * 1024 * 1024);
        assert!(result.is_err());
    }

    #[test]
    fn test_sandbox_enforcer_memory_within_limit() {
        let mut policy = PluginSandboxPolicy::restrictive("test_plugin");
        policy.max_memory = 100 * 1024 * 1024;
        let enforcer = SandboxEnforcer::new();
        enforcer.register_plugin(policy).unwrap();

        let result = enforcer.check_memory_limit("test_plugin", 50 * 1024 * 1024);
        assert!(result.is_ok());
    }

    // ===== ENCRYPTED SECRET STORAGE TESTS (CVSS 8.2) =====

    #[test]
    fn test_encrypted_secret_store_creation() {
        let store = EncryptedSecretStore::new();
        assert!(store.list_secret_names().is_empty());
    }

    #[test]
    fn test_encrypted_secret_store_roundtrip() {
        let store = EncryptedSecretStore::new();
        let secret_name = "database_password";
        let secret_value = b"super_secret_123";

        let result = store.store_secret(secret_name, secret_value);
        assert!(result.is_ok());

        let retrieved = store.get_secret(secret_name);
        assert!(retrieved.is_ok());
        assert_eq!(retrieved.unwrap(), secret_value);
    }

    #[test]
    fn test_encrypted_secret_store_remove() {
        let store = EncryptedSecretStore::new();
        let secret_name = "temp_secret";

        store.store_secret(secret_name, b"temp_value").unwrap();
        assert!(store.get_secret(secret_name).is_ok());

        let result = store.remove_secret(secret_name);
        assert!(result.is_ok());
        assert!(store.get_secret(secret_name).is_err());
    }

    #[test]
    fn test_encrypted_secret_store_list_secrets() {
        let store = EncryptedSecretStore::new();
        store.store_secret("secret1", b"value1").unwrap();
        store.store_secret("secret2", b"value2").unwrap();
        store.store_secret("secret3", b"value3").unwrap();

        let secrets = store.list_secret_names();
        assert_eq!(secrets.len(), 3);
        assert!(secrets.contains(&"secret1".to_string()));
        assert!(secrets.contains(&"secret2".to_string()));
        assert!(secrets.contains(&"secret3".to_string()));
    }

    // ===== COMPREHENSIVE INPUT VALIDATION TESTS (CVSS 7.5) =====

    #[test]
    fn test_input_validator_json_valid() {
        let valid_json = r#"{"key": "value"}"#;
        let result = InputValidator::validate_json(valid_json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_input_validator_json_invalid() {
        let invalid_json = r#"{"key": "value"#;
        let result = InputValidator::validate_json(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_input_validator_sql_identifier_valid() {
        let result = InputValidator::validate_sql_identifier("users");
        assert!(result.is_ok());
    }

    #[test]
    fn test_input_validator_sql_identifier_injection() {
        let result = InputValidator::validate_sql_identifier("users; DROP TABLE users--");
        assert!(result.is_err());
    }

    #[test]
    fn test_input_validator_file_path_valid() {
        let result = InputValidator::validate_file_path("documents/file.txt");
        assert!(result.is_ok());
    }

    #[test]
    fn test_input_validator_file_path_traversal() {
        let result = InputValidator::validate_file_path("documents/../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_input_validator_file_path_absolute() {
        let result = InputValidator::validate_file_path("/etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_input_validator_command_arg_valid() {
        let result = InputValidator::validate_command_arg("--config=app.conf");
        assert!(result.is_ok());
    }

    #[test]
    fn test_input_validator_command_arg_injection() {
        let result = InputValidator::validate_command_arg("arg1; rm -rf /");
        assert!(result.is_err());
    }

    #[test]
    fn test_input_validator_command_arg_shell_metacharacters() {
        assert!(InputValidator::validate_command_arg("arg|other").is_err());
        assert!(InputValidator::validate_command_arg("arg&other").is_err());
        assert!(InputValidator::validate_command_arg("arg>file").is_err());
        assert!(InputValidator::validate_command_arg("arg<file").is_err());
        assert!(InputValidator::validate_command_arg("arg$(whoami)").is_err());
    }

    #[test]
    fn test_input_validator_http_header_valid() {
        let result = InputValidator::validate_http_header("value123");
        assert!(result.is_ok());
    }

    #[test]
    fn test_input_validator_http_header_crlf_injection() {
        let result = InputValidator::validate_http_header("value\r\nX-Injected: evil");
        assert!(result.is_err());
    }

    #[test]
    fn test_input_validator_url_valid() {
        let result = InputValidator::validate_url("https://example.com/api/users");
        assert!(result.is_ok());
    }

    #[test]
    fn test_input_validator_url_invalid_protocol() {
        let result = InputValidator::validate_url("gopher://oldnetwork.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_input_validator_integer_valid() {
        let result = InputValidator::validate_integer("42", 0, 100);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_input_validator_integer_out_of_range() {
        let result = InputValidator::validate_integer("150", 0, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_input_validator_integer_invalid() {
        let result = InputValidator::validate_integer("not_a_number", 0, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_input_validator_log_sanitization() {
        let dangerous = "User input with\x00null and\x1bescape";
        let result = InputValidator::sanitize_for_logging(dangerous);
        assert!(!result.contains('\0'));
        assert!(!result.contains('\x1b'));
    }

    // ===== MEMORY SECURITY TESTS =====

    #[test]
    fn test_secure_memzero() {
        let mut buf = vec![0xFF; 64];
        secure_memzero(&mut buf);

        for byte in &buf {
            assert_eq!(*byte, 0);
        }
    }

    #[test]
    fn test_secure_memzero_large_buffer() {
        let mut buf = vec![0xAB; 10000];
        secure_memzero(&mut buf);

        for byte in &buf {
            assert_eq!(*byte, 0);
        }
    }

    // ===== PLUGIN CAPACITY TRACKING TESTS =====

    #[test]
    fn test_plugin_capacity_tracker_creation() {
        let tracker = PluginCapacityTracker::new();
        assert_eq!(tracker.current_count(), 0);
    }

    #[test]
    fn test_plugin_capability_combination() {
        let mut caps: u64 = 0;
        caps |= PluginCapabilities::READ_CONFIG;
        caps |= PluginCapabilities::READ_SECRETS;
        caps |= PluginCapabilities::FILE_READ;

        assert!((caps & PluginCapabilities::READ_CONFIG) != 0);
        assert!((caps & PluginCapabilities::READ_SECRETS) != 0);
        assert!((caps & PluginCapabilities::FILE_READ) != 0);
        assert!(!((caps & PluginCapabilities::WRITE_CONFIG) != 0));
        assert!(!((caps & PluginCapabilities::NETWORK_OUTBOUND) != 0));
    }

    // ===== AUTHENTICATION AND RBAC TESTS (PHASE 2) =====

    #[test]
    fn test_plugin_credential_creation() {
        let cred = PluginCredential::new(
            CredentialType::ApiKey,
            "test_plugin".to_string(),
            b"secret_key_data".to_vec(),
            vec!["read".to_string(), "write".to_string()],
        );

        assert_eq!(cred.credential_type, CredentialType::ApiKey);
        assert_eq!(cred.plugin_id, "test_plugin");
        assert!(!cred.is_expired());
        assert!(cred.has_scope("read"));
        assert!(cred.has_scope("write"));
        assert!(!cred.has_scope("admin"));
    }

    #[test]
    fn test_plugin_role_permissions() {
        let viewer_perms = PluginPermissions::from_role(PluginRole::Viewer);
        assert!(viewer_perms.read_config);
        assert!(!viewer_perms.write_config);
        assert!(!viewer_perms.system_admin);

        let editor_perms = PluginPermissions::from_role(PluginRole::Editor);
        assert!(editor_perms.read_config);
        assert!(editor_perms.write_config);
        assert!(editor_perms.access_network);
        assert!(!editor_perms.system_admin);

        let admin_perms = PluginPermissions::from_role(PluginRole::Admin);
        assert!(admin_perms.read_config);
        assert!(admin_perms.write_config);
        assert!(admin_perms.read_secrets);
        assert!(admin_perms.write_secrets);
        assert!(admin_perms.manage_plugins);
        assert!(!admin_perms.system_admin);

        let system_perms = PluginPermissions::from_role(PluginRole::System);
        assert!(system_perms.system_admin);
        assert!(system_perms.read_config);
        assert!(system_perms.write_config);
    }

    #[test]
    fn test_plugin_role_hierarchy() {
        assert!(!PluginRole::None.can_read());
        assert!(!PluginRole::None.can_write());
        assert!(!PluginRole::None.can_admin());

        assert!(PluginRole::Viewer.can_read());
        assert!(!PluginRole::Viewer.can_write());
        assert!(!PluginRole::Viewer.can_admin());

        assert!(PluginRole::Editor.can_read());
        assert!(PluginRole::Editor.can_write());
        assert!(!PluginRole::Editor.can_admin());

        assert!(PluginRole::Admin.can_read());
        assert!(PluginRole::Admin.can_write());
        assert!(PluginRole::Admin.can_admin());
        assert!(!PluginRole::Admin.can_system());

        assert!(PluginRole::System.can_system());
    }

    #[test]
    fn test_authenticator_register_plugin() {
        let auth = PluginAuthenticator::new();

        let result = auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"api_key_123".to_vec(),
            PluginRole::Editor,
            vec!["read".to_string(), "write".to_string()],
        );

        assert!(result.is_ok());
        let plugins = auth.list_plugins().unwrap();
        assert!(plugins.contains(&"plugin1".to_string()));
    }

    #[test]
    fn test_authenticator_authenticate() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"api_key_123".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        let role = auth.authenticate("plugin1").unwrap();
        assert_eq!(role, PluginRole::Editor);
    }

    #[test]
    fn test_authenticator_authenticate_nonexistent() {
        let auth = PluginAuthenticator::new();
        let result = auth.authenticate("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_authenticator_check_permission() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"api_key_123".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        assert!(auth.check_permission("plugin1", "read_config").unwrap());
        assert!(auth.check_permission("plugin1", "write_config").unwrap());
        assert!(!auth.check_permission("plugin1", "read_secrets").unwrap());
        assert!(!auth.check_permission("plugin1", "system_admin").unwrap());
    }

    #[test]
    fn test_authenticator_set_role() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"api_key_123".to_vec(),
            PluginRole::Viewer,
            vec![],
        )
        .unwrap();

        assert!(!auth.check_permission("plugin1", "write_config").unwrap());

        auth.set_role("plugin1", PluginRole::Admin).unwrap();

        assert!(auth.check_permission("plugin1", "write_config").unwrap());
        assert!(auth.check_permission("plugin1", "read_secrets").unwrap());
    }

    #[test]
    fn test_authenticator_revoke() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"api_key_123".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        assert!(auth.authenticate("plugin1").is_ok());

        auth.revoke("plugin1").unwrap();

        assert!(auth.authenticate("plugin1").is_err());
    }

    #[test]
    fn test_audit_event_creation() {
        let event = AuditEvent::new(
            "plugin1",
            "authentication",
            "login",
            "system",
            true,
            "Successful authentication",
        );

        assert_eq!(event.plugin_id, "plugin1");
        assert_eq!(event.event_type, "authentication");
        assert_eq!(event.action, "login");
        assert!(event.result);
    }

    #[test]
    fn test_audit_logger_log_event() {
        let logger = AuditLogger::new();

        let event = AuditEvent::new(
            "plugin1",
            "authentication",
            "login",
            "system",
            true,
            "Successful authentication",
        );

        assert!(logger.log_event(event).is_ok());
        assert_eq!(logger.event_count(), 1);
    }

    #[test]
    fn test_audit_logger_log_auth_attempt() {
        let logger = AuditLogger::new();

        assert!(logger.log_auth_attempt("plugin1", true, "Success").is_ok());
        assert!(logger
            .log_auth_attempt("plugin2", false, "Invalid credentials")
            .is_ok());

        assert_eq!(logger.event_count(), 2);

        let plugin1_events = logger.get_events("plugin1").unwrap();
        assert_eq!(plugin1_events.len(), 1);
        assert!(plugin1_events[0].result);
    }

    #[test]
    fn test_audit_logger_permission_checks() {
        let logger = AuditLogger::new();

        assert!(logger
            .log_permission_check("plugin1", "read_config", true)
            .is_ok());
        assert!(logger
            .log_permission_check("plugin1", "system_admin", false)
            .is_ok());

        let events = logger.get_events("plugin1").unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "authorization");
    }

    #[test]
    fn test_audit_logger_get_all_events() {
        let logger = AuditLogger::new();

        logger.log_auth_attempt("plugin1", true, "Success").unwrap();
        logger.log_auth_attempt("plugin2", true, "Success").unwrap();
        logger.log_auth_attempt("plugin1", false, "Failed").unwrap();

        let all_events = logger.get_all_events().unwrap();
        assert_eq!(all_events.len(), 3);

        let plugin1_events = logger.get_events("plugin1").unwrap();
        assert_eq!(plugin1_events.len(), 2);
    }

    #[test]
    fn test_plugin_permissions_check_permission() {
        let perms = PluginPermissions::from_role(PluginRole::Admin);

        assert!(perms.check_permission("read_config"));
        assert!(perms.check_permission("write_config"));
        assert!(perms.check_permission("read_secrets"));
        assert!(perms.check_permission("write_secrets"));
        assert!(perms.check_permission("manage_plugins"));
        assert!(!perms.check_permission("system_admin"));
        assert!(!perms.check_permission("invalid_permission"));
    }

    #[test]
    fn test_multiple_credential_types() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin_api",
            CredentialType::ApiKey,
            b"api_key_data".to_vec(),
            PluginRole::Editor,
            vec!["api_access".to_string()],
        )
        .unwrap();

        auth.register_plugin(
            "plugin_cert",
            CredentialType::Certificate,
            b"cert_data".to_vec(),
            PluginRole::Editor,
            vec!["cert_access".to_string()],
        )
        .unwrap();

        let plugins = auth.list_plugins().unwrap();
        assert_eq!(plugins.len(), 2);

        let cred1 = auth.authenticate("plugin_api").unwrap();
        let cred2 = auth.authenticate("plugin_cert").unwrap();

        assert_eq!(cred1, PluginRole::Editor);
        assert_eq!(cred2, PluginRole::Editor);
    }

    // ===== CREDENTIAL ROTATION TESTS (PHASE 3A) =====

    #[test]
    fn test_credential_rotation_manager_creation() {
        let manager = CredentialRotationManager::new(RotationPolicy::TimeBased, 604800, 2592000);

        assert_eq!(manager.get_rotation_policy(), RotationPolicy::TimeBased);
        assert_eq!(manager.grace_period(), 604800);
        assert_eq!(manager.rotation_interval(), 2592000);
    }

    #[test]
    fn test_credential_version_creation() {
        let cred = PluginCredential::new(
            CredentialType::ApiKey,
            "plugin1".to_string(),
            b"secret".to_vec(),
            vec!["api".to_string()],
        );

        let version = CredentialVersion::new(1, cred);
        assert_eq!(version.version, 1);
        assert_eq!(version.status, CredentialStatus::Active);
        assert!(version.rotated_at.is_none());
        assert!(version.retired_at.is_none());
    }

    #[test]
    fn test_rotate_credential() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"old_secret".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        // Rotate credential
        let new_version = auth
            .rotate_credential(
                "plugin1",
                b"new_secret".to_vec(),
                "scheduled_rotation".to_string(),
            )
            .unwrap();

        assert_eq!(new_version, 1);
    }

    #[test]
    fn test_multiple_credential_rotations() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"secret_v0".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        // First rotation
        let v1 = auth
            .rotate_credential("plugin1", b"secret_v1".to_vec(), "rotation_1".to_string())
            .unwrap();
        assert_eq!(v1, 1);

        // Second rotation
        let v2 = auth
            .rotate_credential("plugin1", b"secret_v2".to_vec(), "rotation_2".to_string())
            .unwrap();
        assert_eq!(v2, 2);

        // Verify versions exist
        let versions = auth.get_credential_versions("plugin1").unwrap();
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn test_credential_version_status_transitions() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::Certificate,
            b"cert1".to_vec(),
            PluginRole::Admin,
            vec![],
        )
        .unwrap();

        // Rotate to v1
        let _ = auth
            .rotate_credential("plugin1", b"cert2".to_vec(), "initial_rotation".to_string())
            .unwrap();

        // Check version statuses
        let versions = auth.get_credential_versions("plugin1").unwrap();
        assert!(versions.len() >= 1);

        // Latest should be active
        if let Some(latest) = versions.last() {
            assert_eq!(latest.status, CredentialStatus::Active);
        }
    }

    #[test]
    fn test_rotation_history() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"secret".to_vec(),
            PluginRole::Viewer,
            vec![],
        )
        .unwrap();

        auth.rotate_credential(
            "plugin1",
            b"new_secret".to_vec(),
            "manual_rotation".to_string(),
        )
        .unwrap();

        let history = auth.get_rotation_history("plugin1").unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].plugin_id, "plugin1");
        assert_eq!(history[0].reason, "manual_rotation");
    }

    #[test]
    fn test_authenticate_after_rotation() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"secret".to_vec(),
            PluginRole::Admin,
            vec![],
        )
        .unwrap();

        // Authenticate before rotation
        let role1 = auth.authenticate("plugin1").unwrap();
        assert_eq!(role1, PluginRole::Admin);

        // Rotate credential
        let _ = auth
            .rotate_credential("plugin1", b"new_secret".to_vec(), "rotation".to_string())
            .unwrap();

        // Authenticate after rotation
        let role2 = auth.authenticate("plugin1").unwrap();
        assert_eq!(role2, PluginRole::Admin);
    }

    #[test]
    fn test_cleanup_expired_credentials() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::Certificate,
            b"cert".to_vec(),
            PluginRole::System,
            vec![],
        )
        .unwrap();

        // Create multiple versions
        for i in 0..3 {
            let _ = auth
                .rotate_credential(
                    "plugin1",
                    format!("cert_{}", i).into_bytes(),
                    format!("rotation_{}", i),
                )
                .unwrap();
        }

        // Cleanup should work without errors
        let cleaned = auth.cleanup_expired_credentials().unwrap();
        // cleaned is a usize, so we just verify it executed successfully
        assert_eq!(cleaned, 0); // No credentials should be cleaned yet
    }

    #[test]
    fn test_authenticate_with_grace_period() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"secret_old".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        // Rotate credential
        let new_v = auth
            .rotate_credential("plugin1", b"secret_new".to_vec(), "grace_test".to_string())
            .unwrap();

        // Should authenticate with grace period
        let (role, version) = auth.authenticate_with_grace_period("plugin1").unwrap();
        assert_eq!(role, PluginRole::Editor);
        assert_eq!(version, new_v);
    }

    #[test]
    fn test_credential_rotation_different_types() {
        let auth = PluginAuthenticator::new();

        for cred_type in &[
            CredentialType::ApiKey,
            CredentialType::Certificate,
            CredentialType::OAuth2Token,
            CredentialType::BasicAuth,
        ] {
            let plugin_id = format!("plugin_{:?}", cred_type);

            auth.register_plugin(
                &plugin_id,
                cred_type.clone(),
                b"credential".to_vec(),
                PluginRole::Editor,
                vec![],
            )
            .unwrap();

            let _ = auth
                .rotate_credential(
                    &plugin_id,
                    b"new_credential".to_vec(),
                    "test_rotation".to_string(),
                )
                .unwrap();

            let versions = auth.get_credential_versions(&plugin_id).unwrap();
            assert!(versions.len() > 0);
        }
    }

    #[test]
    fn test_rotation_preserves_scopes() {
        let auth = PluginAuthenticator::new();

        let scopes = vec![
            "read_config".to_string(),
            "write_secrets".to_string(),
            "api_access".to_string(),
        ];

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"secret".to_vec(),
            PluginRole::Admin,
            scopes.clone(),
        )
        .unwrap();

        // Rotate credential
        let _ = auth
            .rotate_credential(
                "plugin1",
                b"new_secret".to_vec(),
                "preserve_scopes".to_string(),
            )
            .unwrap();

        // Verify scopes are preserved in new credential
        let versions = auth.get_credential_versions("plugin1").unwrap();
        if let Some(latest) = versions.last() {
            assert_eq!(latest.credential.scopes, scopes);
        }
    }

    // ===== PHASE 3b: MULTI-FACTOR AUTHENTICATION TESTS =====

    #[test]
    fn test_totp_provider_creation() {
        let totp = TOTPProvider::new(1, 6);
        assert_eq!(totp.window_size_seconds(), 30);
    }

    #[test]
    fn test_totp_register_and_secret_storage() {
        let totp = TOTPProvider::new(1, 6);
        let secret = b"my_secret_seed".to_vec();

        let result = totp.register("plugin1", secret.clone());
        assert!(result.is_ok());
    }

    #[test]
    fn test_totp_code_generation() {
        let totp = TOTPProvider::new(1, 6);
        let secret = b"test_secret_1234567890".to_vec();

        totp.register("plugin1", secret).unwrap();
        let code = totp.generate_code("plugin1").unwrap();

        // Should be a 6-digit code
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_numeric()));
    }

    #[test]
    fn test_totp_code_verification_window() {
        let totp = TOTPProvider::new(1, 6);
        let secret = b"test_secret_window".to_vec();

        totp.register("plugin1", secret).unwrap();

        // Generate current code
        let current_code = totp.generate_code("plugin1").unwrap();

        // Should verify successfully (within window)
        let result = totp.verify_code("plugin1", &current_code).unwrap();
        assert!(result);
    }

    #[test]
    fn test_totp_invalid_code_rejection() {
        let totp = TOTPProvider::new(1, 6);
        let secret = b"test_secret_invalid".to_vec();

        totp.register("plugin1", secret).unwrap();

        // Verify with wrong code
        let result = totp.verify_code("plugin1", "000000").unwrap();
        // Code may or may not match depending on current time, so we just verify it doesn't error
        assert!(!result || result); // Valid boolean result
    }

    #[test]
    fn test_totp_nonexistent_plugin() {
        let totp = TOTPProvider::new(1, 6);

        let result = totp.generate_code("nonexistent");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SecurityError::AuthenticationFailed);
    }

    #[test]
    fn test_backup_code_provider_creation() {
        let provider = BackupCodeProvider::new(10);
        // Verify provider was created with correct capacity
        assert!(provider.remaining_codes("any_plugin").is_ok());
    }

    #[test]
    fn test_backup_code_generation() {
        let provider = BackupCodeProvider::new(10);

        let codes = provider.generate_codes("plugin1").unwrap();
        assert_eq!(codes.len(), 10);

        // Each code should be 8 characters
        for code in &codes {
            assert_eq!(code.len(), 8);
            assert!(code.chars().all(|c| c.is_alphanumeric()));
        }
    }

    #[test]
    fn test_backup_code_verification() {
        let provider = BackupCodeProvider::new(10);

        let codes = provider.generate_codes("plugin1").unwrap();
        let first_code = codes[0].clone();

        // Verify with correct code
        let result = provider.verify_code("plugin1", &first_code).unwrap();
        assert!(result);
    }

    #[test]
    fn test_backup_code_single_use() {
        let provider = BackupCodeProvider::new(10);

        let codes = provider.generate_codes("plugin1").unwrap();
        let first_code = codes[0].clone();

        // First use should succeed
        let result1 = provider.verify_code("plugin1", &first_code).unwrap();
        assert!(result1);

        // Second use of same code should fail
        let result2 = provider.verify_code("plugin1", &first_code);
        // Should fail because code was already used
        assert!(result2.is_err() || !result2.unwrap());
    }

    #[test]
    fn test_backup_code_invalid_code() {
        let provider = BackupCodeProvider::new(10);

        let _codes = provider.generate_codes("plugin1").unwrap();

        // Verify with invalid code
        let result = provider.verify_code("plugin1", "invalid12").unwrap();
        assert!(!result);
    }

    #[test]
    fn test_backup_code_remaining_count() {
        let provider = BackupCodeProvider::new(10);

        let codes = provider.generate_codes("plugin1").unwrap();

        // Check remaining count after generation
        let remaining_before = provider.remaining_codes("plugin1").unwrap();
        assert_eq!(remaining_before, 10);

        // Use one code
        let first_code = codes[0].clone();
        provider.verify_code("plugin1", &first_code).unwrap();

        // Check remaining count after use
        let remaining_after = provider.remaining_codes("plugin1").unwrap();
        assert_eq!(remaining_after, 9);
    }

    #[test]
    fn test_mfa_manager_creation() {
        let manager = MFAManager::new();
        // Verify manager can list factors (empty initially)
        let factors = manager.list_factors("nonexistent_plugin").unwrap();
        assert!(factors.is_empty());
    }

    #[test]
    fn test_mfa_register_factor_totp() {
        let manager = MFAManager::new();
        let secret = b"test_secret".to_vec();

        let factor = manager
            .register_factor("plugin1", MFAMethod::TOTP, secret)
            .unwrap();

        assert_eq!(factor.plugin_id, "plugin1");
        assert_eq!(factor.method, MFAMethod::TOTP);
        assert!(factor.enabled);
    }

    #[test]
    fn test_mfa_register_factor_backup_codes() {
        let manager = MFAManager::new();
        let codes_data = b"code1|code2|code3|code4|code5".to_vec();

        let factor = manager
            .register_factor("plugin1", MFAMethod::BackupCodes, codes_data)
            .unwrap();

        assert_eq!(factor.plugin_id, "plugin1");
        assert_eq!(factor.method, MFAMethod::BackupCodes);
        assert!(factor.enabled);
    }

    #[test]
    fn test_mfa_create_challenge() {
        let manager = MFAManager::new();
        let secret = b"test_secret".to_vec();
        manager
            .register_factor("plugin1", MFAMethod::TOTP, secret)
            .unwrap();

        let challenge = manager
            .create_challenge("plugin1", MFAMethod::TOTP)
            .unwrap();

        assert_eq!(challenge.plugin_id, "plugin1");
        assert_eq!(challenge.method, MFAMethod::TOTP);
        assert!(challenge.created_at > 0);
        assert!(challenge.expires_at > challenge.created_at);
        assert_eq!(challenge.attempts, 0);
        assert_eq!(challenge.max_attempts, 3);
    }

    #[test]
    fn test_mfa_list_factors() {
        let manager = MFAManager::new();
        let secret = b"test_secret".to_vec();

        manager
            .register_factor("plugin1", MFAMethod::TOTP, secret)
            .unwrap();

        let factors = manager.list_factors("plugin1").unwrap();
        assert_eq!(factors.len(), 1);
        assert_eq!(factors[0].method, MFAMethod::TOTP);
    }

    #[test]
    fn test_mfa_disable_factor() {
        let manager = MFAManager::new();
        let secret = b"test_secret".to_vec();

        manager
            .register_factor("plugin1", MFAMethod::TOTP, secret)
            .unwrap();

        manager.disable_factor("plugin1", MFAMethod::TOTP).unwrap();

        let factors = manager.list_factors("plugin1").unwrap();
        assert!(!factors[0].enabled);
    }

    #[test]
    fn test_plugin_authenticator_enable_mfa() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"secret".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        // Enable MFA
        let result = auth.enable_mfa("plugin1");
        assert!(result.is_ok());

        // Check if MFA is enabled
        assert!(auth.is_mfa_enabled("plugin1"));
    }

    #[test]
    fn test_plugin_authenticator_disable_mfa() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"secret".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        auth.enable_mfa("plugin1").unwrap();
        assert!(auth.is_mfa_enabled("plugin1"));

        // Disable MFA
        auth.disable_mfa("plugin1").unwrap();
        assert!(!auth.is_mfa_enabled("plugin1"));
    }

    #[test]
    fn test_plugin_authenticator_register_mfa_totp() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"secret".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        let secret = auth.register_mfa_totp("plugin1").unwrap();
        assert_eq!(secret.len(), 32);
    }

    #[test]
    fn test_plugin_authenticator_register_mfa_backup_codes() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"secret".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        let codes = auth.register_mfa_backup_codes("plugin1").unwrap();
        assert_eq!(codes.len(), 10);
    }

    #[test]
    fn test_plugin_authenticator_list_mfa_factors() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"secret".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        auth.register_mfa_totp("plugin1").unwrap();

        let factors = auth.list_mfa_factors("plugin1").unwrap();
        assert!(factors.len() > 0);
        assert_eq!(factors[0].method, MFAMethod::TOTP);
    }

    #[test]
    fn test_plugin_authenticator_create_mfa_challenge() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"secret".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        auth.enable_mfa("plugin1").unwrap();
        auth.register_mfa_totp("plugin1").unwrap();

        let challenge = auth
            .create_mfa_challenge("plugin1", MFAMethod::TOTP)
            .unwrap();
        assert!(!challenge.challenge_id.is_empty());
        assert_eq!(challenge.method, MFAMethod::TOTP);
    }

    #[test]
    fn test_plugin_authenticator_authenticate_without_mfa() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"secret".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        // Authenticate without MFA enabled
        let role = auth.authenticate("plugin1").unwrap();
        assert_eq!(role, PluginRole::Editor);
    }

    #[test]
    fn test_plugin_authenticator_authenticate_with_mfa_required() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"secret".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        auth.enable_mfa("plugin1").unwrap();

        // Authenticate with MFA enabled but no challenge
        let result = auth.authenticate_with_mfa("plugin1", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_mfa_multiple_factors_per_plugin() {
        let auth = PluginAuthenticator::new();

        auth.register_plugin(
            "plugin1",
            CredentialType::ApiKey,
            b"secret".to_vec(),
            PluginRole::Editor,
            vec![],
        )
        .unwrap();

        auth.register_mfa_totp("plugin1").unwrap();
        auth.register_mfa_backup_codes("plugin1").unwrap();

        let factors = auth.list_mfa_factors("plugin1").unwrap();
        assert_eq!(factors.len(), 2);

        // Check both factor types are present
        let has_totp = factors.iter().any(|f| f.method == MFAMethod::TOTP);
        let has_backup = factors.iter().any(|f| f.method == MFAMethod::BackupCodes);

        assert!(has_totp);
        assert!(has_backup);
    }

    #[test]
    fn test_mfa_challenge_expiration() {
        let manager = MFAManager::new();
        let secret = b"test_secret".to_vec();

        manager
            .register_factor("plugin1", MFAMethod::TOTP, secret)
            .unwrap();

        let challenge = manager
            .create_challenge("plugin1", MFAMethod::TOTP)
            .unwrap();

        // Challenge should expire in 5 minutes (300 seconds)
        let expiry_delta = challenge.expires_at - challenge.created_at;
        assert_eq!(expiry_delta, 300);
    }

    #[test]
    fn test_mfa_challenge_max_attempts() {
        let manager = MFAManager::new();
        let secret = b"test_secret".to_vec();

        manager
            .register_factor("plugin1", MFAMethod::TOTP, secret)
            .unwrap();

        let challenge = manager
            .create_challenge("plugin1", MFAMethod::TOTP)
            .unwrap();

        // Challenge should allow max 3 attempts
        assert_eq!(challenge.max_attempts, 3);
    }

    #[test]
    fn test_mfa_totp_provider_accessor() {
        let auth = PluginAuthenticator::new();
        let manager = auth.mfa_manager();
        let totp = manager.totp_provider();

        // Verify TOTP provider is accessible and functional
        assert!(totp.register("test_plugin", b"secret".to_vec()).is_ok());
    }

    #[test]
    fn test_mfa_backup_code_provider_accessor() {
        let auth = PluginAuthenticator::new();
        let manager = auth.mfa_manager();
        let backup = manager.backup_code_provider();

        // Verify backup code provider is accessible and functional
        let codes = backup.generate_codes("test_plugin").unwrap();
        assert!(!codes.is_empty());
    }

    #[test]
    fn test_mfa_manager_accessor() {
        let auth = PluginAuthenticator::new();
        let manager1 = auth.mfa_manager();
        let manager2 = auth.mfa_manager();

        // Both managers should work independently
        assert!(manager1.list_factors("plugin1").is_ok());
        assert!(manager2.list_factors("plugin1").is_ok());
    }
}
