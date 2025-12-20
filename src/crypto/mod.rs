//! Cryptographic module providing encryption/decryption capabilities
//!
//! This module supports multiple cipher algorithms including:
//! - AES-256-GCM: Industry-standard symmetric AEAD encryption
//! - ChaCha20-Poly1305: Modern AEAD cipher (fast, secure)
//! - XOR: Simple stream cipher for lightweight encryption
//! - Plain: No encryption (passthrough mode)

pub mod aes256;
pub mod chacha20;
pub mod plain;
pub mod xor;

use crate::crypto::aes256::Aes256Block;
use crate::crypto::chacha20::ChaCha20Poly1305Block;
use crate::crypto::plain::PlainBlock;
use crate::crypto::xor::XorBlock;
use serde::{Deserialize, Serialize};

/// Core encryption/decryption trait
///
/// All cipher implementations must implement this trait to provide
/// consistent encryption and decryption interfaces. The trait is
/// marked as `Send + Sync` to enable safe concurrent usage across threads.
pub trait Block: Send + Sync {
    /// Encrypts data in-place
    ///
    /// # Arguments
    /// * `data` - Mutable byte vector to be encrypted
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err` if encryption fails
    fn encrypt(&self, data: &mut Vec<u8>) -> crate::Result<()>;

    /// Decrypts data in-place
    ///
    /// # Arguments
    /// * `data` - Mutable byte vector to be decrypted
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err` if decryption fails
    fn decrypt(&self, data: &mut Vec<u8>) -> crate::Result<()>;
}

/// Factory function to create cipher blocks from configuration
///
/// # Arguments
/// * `cfg` - Cryptographic configuration specifying the cipher type and parameters
///
/// # Returns
/// * Boxed trait object implementing the Block trait
///
/// # Examples
/// ```
/// use rustun::crypto::new_block;
/// use rustun::crypto::CryptoConfig;
/// let config = CryptoConfig::Aes256("secret_key".to_string());
/// let cipher = new_block(&config);
/// ```
pub fn new_block(cfg: &CryptoConfig) -> Box<dyn Block> {
    match cfg {
        CryptoConfig::Aes256(aes) => Box::new(Aes256Block::from_string(aes.as_str())),
        CryptoConfig::ChaCha20Poly1305(key) => {
            Box::new(ChaCha20Poly1305Block::from_string(key.as_str()))
        }
        CryptoConfig::Xor(xor) => Box::new(XorBlock::from_string(xor.as_str())),
        CryptoConfig::Plain => Box::new(PlainBlock::new()),
    }
}

/// Cryptographic configuration enum
///
/// Defines the available cipher algorithms and their configuration parameters.
/// Serialized to lowercase format for TOML/JSON compatibility.
///
/// # Variants
/// * `Aes256(String)` - AES-256-GCM AEAD encryption
/// * `ChaCha20Poly1305(String)` - ChaCha20-Poly1305 AEAD encryption
/// * `Xor(String)` - XOR stream cipher (lightweight, less secure)
/// * `Plain` - No encryption (data passthrough)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CryptoConfig {
    /// AES-256-GCM authenticated encryption
    /// Parameter: 32-byte key (as string, padded/truncated automatically)
    Aes256(String),

    /// ChaCha20-Poly1305 authenticated encryption (recommended)
    /// Parameter: 32-byte key (as string, padded/truncated automatically)
    /// Fast on all platforms, widely used in modern protocols (TLS 1.3, WireGuard)
    ChaCha20Poly1305(String),

    /// No encryption (passthrough mode)
    Plain,

    /// XOR stream cipher (simple, fast, but cryptographically weak)
    /// Parameter: String key for XOR operations
    Xor(String),
}
