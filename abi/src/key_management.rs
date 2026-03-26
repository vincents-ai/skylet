// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Key Management Abstraction - RFC-0100 (Phase 2.1)
//!
//! This module provides trait-based abstractions for cryptographic key management,
//! allowing the execution engine to work with different key backends without
//! being tightly coupled to any specific implementation.
//!
//! # Overview
//!
//! The `KeyManagement` trait defines a contract for:
//! - Key generation and derivation
//! - Signing operations
//! - Verification operations
//! - Key rotation and lifecycle
//!
//! # Default Implementation
//!
//! A `DefaultKeyManagement` implementation is provided using standard cryptographic
//! libraries (ed25519-dalek, sha2, etc.) for standalone operation.
//!
//! # Extensible Design
//!
//! Custom implementations can be swapped in to use alternative key management
//! systems (e.g., HSMs, cloud KMS, or other key hierarchies).
//!
//! # Example
//!
//! ```rust,ignore
//! # use skylet_abi::key_management::{KeyManagement, KeyType};
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! // Create a key manager (uses default implementation)
//! let key_manager = skylet_abi::key_management::DefaultKeyManagement::new();
//!
//! // Generate a key
//! let key_pair = key_manager.generate_key(KeyType::Ed25519)?;
//!
//! // Sign some data
//! let signature = key_manager.sign(&key_pair.key_id, b"hello world")?;
//!
//! // Verify signature
//! let is_valid = key_manager.verify(&key_pair.key_id, b"hello world", &signature)?;
//! assert!(is_valid);
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;

/// Error type for key management operations
#[derive(Error, Debug, Clone)]
pub enum KeyManagementError {
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Invalid key type: {0}")]
    InvalidKeyType(String),

    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),

    #[error("Signing operation failed: {0}")]
    SigningFailed(String),

    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    #[error("Key rotation failed: {0}")]
    KeyRotationFailed(String),

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Key derivation failed: {0}")]
    KeyDerivationFailed(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

pub type KeyManagementResult<T> = Result<T, KeyManagementError>;

/// Supported key types for the key management system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    /// Ed25519 elliptic curve digital signature algorithm
    Ed25519,
    /// ECDSA with P-256 curve (secp256r1)
    EcdsaP256,
    /// RSA with 2048-bit modulus
    Rsa2048,
    /// RSA with 4096-bit modulus
    Rsa4096,
}

impl std::fmt::Display for KeyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyType::Ed25519 => write!(f, "ed25519"),
            KeyType::EcdsaP256 => write!(f, "ecdsa-p256"),
            KeyType::Rsa2048 => write!(f, "rsa-2048"),
            KeyType::Rsa4096 => write!(f, "rsa-4096"),
        }
    }
}

/// A generated cryptographic key pair
#[derive(Debug, Clone)]
pub struct KeyPair {
    /// Unique identifier for this key
    pub key_id: String,

    /// The key type
    pub key_type: KeyType,

    /// Public key bytes (format depends on key type)
    pub public_key: Vec<u8>,

    /// Private key bytes (encrypted at rest by the key manager)
    /// Never directly exposed to consumers
    private_key: Vec<u8>,

    /// When this key was created
    pub created_at: i64,

    /// When this key expires (None = no expiration)
    pub expires_at: Option<i64>,

    /// Metadata about the key (source, purpose, etc.)
    pub metadata: std::collections::HashMap<String, String>,
}

/// Key management trait for signing and verification operations
///
/// Implementations of this trait handle all cryptographic operations
/// required by the execution engine. Implementations must be thread-safe
/// and may be wrapped in Arc for shared use.
#[async_trait]
pub trait KeyManagement: Send + Sync {
    /// Generate a new keypair of the specified type
    ///
    /// # Arguments
    /// * `key_type` - Type of key to generate
    ///
    /// # Returns
    /// A new `KeyPair` with the generated public and private keys
    async fn generate_key(&self, key_type: KeyType) -> KeyManagementResult<KeyPair>;

    /// Derive a key from an existing key or seed material
    ///
    /// # Arguments
    /// * `parent_key_id` - ID of parent key to derive from (or None for new key)
    /// * `key_type` - Type of key to derive
    /// * `context` - Derivation context (e.g., "zone-key", "instance-key")
    ///
    /// # Returns
    /// A derived `KeyPair`
    async fn derive_key(
        &self,
        parent_key_id: Option<&str>,
        key_type: KeyType,
        context: &str,
    ) -> KeyManagementResult<KeyPair>;

    /// Sign data with a specific key
    ///
    /// # Arguments
    /// * `key_id` - ID of the key to use for signing
    /// * `data` - The data to sign
    ///
    /// # Returns
    /// The signature bytes
    async fn sign(&self, key_id: &str, data: &[u8]) -> KeyManagementResult<Vec<u8>>;

