use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use rand::RngCore;

use super::Block;

pub struct Aes256Block {
    cipher: Aes256Gcm,
}

impl Aes256Block {
    pub fn new(key: &[u8; 32]) -> Self {
        let cipher = Aes256Gcm::new(key.into());
        Self { cipher }
    }

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

    fn generate_nonce() -> [u8; 12] {
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        nonce
    }
}

impl Block for Aes256Block {
    fn encrypt(&mut self, data: &mut Vec<u8>) -> crate::Result<()> {
        let nonce_bytes = Self::generate_nonce();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self.cipher
            .encrypt(nonce, data.as_ref())
            .map_err(|e| format!("Encryption failed: {}", e))?;

        data.clear();
        data.extend_from_slice(&nonce_bytes);
        data.extend_from_slice(&ciphertext);

        Ok(())
    }

    fn decrypt(&mut self, data: &mut Vec<u8>) -> crate::Result<()> {
        if data.len() < 28 {
            return Err("Data too short for decryption".into());
        }

        let nonce = Nonce::from_slice(&data[0..12]);
        let ciphertext = &data[12..];

        let plaintext = self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("Decryption failed: {}", e))?;

        data.clear();
        data.extend_from_slice(&plaintext);

        Ok(())
    }
}

