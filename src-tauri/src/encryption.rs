//! Device-specific encryption for secure password storage
//!
//! Uses AES-256-GCM with a device-specific key derived from:
//! - Machine ID (hardware-based unique identifier)
//! - Username (OS user)
//! - Application-specific salt
//!
//! This provides transparent encryption without requiring a master password,
//! while ensuring passwords are secure at rest and device-specific.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, ParamsBuilder};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use thiserror::Error;
use tracing::debug;

/// Application-specific salt for key derivation
/// This should be unique per application to prevent cross-app key derivation
const APP_SALT: &[u8] = b"eddie.chat.v1.encryption.salt.2026";

/// Nonce size for AES-GCM (96 bits / 12 bytes)
const NONCE_SIZE: usize = 12;

/// Encryption errors
#[derive(Debug, Error)]
pub enum EncryptionError {
    #[error("Failed to derive encryption key: {0}")]
    KeyDerivation(String),

    #[error("Encryption failed: {0}")]
    Encryption(String),

    #[error("Decryption failed: {0}")]
    Decryption(String),

    #[error("Invalid encrypted data format: {0}")]
    InvalidFormat(String),

    #[error("Failed to get device identifier: {0}")]
    DeviceId(String),
}

/// Device-specific encryption manager
pub struct DeviceEncryption {
    cipher: Aes256Gcm,
}

impl DeviceEncryption {
    /// Create a new encryption manager with device-specific key
    pub fn new() -> Result<Self, EncryptionError> {
        let key = Self::derive_device_key()?;
        let cipher = Aes256Gcm::new(&key.into());
        debug!("Initialized device-specific encryption");
        Ok(Self { cipher })
    }

    /// Derive a device-specific encryption key
    fn derive_device_key() -> Result<[u8; 32], EncryptionError> {
        // Get machine-specific identifier
        let machine_id = machine_uid::get().map_err(|e| {
            EncryptionError::DeviceId(format!("Failed to get machine ID: {}", e))
        })?;

        // Get current username
        let username = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "default".to_string());

        // Combine device-specific data
        let mut key_material = Vec::new();
        key_material.extend_from_slice(machine_id.as_bytes());
        key_material.extend_from_slice(username.as_bytes());
        key_material.extend_from_slice(APP_SALT);

        debug!(
            "Deriving encryption key from machine_id (len: {}), username: {}, salt (len: {})",
            machine_id.len(),
            username,
            APP_SALT.len()
        );

        // Use Argon2 to derive a secure key
        // Argon2 is designed for password hashing and key derivation
        let mut output_key = [0u8; 32]; // AES-256 key size

        // Configure Argon2 parameters
        let params = ParamsBuilder::new()
            .m_cost(65536) // 64 MiB memory
            .t_cost(3) // 3 iterations
            .p_cost(4) // 4 parallelism
            .build()
            .map_err(|e| {
                EncryptionError::KeyDerivation(format!("Failed to build Argon2 params: {}", e))
            })?;

