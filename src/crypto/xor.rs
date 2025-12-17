//! XOR cipher implementation
//! 
//! This module implements a simple XOR stream cipher. While XOR encryption is fast
//! and lightweight, it is cryptographically weak and should NOT be used for securing
//! sensitive data. It's primarily useful for obfuscation, testing, or scenarios where
//! strong security is not required.
//! 
//! ⚠️ SECURITY WARNING: XOR cipher is vulnerable to known-plaintext attacks and
//! frequency analysis. Use ChaCha20-Poly1305 or AES-256-GCM for production security.
//! 
//! # How it works
//! Each byte of data is XORed with the corresponding byte of the key (cycling through
//! the key if necessary). Since XOR is its own inverse, encryption and decryption
//! are identical operations.

use super::Block;

/// XOR cipher block
/// 
/// A simple stream cipher that XORs data with a repeating key pattern.
/// The key can be any length, but longer keys provide slightly better obfuscation.
/// 
/// # Properties
/// - Symmetric: encryption and decryption are the same operation
/// - Fast: O(n) time complexity with minimal overhead
/// - Deterministic: same key + data always produces same output
/// - Weak: vulnerable to cryptanalysis, not suitable for sensitive data
pub struct XorBlock {
    key: Vec<u8>,
}

impl XorBlock {
    /// Creates a new XOR cipher from a byte key
    /// 
    /// # Arguments
    /// * `key` - Byte array to use as the XOR key
    /// 
    /// # Panics
    /// Panics if the key is empty
    pub fn new(key: &[u8]) -> Self {
        if key.is_empty() {
            panic!("XOR key cannot be empty");
        }
        Self {
            key: key.to_vec(),
        }
    }

    /// Creates a new XOR cipher from a string
    /// 
    /// The string is converted to bytes and used as the XOR key.
    /// 
    /// # Arguments
    /// * `s` - String to use as the XOR key
    /// 
    /// # Panics
    /// Panics if the string is empty
    pub fn from_string(s: &str) -> Self {
        let key = s.as_bytes();
        if key.is_empty() {
            panic!("XOR key cannot be empty");
        }
        Self {
            key: key.to_vec(),
        }
    }

    /// XORs data with the key in-place
    /// 
    /// Applies XOR operation: `data[i] ^= key[i % key.len()]`
    /// The key repeats cyclically if the data is longer than the key.
    fn xor_data(&self, data: &mut [u8]) {
        let key_len = self.key.len();
        for (i, byte) in data.iter_mut().enumerate() {
            *byte ^= self.key[i % key_len];
        }
    }
}

impl Block for XorBlock {
    /// Encrypts data in-place using XOR
    /// 
    /// Applies the XOR operation with the key to obfuscate the data.
    /// Note: This provides minimal security and is vulnerable to cryptanalysis.
    /// 
    /// # Arguments
    /// * `data` - Plaintext to encrypt (will be modified in-place)
    /// 
    /// # Returns
    /// * Always returns `Ok(())`
    fn encrypt(&self, data: &mut Vec<u8>) -> crate::Result<()> {
        self.xor_data(data);
        Ok(())
    }

    /// Decrypts data in-place using XOR
    /// 
    /// Since XOR is symmetric (A ⊕ B ⊕ B = A), decryption is identical to encryption.
    /// Simply applies the same XOR operation to recover the original data.
    /// 
    /// # Arguments
    /// * `data` - Ciphertext to decrypt (will be modified in-place)
    /// 
    /// # Returns
    /// * Always returns `Ok(())`
    fn decrypt(&self, data: &mut Vec<u8>) -> crate::Result<()> {
        // XOR encryption is symmetric: decrypt is the same as encrypt
        self.xor_data(data);
        Ok(())
    }
}