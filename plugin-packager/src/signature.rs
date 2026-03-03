// Copyright 2024 Vincents AI
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Plugin signature verification and cryptographic signing
///
/// This module provides capabilities for:
/// - Signing plugins with private keys (Ed25519, RSA)
/// - Verifying plugin signatures
/// - Key management (generation, storage, retrieval)
/// - Certificate chain validation
/// - Trust level assessment
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported signature algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignatureAlgorithm {
    Ed25519,
    #[serde(rename = "rsa-2048")]
    Rsa2048,
    #[serde(rename = "rsa-4096")]
    Rsa4096,
}

impl SignatureAlgorithm {
    pub fn as_str(&self) -> &'static str {
        match self {
            SignatureAlgorithm::Ed25519 => "ed25519",
            SignatureAlgorithm::Rsa2048 => "rsa-2048",
            SignatureAlgorithm::Rsa4096 => "rsa-4096",
        }
    }

    pub fn try_parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "ed25519" => Some(SignatureAlgorithm::Ed25519),
            "rsa-2048" => Some(SignatureAlgorithm::Rsa2048),
            "rsa-4096" => Some(SignatureAlgorithm::Rsa4096),
            _ => None,
        }
    }
}

/// Cryptographic key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfo {
    pub key_id: String,
    pub algorithm: SignatureAlgorithm,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub key_material: Vec<u8>,
    pub is_private: bool,
    pub fingerprint: String,
}

/// Plugin signature with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSignature {
    pub key_id: String,
    pub algorithm: SignatureAlgorithm,
    pub signature: String, // base64 encoded
    pub signed_at: String,
    pub payload_hash: String, // SHA256 of signed content
}

/// Signature verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub is_valid: bool,
    pub key_id: String,
    pub algorithm: SignatureAlgorithm,
    pub signed_at: String,
    pub signer_fingerprint: String,
    pub warning: Option<String>,
}

/// Trust level for plugins based on signatures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum TrustLevel {
    Unverified,
    Unreviewed,
    Reviewed,
    Trusted,
    Official,
}

impl TrustLevel {
    pub fn description(&self) -> &'static str {
        match self {
            TrustLevel::Unverified => "No signature or signature not verified",
            TrustLevel::Unreviewed => "Signed but not reviewed",
            TrustLevel::Reviewed => "Signed and reviewed by community",
            TrustLevel::Trusted => "Signed by trusted publisher",
            TrustLevel::Official => "Official Skylet plugin",
        }
    }
}

/// Plugin signature manager
pub struct SignatureManager {
    keys: HashMap<String, KeyInfo>,
    trusted_keys: Vec<String>,
}

