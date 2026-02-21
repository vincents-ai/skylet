// Copyright 2024 Vincents AI
// SPDX-License-Identifier: Apache-2.0

#![allow(non_camel_case_types)]
#![allow(static_mut_refs)]

mod v2_ffi;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use skylet_abi::audit::{
    AuditEvent, AuditEventType, AuditPluginRegistry, AuditSeverity, DefaultAuditRegistry,
};
use skylet_abi::security::EncryptedSecretStore;
use skylet_abi::*;
use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::RwLock;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};
use zeroize::Zeroize;

// ============================================================================
// Service Definitions
// ============================================================================

/// Service for managing secrets - register as "SecretsService" with type "SecretsService"
#[repr(C)]
pub struct SecretsService {
    pub get_secret: extern "C" fn(path: *const c_char) -> SecretResult,
    pub set_secret: extern "C" fn(path: *const c_char, value: *const c_char) -> SecretResult,
    pub delete_secret: extern "C" fn(path: *const c_char) -> SecretResult,
    pub list_secrets: extern "C" fn(prefix: *const c_char) -> SecretListResult,
    pub free_string: extern "C" fn(ptr: *mut c_char),
    pub free_list: extern "C" fn(ptr: *mut SecretListResult),
}

#[repr(C)]
pub struct SecretResult {
    pub success: i32,
    pub value: *const c_char,
    pub error_message: *const c_char,
}

#[repr(C)]
pub struct SecretListResult {
    pub success: i32,
    pub secrets: *mut *const c_char,
    pub count: usize,
    pub error_message: *const c_char,
}

// ============================================================================
// Internal Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretKey(String);

impl SecretKey {
    pub fn new(key: String) -> Result<Self> {
        if key.is_empty() {
            return Err(anyhow!("Secret key cannot be empty"));
        }
        Ok(SecretKey(key))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Wrapper for secret values with automatic clearing on drop
#[derive(Clone, Serialize, Deserialize)]
pub struct SecretValue {
    value: String,
}

impl SecretValue {
    pub fn new(value: String) -> Self {
        SecretValue { value }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl Drop for SecretValue {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

impl std::fmt::Display for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretValue")
            .field("value", &"***REDACTED***")
            .finish()
    }
}

/// Backend trait for different secret storage backends
pub trait SecretBackend: Send + Sync {
    fn get(&self, key: &str) -> Result<SecretValue>;
    fn set(&self, key: &str, value: SecretValue) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
    fn list(&self, prefix: &str) -> Result<Vec<String>>;
}

/// In-memory backend implementation for development/testing
pub struct InMemoryBackend {
    secrets: Arc<RwLock<HashMap<String, SecretValue>>>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        InMemoryBackend {
            secrets: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretBackend for InMemoryBackend {
    fn get(&self, key: &str) -> Result<SecretValue> {
        let secrets = self
            .secrets
            .read()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        secrets
            .get(key)
            .cloned()
            .ok_or_else(|| anyhow!("Secret not found: {}", key))
    }

    fn set(&self, key: &str, value: SecretValue) -> Result<()> {
        let mut secrets = self
            .secrets
            .write()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        secrets.insert(key.to_string(), value);
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<()> {
        let mut secrets = self
            .secrets
            .write()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        if secrets.remove(key).is_none() {
            return Err(anyhow!("Secret not found: {}", key));
        }
        Ok(())
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let secrets = self
            .secrets
            .read()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        let mut results: Vec<String> = secrets
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        results.sort();
        Ok(results)
    }
}

/// AES-256-GCM encrypted backend for production use (CVSS 8.2)
///
/// Provides encrypted-at-rest security for secrets using:
/// - AES-256-GCM authenticated encryption
/// - Random 96-bit nonces per secret
/// - Automatic key rotation support
/// - Tampering detection via authentication tags
pub struct EncryptedSecretBackend {
    store: Arc<EncryptedSecretStore>,
}

impl EncryptedSecretBackend {
    pub fn new() -> Self {
        EncryptedSecretBackend {
            store: Arc::new(EncryptedSecretStore::new()),
        }
    }
}

impl Default for EncryptedSecretBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretBackend for EncryptedSecretBackend {
    fn get(&self, key: &str) -> Result<SecretValue> {
        let encrypted_bytes = self
            .store
            .get_secret(key)
            .map_err(|e| anyhow!("Failed to retrieve encrypted secret: {:?}", e))?;

        // Convert bytes back to string
        let value = String::from_utf8(encrypted_bytes)
            .map_err(|e| anyhow!("Failed to decode secret value: {}", e))?;

        Ok(SecretValue::new(value))
    }

    fn set(&self, key: &str, value: SecretValue) -> Result<()> {
        let secret_bytes = value.as_str().as_bytes();
        self.store
            .store_secret(key, secret_bytes)
            .map_err(|e| anyhow!("Failed to store encrypted secret: {:?}", e))
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.store
            .remove_secret(key)
            .map_err(|e| anyhow!("Failed to delete secret: {:?}", e))
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let secret_names = self.store.list_secret_names();
        let mut results: Vec<String> = secret_names
            .iter()
            .filter(|name| name.starts_with(prefix))
            .cloned()
            .collect();
        results.sort();
        Ok(results)
    }
}

/// Main SecretsManager struct
pub struct SecretsManager {
    backend: Arc<dyn SecretBackend>,
}

impl SecretsManager {
    pub fn new(backend: Arc<dyn SecretBackend>) -> Self {
        SecretsManager { backend }
    }

    pub fn with_in_memory() -> Self {
        SecretsManager {
            backend: Arc::new(InMemoryBackend::new()),
        }
    }

    /// Create a new secrets manager with AES-256-GCM encrypted storage (CVSS 8.2)
    ///
    /// This backend provides:
    /// - Encrypted-at-rest security for all secrets
    /// - Authenticated encryption (AES-256-GCM)
    /// - Automatic random nonce generation
    /// - Tampering detection
    /// - Recommended for production use
    pub fn with_encrypted() -> Self {
        SecretsManager {
            backend: Arc::new(EncryptedSecretBackend::new()),
        }
    }

    pub fn get_secret(&self, key: &str) -> Result<SecretValue> {
        self.backend.get(key)
    }

    pub fn set_secret(&self, key: &str, value: SecretValue) -> Result<()> {
        self.backend.set(key, value)
    }

    pub fn delete_secret(&self, key: &str) -> Result<()> {
        self.backend.delete(key)
    }

    pub fn list_secrets(&self, prefix: &str) -> Result<Vec<String>> {
        self.backend.list(prefix)
    }
}

// ============================================================================
// Rotation Policy Configuration
// ============================================================================

/// Rotation policy configuration for secrets management
///
/// This configuration struct defines how secrets are rotated, including:
/// - Rotation interval: How often to rotate secrets (in days)
/// - Auto-rotation trigger: When to automatically trigger rotation (in days)
/// - Rotation window: Time window for rotation to occur (in hours)
/// - Notification lead time: When to notify about upcoming rotations (in days)
/// - Maximum secret age: Maximum age before forced rotation (in days)
/// - Key overlap period: How long to keep old keys during transition (in days)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RotationPolicyConfig {
    /// Rotation interval in days (default: 90)
    #[serde(default = "default_interval_days")]
    pub interval_days: u32,

    /// Auto-rotation trigger in days (default: 85)
    #[serde(default = "default_auto_rotate_days")]
    pub auto_rotate_days: u32,

    /// Rotation window in hours (default: 4)
    #[serde(default = "default_rotation_window_hours")]
    pub rotation_window_hours: u32,

    /// Notification lead time in days (default: 7)
    #[serde(default = "default_notification_lead_days")]
    pub notification_lead_days: u32,

    /// Maximum secret age in days (default: 365)
    #[serde(default = "default_max_age_days")]
    pub max_age_days: u32,

    /// Key overlap period in days for graceful transition (default: 7)
    #[serde(default = "default_key_overlap_days")]
    pub key_overlap_days: u32,
}

// Default value functions for serde
fn default_interval_days() -> u32 {
    90
}
fn default_auto_rotate_days() -> u32 {
    95
}
fn default_rotation_window_hours() -> u32 {
    4
}
fn default_notification_lead_days() -> u32 {
    7
}
fn default_max_age_days() -> u32 {
    365
}
fn default_key_overlap_days() -> u32 {
    7
}

impl Default for RotationPolicyConfig {
    fn default() -> Self {
        RotationPolicyConfig {
            interval_days: default_interval_days(),
            auto_rotate_days: default_auto_rotate_days(),
            rotation_window_hours: default_rotation_window_hours(),
            notification_lead_days: default_notification_lead_days(),
            max_age_days: default_max_age_days(),
            key_overlap_days: default_key_overlap_days(),
        }
    }
}

impl RotationPolicyConfig {
    /// Validate the configuration to ensure consistency
    pub fn validate(&self) -> Result<()> {
        // Ensure max_age_days >= auto_rotate_days >= interval_days
        if self.max_age_days < self.auto_rotate_days {
            return Err(anyhow!(
                "max_age_days ({}) must be >= auto_rotate_days ({})",
                self.max_age_days,
                self.auto_rotate_days
            ));
        }

        if self.auto_rotate_days < self.interval_days {
            return Err(anyhow!(
                "auto_rotate_days ({}) must be >= interval_days ({})",
                self.auto_rotate_days,
                self.interval_days
            ));
        }

        // Ensure notification_lead_days is positive
        if self.notification_lead_days == 0 {
            return Err(anyhow!("notification_lead_days must be greater than 0"));
        }

        // Ensure key_overlap_days is positive
        if self.key_overlap_days == 0 {
            return Err(anyhow!("key_overlap_days must be greater than 0"));
        }

        Ok(())
    }

    /// Load configuration from a TOML file
    pub fn from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(anyhow!("Configuration file not found: {:?}", path));
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read configuration file: {}", e))?;

        Self::parse(&content)
    }

    /// Parse configuration from TOML string
    pub fn parse(content: &str) -> Result<Self> {
        let config: RotationPolicyConfig =
            toml::from_str(content).map_err(|e| anyhow!("Failed to parse configuration: {}", e))?;

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from environment variable or file
    /// Looks for SKYNET_ROTATION_POLICY_CONFIG env var pointing to a file,
    /// or uses the default configuration
    pub fn load_from_env_or_default() -> Result<Self> {
        if let Ok(config_path) = std::env::var("SKYNET_ROTATION_POLICY_CONFIG") {
            debug!(
                "Loading rotation policy configuration from: {}",
                config_path
            );
            Self::from_file(Path::new(&config_path))
        } else {
            debug!("Using default rotation policy configuration");
            Ok(Self::default())
        }
    }

    /// Export configuration to TOML format
    pub fn to_toml_string(&self) -> Result<String> {
        toml::to_string_pretty(self)
            .map_err(|e| anyhow!("Failed to serialize configuration: {}", e))
    }

    /// Write configuration to a file
    pub fn to_file(&self, path: &Path) -> Result<()> {
        let content = self.to_toml_string()?;
        std::fs::write(path, content)
            .map_err(|e| anyhow!("Failed to write configuration file: {}", e))
    }
}

// ============================================================================
// Versioned Secret Storage with Overlap Support
// ============================================================================

/// Status of a secret version in its lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecretVersionStatus {
    /// Currently active version - returned by default on get operations
    Active,
    /// Deprecated version during overlap period - still accessible but not default
    Deprecated,
    /// Pending deletion - overlap period has expired
    PendingDeletion,
    /// Soft-deleted version (kept for audit trail)
    Deleted,
}

impl std::fmt::Display for SecretVersionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecretVersionStatus::Active => write!(f, "active"),
            SecretVersionStatus::Deprecated => write!(f, "deprecated"),
            SecretVersionStatus::PendingDeletion => write!(f, "pending_deletion"),
            SecretVersionStatus::Deleted => write!(f, "deleted"),
        }
    }
}

/// Individual secret version with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretVersion {
    /// Unique version identifier (UUID)
    pub version_id: String,
    /// The logical secret key
    pub secret_key: String,
    /// The secret value
    pub value: SecretValue,
    /// When this version was created
    pub created_at: u64,
    /// When this version expires (for overlap period tracking)
    pub expires_at: Option<u64>,
    /// Current status in lifecycle
    pub status: SecretVersionStatus,
    /// Reason for rotation (if this is a rotated version)
    pub rotation_reason: Option<String>,
    /// Who/what triggered the rotation
    pub rotated_by: Option<String>,
}