        let argon2 = Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            params,
        );

        argon2
            .hash_password_into(&key_material, APP_SALT, &mut output_key)
            .map_err(|e| {
                EncryptionError::KeyDerivation(format!("Argon2 key derivation failed: {}", e))
            })?;

        debug!("Successfully derived device-specific encryption key");
        Ok(output_key)
    }

    /// Encrypt a plaintext string
    ///
    /// Returns a base64-encoded string containing: nonce || ciphertext
    pub fn encrypt(&self, plaintext: &str) -> Result<String, EncryptionError> {
        if plaintext.is_empty() {
            return Err(EncryptionError::Encryption(
                "Cannot encrypt empty plaintext".to_string(),
            ));
        }

        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        use aes_gcm::aead::rand_core::RngCore;
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| EncryptionError::Encryption(format!("AES-GCM encryption failed: {}", e)))?;

        // Combine nonce + ciphertext
        let mut combined = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        // Encode as base64
        let encoded = BASE64.encode(&combined);
        debug!(
            "Encrypted data (plaintext_len: {}, ciphertext_len: {})",
            plaintext.len(),
            ciphertext.len()
        );

        Ok(encoded)
    }

    /// Decrypt a base64-encoded encrypted string
    ///
    /// Expects format: nonce || ciphertext (both base64-encoded)
    pub fn decrypt(&self, encrypted: &str) -> Result<String, EncryptionError> {
        if encrypted.is_empty() {
            return Err(EncryptionError::InvalidFormat(
                "Cannot decrypt empty string".to_string(),
            ));
        }

        // Decode from base64
        let combined = BASE64.decode(encrypted).map_err(|e| {
            EncryptionError::InvalidFormat(format!("Invalid base64 encoding: {}", e))
        })?;

        // Extract nonce and ciphertext
        if combined.len() < NONCE_SIZE {
            return Err(EncryptionError::InvalidFormat(format!(
                "Encrypted data too short: {} bytes",
                combined.len()
            )));
        }

        let (nonce_bytes, ciphertext) = combined.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        let plaintext_bytes = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| {
                EncryptionError::Decryption(format!(
                    "AES-GCM decryption failed (possibly wrong key or corrupted data): {}",
                    e
                ))
            })?;

        // Convert to string
        let plaintext = String::from_utf8(plaintext_bytes).map_err(|e| {
            EncryptionError::Decryption(format!("Decrypted data is not valid UTF-8: {}", e))
        })?;

        debug!("Successfully decrypted data (length: {})", plaintext.len());
        Ok(plaintext)
    }
}

impl Default for DeviceEncryption {
    fn default() -> Self {
        Self::new().expect("Failed to initialize device encryption")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let encryption = DeviceEncryption::new().unwrap();

        let plaintext = "my_secret_password_123!";
        let encrypted = encryption.encrypt(plaintext).unwrap();

        // Encrypted should be different from plaintext
        assert_ne!(encrypted, plaintext);

        // Should be base64-encoded
        assert!(BASE64.decode(&encrypted).is_ok());

        // Decrypt should return original plaintext
        let decrypted = encryption.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_same_plaintext_different_ciphertext() {
        let encryption = DeviceEncryption::new().unwrap();

        let plaintext = "same_password";
        let encrypted1 = encryption.encrypt(plaintext).unwrap();
        let encrypted2 = encryption.encrypt(plaintext).unwrap();

        // Should produce different ciphertexts due to random nonces
        assert_ne!(encrypted1, encrypted2);

        // Both should decrypt to same plaintext
        assert_eq!(encryption.decrypt(&encrypted1).unwrap(), plaintext);
        assert_eq!(encryption.decrypt(&encrypted2).unwrap(), plaintext);
    }

    #[test]
    fn test_empty_plaintext_fails() {
        let encryption = DeviceEncryption::new().unwrap();
        assert!(encryption.encrypt("").is_err());
    }

    #[test]
    fn test_invalid_encrypted_data() {
        let encryption = DeviceEncryption::new().unwrap();

        // Invalid base64
        assert!(encryption.decrypt("not_base64!@#$%").is_err());

        // Valid base64 but too short
        assert!(encryption.decrypt(BASE64.encode("short")).is_err());

        // Valid base64 but wrong data
        let wrong_data = BASE64.encode(&[0u8; 32]);
        assert!(encryption.decrypt(&wrong_data).is_err());
    }

    #[test]
    fn test_device_key_derivation_deterministic() {
        // Same device should produce same key
        let key1 = DeviceEncryption::derive_device_key().unwrap();
        let key2 = DeviceEncryption::derive_device_key().unwrap();

        assert_eq!(key1, key2, "Device key should be deterministic");
    }

    #[test]
    fn test_long_password() {
        let encryption = DeviceEncryption::new().unwrap();

        // Test with a very long password
        let plaintext = "a".repeat(10000);
        let encrypted = encryption.encrypt(&plaintext).unwrap();
        let decrypted = encryption.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_special_characters() {
        let encryption = DeviceEncryption::new().unwrap();

        let plaintext = "pässwörd!@#$%^&*()_+-=[]{}|;:',.<>?/~`";
        let encrypted = encryption.encrypt(plaintext).unwrap();
        let decrypted = encryption.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }
}
