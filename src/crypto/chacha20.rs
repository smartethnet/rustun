//! ChaCha20-Poly1305 AEAD cipher implementation
//! 
//! ChaCha20-Poly1305 is a modern authenticated encryption algorithm that provides
//! both confidentiality and authenticity. It's faster than AES on platforms without
//! hardware AES acceleration and is used in protocols like TLS 1.3 and WireGuard.

use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
    ChaCha20Poly1305, Nonce,
};
use super::Block;

/// ChaCha20-Poly1305 cipher block
/// 
/// This implementation uses a 256-bit (32-byte) key and generates a unique
/// 96-bit (12-byte) nonce for each encryption operation. The nonce is prepended
/// to the ciphertext for decryption.
pub struct ChaCha20Poly1305Block {
    cipher: ChaCha20Poly1305,
}

impl ChaCha20Poly1305Block {
    /// Creates a new ChaCha20-Poly1305 cipher from a 32-byte key
    /// 
    /// # Arguments
    /// * `key` - 256-bit (32-byte) encryption key
    pub fn new(key: &[u8; 32]) -> Self {
        let cipher = ChaCha20Poly1305::new(key.into());
        Self { cipher }
    }

    /// Creates a new ChaCha20-Poly1305 cipher from a string
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

impl Block for ChaCha20Poly1305Block {
    /// Encrypts data in-place with ChaCha20-Poly1305
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

        let ciphertext = self.cipher
            .encrypt(nonce, data.as_ref())
            .map_err(|e| format!("ChaCha20-Poly1305 encryption failed: {}", e))?;

        // Replace data with: nonce || ciphertext (ciphertext already includes auth tag)
        data.clear();
        data.extend_from_slice(&nonce_bytes);
        data.extend_from_slice(&ciphertext);

        Ok(())
    }

    /// Decrypts data in-place with ChaCha20-Poly1305
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
            return Err("Data too short for ChaCha20-Poly1305 decryption".into());
        }

        let nonce = Nonce::from_slice(&data[0..12]);
        let ciphertext = &data[12..];

        let plaintext = self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("ChaCha20-Poly1305 decryption failed: {}", e))?;

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
    fn test_encrypt_decrypt() {
        let key = b"test_key_32_bytes_long_secret!!!";
        let cipher = ChaCha20Poly1305Block::new(key);

        let original = b"Hello, ChaCha20-Poly1305!".to_vec();
        let mut data = original.clone();

        // Encrypt
        cipher.encrypt(&mut data).unwrap();
        assert_ne!(data, original);
        assert!(data.len() > original.len()); // nonce + ciphertext + tag

        // Decrypt
        cipher.decrypt(&mut data).unwrap();
        assert_eq!(data, original);
    }

    #[test]
    fn test_from_string() {
        let cipher = ChaCha20Poly1305Block::from_string("my_secret_password");
        let mut data = b"Secret message".to_vec();

        cipher.encrypt(&mut data).unwrap();
        cipher.decrypt(&mut data).unwrap();
        
        assert_eq!(data, b"Secret message");
    }

    #[test]
    fn test_authentication_failure() {
        let cipher = ChaCha20Poly1305Block::from_string("correct_key");
        let mut data = b"Test data".to_vec();
        
        cipher.encrypt(&mut data).unwrap();
        
        // Tamper with ciphertext
        data[15] ^= 0xFF;
        
        // Decryption should fail due to authentication tag mismatch
        assert!(cipher.decrypt(&mut data).is_err());
    }

    #[test]
    fn test_nonce_uniqueness() {
        let cipher = ChaCha20Poly1305Block::from_string("test_key");
        let original = b"Same plaintext".to_vec();
        
        let mut data1 = original.clone();
        let mut data2 = original.clone();
        
        cipher.encrypt(&mut data1).unwrap();
        cipher.encrypt(&mut data2).unwrap();
        
        // Different nonces should produce different ciphertexts
        assert_ne!(data1, data2);
    }
}