impl SecretVersion {
    /// Create a new active secret version
    pub fn new(
        secret_key: String,
        value: SecretValue,
        rotation_reason: Option<String>,
        rotated_by: Option<String>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            version_id: uuid::Uuid::new_v4().to_string(),
            secret_key,
            value,
            created_at: now,
            expires_at: None,
            status: SecretVersionStatus::Active,
            rotation_reason,
            rotated_by,
        }
    }

    /// Check if this version is accessible (Active or Deprecated)
    pub fn is_accessible(&self) -> bool {
        matches!(
            self.status,
            SecretVersionStatus::Active | SecretVersionStatus::Deprecated
        )
    }

    /// Check if this version is the active one
    pub fn is_active(&self) -> bool {
        self.status == SecretVersionStatus::Active
    }

    /// Check if this version has expired (past its overlap period)
    pub fn has_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            now >= expires
        } else {
            false
        }
    }
}

/// Metadata for a secret key tracking its versions and rotation policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    /// The secret key
    pub key: String,
    /// ID of the currently active version
    pub current_version_id: String,
    /// When the secret was first created
    pub created_at: u64,
    /// When the secret was last rotated
    pub last_rotated_at: Option<u64>,
    /// How many times this secret has been rotated
    pub rotation_count: u32,
    /// Rotation policy for this specific secret (overrides global default)
    pub rotation_policy: RotationPolicyConfig,
}

/// Backend trait for versioned secret storage with overlap support
pub trait VersionedSecretBackend: Send + Sync {
    /// Get the active version of a secret
    fn get(&self, key: &str) -> Result<SecretValue>;

    /// Get a specific version by ID
    fn get_version(&self, key: &str, version_id: &str) -> Result<SecretVersion>;

    /// Get the active version metadata
    fn get_active_version(&self, key: &str) -> Result<SecretVersion>;

    /// Get all accessible versions (Active and Deprecated) for a secret
    fn get_all_versions(&self, key: &str) -> Result<Vec<SecretVersion>>;

    /// Set/create a new secret (creates first version)
    fn set(&self, key: &str, value: SecretValue) -> Result<SecretVersion>;

    /// Rotate a secret - create new version, deprecate old
    fn rotate(
        &self,
        key: &str,
        new_value: SecretValue,
        reason: Option<&str>,
        rotated_by: Option<&str>,
    ) -> Result<SecretVersion>;

    /// Delete all versions of a secret
    fn delete(&self, key: &str) -> Result<()>;

    /// Delete a specific version
    fn delete_version(&self, key: &str, version_id: &str) -> Result<()>;

    /// List all secret keys
    fn list(&self, prefix: &str) -> Result<Vec<String>>;

    /// Get metadata for a secret
    fn get_metadata(&self, key: &str) -> Result<SecretMetadata>;

    /// Update rotation policy for a secret
    fn update_rotation_policy(&self, key: &str, policy: RotationPolicyConfig) -> Result<()>;

    /// Clean up expired versions (mark PendingDeletion)
    /// Returns count of versions marked for deletion
    fn cleanup_expired_versions(&self) -> Result<u32>;

    /// Permanently delete versions marked for deletion
    /// Returns count of versions permanently deleted
    fn purge_deleted_versions(&self) -> Result<u32>;
}

/// In-memory implementation of versioned secret storage
pub struct VersionedInMemoryBackend {
    /// Stores all versions per key: key -> Vec<versions>
    versions: Arc<RwLock<HashMap<String, Vec<SecretVersion>>>>,
    /// Stores metadata per key: key -> metadata
    metadata: Arc<RwLock<HashMap<String, SecretMetadata>>>,
    /// Default rotation policy
    default_policy: RotationPolicyConfig,
}

impl VersionedInMemoryBackend {
    pub fn new() -> Self {
        Self {
            versions: Arc::new(RwLock::new(HashMap::new())),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            default_policy: RotationPolicyConfig::default(),
        }
    }

    pub fn with_policy(policy: RotationPolicyConfig) -> Self {
        Self {
            versions: Arc::new(RwLock::new(HashMap::new())),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            default_policy: policy,
        }
    }

    /// Calculate expiration time based on overlap period
    fn calculate_expiration(&self, overlap_days: u32) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now + (overlap_days as u64 * 24 * 60 * 60)
    }
}

impl Default for VersionedInMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl VersionedSecretBackend for VersionedInMemoryBackend {
    fn get(&self, key: &str) -> Result<SecretValue> {
        let versions = self
            .versions
            .read()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        let key_versions = versions
            .get(key)
            .ok_or_else(|| anyhow!("Secret not found: {}", key))?;

        // Find the active version
        let active = key_versions
            .iter()
            .find(|v| v.status == SecretVersionStatus::Active)
            .ok_or_else(|| anyhow!("No active version found for secret: {}", key))?;

        Ok(active.value.clone())
    }

    fn get_version(&self, key: &str, version_id: &str) -> Result<SecretVersion> {
        let versions = self
            .versions
            .read()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        let key_versions = versions
            .get(key)
            .ok_or_else(|| anyhow!("Secret not found: {}", key))?;

        key_versions
            .iter()
            .find(|v| v.version_id == version_id && v.is_accessible())
            .cloned()
            .ok_or_else(|| anyhow!("Version {} not found for secret: {}", version_id, key))
    }

    fn get_active_version(&self, key: &str) -> Result<SecretVersion> {
        let versions = self
            .versions
            .read()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        let key_versions = versions
            .get(key)
            .ok_or_else(|| anyhow!("Secret not found: {}", key))?;

        key_versions
            .iter()
            .find(|v| v.status == SecretVersionStatus::Active)
            .cloned()
            .ok_or_else(|| anyhow!("No active version found for secret: {}", key))
    }

    fn get_all_versions(&self, key: &str) -> Result<Vec<SecretVersion>> {
        let versions = self
            .versions
            .read()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        let key_versions = versions
            .get(key)
            .ok_or_else(|| anyhow!("Secret not found: {}", key))?;

        Ok(key_versions
            .iter()
            .filter(|v| v.is_accessible())
            .cloned()
            .collect())
    }

    fn set(&self, key: &str, value: SecretValue) -> Result<SecretVersion> {
        let mut versions = self
            .versions
            .write()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        let mut metadata = self
            .metadata
            .write()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Create new version
        let version = SecretVersion::new(key.to_string(), value, None, None);

        // Store version
        let key_versions = versions.entry(key.to_string()).or_default();
        key_versions.push(version.clone());

        // Create/update metadata
        let meta = SecretMetadata {
            key: key.to_string(),
            current_version_id: version.version_id.clone(),
            created_at: now,
            last_rotated_at: None,
            rotation_count: 0,
            rotation_policy: self.default_policy.clone(),
        };
        metadata.insert(key.to_string(), meta);

        Ok(version)
    }

    fn rotate(
        &self,
        key: &str,
        new_value: SecretValue,
        reason: Option<&str>,
        rotated_by: Option<&str>,
    ) -> Result<SecretVersion> {
        let mut versions = self
            .versions
            .write()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        let mut metadata = self
            .metadata
            .write()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Get the policy for overlap calculation
        let policy = metadata
            .get(key)
            .map(|m| m.rotation_policy.clone())
            .unwrap_or_else(|| self.default_policy.clone());

        // Deprecate existing active version
        if let Some(key_versions) = versions.get_mut(key) {
            for version in key_versions.iter_mut() {
                if version.status == SecretVersionStatus::Active {
                    version.status = SecretVersionStatus::Deprecated;
                    version.expires_at = Some(self.calculate_expiration(policy.key_overlap_days));
                }
            }
        }

        // Create new active version
        let new_version = SecretVersion::new(
            key.to_string(),
            new_value,
            reason.map(|s| s.to_string()),
            rotated_by.map(|s| s.to_string()),
        );

        // Store new version
        let key_versions = versions.entry(key.to_string()).or_default();
        key_versions.push(new_version.clone());

        // Update metadata
        let meta = metadata.entry(key.to_string()).or_insert(SecretMetadata {
            key: key.to_string(),
            current_version_id: new_version.version_id.clone(),
            created_at: now,
            last_rotated_at: None,
            rotation_count: 0,
            rotation_policy: policy,
        });

        meta.current_version_id = new_version.version_id.clone();
        meta.last_rotated_at = Some(now);
        meta.rotation_count += 1;

        Ok(new_version)
    }

    fn delete(&self, key: &str) -> Result<()> {
        let mut versions = self
            .versions
            .write()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        let mut metadata = self
            .metadata
            .write()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        // Check if secret exists
        if !versions.contains_key(key) {
            return Err(anyhow!("Secret not found: {}", key));
        }

        // Remove all versions for this key
        versions.remove(key);

        // Remove metadata
        metadata.remove(key);

        Ok(())
    }

    fn delete_version(&self, key: &str, version_id: &str) -> Result<()> {
        let mut versions = self
            .versions
            .write()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        let key_versions = versions
            .get_mut(key)
            .ok_or_else(|| anyhow!("Secret not found: {}", key))?;

        let version = key_versions
            .iter_mut()
            .find(|v| v.version_id == version_id)
            .ok_or_else(|| anyhow!("Version {} not found for secret: {}", version_id, key))?;

        version.status = SecretVersionStatus::Deleted;

        Ok(())
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let metadata = self
            .metadata
            .read()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        let mut results: Vec<String> = metadata
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        results.sort();
        Ok(results)
    }

    fn get_metadata(&self, key: &str) -> Result<SecretMetadata> {
        let metadata = self
            .metadata
            .read()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        metadata
            .get(key)
            .cloned()
            .ok_or_else(|| anyhow!("Secret metadata not found: {}", key))
    }

    fn update_rotation_policy(&self, key: &str, policy: RotationPolicyConfig) -> Result<()> {
        let mut metadata = self
            .metadata
            .write()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        let meta = metadata
            .get_mut(key)
            .ok_or_else(|| anyhow!("Secret not found: {}", key))?;

        policy.validate()?;
        meta.rotation_policy = policy;

        Ok(())
    }

    fn cleanup_expired_versions(&self) -> Result<u32> {
        let mut versions = self
            .versions
            .write()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut count = 0u32;

        for key_versions in versions.values_mut() {
            for version in key_versions.iter_mut() {
                if version.status == SecretVersionStatus::Deprecated {
                    if let Some(expires) = version.expires_at {
                        if now >= expires {
                            version.status = SecretVersionStatus::PendingDeletion;
                            count += 1;
                        }
                    }
                }
            }
        }

        Ok(count)
    }

    fn purge_deleted_versions(&self) -> Result<u32> {
        let mut versions = self
            .versions
            .write()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        let mut count = 0u32;

        for key_versions in versions.values_mut() {
            let before_len = key_versions.len();
            // Remove both PendingDeletion and Deleted versions
            key_versions.retain(|v| {
                v.status != SecretVersionStatus::PendingDeletion
                    && v.status != SecretVersionStatus::Deleted
            });
            count += (before_len - key_versions.len()) as u32;
        }

        // Remove empty version lists
        versions.retain(|_, v| !v.is_empty());

        Ok(count)
    }
}

// ============================================================================

/// Manager for rotation policies across secrets
pub struct RotationManager {
    backend: Arc<dyn VersionedSecretBackend>,
    default_policy: std::sync::RwLock<RotationPolicyConfig>,
}

impl RotationManager {
    pub fn new(
        backend: Arc<dyn VersionedSecretBackend>,
        default_policy: RotationPolicyConfig,
    ) -> Self {
        Self {
            backend,
            default_policy: std::sync::RwLock::new(default_policy),
        }
    }

    /// Create a new RotationManager with the given backend and default rotation policy
    pub fn with_backend(backend: Arc<dyn VersionedSecretBackend>) -> Self {
        Self::new(backend, RotationPolicyConfig::default())
    }

    pub fn get_policy(&self, key: &str) -> Result<RotationPolicyConfig> {
        let metadata = self.backend.get_metadata(key)?;
        Ok(metadata.rotation_policy.clone())
    }

    pub fn set_policy(&self, key: &str, policy: RotationPolicyConfig) -> Result<()> {
        policy.validate()?;
        self.backend.update_rotation_policy(key, policy)
    }

    pub fn remove_policy(&self, key: &str) -> Result<()> {
        let default = self
            .default_policy
            .read()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        self.backend.update_rotation_policy(key, default.clone())
    }

    pub fn get_default_policy(&self) -> RotationPolicyConfig {
        self.default_policy
            .read()
            .map(|p| p.clone())
            .unwrap_or_default()
    }

    /// Set the default rotation policy for new secrets
    pub fn set_default_policy(&mut self, policy: RotationPolicyConfig) -> Result<()> {
        policy.validate()?;
        let mut default = self
            .default_policy
            .write()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        *default = policy;
        Ok(())
    }

