#[cfg(feature = "encryption")]
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};

use crate::error::{PackError, Result};

#[cfg(feature = "encryption")]
#[derive(Clone)]
pub struct EncryptionKey {
    key: [u8; 32],
}

#[cfg(feature = "encryption")]
impl EncryptionKey {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    pub fn generate() -> Self {
        use aes_gcm::aead::rand_core::RngCore;
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        Self { key }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(PackError::Encryption(
                "Key must be exactly 32 bytes".to_string()
            ));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(bytes);
        Ok(Self { key })
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

#[cfg(feature = "encryption")]
pub fn encrypt_snapshot(data: &[u8], key: &EncryptionKey) -> Result<Vec<u8>> {
    use aes_gcm::aead::rand_core::RngCore;

    let cipher = Aes256Gcm::new_from_slice(&key.key)
        .map_err(|e| PackError::Encryption(e.to_string()))?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| PackError::Encryption(e.to_string()))?;

    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

#[cfg(feature = "encryption")]
pub fn decrypt_snapshot(data: &[u8], key: &EncryptionKey) -> Result<Vec<u8>> {
    if data.len() < 12 {
        return Err(PackError::Decryption(
            "Encrypted data too short".to_string()
        ));
    }

    let cipher = Aes256Gcm::new_from_slice(&key.key)
        .map_err(|e| PackError::Decryption(e.to_string()))?;

    let nonce = Nonce::from_slice(&data[0..12]);
    let ciphertext = &data[12..];

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| PackError::Decryption(e.to_string()))?;

    Ok(plaintext)
}

#[cfg(not(feature = "encryption"))]
pub struct EncryptionKey;

#[cfg(not(feature = "encryption"))]
impl EncryptionKey {
    pub fn new(_key: [u8; 32]) -> Self {
        Self
    }

    pub fn generate() -> Self {
        Self
    }
}

#[cfg(not(feature = "encryption"))]
pub fn encrypt_snapshot(_data: &[u8], _key: &EncryptionKey) -> Result<Vec<u8>> {
    Err(PackError::Encryption(
        "Encryption feature not enabled".to_string()
    ))
}

#[cfg(not(feature = "encryption"))]
pub fn decrypt_snapshot(_data: &[u8], _key: &EncryptionKey) -> Result<Vec<u8>> {
    Err(PackError::Decryption(
        "Encryption feature not enabled".to_string()
    ))
}

#[cfg(all(test, feature = "encryption"))]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_decryption() {
        let data = b"Hello, World! This is sensitive data.";
        let key = EncryptionKey::generate();

        let encrypted = encrypt_snapshot(data, &key).unwrap();
        assert_ne!(data.as_slice(), encrypted.as_slice());

        let decrypted = decrypt_snapshot(&encrypted, &key).unwrap();
        assert_eq!(data.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_wrong_key() {
        let data = b"Hello, World!";
        let key1 = EncryptionKey::generate();
        let key2 = EncryptionKey::generate();

        let encrypted = encrypt_snapshot(data, &key1).unwrap();
        let result = decrypt_snapshot(&encrypted, &key2);

        assert!(result.is_err());
    }
}