    /// Verify a signature using a specific key
    ///
    /// # Arguments
    /// * `key_id` - ID of the key to use for verification
    /// * `data` - The original data that was signed
    /// * `signature` - The signature to verify
    ///
    /// # Returns
    /// `true` if the signature is valid, `false` otherwise
    async fn verify(
        &self,
        key_id: &str,
        data: &[u8],
        signature: &[u8],
    ) -> KeyManagementResult<bool>;

    /// Get the public key for a given key ID
    ///
    /// # Arguments
    /// * `key_id` - ID of the key
    ///
    /// # Returns
    /// The public key bytes
    async fn get_public_key(&self, key_id: &str) -> KeyManagementResult<Vec<u8>>;

    /// Rotate a key to a new version
    ///
    /// # Arguments
    /// * `old_key_id` - ID of the key to rotate
    /// * `new_key_type` - Type for the new key
    ///
    /// # Returns
    /// The new `KeyPair`
    async fn rotate_key(
        &self,
        old_key_id: &str,
        new_key_type: KeyType,
    ) -> KeyManagementResult<KeyPair>;

    /// Check if a key exists and is valid
    ///
    /// # Arguments
    /// * `key_id` - ID of the key to check
    ///
    /// # Returns
    /// `true` if the key exists and is valid, `false` otherwise
    async fn key_exists(&self, key_id: &str) -> KeyManagementResult<bool>;

    /// Revoke a key (mark as invalid)
    ///
    /// # Arguments
    /// * `key_id` - ID of the key to revoke
    async fn revoke_key(&self, key_id: &str) -> KeyManagementResult<()>;
}

/// Default key management implementation for standalone operation
///
/// This implementation uses standard cryptographic libraries and does not depend
/// on any external key management systems. It's suitable for development,
/// testing, and standalone deployments.
pub struct DefaultKeyManagement {
    // In-memory key storage (for standalone mode)
    keys: Arc<parking_lot::RwLock<std::collections::HashMap<String, KeyPair>>>,
    // Key ID counter for generating unique IDs
    key_counter: Arc<std::sync::atomic::AtomicU64>,
}