    pub fn list_custom_policies(&self) -> Result<Vec<String>> {
        let all_keys = self.backend.list("")?;
        let mut custom_keys = Vec::new();
        let default = self
            .default_policy
            .read()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        for key in all_keys {
            if let Ok(meta) = self.backend.get_metadata(&key) {
                if meta.rotation_policy != *default {
                    custom_keys.push(key);
                }
            }
        }

        Ok(custom_keys)
    }

    pub fn rotate_secret(
        &self,
        key: &str,
        reason: Option<&str>,
        rotated_by: Option<&str>,
    ) -> Result<SecretVersion> {
        let new_value = generate_new_secret_value();
        self.backend.rotate(key, new_value, reason, rotated_by)
    }

    pub fn check_rotation_eligibility(&self, key: &str) -> Result<Option<RotationEligibility>> {
        let metadata = self.backend.get_metadata(key)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Ok(check_rotation_eligibility(&metadata, now))
    }

    /// Get all secrets that need rotation based on their policies
    pub fn get_secrets_needing_rotation(&self) -> Result<Vec<String>> {
        let all_keys = self.backend.list("")?;
        let mut needing_rotation = Vec::new();

        for key in all_keys {
            if let Ok(Some(eligibility)) = self.check_rotation_eligibility(&key) {
                match eligibility {
                    RotationEligibility::Forced { .. } | RotationEligibility::Scheduled { .. } => {
                        needing_rotation.push(key);
                    }
                    RotationEligibility::Warning { .. } => {
                        // Warnings don't trigger automatic rotation
                    }
                }
            }
        }

        Ok(needing_rotation)
    }

    /// Rotate all secrets that need rotation and return the count of rotated secrets
    pub fn rotate_needing_secrets(&self) -> Result<u32> {
        let needing = self.get_secrets_needing_rotation()?;
        let mut count = 0u32;

        for key in needing {
            if self
                .rotate_secret(&key, Some("auto-rotation"), Some("rotation_manager"))
                .is_ok()
            {
                count += 1;
            }
        }

        Ok(count)
    }
}
// Rotation Scheduler
// ============================================================================

/// Controls the rotation scheduler background task
static ROTATION_SCHEDULER_RUNNING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Configuration for the rotation scheduler
#[derive(Debug, Clone)]
pub struct RotationSchedulerConfig {
    /// How often to check for rotations (in seconds)
    pub check_interval_secs: u64,
    /// How often to clean up expired versions (in seconds)
    pub cleanup_interval_secs: u64,
    /// Enable automatic rotation execution
    pub auto_rotate_enabled: bool,
    /// Enable cleanup tasks
    pub cleanup_enabled: bool,
}

impl Default for RotationSchedulerConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 3600,    // Check every hour
            cleanup_interval_secs: 86400, // Cleanup daily
            auto_rotate_enabled: true,
            cleanup_enabled: true,
        }
    }
}

impl RotationSchedulerConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(val) = std::env::var("SKYNET_ROTATION_CHECK_INTERVAL") {
            if let Ok(secs) = val.parse() {
                config.check_interval_secs = secs;
            }
        }

        if let Ok(val) = std::env::var("SKYNET_ROTATION_CLEANUP_INTERVAL") {
            if let Ok(secs) = val.parse() {
                config.cleanup_interval_secs = secs;
            }
        }

        if let Ok(val) = std::env::var("SKYNET_ROTATION_AUTO_ENABLED") {
            config.auto_rotate_enabled = val.to_lowercase() == "true" || val == "1";
        }

        if let Ok(val) = std::env::var("SKYNET_ROTATION_CLEANUP_ENABLED") {
            config.cleanup_enabled = val.to_lowercase() == "true" || val == "1";
        }

        config
    }
}

/// Check if a secret needs rotation based on its policy
fn check_rotation_eligibility(metadata: &SecretMetadata, now: u64) -> Option<RotationEligibility> {
    let policy = &metadata.rotation_policy;

    // Determine baseline timestamp
    let baseline = metadata.last_rotated_at.unwrap_or(metadata.created_at);

    // Calculate days elapsed
    let days_elapsed = (now - baseline) / (24 * 60 * 60);

    // Check if past max_age_days (forced rotation)
    if days_elapsed >= policy.max_age_days as u64 {
        return Some(RotationEligibility::Forced {
            days_elapsed,
            max_age: policy.max_age_days,
        });
    }

    // Check if past auto_rotate_days (scheduled rotation)
    if days_elapsed >= policy.auto_rotate_days as u64 {
        return Some(RotationEligibility::Scheduled {
            days_elapsed,
            auto_rotate_days: policy.auto_rotate_days,
        });
    }

    // Check if approaching auto_rotate (warning window)
    let warning_days = policy
        .auto_rotate_days
        .saturating_sub(policy.notification_lead_days);
    if days_elapsed >= warning_days as u64 {
        return Some(RotationEligibility::Warning {
            days_elapsed,
            auto_rotate_days: policy.auto_rotate_days,
            days_remaining: policy.auto_rotate_days.saturating_sub(days_elapsed as u32),
        });
    }

    None
}

/// Eligibility status for secret rotation
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum RotationEligibility {
    /// Approaching rotation deadline (warning)
    Warning {
        days_elapsed: u64,
        auto_rotate_days: u32,
        days_remaining: u32,
    },
    /// Past auto-rotate threshold (scheduled)
    Scheduled {
        days_elapsed: u64,
        auto_rotate_days: u32,
    },
    /// Past max age (forced)
    Forced { days_elapsed: u64, max_age: u32 },
}

impl RotationEligibility {
    /// Get the reason string for rotation
    fn reason(&self) -> String {
        match self {
            RotationEligibility::Warning { days_remaining, .. } => {
                format!(
                    "approaching rotation deadline ({} days remaining)",
                    days_remaining
                )
            }
            RotationEligibility::Scheduled {
                days_elapsed,
                auto_rotate_days,
            } => {
                let _ = days_elapsed; // Acknowledge field usage
                let _ = auto_rotate_days; // Acknowledge field usage
                format!(
                    "scheduled rotation ({} days elapsed, threshold: {} days)",
                    days_elapsed, auto_rotate_days
                )
            }
            RotationEligibility::Forced {
                days_elapsed,
                max_age,
            } => {
                format!(
                    "forced rotation - exceeded max age ({} days elapsed, max: {} days)",
                    days_elapsed, max_age
                )
            }
        }
    }

    /// Check if this eligibility requires rotation action
    fn requires_rotation(&self) -> bool {
        matches!(
            self,
            RotationEligibility::Scheduled { .. } | RotationEligibility::Forced { .. }
        )
    }
}

/// Generate a new secret value for rotation
/// In production, this would integrate with a proper secret generation service
fn generate_new_secret_value() -> SecretValue {
    // Generate a random secret value
    // Using uuid for demonstration - in production this would use proper cryptographic generation
    let value = format!(
        "rotated-{}-{}",
        uuid::Uuid::new_v4(),
        chrono::Utc::now().timestamp()
    );
    SecretValue::new(value)
}

/// Background task that periodically checks and executes rotation policies
async fn rotation_scheduler_task(
    backend: Arc<dyn VersionedSecretBackend>,
    config: RotationSchedulerConfig,
) {
    use tokio::time::{interval, Duration};

    let mut check_interval = interval(Duration::from_secs(config.check_interval_secs));
    let mut cleanup_interval = interval(Duration::from_secs(config.cleanup_interval_secs));

    info!(
        "RotationScheduler: Started (check_interval: {}s, cleanup_interval: {}s, auto_rotate: {}, cleanup: {})",
        config.check_interval_secs,
        config.cleanup_interval_secs,
        config.auto_rotate_enabled,
        config.cleanup_enabled
    );

    while ROTATION_SCHEDULER_RUNNING.load(std::sync::atomic::Ordering::Relaxed) {
        tokio::select! {
            _ = check_interval.tick() => {
                if config.auto_rotate_enabled {
                    if let Err(e) = check_and_execute_rotations(&*backend).await {
                        error!("RotationScheduler: Error during rotation check: {}", e);
                    }
                }
            }
            _ = cleanup_interval.tick() => {
                if config.cleanup_enabled {
                    if let Err(e) = perform_cleanup(&*backend).await {
                        error!("RotationScheduler: Error during cleanup: {}", e);
                    }
                }
            }
        }
    }

    info!("RotationScheduler: Shutting down gracefully");
}

/// Check all secrets and execute rotations for eligible ones
async fn check_and_execute_rotations(backend: &dyn VersionedSecretBackend) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Get all secret keys
    let keys = backend.list("")?;

    let mut checked = 0u32;
    let mut rotated = 0u32;
    let mut warned = 0u32;

    for key in keys {
        checked += 1;

        let metadata = match backend.get_metadata(&key) {
            Ok(meta) => meta,
            Err(e) => {
                warn!(
                    "RotationScheduler: Failed to get metadata for '{}': {}",
                    key, e
                );
                continue;
            }
        };

        if let Some(eligibility) = check_rotation_eligibility(&metadata, now) {
            let reason = eligibility.reason();

            if eligibility.requires_rotation() {
                debug!("RotationScheduler: Rotating secret '{}' - {}", key, reason);

                let new_value = generate_new_secret_value();

                match backend.rotate(&key, new_value, Some(&reason), Some("rotation_scheduler")) {
                    Ok(version) => {
                        rotated += 1;
                        log_secret_operation(
                            "rotate",
                            &key,
                            true,
                            Some(&format!("New version: {}", version.version_id)),
                        );
                        info!(
                            "RotationScheduler: Successfully rotated '{}' to version {}",
                            key, version.version_id
                        );
                    }
                    Err(e) => {
                        log_secret_operation("rotate", &key, false, Some(&e.to_string()));
                        error!("RotationScheduler: Failed to rotate '{}': {}", key, e);
                    }
                }
            } else {
                warned += 1;
                warn!(
                    "RotationScheduler: Warning for secret '{}' - {}",
                    key, reason
                );
            }
        }
    }

    if checked > 0 {
        debug!(
            "RotationScheduler: Checked {} secrets, rotated {}, warned {}",
            checked, rotated, warned
        );
    }

    Ok(())
}

async fn perform_cleanup(backend: &dyn VersionedSecretBackend) -> Result<()> {
    debug!("RotationScheduler: Starting cleanup task");

    match backend.cleanup_expired_versions() {
        Ok(count) => {
            if count > 0 {
                info!(
                    "RotationScheduler: Marked {} expired versions for deletion",
                    count
                );
            }
        }
        Err(e) => {
            error!(
                "RotationScheduler: Error cleaning up expired versions: {}",
                e
            );
        }
    }

    match backend.purge_deleted_versions() {
        Ok(count) => {
            if count > 0 {
                info!("RotationScheduler: Purged {} deleted versions", count);
            }
        }
        Err(e) => {
            error!("RotationScheduler: Error purging deleted versions: {}", e);
        }
    }

    debug!("RotationScheduler: Cleanup task completed");
    Ok(())
}

/// Start the rotation scheduler background task
fn start_rotation_scheduler(backend: Arc<dyn VersionedSecretBackend>) {
    ROTATION_SCHEDULER_RUNNING.store(true, std::sync::atomic::Ordering::SeqCst);

    let config = RotationSchedulerConfig::from_env();

    // Spawn background task in a dedicated thread with its own Tokio runtime
    // This ensures the plugin can initialize background tasks even when loaded
    // from a synchronous context (like plugin_init)
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime for rotation scheduler");

        rt.block_on(async move {
            rotation_scheduler_task(backend, config).await;
        });
    });

    info!("RotationScheduler: Background task started");
}

fn stop_rotation_scheduler() {
    ROTATION_SCHEDULER_RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
    info!("RotationScheduler: Stop signal sent");
}

// ============================================================================
// Compliance Tracking
// ============================================================================

/// Compliance status for a secret based on rotation policies
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceStatus {
    /// Secret is fully compliant with all policies
    Compliant,
    /// Secret needs rotation (past auto-rotate threshold)
    NeedsRotation,
    /// Secret is past maximum age (non-compliant)
    NonCompliant,
    /// Secret is in warning window (approaching rotation)
    Warning,
}

/// Compliance record for audit and reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceRecord {
    /// When compliance was checked
    pub checked_at: u64,
    /// Secret key that was checked
    pub secret_key: String,
    /// Compliance status at check time
    pub status: ComplianceStatus,
    /// Days since last rotation
    pub days_since_rotation: u64,
    /// Rotation policy being applied
    pub policy: RotationPolicyConfig,
    /// Reason if non-compliant
    pub reason: Option<String>,
}

/// Track compliance across all secrets
pub struct ComplianceTracker {
    backend: Arc<dyn VersionedSecretBackend>,
    records: Arc<RwLock<HashMap<String, ComplianceRecord>>>,
}