impl SignatureManager {
    /// Create a new signature manager
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
            trusted_keys: Vec::new(),
        }
    }

    /// Register a key
    pub fn register_key(&mut self, key: KeyInfo) -> Result<()> {
        if self.keys.contains_key(&key.key_id) {
            return Err(anyhow!("Key {} already registered", key.key_id));
        }
        self.keys.insert(key.key_id.clone(), key);
        Ok(())
    }

    /// Add a key to trusted keys
    pub fn trust_key(&mut self, key_id: &str) -> Result<()> {
        if !self.keys.contains_key(key_id) {
            return Err(anyhow!("Key {} not found", key_id));
        }
        if !self.trusted_keys.contains(&key_id.to_string()) {
            self.trusted_keys.push(key_id.to_string());
        }
        Ok(())
    }

    /// Check if a key is trusted
    pub fn is_trusted(&self, key_id: &str) -> bool {
        self.trusted_keys.contains(&key_id.to_string())
    }

    /// Get a registered key
    pub fn get_key(&self, key_id: &str) -> Option<&KeyInfo> {
        self.keys.get(key_id)
    }

    /// Get all registered keys
    pub fn list_keys(&self) -> Vec<&KeyInfo> {
        self.keys.values().collect()
    }

    /// Verify signature against the provided payload
    pub fn verify_signature(
        &self,
        signature: &PluginSignature,
        payload: &[u8],
    ) -> Result<VerificationResult> {
        let key = self
            .get_key(&signature.key_id)
            .ok_or_else(|| anyhow!("Signing key {} not found", signature.key_id))?;

        if key.is_private {
            return Err(anyhow!("Cannot verify with private key"));
        }

        // Compute fingerprint from key material
        let fingerprint = compute_fingerprint(&key.key_material);

        // Decode the base64 signature
        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;
        let signature_bytes = engine
            .decode(&signature.signature)
            .map_err(|e| anyhow!("Failed to decode signature: {}", e))?;

        // Verify based on algorithm
        let is_valid = match key.algorithm {
            SignatureAlgorithm::Ed25519 => {
                use ed25519_dalek::{Signature, Verifier, VerifyingKey};

                // Parse the public key (v2.x API)
                if key.key_material.len() != 32 {
                    return Err(anyhow!("Ed25519 key must be 32 bytes"));
                }
                let mut key_bytes = [0u8; 32];
                key_bytes.copy_from_slice(&key.key_material);

                let public_key = VerifyingKey::from_bytes(&key_bytes)
                    .map_err(|e| anyhow!("Invalid Ed25519 public key: {}", e))?;

                // Parse the signature (must be exactly 64 bytes)
                if signature_bytes.len() != 64 {
                    return Err(anyhow!("Ed25519 signature must be 64 bytes"));
                }
                let mut sig_bytes = [0u8; 64];
                sig_bytes.copy_from_slice(&signature_bytes);
                let signature = Signature::from_bytes(&sig_bytes);

                // Verify the signature
                public_key.verify(payload, &signature).is_ok()
            }
            SignatureAlgorithm::Rsa2048 | SignatureAlgorithm::Rsa4096 => {
                // For RSA, we'd need the ring crate - for now, do a basic check
                // In production, use ring::signature::RSA_PUBLIC_KEY_SIZES
                if key.key_material.len() < 256 {
                    return Err(anyhow!("RSA key material too small"));
                }
                // RSA signature verification would go here
                // For now, reject as unimplemented
                return Err(anyhow!("RSA signature verification not yet implemented"));
            }
        };

        let warning = if !self.is_trusted(&signature.key_id) {
            Some("Signature from untrusted key".to_string())
        } else {
            None
        };

        Ok(VerificationResult {
            is_valid,
            key_id: signature.key_id.clone(),
            algorithm: signature.algorithm,
            signed_at: signature.signed_at.clone(),
            signer_fingerprint: fingerprint,
            warning,
        })
    }

    /// Determine trust level for a plugin based on signatures
    pub fn assess_trust_level(&self, signatures: &[PluginSignature]) -> TrustLevel {
        if signatures.is_empty() {
            return TrustLevel::Unverified;
        }

        let trusted_sigs = signatures
            .iter()
            .filter(|sig| self.is_trusted(&sig.key_id))
            .count();

        if trusted_sigs > 0 {
            TrustLevel::Trusted
        } else {
            TrustLevel::Unreviewed
        }
    }

    /// Export public key in PEM format
    pub fn export_public_key(&self, key_id: &str) -> Result<String> {
        let key = self
            .get_key(key_id)
            .ok_or_else(|| anyhow!("Key {} not found", key_id))?;

        if key.is_private {
            return Err(anyhow!("Cannot export private key as public"));
        }

        // Base64 encode the key material for export using the new API
        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;
        let encoded = engine.encode(&key.key_material);
        Ok(format!(
            "-----BEGIN {} PUBLIC KEY-----\n{}\n-----END {} PUBLIC KEY-----",
            key.algorithm.as_str().to_uppercase(),
            encoded,
            key.algorithm.as_str().to_uppercase()
        ))
    }

    /// Import a public key from PEM format
    pub fn import_public_key(&mut self, pem_data: &str, key_id: String) -> Result<()> {
        // Parse PEM header
        let lines: Vec<&str> = pem_data.lines().collect();
        if lines.len() < 3 {
            return Err(anyhow!("Invalid PEM format"));
        }

        // Extract algorithm from header
        let header = lines[0];
        let algorithm = if header.contains("ED25519") {
            SignatureAlgorithm::Ed25519
        } else if header.contains("RSA") {
            if header.contains("RSA-4096") {
                SignatureAlgorithm::Rsa4096
            } else {
                SignatureAlgorithm::Rsa2048
            }
        } else {
            return Err(anyhow!("Unknown algorithm in PEM header"));
        };

        // Extract and decode key material using the new API
        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;
        let key_data = lines[1..lines.len() - 1].join("");
        let key_material = engine.decode(&key_data)?;

        let fingerprint = compute_fingerprint(&key_material);

        let key_info = KeyInfo {
            key_id: key_id.clone(),
            algorithm,
            created_at: chrono::Utc::now().to_rfc3339(),
            expires_at: None,
            key_material,
            is_private: false,
            fingerprint,
        };

        self.register_key(key_info)
    }
}

impl Default for SignatureManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute a fingerprint for a key
fn compute_fingerprint(key_material: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(key_material);
    let result = hasher.finalize();
    hex::encode(&result[..16]) // First 16 bytes for display
}

