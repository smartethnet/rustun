//! AES-256-GCM AEAD cipher implementation
//!
//! AES-256-GCM (Galois/Counter Mode) is an industry-standard authenticated encryption
//! algorithm that provides both confidentiality and authenticity. It offers excellent
//! performance on platforms with hardware AES acceleration (AES-NI) and is widely
//! used in TLS, IPsec, and other security protocols.

use super::Block;
use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
};

/// AES-256-GCM cipher block
///
/// This implementation uses a 256-bit (32-byte) key and generates a unique
/// 96-bit (12-byte) nonce for each encryption operation. The nonce is prepended
/// to the ciphertext for decryption.
pub struct Aes256Block {
    cipher: Aes256Gcm,
}

impl Aes256Block {
    /// Creates a new AES-256-GCM cipher from a 32-byte key
    ///
    /// # Arguments
    /// * `key` - 256-bit (32-byte) encryption key
    pub fn new(key: &[u8; 32]) -> Self {
        let cipher = Aes256Gcm::new(key.into());
        Self { cipher }
    }

    /// Creates a new AES-256-GCM cipher from a string
    ///
    /// The string is converted to bytes and padded/truncated to 32 bytes.
    /// If the string is shorter than 32 bytes, it's zero-padded.
    /// If longer, only the first 32 bytes are used.
    ///
    /// # Arguments
    /// * `s` - String to derive the key from
    pub fn from_string(s: &str) -> Self {
        let mut key = [0u8; 32];
        let bytes = s.as_bytes();

        if bytes.len() >= 32 {
            key.copy_from_slice(&bytes[..32]);
        } else {
            key[..bytes.len()].copy_from_slice(bytes);
        }

        Self::new(&key)
    }

    /// Generates a random 12-byte nonce
    ///
    /// Each encryption operation should use a unique nonce to ensure security.
    /// This function uses the system's cryptographically secure random number generator.
    fn generate_nonce() -> [u8; 12] {
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        nonce
    }
}

impl Block for Aes256Block {
    /// Encrypts data in-place with AES-256-GCM
    ///
    /// The encrypted output format is: [nonce(12 bytes)][ciphertext][tag(16 bytes)]
    /// The authentication tag is automatically appended by the AEAD cipher.
    ///
    /// # Arguments
    /// * `data` - Plaintext to encrypt (will be replaced with nonce + ciphertext + tag)
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err` if encryption fails
    fn encrypt(&self, data: &mut Vec<u8>) -> crate::Result<()> {
        let nonce_bytes = Self::generate_nonce();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, data.as_ref())
            .map_err(|e| format!("AES-256-GCM encryption failed: {}", e))?;

        // Replace data with: nonce || ciphertext (ciphertext already includes auth tag)
        data.clear();
        data.extend_from_slice(&nonce_bytes);
        data.extend_from_slice(&ciphertext);

        Ok(())
    }

    /// Decrypts data in-place with AES-256-GCM
    ///
    /// Expects input format: [nonce(12 bytes)][ciphertext][tag(16 bytes)]
    /// The authentication tag is automatically verified during decryption.
    ///
    /// # Arguments
    /// * `data` - Encrypted data (nonce + ciphertext + tag) to decrypt
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err` if data is too short, decryption fails, or authentication fails
    fn decrypt(&self, data: &mut Vec<u8>) -> crate::Result<()> {
        // Minimum length: 12 (nonce) + 16 (tag) = 28 bytes
        if data.len() < 28 {
            return Err("Data too short for AES-256-GCM decryption".into());
        }

        let nonce = Nonce::from_slice(&data[0..12]);
        let ciphertext = &data[12..];

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("AES-256-GCM decryption failed: {}", e))?;

        // Replace data with plaintext
        data.clear();
        data.extend_from_slice(&plaintext);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_aes_256_gcm() {
        let block = Aes256Block::from_string("rustun");
        let msg = String::from("hello");
        let mut msg_bytes = msg.as_bytes().to_vec();
        block.encrypt(&mut msg_bytes).unwrap();
        println!("msg_bytes = {:?}", msg_bytes);
        block.decrypt(&mut msg_bytes).unwrap();
        println!("msg_bytes = {:?}", msg_bytes);
    }
}