impl ComplianceTracker {
    pub fn new(backend: Arc<dyn VersionedSecretBackend>) -> Self {
        Self {
            backend,
            records: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check compliance of a specific secret
    pub async fn check_secret(&self, key: &str) -> Result<ComplianceRecord> {
        let metadata = self.backend.get_metadata(key)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let baseline = metadata.last_rotated_at.unwrap_or(metadata.created_at);
        let days_since_rotation = (now - baseline) / (24 * 60 * 60);

        let policy = &metadata.rotation_policy;
        let (status, reason) = if days_since_rotation >= policy.max_age_days as u64 {
            (
                ComplianceStatus::NonCompliant,
                Some(format!(
                    "Exceeded maximum age of {} days",
                    policy.max_age_days
                )),
            )
        } else if days_since_rotation >= policy.auto_rotate_days as u64 {
            (
                ComplianceStatus::NeedsRotation,
                Some(format!(
                    "Past auto-rotate threshold of {} days",
                    policy.auto_rotate_days
                )),
            )
        } else {
            let warning_days = policy
                .auto_rotate_days
                .saturating_sub(policy.notification_lead_days);
            if days_since_rotation >= warning_days as u64 {
                (
                    ComplianceStatus::Warning,
                    Some(format!(
                        "Approaching rotation deadline ({} days remaining)",
                        policy
                            .auto_rotate_days
                            .saturating_sub(days_since_rotation as u32)
                    )),
                )
            } else {
                (ComplianceStatus::Compliant, None)
            }
        };

        let record = ComplianceRecord {
            checked_at: now,
            secret_key: key.to_string(),
            status: status.clone(),
            days_since_rotation,
            policy: policy.clone(),
            reason: reason.clone(),
        };

        // Store record
        let mut records = self.records.write().unwrap();
        records.insert(key.to_string(), record.clone());
        drop(records);

        // Log compliance check
        log_secret_operation(
            "compliance_check",
            key,
            status == ComplianceStatus::Compliant,
            reason.as_deref(),
        );

        Ok(record)
    }

    /// Check compliance of all secrets
    pub async fn check_all(&self) -> Result<Vec<ComplianceRecord>> {
        let all_keys = self.backend.list("")?;
        let mut results = Vec::new();

        for key in all_keys {
            match self.check_secret(&key).await {
                Ok(record) => results.push(record),
                Err(e) => {
                    warn!("ComplianceTracker: Failed to check '{}': {}", key, e);
                }
            }
        }

        Ok(results)
    }

    /// Get compliance summary statistics
    pub async fn get_summary(&self) -> Result<ComplianceSummary> {
        let records = self.records.read().unwrap();

        let total = records.len();
        let compliant = records
            .values()
            .filter(|r| r.status == ComplianceStatus::Compliant)
            .count();
        let needs_rotation = records
            .values()
            .filter(|r| r.status == ComplianceStatus::NeedsRotation)
            .count();
        let non_compliant = records
            .values()
            .filter(|r| r.status == ComplianceStatus::NonCompliant)
            .count();
        let warning = records
            .values()
            .filter(|r| r.status == ComplianceStatus::Warning)
            .count();

        Ok(ComplianceSummary {
            total_secrets: total,
            compliant,
            needs_rotation,
            non_compliant,
            warning,
        })
    }

    /// Get compliance record for a specific secret
    pub async fn get_record(&self, key: &str) -> Option<ComplianceRecord> {
        let records = self.records.read().unwrap();
        records.get(key).cloned()
    }

    /// Get all compliance records
    pub async fn get_all_records(&self) -> Vec<ComplianceRecord> {
        let records = self.records.read().unwrap();
        records.values().cloned().collect()
    }
}

/// Compliance summary for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceSummary {
    pub total_secrets: usize,
    pub compliant: usize,
    pub needs_rotation: usize,
    pub non_compliant: usize,
    pub warning: usize,
}

impl ComplianceSummary {
    /// Calculate compliance percentage
    pub fn compliance_rate(&self) -> f64 {
        if self.total_secrets == 0 {
            return 100.0;
        }
        (self.compliant as f64 / self.total_secrets as f64) * 100.0
    }

    /// Check if compliance is acceptable
    pub fn is_acceptable(&self) -> bool {
        self.non_compliant == 0
    }
}

// ============================================================================
// Plugin Notification Hooks
// ============================================================================

/// Notification event type for secret lifecycle changes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecretNotificationType {
    /// Secret was rotated to new version
    Rotated,
    /// Secret is approaching rotation deadline
    RotationWarning,
    /// Secret has exceeded maximum age
    RotationOverdue,
    /// Secret was deleted
    Deleted,
    /// New secret was created
    Created,
}

/// Notification payload for plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretNotification {
    /// Type of notification
    pub notification_type: SecretNotificationType,
    /// Secret key
    pub secret_key: String,
    /// Current version ID
    pub version_id: String,
    /// Timestamp of event
    pub timestamp: u64,
    /// Additional context
    pub context: String,
    /// Rotation reason (if applicable)
    pub rotation_reason: Option<String>,
}

/// Manager for plugin notification hooks
pub struct NotificationHookManager {
    hooks: Arc<RwLock<Vec<String>>>, // Plugin names that registered hooks
}

impl NotificationHookManager {
    pub fn new() -> Self {
        Self {
            hooks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a plugin for notification hooks
    pub async fn register_plugin(&self, plugin_name: String) {
        let mut hooks = self.hooks.write().unwrap();
        if !hooks.contains(&plugin_name) {
            debug!("NotificationHook: Registered plugin '{}'", plugin_name);
            hooks.push(plugin_name);
        }
    }

    pub async fn unregister_plugin(&self, plugin_name: &str) {
        let mut hooks = self.hooks.write().unwrap();
        if let Some(pos) = hooks.iter().position(|p| p == plugin_name) {
            hooks.remove(pos);
            debug!("NotificationHook: Unregistered plugin '{}'", plugin_name);
        }
    }

    /// Check if a plugin has registered for hooks
    pub async fn is_registered(&self, plugin_name: &str) -> bool {
        let hooks = self.hooks.read().unwrap();
        hooks.contains(&plugin_name.to_string())
    }

    /// Get all registered plugins
    pub async fn get_registered(&self) -> Vec<String> {
        let hooks = self.hooks.read().unwrap();
        hooks.clone()
    }

    /// Send notification to all registered plugins
    pub async fn notify_all(&self, notification: &SecretNotification) {
        let hooks = self.hooks.read().unwrap();

        // Log notification event
        log_secret_operation(
            "notification",
            &notification.secret_key,
            true,
            Some(&format!("type={:?}", notification.notification_type)),
        );

        for plugin_name in hooks.iter() {
            debug!(
                "NotificationHook: Sending notification to plugin '{}': key={}, type={:?}",
                plugin_name, notification.secret_key, notification.notification_type
            );
        }
    }
}

impl Default for NotificationHookManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Start audit flush background task
fn start_audit_flush_task() {
    AUDIT_FLUSH_RUNNING.store(true, std::sync::atomic::Ordering::SeqCst);

    // Spawn background task in a dedicated thread with its own Tokio runtime
    // This ensures that plugin can initialize background tasks even when loaded
    // from a synchronous context (like plugin_init)
    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime for audit flush task");

        rt.block_on(async move {
            audit_flush_task().await;
        });
    });
}

// ============================================================================
// Global Plugin State
// ============================================================================

static mut SECRETS_MANAGER: Option<Arc<Mutex<SecretsManager>>> = None;
static mut ROTATION_POLICY_CONFIG: Option<Arc<RotationPolicyConfig>> = None;
static mut SECRETS_SERVICE: Option<SecretsService> = None;
static mut AUDIT_REGISTRY: Option<DefaultAuditRegistry> = None;
static PLUGIN_INFO_V2: AtomicPtr<PluginInfoV2> = AtomicPtr::new(ptr::null_mut());

// Versioned backend for rotation scheduler (optional, only initialized when using versioned storage)
static mut VERSIONED_BACKEND: Option<Arc<VersionedInMemoryBackend>> = None;

// Audit event queue for bridging sync FFI calls with async audit writes
static AUDIT_EVENT_QUEUE: Mutex<Vec<AuditEvent>> = Mutex::new(Vec::new());
static AUDIT_FLUSH_RUNNING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

// Dependencies array for skylet-abi >= 0.2.0
static mut DEPENDENCIES: [DependencyInfo; 1] = [DependencyInfo {
    name: std::ptr::null(),
    version_range: std::ptr::null(),
    required: true,
    service_type: std::ptr::null(),
}];

// ============================================================================
// Audit Logging Helpers
// ============================================================================

/// Log an audit event for secret operations
/// This function is safe to call even if audit logging is not initialized
/// Events are queued and flushed asynchronously by the audit flush task
#[allow(static_mut_refs)]
fn log_secret_operation(
    operation: &str,
    secret_path: &str,
    success: bool,
    error_message: Option<&str>,
) {
    let severity = if success {
        AuditSeverity::Info
    } else {
        AuditSeverity::Warning
    };

    let event_type = match operation {
        "get" => {
            if success {
                AuditEventType::LoadSucceeded
            } else {
                AuditEventType::LoadFailed
            }
        }
        "set" => {
            if success {
                AuditEventType::LoadSucceeded
            } else {
                AuditEventType::LoadFailed
            }
        }
        "delete" => {
            if success {
                AuditEventType::LoadSucceeded
            } else {
                AuditEventType::LoadFailed
            }
        }
        "list" => {
            if success {
                AuditEventType::LoadSucceeded
            } else {
                AuditEventType::LoadFailed
            }
        }
        _ => AuditEventType::LoadSucceeded,
    };

    let message = if let Some(err) = error_message {
        format!(
            "Secret {} operation {} on path '{}': {}",
            operation,
            if success { "succeeded" } else { "failed" },
            secret_path,
            err
        )
    } else {
        format!(
            "Secret {} operation {} on path '{}'",
            operation,
            if success { "succeeded" } else { "failed" },
            secret_path
        )
    };

    let mut event = AuditEvent::new(event_type, severity, "secrets-manager", message);
    event.metadata = format!("operation: {}, path: {}", operation, secret_path);

    // Queue the event for async processing
    if let Ok(mut queue) = AUDIT_EVENT_QUEUE.lock() {
        queue.push(event);
    }

    debug!(
        "[AUDIT] Secret {}: operation={}, path={}, success={}",
        operation, operation, secret_path, success
    );
}

/// Flush queued audit events to the backend
/// This should be called from an async context (e.g., a background task)
async fn flush_audit_events() {
    let events_to_flush = {
        if let Ok(mut queue) = AUDIT_EVENT_QUEUE.lock() {
            if queue.is_empty() {
                return;
            }
            let events: Vec<AuditEvent> = queue.drain(..).collect();
            events
        } else {
            return;
        }
    };

    unsafe {
        if let Some(registry) = AUDIT_REGISTRY.as_ref() {
            if let Some(backend) = registry.get("memory") {
                for event in events_to_flush {
                    if let Err(e) = backend.write(&event).await {
                        error!("[AUDIT ERROR] Failed to write audit event: {}", e);
                    }
                }
            }
        }
    }
}

/// Background task that periodically flushes audit events
async fn audit_flush_task() {
    use tokio::time::{interval, Duration};

    let mut interval = interval(Duration::from_secs(1));

    while AUDIT_FLUSH_RUNNING.load(std::sync::atomic::Ordering::Relaxed) {
        interval.tick().await;
        flush_audit_events().await;
    }

    // Final flush before shutting down
    flush_audit_events().await;
}

// ============================================================================
// FFI Service Functions
// ============================================================================

#[allow(static_mut_refs)]
extern "C" fn secrets_get_secret(path: *const c_char) -> SecretResult {
    if path.is_null() {
        return SecretResult {
            success: 0,
            value: std::ptr::null(),
            error_message: CString::new("Path is null").unwrap().into_raw(),
        };
    }

    let path_str = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => {
            log_secret_operation("get", "", false, Some("Invalid UTF-8 in path"));
            return SecretResult {
                success: 0,
                value: std::ptr::null(),
                error_message: CString::new("Invalid UTF-8 in path").unwrap().into_raw(),
            };
        }
    };

    unsafe {
        match SECRETS_MANAGER.as_ref() {
            Some(manager) => {
                let manager = match manager.lock() {
                    Ok(m) => m,
                    Err(e) => {
                        let error_msg = format!("Lock error: {}", e);
                        log_secret_operation("get", path_str, false, Some(&error_msg));
                        return SecretResult {
                            success: 0,
                            value: std::ptr::null(),
                            error_message: CString::new(error_msg).unwrap().into_raw(),
                        };
                    }
                };

                match manager.get_secret(path_str) {
                    Ok(secret) => {
                        log_secret_operation("get", path_str, true, None);
                        let value_cstring = CString::new(secret.to_string()).unwrap();
                        SecretResult {
                            success: 1,
                            value: value_cstring.into_raw(),
                            error_message: std::ptr::null(),
                        }
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        log_secret_operation("get", path_str, false, Some(&error_msg));
                        SecretResult {
                            success: 0,
                            value: std::ptr::null(),
                            error_message: CString::new(error_msg).unwrap().into_raw(),
                        }
                    }
                }
            }
            None => {
                log_secret_operation(
                    "get",
                    path_str,
                    false,
                    Some("SecretsManager not initialized"),
                );
                SecretResult {
                    success: 0,
                    value: std::ptr::null(),
                    error_message: CString::new("SecretsManager not initialized")
                        .unwrap()
                        .into_raw(),
                }
            }
        }
    }
}