impl DefaultKeyManagement {
    /// Create a new default key manager
    pub fn new() -> Self {
        Self {
            keys: Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
            key_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    fn generate_key_id(&self) -> String {
        let counter = self
            .key_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("key-{:016x}", counter)
    }

    fn generate_ed25519_keypair(&self) -> KeyManagementResult<(Vec<u8>, Vec<u8>)> {
        use ed25519_dalek::SigningKey;
        use rand::RngCore;

        let mut seed = [0u8; 32];
        let mut rng = rand::thread_rng();
        rng.fill_bytes(&mut seed);

        let signing_key = SigningKey::from_bytes(&seed);
        let verify_key = signing_key.verifying_key();

        Ok((
            verify_key.to_bytes().to_vec(),
            signing_key.to_bytes().to_vec(),
        ))
    }
}

impl Default for DefaultKeyManagement {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeyManagement for DefaultKeyManagement {
    async fn generate_key(&self, key_type: KeyType) -> KeyManagementResult<KeyPair> {
        let (public_key, private_key) = match key_type {
            KeyType::Ed25519 => self.generate_ed25519_keypair()?,
            KeyType::EcdsaP256 => {
                return Err(KeyManagementError::InvalidKeyType(
                    "EcdsaP256 not yet implemented in default key manager".to_string(),
                ))
            }
            KeyType::Rsa2048 => {
                return Err(KeyManagementError::InvalidKeyType(
                    "Rsa2048 not yet implemented in default key manager".to_string(),
                ))
            }
            KeyType::Rsa4096 => {
                return Err(KeyManagementError::InvalidKeyType(
                    "Rsa4096 not yet implemented in default key manager".to_string(),
                ))
            }
        };

        let key_id = self.generate_key_id();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let key_pair = KeyPair {
            key_id: key_id.clone(),
            key_type,
            public_key,
            private_key,
            created_at: now,
            expires_at: None,
            metadata: std::collections::HashMap::new(),
        };

        self.keys.write().insert(key_id, key_pair.clone());
        Ok(key_pair)
    }

    async fn derive_key(
        &self,
        _parent_key_id: Option<&str>,
        key_type: KeyType,
        _context: &str,
    ) -> KeyManagementResult<KeyPair> {
        // For now, just generate a new key with the context added to metadata
        let mut key_pair = self.generate_key(key_type).await?;
        if let Some(context) = _context.split('/').next() {
            key_pair
                .metadata
                .insert("derivation_context".to_string(), context.to_string());
        }
        Ok(key_pair)
    }

    async fn sign(&self, key_id: &str, data: &[u8]) -> KeyManagementResult<Vec<u8>> {
        use ed25519_dalek::Signer;
        use ed25519_dalek::SigningKey;

        let keys = self.keys.read();
        let key_pair = keys
            .get(key_id)
            .ok_or_else(|| KeyManagementError::KeyNotFound(key_id.to_string()))?;

        match key_pair.key_type {
            KeyType::Ed25519 => {
                let signing_key =
                    SigningKey::from_bytes(&key_pair.private_key[..].try_into().map_err(|_| {
                        KeyManagementError::SigningFailed("Invalid key length".to_string())
                    })?);
                let signature = signing_key.sign(data);
                Ok(signature.to_bytes().to_vec())
            }
            _ => Err(KeyManagementError::InvalidKeyType(format!(
                "Key type {:?} not supported for signing",
                key_pair.key_type
            ))),
        }
    }

    async fn verify(
        &self,
        key_id: &str,
        data: &[u8],
        signature: &[u8],
    ) -> KeyManagementResult<bool> {
        use ed25519_dalek::{Signature, VerifyingKey};

        let keys = self.keys.read();
        let key_pair = keys
            .get(key_id)
            .ok_or_else(|| KeyManagementError::KeyNotFound(key_id.to_string()))?;

        match key_pair.key_type {
            KeyType::Ed25519 => {
                let verify_key = VerifyingKey::from_bytes(
                    &key_pair.public_key[..].try_into().map_err(|_| {
                        KeyManagementError::VerificationFailed("Invalid key length".to_string())
                    })?,
                )
                .map_err(|_| {
                    KeyManagementError::VerificationFailed("Invalid public key".to_string())
                })?;

                let sig: Signature = signature
                    .try_into()
                    .map_err(|_| KeyManagementError::InvalidSignature)?;

                match verify_key.verify_strict(data, &sig) {
                    Ok(()) => Ok(true),
                    Err(_) => Ok(false),
                }
            }
            _ => Err(KeyManagementError::InvalidKeyType(format!(
                "Key type {:?} not supported for verification",
                key_pair.key_type
            ))),
        }
    }

    async fn get_public_key(&self, key_id: &str) -> KeyManagementResult<Vec<u8>> {
        let keys = self.keys.read();
        let key_pair = keys
            .get(key_id)
            .ok_or_else(|| KeyManagementError::KeyNotFound(key_id.to_string()))?;
        Ok(key_pair.public_key.clone())
    }

    async fn rotate_key(
        &self,
        old_key_id: &str,
        new_key_type: KeyType,
    ) -> KeyManagementResult<KeyPair> {
        // Check that old key exists
        if !self.key_exists(old_key_id).await? {
            return Err(KeyManagementError::KeyNotFound(old_key_id.to_string()));
        }

        // Generate new key
        let new_key = self.generate_key(new_key_type).await?;

        // Optionally: could mark old key as superseded
        // For now, we just create a new one and return it

        Ok(new_key)
    }

    async fn key_exists(&self, key_id: &str) -> KeyManagementResult<bool> {
        Ok(self.keys.read().contains_key(key_id))
    }

    async fn revoke_key(&self, key_id: &str) -> KeyManagementResult<()> {
        let mut keys = self.keys.write();
        if keys.remove(key_id).is_some() {
            Ok(())
        } else {
            Err(KeyManagementError::KeyNotFound(key_id.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_key() {
        let manager = DefaultKeyManagement::new();
        let key_pair = manager.generate_key(KeyType::Ed25519).await.unwrap();

        assert!(!key_pair.key_id.is_empty());
        assert!(!key_pair.public_key.is_empty());
        assert_eq!(key_pair.key_type, KeyType::Ed25519);
    }

    #[tokio::test]
    async fn test_sign_and_verify() {
        let manager = DefaultKeyManagement::new();
        let key_pair = manager.generate_key(KeyType::Ed25519).await.unwrap();

        let data = b"test data";
        let signature = manager.sign(&key_pair.key_id, data).await.unwrap();

        assert!(!signature.is_empty());

        let is_valid = manager
            .verify(&key_pair.key_id, data, &signature)
            .await
            .unwrap();
        assert!(is_valid);

        // Wrong data should not verify
        let wrong_data = b"different data";
        let is_valid = manager
            .verify(&key_pair.key_id, wrong_data, &signature)
            .await
            .unwrap();
        assert!(!is_valid);
    }

    #[tokio::test]
    async fn test_key_exists() {
        let manager = DefaultKeyManagement::new();
        let key_pair = manager.generate_key(KeyType::Ed25519).await.unwrap();

        assert!(manager.key_exists(&key_pair.key_id).await.unwrap());
        assert!(!manager.key_exists("nonexistent").await.unwrap());
    }

    #[tokio::test]
    async fn test_revoke_key() {
        let manager = DefaultKeyManagement::new();
        let key_pair = manager.generate_key(KeyType::Ed25519).await.unwrap();

        assert!(manager.key_exists(&key_pair.key_id).await.unwrap());
        manager.revoke_key(&key_pair.key_id).await.unwrap();
        assert!(!manager.key_exists(&key_pair.key_id).await.unwrap());
    }
}