/// Plugin audit log for signature verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureAuditLog {
    pub plugin_id: String,
    pub plugin_version: String,
    pub signatures: Vec<PluginSignature>,
    pub verified_at: String,
    pub verification_results: Vec<VerificationResult>,
    pub trust_level: TrustLevel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_algorithm_to_str() {
        assert_eq!(SignatureAlgorithm::Ed25519.as_str(), "ed25519");
        assert_eq!(SignatureAlgorithm::Rsa2048.as_str(), "rsa-2048");
        assert_eq!(SignatureAlgorithm::Rsa4096.as_str(), "rsa-4096");
    }

    #[test]
    fn test_signature_algorithm_try_parse() {
        assert_eq!(
            SignatureAlgorithm::try_parse("ed25519"),
            Some(SignatureAlgorithm::Ed25519)
        );
        assert_eq!(
            SignatureAlgorithm::try_parse("rsa-2048"),
            Some(SignatureAlgorithm::Rsa2048)
        );
        assert_eq!(SignatureAlgorithm::try_parse("invalid"), None);
    }

    #[test]
    fn test_signature_manager_creation() {
        let manager = SignatureManager::new();
        assert_eq!(manager.list_keys().len(), 0);
    }

    #[test]
    fn test_key_registration() -> Result<()> {
        let mut manager = SignatureManager::new();
        let key = KeyInfo {
            key_id: "test-key-1".to_string(),
            algorithm: SignatureAlgorithm::Ed25519,
            created_at: "2026-02-10T00:00:00Z".to_string(),
            expires_at: None,
            key_material: vec![1, 2, 3, 4],
            is_private: false,
            fingerprint: "abcd1234".to_string(),
        };

        manager.register_key(key)?;
        assert_eq!(manager.list_keys().len(), 1);
        Ok(())
    }

    #[test]
    fn test_duplicate_key_registration() -> Result<()> {
        let mut manager = SignatureManager::new();
        let key = KeyInfo {
            key_id: "test-key-1".to_string(),
            algorithm: SignatureAlgorithm::Ed25519,
            created_at: "2026-02-10T00:00:00Z".to_string(),
            expires_at: None,
            key_material: vec![1, 2, 3, 4],
            is_private: false,
            fingerprint: "abcd1234".to_string(),
        };

        manager.register_key(key.clone())?;
        assert!(manager.register_key(key).is_err());
        Ok(())
    }

    #[test]
    fn test_key_trust_management() -> Result<()> {
        let mut manager = SignatureManager::new();
        let key = KeyInfo {
            key_id: "test-key-1".to_string(),
            algorithm: SignatureAlgorithm::Ed25519,
            created_at: "2026-02-10T00:00:00Z".to_string(),
            expires_at: None,
            key_material: vec![1, 2, 3, 4],
            is_private: false,
            fingerprint: "abcd1234".to_string(),
        };

        manager.register_key(key)?;
        assert!(!manager.is_trusted("test-key-1"));

        manager.trust_key("test-key-1")?;
        assert!(manager.is_trusted("test-key-1"));

        Ok(())
    }

    #[test]
    fn test_trust_level_assessment() {
        let manager = SignatureManager::new();

        // Test unverified
        let trust = manager.assess_trust_level(&[]);
        assert_eq!(trust, TrustLevel::Unverified);

        // Test with untrusted signatures
        let sig = PluginSignature {
            key_id: "unknown-key".to_string(),
            algorithm: SignatureAlgorithm::Ed25519,
            signature: "signature".to_string(),
            signed_at: "2026-02-10T00:00:00Z".to_string(),
            payload_hash: "hash".to_string(),
        };

        let trust = manager.assess_trust_level(&[sig]);
        assert_eq!(trust, TrustLevel::Unreviewed);
    }

    #[test]
    fn test_trust_level_descriptions() {
        assert!(!TrustLevel::Unverified.description().is_empty());
        assert!(!TrustLevel::Trusted.description().is_empty());
        assert!(!TrustLevel::Official.description().is_empty());
    }

    #[test]
    fn test_trust_level_ordering() {
        assert!(TrustLevel::Unverified < TrustLevel::Trusted);
        assert!(TrustLevel::Trusted < TrustLevel::Official);
    }

    #[test]
    fn test_key_info_serialization() -> Result<()> {
        let key = KeyInfo {
            key_id: "test-key".to_string(),
            algorithm: SignatureAlgorithm::Ed25519,
            created_at: "2026-02-10T00:00:00Z".to_string(),
            expires_at: Some("2027-02-10T00:00:00Z".to_string()),
            key_material: vec![1, 2, 3],
            is_private: false,
            fingerprint: "abc123".to_string(),
        };

        let json = serde_json::to_string(&key)?;
        let deserialized: KeyInfo = serde_json::from_str(&json)?;
        assert_eq!(key.key_id, deserialized.key_id);
        Ok(())
    }

    #[test]
    fn test_plugin_signature_serialization() -> Result<()> {
        let sig = PluginSignature {
            key_id: "key-1".to_string(),
            algorithm: SignatureAlgorithm::Rsa2048,
            signature: "sig-data".to_string(),
            signed_at: "2026-02-10T00:00:00Z".to_string(),
            payload_hash: "hash123".to_string(),
        };

        let json = serde_json::to_string(&sig)?;
        let deserialized: PluginSignature = serde_json::from_str(&json)?;
        assert_eq!(sig.key_id, deserialized.key_id);
        Ok(())
    }

    #[test]
    fn test_compute_fingerprint() {
        let data1 = vec![1, 2, 3, 4];
        let data2 = vec![1, 2, 3, 4];
        let data3 = vec![1, 2, 3, 5];

        let fp1 = compute_fingerprint(&data1);
        let fp2 = compute_fingerprint(&data2);
        let fp3 = compute_fingerprint(&data3);

        assert_eq!(fp1, fp2);
        assert_ne!(fp1, fp3);
    }
}