#[allow(static_mut_refs)]
extern "C" fn secrets_set_secret(path: *const c_char, value: *const c_char) -> SecretResult {
    if path.is_null() || value.is_null() {
        return SecretResult {
            success: 0,
            value: std::ptr::null(),
            error_message: CString::new("Path or value is null").unwrap().into_raw(),
        };
    }

    let path_str = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => {
            log_secret_operation("set", "", false, Some("Invalid UTF-8 in path"));
            return SecretResult {
                success: 0,
                value: std::ptr::null(),
                error_message: CString::new("Invalid UTF-8 in path").unwrap().into_raw(),
            };
        }
    };

    let value_str = match unsafe { CStr::from_ptr(value).to_str() } {
        Ok(s) => s,
        Err(_) => {
            log_secret_operation("set", path_str, false, Some("Invalid UTF-8 in value"));
            return SecretResult {
                success: 0,
                value: std::ptr::null(),
                error_message: CString::new("Invalid UTF-8 in value").unwrap().into_raw(),
            };
        }
    };

    unsafe {
        match SECRETS_MANAGER.as_ref() {
            Some(manager) => {
                let manager = match manager.lock() {
                    Ok(m) => m,
                    Err(e) => {
                        let error_msg = format!("Lock error: {}", e);
                        log_secret_operation("set", path_str, false, Some(&error_msg));
                        return SecretResult {
                            success: 0,
                            value: std::ptr::null(),
                            error_message: CString::new(error_msg).unwrap().into_raw(),
                        };
                    }
                };

                match manager.set_secret(path_str, SecretValue::new(value_str.to_string())) {
                    Ok(_) => {
                        log_secret_operation("set", path_str, true, None);
                        SecretResult {
                            success: 1,
                            value: std::ptr::null(),
                            error_message: std::ptr::null(),
                        }
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        log_secret_operation("set", path_str, false, Some(&error_msg));
                        SecretResult {
                            success: 0,
                            value: std::ptr::null(),
                            error_message: CString::new(error_msg).unwrap().into_raw(),
                        }
                    }
                }
            }
            None => {
                log_secret_operation(
                    "set",
                    path_str,
                    false,
                    Some("SecretsManager not initialized"),
                );
                SecretResult {
                    success: 0,
                    value: std::ptr::null(),
                    error_message: CString::new("SecretsManager not initialized")
                        .unwrap()
                        .into_raw(),
                }
            }
        }
    }
}

#[allow(static_mut_refs)]
extern "C" fn secrets_delete_secret(path: *const c_char) -> SecretResult {
    if path.is_null() {
        return SecretResult {
            success: 0,
            value: std::ptr::null(),
            error_message: CString::new("Path is null").unwrap().into_raw(),
        };
    }

    let path_str = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => {
            log_secret_operation("delete", "", false, Some("Invalid UTF-8 in path"));
            return SecretResult {
                success: 0,
                value: std::ptr::null(),
                error_message: CString::new("Invalid UTF-8 in path").unwrap().into_raw(),
            };
        }
    };

    unsafe {
        match SECRETS_MANAGER.as_ref() {
            Some(manager) => {
                let manager = match manager.lock() {
                    Ok(m) => m,
                    Err(e) => {
                        let error_msg = format!("Lock error: {}", e);
                        log_secret_operation("delete", path_str, false, Some(&error_msg));
                        return SecretResult {
                            success: 0,
                            value: std::ptr::null(),
                            error_message: CString::new(error_msg).unwrap().into_raw(),
                        };
                    }
                };

                match manager.delete_secret(path_str) {
                    Ok(_) => {
                        log_secret_operation("delete", path_str, true, None);
                        SecretResult {
                            success: 1,
                            value: std::ptr::null(),
                            error_message: std::ptr::null(),
                        }
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        log_secret_operation("delete", path_str, false, Some(&error_msg));
                        SecretResult {
                            success: 0,
                            value: std::ptr::null(),
                            error_message: CString::new(error_msg).unwrap().into_raw(),
                        }
                    }
                }
            }
            None => {
                log_secret_operation(
                    "delete",
                    path_str,
                    false,
                    Some("SecretsManager not initialized"),
                );
                SecretResult {
                    success: 0,
                    value: std::ptr::null(),
                    error_message: CString::new("SecretsManager not initialized")
                        .unwrap()
                        .into_raw(),
                }
            }
        }
    }
}

#[allow(static_mut_refs)]
extern "C" fn secrets_list_secrets(prefix: *const c_char) -> SecretListResult {
    let prefix_str = if prefix.is_null() {
        ""
    } else {
        match unsafe { CStr::from_ptr(prefix).to_str() } {
            Ok(s) => s,
            Err(_) => {
                log_secret_operation("list", "", false, Some("Invalid UTF-8 in prefix"));
                return SecretListResult {
                    success: 0,
                    secrets: std::ptr::null_mut(),
                    count: 0,
                    error_message: CString::new("Invalid UTF-8 in prefix").unwrap().into_raw(),
                };
            }
        }
    };

    unsafe {
        match SECRETS_MANAGER.as_ref() {
            Some(manager) => {
                let manager = match manager.lock() {
                    Ok(m) => m,
                    Err(e) => {
                        let error_msg = format!("Lock error: {}", e);
                        log_secret_operation("list", prefix_str, false, Some(&error_msg));
                        return SecretListResult {
                            success: 0,
                            secrets: std::ptr::null_mut(),
                            count: 0,
                            error_message: CString::new(error_msg).unwrap().into_raw(),
                        };
                    }
                };

                match manager.list_secrets(prefix_str) {
                    Ok(secrets) => {
                        log_secret_operation("list", prefix_str, true, None);
                        let cstring_secrets: Vec<*const c_char> = secrets
                            .into_iter()
                            .map(|s| CString::new(s).unwrap().into_raw() as *const c_char)
                            .collect();

                        let count = cstring_secrets.len();
                        let secrets_ptr = if count > 0 {
                            let boxed = Box::new(cstring_secrets);
                            Box::into_raw(boxed) as *mut *const c_char
                        } else {
                            std::ptr::null_mut()
                        };

                        SecretListResult {
                            success: 1,
                            secrets: secrets_ptr,
                            count,
                            error_message: std::ptr::null(),
                        }
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        log_secret_operation("list", prefix_str, false, Some(&error_msg));
                        SecretListResult {
                            success: 0,
                            secrets: std::ptr::null_mut(),
                            count: 0,
                            error_message: CString::new(error_msg).unwrap().into_raw(),
                        }
                    }
                }
            }
            None => {
                log_secret_operation(
                    "list",
                    prefix_str,
                    false,
                    Some("SecretsManager not initialized"),
                );
                SecretListResult {
                    success: 0,
                    secrets: std::ptr::null_mut(),
                    count: 0,
                    error_message: CString::new("SecretsManager not initialized")
                        .unwrap()
                        .into_raw(),
                }
            }
        }
    }
}

extern "C" fn secrets_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

extern "C" fn secrets_free_list(ptr: *mut SecretListResult) {
    if ptr.is_null() {
        return;
    }

    unsafe {
        let list = std::ptr::read(ptr);
        if !list.secrets.is_null() {
            for i in 0..list.count {
                let secret_ptr = *list.secrets.add(i);
                if !secret_ptr.is_null() {
                    let _ = CString::from_raw(secret_ptr as *mut c_char);
                }
            }
            let _ = Box::from_raw(list.secrets);
        }
        if !list.error_message.is_null() {
            let _ = CString::from_raw(list.error_message as *mut c_char);
        }
        let _ = Box::from_raw(ptr);
    }
}

// ============================================================================
// Plugin ABI Functions
// ============================================================================

#[no_mangle]
#[allow(static_mut_refs)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn plugin_init(context: *const PluginContext) -> PluginResult {
    if context.is_null() {
        return PluginResult::Error;
    }

    unsafe {
        // Initialize PluginInfoV2 if not already done
        if PLUGIN_INFO_V2.load(Ordering::SeqCst).is_null() {
            // Create dependency info for skylet-abi >= 0.2.0
            let abi_name = CString::new("skylet-abi").unwrap();
            let abi_version = CString::new(">=0.2.0").unwrap();
            let abi_service_type = CString::new("security").unwrap();

            DEPENDENCIES[0] = DependencyInfo {
                name: abi_name.into_raw(),
                version_range: abi_version.into_raw(),
                required: true,
                service_type: abi_service_type.into_raw(),
            };

            // Create v2 plugin info with security category
            let name_str = CString::new("secrets-manager").unwrap();
            let version_str = CString::new(env!("CARGO_PKG_VERSION")).unwrap();
            let author_str = CString::new("Skylet").unwrap();
            let description_str =
                CString::new("Secure secrets management service with AES-256-GCM encryption")
                    .unwrap();
            let license_str = CString::new("MIT OR Apache-2.0").unwrap();
            let homepage_str = CString::new("https://github.com/vincents-ai/skylet").unwrap();
            let abi_version_str = CString::new("2.0").unwrap();
            let skynet_min_str = CString::new("0.1.0").unwrap();
            let skynet_max_str = CString::new("1.0.0").unwrap();

            // Create tags for categorization
            static TAG1: &[u8] = b"security\0";
            static TAG2: &[u8] = b"encryption\0";
            static TAG3: &[u8] = b"secrets\0";
            let tags_ptrs: [*const c_char; 3] = [
                TAG1.as_ptr() as *const c_char,
                TAG2.as_ptr() as *const c_char,
                TAG3.as_ptr() as *const c_char,
            ];

            let build_timestamp = CString::new(env!("CARGO_PKG_VERSION")).unwrap();
            let tagline_str = CString::new("Enterprise-grade encrypted secret storage").unwrap();

            let info = Box::new(PluginInfoV2 {
                // Basic metadata
                name: name_str.into_raw(),
                version: version_str.into_raw(),
                description: description_str.into_raw(),
                author: author_str.into_raw(),
                license: license_str.into_raw(),
                homepage: homepage_str.into_raw(),

                // Version compatibility
                skynet_version_min: skynet_min_str.into_raw(),
                skynet_version_max: skynet_max_str.into_raw(),
                abi_version: abi_version_str.into_raw(),

                // Dependencies (skylet-abi >= 0.2.0)
                dependencies: &DEPENDENCIES as *const DependencyInfo,
                num_dependencies: 1,
                provides_services: ptr::null(),
                num_provides_services: 0,
                requires_services: ptr::null(),
                num_requires_services: 0,

                // Capabilities (encryption, storage)
                capabilities: ptr::null(),
                num_capabilities: 0,

                // Resource requirements
                min_resources: ptr::null(),
                max_resources: ptr::null(),

                // Tags and categorization
                tags: tags_ptrs.as_ptr(),
                num_tags: 3,
                category: PluginCategory::Security, // Security category for secrets

                // Runtime capabilities
                supports_hot_reload: false,
                supports_async: true, // Supports async encryption operations
                supports_streaming: false,
                max_concurrency: 50, // Moderate concurrency for security

                // Marketplace and monetization
                monetization_model: MonetizationModel::Free,
                price_usd: 0.0,
                purchase_url: ptr::null(),
                subscription_url: ptr::null(),
                marketplace_category: ptr::null(),
                tagline: tagline_str.into_raw(),
                icon_url: ptr::null(),

                // Build and deployment information
                maturity_level: MaturityLevel::Beta,
                build_timestamp: build_timestamp.into_raw(),
                build_hash: ptr::null(),
                git_commit: ptr::null(),
                build_environment: ptr::null(),

                // Arbitrary metadata
                metadata: ptr::null(),
            });

            PLUGIN_INFO_V2.store(Box::into_raw(info), Ordering::SeqCst);
        }

        // =================================================================
        // SECURITY: Initialize with AES-256-GCM encrypted storage (CVSS 8.2)
        // =================================================================
        // Check environment variable for backend selection
        let use_encrypted = std::env::var("SKYNET_SECRETS_ENCRYPTED")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(true); // Default to encrypted for security

        // Check if versioned storage should be used
        let use_versioned = std::env::var("SKYNET_SECRETS_VERSIONED")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(true); // Default to versioned storage

        let manager = if use_encrypted {
            info!("SecretsManager: Initializing with AES-256-GCM encrypted backend");
            Arc::new(Mutex::new(SecretsManager::with_encrypted()))
        } else {
            warn!("SecretsManager: WARNING - Initializing with in-memory plaintext backend (development only)");
            Arc::new(Mutex::new(SecretsManager::with_in_memory()))
        };

        SECRETS_MANAGER = Some(manager);

        // =================================================================
        // Initialize versioned backend for rotation scheduling
        // =================================================================
        if use_versioned {
            debug!("SecretsManager: Initializing versioned secret storage with rotation support");

            // Create versioned backend with rotation policy from config
            let versioned_backend = Arc::new(VersionedInMemoryBackend::new());
            VERSIONED_BACKEND = Some(versioned_backend.clone());

            // Start the rotation scheduler background task
            start_rotation_scheduler(versioned_backend);

            info!("SecretsManager: Rotation scheduler started");
        } else {
            debug!("SecretsManager: Versioned storage disabled (SKYNET_SECRETS_VERSIONED=false)");
        }

        // =================================================================
        // Load rotation policy configuration
        // =================================================================
        match RotationPolicyConfig::load_from_env_or_default() {
            Ok(config) => {
                debug!(
                    "SecretsManager: Loaded rotation policy - interval: {} days, auto_rotate: {} days, max_age: {} days",
                    config.interval_days, config.auto_rotate_days, config.max_age_days
                );
                ROTATION_POLICY_CONFIG = Some(Arc::new(config));
            }
            Err(e) => {
                warn!("SecretsManager: Failed to load rotation policy: {}", e);
                debug!("SecretsManager: Using default rotation policy");
                ROTATION_POLICY_CONFIG = Some(Arc::new(RotationPolicyConfig::default()));
            }
        }

        match DefaultAuditRegistry::with_defaults() {
            Ok(registry) => {
                debug!("SecretsManager: Audit logging initialized with memory backend (RFC-0004)");
                AUDIT_REGISTRY = Some(registry);

                start_audit_flush_task();
                debug!("SecretsManager: Audit flush task started");
            }
            Err(e) => {
                warn!("SecretsManager: Failed to initialize audit logging: {}", e);
                warn!("SecretsManager: Continuing without audit logging");
            }
        }

        // Create and register the service
        let service = SecretsService {
            get_secret: secrets_get_secret,
            set_secret: secrets_set_secret,
            delete_secret: secrets_delete_secret,
            list_secrets: secrets_list_secrets,
            free_string: secrets_free_string,
            free_list: secrets_free_list,
        };

        SECRETS_SERVICE = Some(service);

        let registry = (*context).service_registry;

        // The bootstrap provides a null registry during initialization.
        // This is expected during the bootstrap phase. Service registration
        // will happen later when the full plugin manager is available.
        if !registry.is_null() {
            let name = CString::new("secrets-manager").unwrap();
            let service_type = CString::new("SecretsService").unwrap();

            let service_ptr = SECRETS_SERVICE.as_mut().unwrap() as *mut SecretsService;

            let result = ((*registry).register)(
                context,
                name.as_ptr(),
                service_ptr as *mut std::ffi::c_void,
                service_type.as_ptr(),
            );

            if result == PluginResult::Success {
                log_info("secrets-manager plugin initialized successfully with v2 ABI");
            }

            return result;
        }

        PluginResult::Success
    }
}

