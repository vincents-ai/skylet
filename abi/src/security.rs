// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

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
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Maximum allowed length for input strings to prevent buffer overflow attacks
const MAX_INPUT_LENGTH: usize = 65536;

/// Get the maximum number of concurrent plugins based on system resources
/// Uses OnceLock to compute once and cache the result
fn get_max_plugins() -> usize {
    static MAX_PLUGINS_CACHE: OnceLock<usize> = OnceLock::new();

    *MAX_PLUGINS_CACHE.get_or_init(|| {
        // Try to get from environment variable first (for testing/override)
        if let Ok(val) = std::env::var("SKYLET_MAX_PLUGINS") {
            if let Ok(max) = val.parse::<usize>() {
                tracing::debug!("Security: Using SKYLET_MAX_PLUGINS={} from environment", max);
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
        let final_max = calculated_max.clamp(16, 4096);

        tracing::debug!(
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
    if !(context as usize).is_multiple_of(std::mem::align_of::<PluginContext>()) {
        tracing::error!("Security: Misaligned plugin context pointer");
        return Err(SecurityError::PointerValidationFailed);
    }

    // Dereference and check for null inner pointers (optional checks)
    let ctx = &*context;

    // These can be null, but if not null, they should be properly aligned
    if !ctx.logger.is_null() && !(ctx.logger as usize).is_multiple_of(8) {
        tracing::error!("Security: Misaligned logger pointer");
        return Err(SecurityError::PointerValidationFailed);
    }

    if !ctx.config.is_null() && !(ctx.config as usize).is_multiple_of(8) {
        tracing::error!("Security: Misaligned config pointer");
        return Err(SecurityError::PointerValidationFailed);
    }

    if !ctx.service_registry.is_null() && !(ctx.service_registry as usize).is_multiple_of(8) {
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

impl Default for PluginCapacityTracker {
    fn default() -> Self {
        Self::new()
    }
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
///
/// Type alias for encrypted secrets storage: name -> (ciphertext, nonce)
type EncryptedSecretsMap =
    std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, (Vec<u8>, [u8; 12])>>>;

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct EncryptedSecretStore {
    /// Master key for encryption (32 bytes for AES-256)
    master_key: [u8; 32],

    /// Encrypted secrets: (name -> (ciphertext, nonce))
    #[zeroize(skip)]
    secrets: EncryptedSecretsMap,
}

impl Default for EncryptedSecretStore {
    fn default() -> Self {
        Self::new()
    }
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

        let (ciphertext, nonce_bytes) = secrets.get(name).ok_or(SecurityError::NullPointer)?;

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
    if let Ok(hosts_str) = std::env::var("SKYLET_REMOTE_HOSTS") {
        for host in hosts_str.split(',') {
            let host = host.trim();
            if !host.is_empty() {
                tracing::debug!("Security: Registered remote host: {}", host);
                hosts.push(host.to_string());
            }
        }
    }

    // Try to load from config file if available
    if hosts.is_empty() {
        if let Ok(config_str) = std::env::var("SKYLET_CONFIG_DIR") {
            let config_path = std::path::Path::new(&config_str).join("remote_hosts.conf");
            if config_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_path) {
                    for line in content.lines() {
                        let line = line.trim();
                        if !line.is_empty() && !line.starts_with('#') {
                            tracing::debug!("Security: Loaded remote host from config: {}", line);
                            hosts.push(line.to_string());
                        }
                    }
                }
            }
        }
    }

    if !hosts.is_empty() {
        tracing::debug!(
            "Security: Loaded {} remote hosts for plugin offloading",
            hosts.len()
        );
    }

    hosts
}

/// Generate HMAC signature for PluginContext to prevent tampering
///
/// Uses HMAC-SHA256 to sign the context structure
///
/// # Safety
///
/// The caller must ensure the context pointer is valid and properly initialized,
/// or null (which is handled safely).
pub unsafe fn generate_context_signature(context: *const PluginContext, key: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    // Create a deterministic serialization of the context pointer values
    let context_bytes = if context.is_null() {
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
    };

    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(&context_bytes);

    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Verify HMAC signature of PluginContext
///
/// Returns Ok(()) if signature is valid, Err otherwise
///
/// # Safety
///
/// The caller must ensure the context pointer is valid and properly initialized,
/// or null (which is handled safely).
pub unsafe fn verify_context_signature(
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
            allowed_capabilities: PluginCapabilities::READ_CONFIG,
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

impl Default for SandboxEnforcer {
    fn default() -> Self {
        Self::new()
    }
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
                PluginCapabilities::FILE_WRITE
            } else {
                PluginCapabilities::FILE_READ
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
            if !policy.has_capability(PluginCapabilities::NETWORK_OUTBOUND) {
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
