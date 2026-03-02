use serde::{Deserialize, Serialize};
use sha2::Digest;
use signature::{Signer, Verifier};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub algorithm: String,
    pub public_key_id: String,
    pub signature: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifest {
    pub plugin_id: String,
    pub version: String,
    pub checksum: String,
    pub signature: Signature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPair {
    pub public_key_id: String,
    pub public_key: String,
    #[serde(skip)]
    signing_key: Option<ed25519_dalek::SigningKey>,
}

impl KeyPair {
    pub fn from_secret(public_key_id: String, secret_hex: &str) -> Result<Self, String> {
        let secret_bytes =
            hex::decode(secret_hex).map_err(|e| format!("Invalid secret hex: {}", e))?;

        if secret_bytes.len() != 32 {
            return Err("Invalid secret key length".to_string());
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&secret_bytes);

        let signing_key = ed25519_dalek::SigningKey::from_bytes(&key_bytes);
        let verifying_key: ed25519_dalek::VerifyingKey = (&signing_key).into();

        Ok(Self {
            public_key_id,
            public_key: hex::encode(verifying_key.to_bytes()),
            signing_key: Some(signing_key),
        })
    }

    pub fn generate() -> Self {
        use rand::rngs::OsRng;
        let signing_key = ed25519_dalek::SigningKey::generate(&mut OsRng);
        let verifying_key: ed25519_dalek::VerifyingKey = (&signing_key).into();

        Self {
            public_key_id: "default".to_string(),
            public_key: hex::encode(verifying_key.to_bytes()),
            signing_key: Some(signing_key),
        }
    }

    pub fn public_key_only(&self) -> KeyPair {
        KeyPair {
            public_key_id: self.public_key_id.clone(),
            public_key: self.public_key.clone(),
            signing_key: None,
        }
    }
}

pub struct PluginSigner;

impl PluginSigner {
    pub fn sign_plugin(plugin_path: &Path, key_pair: &KeyPair) -> Result<SignedManifest, String> {
        let manifest_path = plugin_path.join("manifest.json");
        let content = fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Failed to read manifest: {}", e))?;

        let checksum = Self::calculate_checksum(plugin_path)?;

        let manifest: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| format!("Invalid manifest JSON: {}", e))?;

        let plugin_id = manifest["id"]
            .as_str()
            .ok_or("Missing plugin ID in manifest")?
            .to_string();
        let version = manifest["version"]
            .as_str()
            .ok_or("Missing version in manifest")?
            .to_string();

        let signature_data = format!("{}:{}:{}", plugin_id, version, checksum);
        let signature = Self::sign_data(&signature_data, key_pair)?;

        Ok(SignedManifest {
            plugin_id,
            version,
            checksum,
            signature,
        })
    }

    pub fn verify_plugin(plugin_path: &Path, trusted_keys: &[KeyPair]) -> Result<bool, String> {
        let signature_path = plugin_path.join(".signature");
        if !signature_path.exists() {
            return Ok(false);
        }

        let signature_content = fs::read_to_string(&signature_path)
            .map_err(|e| format!("Failed to read signature: {}", e))?;

        let signed_manifest: SignedManifest = serde_json::from_str(&signature_content)
            .map_err(|e| format!("Invalid signature format: {}", e))?;

        let trusted_key = trusted_keys
            .iter()
            .find(|k| k.public_key_id == signed_manifest.signature.public_key_id)
            .ok_or("No trusted key found for this plugin")?;

        let checksum = Self::calculate_checksum(plugin_path)?;
        if checksum != signed_manifest.checksum {
            return Err("Plugin checksum mismatch".to_string());
        }

        let signature_data = format!(
            "{}:{}:{}",
            signed_manifest.plugin_id, signed_manifest.version, signed_manifest.checksum
        );

        Ok(Self::verify_signature(
            &signature_data,
            &signed_manifest.signature,
            trusted_key,
        )?)
    }

    fn calculate_checksum(plugin_path: &Path) -> Result<String, String> {
        let mut files: Vec<_> = fs::read_dir(plugin_path)
            .map_err(|e| format!("Failed to read plugin directory: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file() && e.file_name() != ".signature")
            .collect();

        files.sort_by_key(|e| e.file_name());

        let mut hasher = sha2::Sha256::new();
        for file in files {
            let content =
                fs::read(file.path()).map_err(|e| format!("Failed to read file: {}", e))?;
            hasher.update(&content);
        }

        Ok(hex::encode(hasher.finalize()))
    }

    fn sign_data(data: &str, key_pair: &KeyPair) -> Result<Signature, String> {
        let signing_key = key_pair
            .signing_key
            .as_ref()
            .ok_or("No signing key available")?;

        let signature = signing_key.sign(data.as_bytes());

        Ok(Signature {
            algorithm: "ed25519".to_string(),
            public_key_id: key_pair.public_key_id.clone(),
            signature: hex::encode(signature.to_bytes()),
            timestamp: chrono::Utc::now().timestamp(),
        })
    }

    fn verify_signature(
        data: &str,
        signature: &Signature,
        key_pair: &KeyPair,
    ) -> Result<bool, String> {
        use ed25519_dalek::{Signature as DalekSignature, VerifyingKey};

        if signature.algorithm != "ed25519" {
            return Err("Unsupported signature algorithm".to_string());
        }

        let key_bytes = hex::decode(&key_pair.public_key)
            .map_err(|e| format!("Invalid public key hex: {}", e))?;

        let verifying_key = VerifyingKey::from_bytes(
            key_bytes[..32]
                .try_into()
                .map_err(|_| "Invalid key length")?,
        )
        .map_err(|e| format!("Invalid verifying key: {}", e))?;

        let sig_bytes = hex::decode(&signature.signature)
            .map_err(|e| format!("Invalid signature hex: {}", e))?;

        let dalek_signature = DalekSignature::from_bytes(
            sig_bytes[..64]
                .try_into()
                .map_err(|_| "Invalid signature length")?,
        );

        Ok(verifying_key
            .verify(data.as_bytes(), &dalek_signature)
            .is_ok())
    }
}

pub struct SecurityPolicy {
    trusted_signers: HashMap<String, KeyPair>,
    require_signature: bool,
    allow_unsigned: bool,
}

impl SecurityPolicy {
    pub fn new() -> Self {
        Self {
            trusted_signers: HashMap::new(),
            require_signature: true,
            allow_unsigned: false,
        }
    }

    pub fn with_trusted_signer(mut self, key_pair: KeyPair) -> Self {
        self.trusted_signers
            .insert(key_pair.public_key_id.clone(), key_pair);
        self
    }

    pub fn require_signatures(mut self) -> Self {
        self.require_signature = true;
        self.allow_unsigned = false;
        self
    }

    pub fn verify(&self, plugin_path: &Path) -> Result<bool, String> {
        if self.trusted_signers.is_empty() {
            return Ok(self.allow_unsigned);
        }

        let trusted_keys: Vec<_> = self.trusted_signers.values().cloned().collect();

        match PluginSigner::verify_plugin(plugin_path, &trusted_keys) {
            Ok(result) => Ok(result),
            Err(e) => {
                if self.require_signature {
                    Err(e)
                } else {
                    Ok(false)
                }
            }
        }
    }
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_security_policy_defaults() {
        let policy = SecurityPolicy::new();
        assert!(!policy.require_signature);
        assert!(policy.allow_unsigned);
    }

    #[test]
    fn test_security_policy_with_trusted_signer() {
        let key_pair = KeyPair {
            public_key_id: "test-key".to_string(),
            public_key: "a".repeat(64),
            signing_key: None,
        };

        let policy = SecurityPolicy::new()
            .with_trusted_signer(key_pair.clone())
            .require_signatures();

        assert!(policy.require_signature);
        assert!(!policy.allow_unsigned);
        assert!(policy.trusted_signers.contains_key("test-key"));
    }

    #[test]
    fn test_checksum_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_path = temp_dir.path();

        fs::write(plugin_path.join("file1.txt"), "content1").unwrap();
        fs::write(plugin_path.join("file2.txt"), "content2").unwrap();

        let checksum = PluginSigner::calculate_checksum(plugin_path).unwrap();
        assert!(!checksum.is_empty());
    }
}