#[no_mangle]
pub extern "C" fn plugin_shutdown(_context: *const PluginContext) -> PluginResult {
    unsafe {
        // Signal the audit flush task to stop
        AUDIT_FLUSH_RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);

        // Signal the rotation scheduler to stop
        stop_rotation_scheduler();

        // Clean up v2 plugin info
        let ptr = PLUGIN_INFO_V2.swap(ptr::null_mut(), Ordering::SeqCst);
        if !ptr.is_null() {
            let _ = Box::from_raw(ptr);
        }

        // Clean up manager, config, service, versioned backend, and audit registry
        SECRETS_MANAGER = None;
        ROTATION_POLICY_CONFIG = None;
        SECRETS_SERVICE = None;
        VERSIONED_BACKEND = None;
        AUDIT_REGISTRY = None;
    }
    PluginResult::Success
}

#[no_mangle]
pub extern "C" fn plugin_get_info() -> *const PluginInfoV2 {
    PLUGIN_INFO_V2.load(Ordering::SeqCst)
}

// Logging Helper
#[allow(unused_variables)]
fn log_info(message: &str) {
    if let Ok(msg) = CString::new(message) {
        let _ = msg;
        // Log to stderr for now since we don't have context in this function
        debug!("{}", message);
    }
}

// ============================================================================
// Tests
// ============================================================================

// ============================================================================
// Rotation Manager Tests
// ============================================================================

#[test]
fn test_rotation_manager_new() {
    let backend = Arc::new(VersionedInMemoryBackend::new());
    let default_policy = RotationPolicyConfig::default();
    let manager = RotationManager::new(backend.clone(), default_policy.clone());

    assert_eq!(manager.get_default_policy(), default_policy);
}

#[test]
fn test_rotation_manager_with_backend() {
    let backend = Arc::new(VersionedInMemoryBackend::new());
    let manager = RotationManager::with_backend(backend.clone());

    assert_eq!(
        manager.get_default_policy(),
        RotationPolicyConfig::default()
    );
}

#[test]
fn test_rotation_manager_get_policy() {
    let backend = Arc::new(VersionedInMemoryBackend::new());
    let manager = RotationManager::new(backend.clone(), RotationPolicyConfig::default());

    // Set a secret
    backend
        .set("test_key", SecretValue::new("value".to_string()))
        .unwrap();

    // Get its policy
    let policy = manager.get_policy("test_key").unwrap();
    assert_eq!(policy, RotationPolicyConfig::default());
}

#[test]
fn test_rotation_manager_set_policy() {
    let backend = Arc::new(VersionedInMemoryBackend::new());
    let manager = RotationManager::new(backend.clone(), RotationPolicyConfig::default());

    // Set a secret
    backend
        .set("test_key", SecretValue::new("value".to_string()))
        .unwrap();

    // Set custom policy
    let custom_policy = RotationPolicyConfig {
        interval_days: 60,
        ..RotationPolicyConfig::default()
    };
    manager
        .set_policy("test_key", custom_policy.clone())
        .unwrap();

    // Verify policy was set
    let retrieved = manager.get_policy("test_key").unwrap();
    assert_eq!(retrieved, custom_policy);
}

#[test]
fn test_rotation_manager_remove_policy() {
    let backend = Arc::new(VersionedInMemoryBackend::new());
    let manager = RotationManager::new(backend.clone(), RotationPolicyConfig::default());

    // Set a secret with custom policy
    backend
        .set("test_key", SecretValue::new("value".to_string()))
        .unwrap();
    let custom_policy = RotationPolicyConfig {
        interval_days: 60,
        ..RotationPolicyConfig::default()
    };
    manager
        .set_policy("test_key", custom_policy.clone())
        .unwrap();

    // Remove custom policy (revert to default)
    manager.remove_policy("test_key").unwrap();

    // Verify reverted to default
    let retrieved = manager.get_policy("test_key").unwrap();
    assert_eq!(retrieved, RotationPolicyConfig::default());
}

#[test]
fn test_rotation_manager_set_default_policy() {
    let backend = Arc::new(VersionedInMemoryBackend::new());
    let mut manager = RotationManager::new(backend.clone(), RotationPolicyConfig::default());

    let new_default = RotationPolicyConfig {
        interval_days: 120,
        ..RotationPolicyConfig::default()
    };
    manager.set_default_policy(new_default.clone()).unwrap();

    assert_eq!(manager.get_default_policy(), new_default);
}

#[test]
fn test_rotation_manager_list_custom_policies() {
    let backend = Arc::new(VersionedInMemoryBackend::new());
    let manager = RotationManager::new(backend.clone(), RotationPolicyConfig::default());

    // Create secrets with different policies
    backend
        .set("default_policy_key", SecretValue::new("value1".to_string()))
        .unwrap();
    backend
        .set("custom_policy_key", SecretValue::new("value2".to_string()))
        .unwrap();

    // Set custom policy on one secret
    let custom_policy = RotationPolicyConfig {
        interval_days: 45,
        ..RotationPolicyConfig::default()
    };
    manager
        .set_policy("custom_policy_key", custom_policy.clone())
        .unwrap();

    // List custom policies
    let custom = manager.list_custom_policies().unwrap();

    assert_eq!(custom.len(), 1);
    assert!(custom.contains(&"custom_policy_key".to_string()));
}

#[test]
fn test_rotation_manager_rotate_secret() {
    let backend = Arc::new(VersionedInMemoryBackend::new());
    let manager = RotationManager::new(backend.clone(), RotationPolicyConfig::default());

    // Set a secret
    backend
        .set("test_key", SecretValue::new("value1".to_string()))
        .unwrap();

    // Rotate secret
    let version = manager
        .rotate_secret("test_key", Some("test rotation"), Some("test_agent"))
        .unwrap();

    assert_eq!(version.secret_key, "test_key");
    assert!(version.value.as_str().starts_with("rotated-"));
}

#[test]
fn test_rotation_manager_check_rotation_eligibility() {
    let backend = Arc::new(VersionedInMemoryBackend::new());
    let default_policy = RotationPolicyConfig::default();
    let manager = RotationManager::new(backend.clone(), default_policy.clone());

    // Create a secret
    backend
        .set("test_key", SecretValue::new("value".to_string()))
        .unwrap();

    // Check eligibility - should be None for new secret
    let eligibility = manager.check_rotation_eligibility("test_key").unwrap();
    assert!(eligibility.is_none());
}

#[test]
fn test_rotation_manager_get_secrets_needing_rotation() {
    let backend = Arc::new(VersionedInMemoryBackend::new());
    let default_policy = RotationPolicyConfig::default();
    let manager = RotationManager::new(backend.clone(), default_policy.clone());

    // Create secrets - one that needs rotation
    backend
        .set("fresh_key", SecretValue::new("value1".to_string()))
        .unwrap();

    let old_metadata = SecretMetadata {
        key: "old_key".to_string(),
        current_version_id: "v1".to_string(),
        created_at: 0,
        last_rotated_at: Some(1000000000), // 11574 days ago
        rotation_count: 1,
        rotation_policy: default_policy.clone(),
    };

    // Manually insert old metadata
    let mut metadata_lock = backend.metadata.write().unwrap();
    metadata_lock.insert("old_key".to_string(), old_metadata);
    drop(metadata_lock);

    backend
        .set("old_key", SecretValue::new("value2".to_string()))
        .unwrap();

    // Check needing rotation
    let needing = manager.get_secrets_needing_rotation().unwrap();
    assert!(needing.len() >= 1);
}

#[test]
fn test_rotation_manager_rotate_needing_secrets() {
    let backend = Arc::new(VersionedInMemoryBackend::new());
    let default_policy = RotationPolicyConfig::default();
    let manager = RotationManager::new(backend.clone(), default_policy.clone());

    // Set a secret
    backend
        .set("test_key", SecretValue::new("value1".to_string()))
        .unwrap();

    // Rotate all needing secrets (should work even if none meet criteria)
    // The unwrap() validates the call succeeded; count is a u32 so always >= 0
    let _count = manager.rotate_needing_secrets().unwrap();
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_value_redacted_debug() {
        let secret = SecretValue::new("super_secret_value".to_string());
        let debug_str = format!("{:?}", secret);
        assert!(debug_str.contains("REDACTED"));
        assert!(!debug_str.contains("super_secret_value"));
    }

    #[test]
    fn test_in_memory_backend_set_get() {
        let backend = InMemoryBackend::new();
        let secret = SecretValue::new("test_value".to_string());

        backend.set("test_key", secret).unwrap();
        let retrieved = backend.get("test_key").unwrap();

        assert_eq!(retrieved.as_str(), "test_value");
    }

    #[test]
    fn test_in_memory_backend_delete() {
        let backend = InMemoryBackend::new();
        let secret = SecretValue::new("test_value".to_string());

        backend.set("test_key", secret).unwrap();
        backend.delete("test_key").unwrap();

        assert!(backend.get("test_key").is_err());
    }

    #[test]
    fn test_in_memory_backend_list() {
        let backend = InMemoryBackend::new();

        backend
            .set("prefix/secret1", SecretValue::new("value1".to_string()))
            .unwrap();
        backend
            .set("prefix/secret2", SecretValue::new("value2".to_string()))
            .unwrap();
        backend
            .set("other/secret3", SecretValue::new("value3".to_string()))
            .unwrap();

        let secrets = backend.list("prefix/").unwrap();
        assert_eq!(secrets.len(), 2);
        assert!(secrets.iter().all(|s| s.starts_with("prefix/")));
    }

    #[test]
    fn test_secrets_manager() {
        let backend = Arc::new(InMemoryBackend::new());
        let manager = SecretsManager::new(backend);

        let secret = SecretValue::new("api_key_value".to_string());
        manager.set_secret("api/key", secret).unwrap();

        let retrieved = manager.get_secret("api/key").unwrap();
        assert_eq!(retrieved.as_str(), "api_key_value");
    }

    #[test]
    fn test_secret_key_validation() {
        assert!(SecretKey::new("valid_key".to_string()).is_ok());
        assert!(SecretKey::new("".to_string()).is_err());
    }

    #[test]
    fn test_rotation_policy_config_default() {
        let config = RotationPolicyConfig::default();
        assert_eq!(config.interval_days, 90);
        assert_eq!(config.auto_rotate_days, 95);
        assert_eq!(config.max_age_days, 365);
        assert_eq!(config.key_overlap_days, 7);
    }

    #[test]
    fn test_rotation_policy_config_validation() {
        let mut config = RotationPolicyConfig::default();
        assert!(config.validate().is_ok());

        // Invalid: max_age_days < auto_rotate_days
        config.max_age_days = 80;
        assert!(config.validate().is_err());

        // Fix it
        config.max_age_days = 365;
        assert!(config.validate().is_ok());

        // Invalid: auto_rotate_days < interval_days
        config.auto_rotate_days = 50;
        config.interval_days = 60;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_rotation_policy_config_from_str() {
        let toml_str = r#"
interval_days = 55
auto_rotate_days = 60
rotation_window_hours = 6
notification_lead_days = 10
max_age_days = 180
key_overlap_days = 14
        "#;

        let config = RotationPolicyConfig::parse(toml_str).unwrap();
        assert_eq!(config.interval_days, 55);
        assert_eq!(config.auto_rotate_days, 60);
        assert_eq!(config.rotation_window_hours, 6);
        assert_eq!(config.notification_lead_days, 10);
        assert_eq!(config.max_age_days, 180);
        assert_eq!(config.key_overlap_days, 14);
    }

    #[test]
    fn test_rotation_policy_config_to_toml_string() {
        let config = RotationPolicyConfig {
            interval_days: 90,
            auto_rotate_days: 85,
            rotation_window_hours: 4,
            notification_lead_days: 7,
            max_age_days: 365,
            key_overlap_days: 7,
        };

        let toml_str = config.to_toml_string().unwrap();
        assert!(toml_str.contains("interval_days = 90"));
        assert!(toml_str.contains("auto_rotate_days = 85"));
    }

    #[test]
    fn test_rotation_policy_config_roundtrip() {
        let original = RotationPolicyConfig {
            interval_days: 40,
            auto_rotate_days: 45,
            rotation_window_hours: 8,
            notification_lead_days: 5,
            max_age_days: 200,
            key_overlap_days: 10,
        };

        let toml_str = original.to_toml_string().unwrap();
        let restored = RotationPolicyConfig::parse(&toml_str).unwrap();

        assert_eq!(original.interval_days, restored.interval_days);
        assert_eq!(original.auto_rotate_days, restored.auto_rotate_days);
        assert_eq!(
            original.rotation_window_hours,
            restored.rotation_window_hours
        );
        assert_eq!(
            original.notification_lead_days,
            restored.notification_lead_days
        );
        assert_eq!(original.max_age_days, restored.max_age_days);
        assert_eq!(original.key_overlap_days, restored.key_overlap_days);
    }

    // ========================================================================
    // Audit Logging Tests
    // ========================================================================

    #[test]
    fn test_audit_registry_initialization() {
        let registry = DefaultAuditRegistry::with_defaults();
        assert!(
            registry.is_ok(),
            "Audit registry should initialize successfully"
        );

        let registry = registry.unwrap();
        assert!(
            registry.has("memory"),
            "Memory backend should be registered by default"
        );
    }

    #[test]
    fn test_audit_registry_get_backend() {
        let registry = DefaultAuditRegistry::with_defaults().unwrap();
        let backend = registry.get("memory");
        assert!(
            backend.is_some(),
            "Should be able to retrieve memory backend"
        );
    }

    #[test]
    fn test_audit_registry_list_backends() {
        let registry = DefaultAuditRegistry::with_defaults().unwrap();
        let backends = registry.list_backends();
        assert!(backends.is_ok(), "Should be able to list backends");

        let backend_list = backends.unwrap();
        assert!(
            backend_list.contains(&"memory".to_string()),
            "Memory backend should be in the list"
        );
    }

    #[test]
    fn test_secrets_manager_with_audit_initialization() {
        // This test verifies that audit registry can be initialized without errors
        let registry = DefaultAuditRegistry::with_defaults();
        assert!(
            registry.is_ok(),
            "Audit registry should initialize for secrets-manager"
        );

        // Verify we can get the backend
        let registry = registry.unwrap();
        let backend = registry.get("memory");
        assert!(
            backend.is_some(),
            "Memory audit backend should be available"
        );
    }

    #[test]
    fn test_secrets_manager_audit_registry_count() {
        let registry = DefaultAuditRegistry::with_defaults().unwrap();
        assert_eq!(
            registry.count(),
            1,
            "Newly initialized registry should have 1 backend (memory)"
        );
    }

    #[test]
    fn test_audit_event_creation() {
        use skylet_abi::audit::{AuditEvent, AuditEventType, AuditSeverity};

        let event = AuditEvent::new(
            AuditEventType::LoadSucceeded,
            AuditSeverity::Info,
            "secrets-manager",
            "Secret get operation succeeded on path 'api/key'",
        );

        assert_eq!(event.plugin_name, "secrets-manager");
        assert!(event.message.contains("Secret get operation"));
        assert_eq!(event.severity, AuditSeverity::Info);
        assert_eq!(event.event_type, AuditEventType::LoadSucceeded);
    }

    #[test]
    fn test_audit_event_with_metadata() {
        use skylet_abi::audit::{AuditEvent, AuditEventType, AuditSeverity};

        let mut event = AuditEvent::new(
            AuditEventType::LoadSucceeded,
            AuditSeverity::Info,
            "secrets-manager",
            "Secret operation completed",
        );

        event.metadata = "operation: get, path: api/key".to_string();
        assert_eq!(
            event.metadata, "operation: get, path: api/key",
            "Metadata should be set correctly"
        );
    }

    #[test]
    fn test_audit_event_filter_creation() {
        use skylet_abi::audit::AuditLogFilter;

        let filter = AuditLogFilter::new()
            .with_plugin_name("secrets-manager")
            .with_limit(100);
        assert_eq!(
            filter.limit,
            Some(100),
            "Filter should support chaining methods"
        );
        assert_eq!(
            filter.plugin_name,
            Some("secrets-manager".to_string()),
            "Filter should store plugin name"
        );
    }

    #[test]
    fn test_audit_registry_default_initialization() {
        let registry = DefaultAuditRegistry::with_defaults();
        assert!(
            registry.is_ok(),
            "DefaultAuditRegistry::with_defaults() should not fail"
        );
    }

    #[test]
    fn test_audit_backend_present_after_registry_init() {
        let registry = DefaultAuditRegistry::with_defaults().unwrap();
        let memory_backend = registry.get("memory");

        assert!(
            memory_backend.is_some(),
            "Memory backend should be registered immediately after DefaultAuditRegistry initialization"
        );
    }

    #[test]
    fn test_audit_event_severity_levels() {
        use skylet_abi::audit::AuditSeverity;

        // Test that all severity levels work correctly
        assert!(AuditSeverity::Info <= AuditSeverity::Warning);
        assert!(AuditSeverity::Warning <= AuditSeverity::Error);
        assert!(AuditSeverity::Error <= AuditSeverity::Critical);
    }

    #[test]
    fn test_audit_event_types_for_secrets() {
        use skylet_abi::audit::AuditEventType;

        // These are the event types we use for secret operations
        let _load_succeeded = AuditEventType::LoadSucceeded;
        let _load_failed = AuditEventType::LoadFailed;

        // Verify they can be created and cloned
        let event_type = AuditEventType::LoadSucceeded;
        let cloned = event_type;
        assert_eq!(event_type, cloned);
    }

    // ========================================================================
    // Versioned Secret Storage Tests
    // ========================================================================

    #[test]
    fn test_versioned_in_memory_backend_set_and_get() {
        let backend = VersionedInMemoryBackend::new();
        let secret = SecretValue::new("test_value".to_string());

        // Set a secret
        let version = backend.set("test_key", secret).unwrap();
        assert_eq!(version.secret_key, "test_key");
        assert!(version.is_active());

        // Get the secret
        let retrieved = backend.get("test_key").unwrap();
        assert_eq!(retrieved.as_str(), "test_value");
    }

    #[test]
    fn test_versioned_backend_rotate_creates_new_version() {
        let backend = VersionedInMemoryBackend::new();

        // Set initial version
        backend
            .set("test_key", SecretValue::new("value_v1".to_string()))
            .unwrap();

        // Rotate to create new version
        let v2 = backend
            .rotate(
                "test_key",
                SecretValue::new("value_v2".to_string()),
                Some("scheduled_rotation"),
                Some("admin"),
            )
            .unwrap();

        // Verify v2 is active
        assert!(v2.is_active());
        assert_eq!(v2.rotation_reason, Some("scheduled_rotation".to_string()));
        assert_eq!(v2.rotated_by, Some("admin".to_string()));

        // Get should return v2 (active)
        let retrieved = backend.get("test_key").unwrap();
        assert_eq!(retrieved.as_str(), "value_v2");

        // Get all versions - should have 2
        let all_versions = backend.get_all_versions("test_key").unwrap();
        assert_eq!(all_versions.len(), 2);

        // One should be active, one deprecated
        let active_count = all_versions.iter().filter(|v| v.is_active()).count();
        let deprecated_count = all_versions
            .iter()
            .filter(|v| v.status == SecretVersionStatus::Deprecated)
            .count();
        assert_eq!(active_count, 1);
        assert_eq!(deprecated_count, 1);
    }

    #[test]
    fn test_versioned_backend_get_version_by_id() {
        let backend = VersionedInMemoryBackend::new();

        // Set initial version
        let v1 = backend
            .set("test_key", SecretValue::new("value_v1".to_string()))
            .unwrap();

        // Rotate to create v2
        let v2 = backend
            .rotate(
                "test_key",
                SecretValue::new("value_v2".to_string()),
                None,
                None,
            )
            .unwrap();

        // Get v1 by ID
        let retrieved_v1 = backend.get_version("test_key", &v1.version_id).unwrap();
        assert_eq!(retrieved_v1.value.as_str(), "value_v1");
        assert_eq!(retrieved_v1.status, SecretVersionStatus::Deprecated);

        // Get v2 by ID
        let retrieved_v2 = backend.get_version("test_key", &v2.version_id).unwrap();
        assert_eq!(retrieved_v2.value.as_str(), "value_v2");
        assert!(retrieved_v2.is_active());
    }

    #[test]
    fn test_versioned_backend_list_secrets() {
        let backend = VersionedInMemoryBackend::new();

        backend
            .set("prefix/secret1", SecretValue::new("value1".to_string()))
            .unwrap();
        backend
            .set("prefix/secret2", SecretValue::new("value2".to_string()))
            .unwrap();
        backend
            .set("other/secret3", SecretValue::new("value3".to_string()))
            .unwrap();

        let secrets = backend.list("prefix/").unwrap();
        assert_eq!(secrets.len(), 2);
        assert!(secrets.iter().all(|s| s.starts_with("prefix/")));
    }

    #[test]
    fn test_versioned_backend_delete_secret() {
        let backend = VersionedInMemoryBackend::new();

        // Set and rotate to have multiple versions
        backend
            .set("test_key", SecretValue::new("value_v1".to_string()))
            .unwrap();
        backend
            .rotate(
                "test_key",
                SecretValue::new("value_v2".to_string()),
                None,
                None,
            )
            .unwrap();

        // Delete all versions
        backend.delete("test_key").unwrap();

        // Should no longer be accessible
        assert!(backend.get("test_key").is_err());
        assert!(backend.get_all_versions("test_key").is_err());
    }

    #[test]
    fn test_versioned_backend_delete_specific_version() {
        let backend = VersionedInMemoryBackend::new();

        // Set and rotate
        let v1 = backend
            .set("test_key", SecretValue::new("value_v1".to_string()))
            .unwrap();
        backend
            .rotate(
                "test_key",
                SecretValue::new("value_v2".to_string()),
                None,
                None,
            )
            .unwrap();

        // Delete v1 specifically
        backend.delete_version("test_key", &v1.version_id).unwrap();

        // v1 should no longer be accessible
        assert!(backend.get_version("test_key", &v1.version_id).is_err());

        // But v2 should still work
        assert!(backend.get("test_key").is_ok());
    }

    #[test]
    fn test_versioned_backend_metadata_tracking() {
        let backend = VersionedInMemoryBackend::new();

        // Set initial secret
        backend
            .set("test_key", SecretValue::new("value_v1".to_string()))
            .unwrap();

        // Get metadata
        let meta1 = backend.get_metadata("test_key").unwrap();
        assert_eq!(meta1.key, "test_key");
        assert_eq!(meta1.rotation_count, 0);
        assert!(meta1.last_rotated_at.is_none());

        // Rotate multiple times
        backend
            .rotate(
                "test_key",
                SecretValue::new("value_v2".to_string()),
                None,
                None,
            )
            .unwrap();
        backend
            .rotate(
                "test_key",
                SecretValue::new("value_v3".to_string()),
                None,
                None,
            )
            .unwrap();

        // Check metadata updated
        let meta2 = backend.get_metadata("test_key").unwrap();
        assert_eq!(meta2.rotation_count, 2);
        assert!(meta2.last_rotated_at.is_some());
    }

    #[test]
    fn test_versioned_backend_rotation_policy_update() {
        let backend = VersionedInMemoryBackend::new();

        // Set initial secret
        backend
            .set("test_key", SecretValue::new("value".to_string()))
            .unwrap();

        // Update policy (must satisfy: max_age >= auto_rotate >= interval)
        let new_policy = RotationPolicyConfig {
            interval_days: 55,
            auto_rotate_days: 60,
            rotation_window_hours: 6,
            notification_lead_days: 10,
            max_age_days: 180,
            key_overlap_days: 14,
        };

        backend
            .update_rotation_policy("test_key", new_policy)
            .unwrap();

        // Verify policy was updated
        let meta = backend.get_metadata("test_key").unwrap();
        assert_eq!(meta.rotation_policy.interval_days, 55);
        assert_eq!(meta.rotation_policy.key_overlap_days, 14);
    }

    #[test]
    fn test_versioned_backend_with_custom_policy() {
        let custom_policy = RotationPolicyConfig {
            interval_days: 25,
            auto_rotate_days: 30,
            rotation_window_hours: 2,
            notification_lead_days: 5,
            max_age_days: 90,
            key_overlap_days: 3,
        };

        let backend = VersionedInMemoryBackend::with_policy(custom_policy);

        // Set secret - should use custom policy
        backend
            .set("test_key", SecretValue::new("value".to_string()))
            .unwrap();

        let meta = backend.get_metadata("test_key").unwrap();
        assert_eq!(meta.rotation_policy.interval_days, 25);
        assert_eq!(meta.rotation_policy.key_overlap_days, 3);
    }

    #[test]
    fn test_versioned_backend_cleanup_expired_versions() {
        let backend = VersionedInMemoryBackend::new();

        // Set and rotate to create deprecated version
        backend
            .set("test_key", SecretValue::new("value_v1".to_string()))
            .unwrap();
        backend
            .rotate(
                "test_key",
                SecretValue::new("value_v2".to_string()),
                None,
                None,
            )
            .unwrap();

        // Verify we have 2 accessible versions
        let all_versions = backend.get_all_versions("test_key").unwrap();
        assert_eq!(all_versions.len(), 2);

        // Cleanup expired versions (none should be expired yet since we just created them)
        let cleaned = backend.cleanup_expired_versions().unwrap();
        assert_eq!(cleaned, 0);

        // All versions should still be accessible
        let all_versions_after = backend.get_all_versions("test_key").unwrap();
        assert_eq!(all_versions_after.len(), 2);
    }

    #[test]
    fn test_versioned_backend_purge_deleted_versions() {
        let backend = VersionedInMemoryBackend::new();

        // Set and rotate multiple times
        backend
            .set("test_key", SecretValue::new("value_v1".to_string()))
            .unwrap();
        backend
            .rotate(
                "test_key",
                SecretValue::new("value_v2".to_string()),
                None,
                None,
            )
            .unwrap();
        backend
            .rotate(
                "test_key",
                SecretValue::new("value_v3".to_string()),
                None,
                None,
            )
            .unwrap();

        // Should have 3 versions
        let all_versions = backend.get_all_versions("test_key").unwrap();
        assert_eq!(all_versions.len(), 3);

        // Delete one specific version (marks as deleted)
        let v1_id = &all_versions
            .iter()
            .find(|v| v.value.as_str() == "value_v1")
            .unwrap()
            .version_id;
        backend.delete_version("test_key", v1_id).unwrap();

        // Now purge deleted versions
        let purged = backend.purge_deleted_versions().unwrap();
        assert_eq!(purged, 1);

        // Should now have 2 accessible versions
        let remaining = backend.get_all_versions("test_key").unwrap();
        assert_eq!(remaining.len(), 2);
    }

    #[test]
    fn test_secret_version_status_display() {
        assert_eq!(SecretVersionStatus::Active.to_string(), "active");
        assert_eq!(SecretVersionStatus::Deprecated.to_string(), "deprecated");
        assert_eq!(
            SecretVersionStatus::PendingDeletion.to_string(),
            "pending_deletion"
        );
        assert_eq!(SecretVersionStatus::Deleted.to_string(), "deleted");
    }

    #[test]
    fn test_secret_version_is_accessible() {
        let mut version = SecretVersion::new(
            "test_key".to_string(),
            SecretValue::new("value".to_string()),
            None,
            None,
        );

        // Active version is accessible
        assert!(version.is_accessible());

        // Deprecated version is accessible
        version.status = SecretVersionStatus::Deprecated;
        assert!(version.is_accessible());

        // PendingDeletion is not accessible
        version.status = SecretVersionStatus::PendingDeletion;
        assert!(!version.is_accessible());

        // Deleted is not accessible
        version.status = SecretVersionStatus::Deleted;
        assert!(!version.is_accessible());
    }

    #[test]
    fn test_secret_version_has_expired_no_expiration() {
        let version = SecretVersion::new(
            "test_key".to_string(),
            SecretValue::new("value".to_string()),
            None,
            None,
        );

        // Version with no expiration has not expired
        assert!(!version.has_expired());
    }

    #[test]
    fn test_versioned_backend_multiple_secrets() {
        let backend = VersionedInMemoryBackend::new();

        // Create multiple secrets with rotations
        backend
            .set("secret1", SecretValue::new("s1_v1".to_string()))
            .unwrap();
        backend
            .rotate("secret1", SecretValue::new("s1_v2".to_string()), None, None)
            .unwrap();

        backend
            .set("secret2", SecretValue::new("s2_v1".to_string()))
            .unwrap();
        backend
            .rotate("secret2", SecretValue::new("s2_v2".to_string()), None, None)
            .unwrap();
        backend
            .rotate("secret2", SecretValue::new("s2_v3".to_string()), None, None)
            .unwrap();

        // Verify counts
        assert_eq!(backend.get_all_versions("secret1").unwrap().len(), 2);
        assert_eq!(backend.get_all_versions("secret2").unwrap().len(), 3);

        // Verify isolation
        assert_eq!(backend.get("secret1").unwrap().as_str(), "s1_v2");
        assert_eq!(backend.get("secret2").unwrap().as_str(), "s2_v3");
    }

    // ========================================================================
    // Rotation Scheduler Tests
    // ========================================================================

    #[test]
    fn test_rotation_scheduler_config_default() {
        let config = RotationSchedulerConfig::default();
        assert_eq!(config.check_interval_secs, 3600);
        assert_eq!(config.cleanup_interval_secs, 86400);
        assert!(config.auto_rotate_enabled);
        assert!(config.cleanup_enabled);
    }

    #[test]
    fn test_rotation_eligibility_warning() {
        let now = 1000000000u64;
        let policy = RotationPolicyConfig {
            interval_days: 30,
            auto_rotate_days: 60,
            rotation_window_hours: 4,
            notification_lead_days: 7,
            max_age_days: 90,
            key_overlap_days: 7,
        };

        // Create metadata that should trigger warning (within notification window)
        let metadata = SecretMetadata {
            key: "test_key".to_string(),
            current_version_id: "v1".to_string(),
            created_at: now - (54 * 24 * 60 * 60), // 54 days ago (6 days before auto_rotate)
            last_rotated_at: None,
            rotation_count: 0,
            rotation_policy: policy,
        };

        let eligibility = check_rotation_eligibility(&metadata, now);
        assert!(eligibility.is_some());

        let eligibility = eligibility.unwrap();
        assert!(!eligibility.requires_rotation());
        assert!(eligibility
            .reason()
            .contains("approaching rotation deadline"));
    }

    #[test]
    fn test_rotation_eligibility_scheduled() {
        let now = 1000000000u64;
        let policy = RotationPolicyConfig {
            interval_days: 30,
            auto_rotate_days: 60,
            rotation_window_hours: 4,
            notification_lead_days: 7,
            max_age_days: 90,
            key_overlap_days: 7,
        };

        // Create metadata that should trigger scheduled rotation (past auto_rotate)
        let metadata = SecretMetadata {
            key: "test_key".to_string(),
            current_version_id: "v1".to_string(),
            created_at: now - (65 * 24 * 60 * 60), // 65 days ago (past 60 day threshold)
            last_rotated_at: None,
            rotation_count: 0,
            rotation_policy: policy,
        };

        let eligibility = check_rotation_eligibility(&metadata, now);
        assert!(eligibility.is_some());

        let eligibility = eligibility.unwrap();
        assert!(eligibility.requires_rotation());
        assert!(eligibility.reason().contains("scheduled rotation"));
    }

    #[test]
    fn test_rotation_eligibility_forced() {
        let now = 1000000000u64;
        let policy = RotationPolicyConfig {
            interval_days: 30,
            auto_rotate_days: 60,
            rotation_window_hours: 4,
            notification_lead_days: 7,
            max_age_days: 90,
            key_overlap_days: 7,
        };

        // Create metadata that should trigger forced rotation (past max_age)
        let metadata = SecretMetadata {
            key: "test_key".to_string(),
            current_version_id: "v1".to_string(),
            created_at: now - (95 * 24 * 60 * 60), // 95 days ago (past 90 day max)
            last_rotated_at: None,
            rotation_count: 0,
            rotation_policy: policy,
        };

        let eligibility = check_rotation_eligibility(&metadata, now);
        assert!(eligibility.is_some());

        let eligibility = eligibility.unwrap();
        assert!(eligibility.requires_rotation());
        assert!(eligibility.reason().contains("forced rotation"));
    }

    #[test]
    fn test_rotation_eligibility_no_rotation_needed() {
        let now = 1000000000u64;
        let policy = RotationPolicyConfig {
            interval_days: 30,
            auto_rotate_days: 60,
            rotation_window_hours: 4,
            notification_lead_days: 7,
            max_age_days: 90,
            key_overlap_days: 7,
        };

        // Create metadata that doesn't need rotation (recently created)
        let metadata = SecretMetadata {
            key: "test_key".to_string(),
            current_version_id: "v1".to_string(),
            created_at: now - (10 * 24 * 60 * 60), // 10 days ago
            last_rotated_at: None,
            rotation_count: 0,
            rotation_policy: policy,
        };

        let eligibility = check_rotation_eligibility(&metadata, now);
        assert!(eligibility.is_none());
    }

    #[test]
    fn test_rotation_scheduler_config_from_env() {
        // Set environment variables
        std::env::set_var("SKYNET_ROTATION_CHECK_INTERVAL", "1800");
        std::env::set_var("SKYNET_ROTATION_CLEANUP_INTERVAL", "43200");
        std::env::set_var("SKYNET_ROTATION_AUTO_ENABLED", "false");
        std::env::set_var("SKYNET_ROTATION_CLEANUP_ENABLED", "false");

        let config = RotationSchedulerConfig::from_env();

        assert_eq!(config.check_interval_secs, 1800);
        assert_eq!(config.cleanup_interval_secs, 43200);
        assert!(!config.auto_rotate_enabled);
        assert!(!config.cleanup_enabled);

        // Clean up
        std::env::remove_var("SKYNET_ROTATION_CHECK_INTERVAL");
        std::env::remove_var("SKYNET_ROTATION_CLEANUP_INTERVAL");
        std::env::remove_var("SKYNET_ROTATION_AUTO_ENABLED");
        std::env::remove_var("SKYNET_ROTATION_CLEANUP_ENABLED");
    }
}
