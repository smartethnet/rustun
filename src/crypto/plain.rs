//! Plain (no-op) cipher implementation
//! 
//! This module provides a passthrough cipher that performs no encryption or decryption.
//! It's useful for development, testing, or scenarios where encryption is not required
//! but the cipher interface must be maintained for compatibility.
//! 
//! ⚠️ WARNING: This cipher provides NO security. Data is transmitted in plaintext.
//! Only use this in trusted networks or for testing purposes.

use crate::crypto::Block;

/// Plain cipher block (no encryption)
/// 
/// This is a no-op cipher that passes data through unchanged. Both encrypt and
/// decrypt operations are identity functions that simply return the input data.
/// 
/// # Use Cases
/// - Development and debugging
/// - Testing cipher interfaces
/// - Trusted internal networks where encryption overhead is unnecessary
/// - Baseline performance benchmarking
pub struct PlainBlock {}

impl PlainBlock {
    /// Creates a new plain cipher instance
    /// 
    /// No configuration is needed since this cipher performs no operations.
    pub fn new() -> Self {
        Self {}
    }
}

impl Block for PlainBlock {
    /// "Encrypts" data (no-op, returns data unchanged)
    /// 
    /// # Arguments
    /// * `_data` - Data to "encrypt" (unchanged)
    /// 
    /// # Returns
    /// * Always returns `Ok(())`
    fn encrypt(&self, _data: &mut Vec<u8>) -> crate::Result<()> {
        // No encryption performed
        Ok(())
    }

    /// "Decrypts" data (no-op, returns data unchanged)
    /// 
    /// # Arguments
    /// * `_data` - Data to "decrypt" (unchanged)
    /// 
    /// # Returns
    /// * Always returns `Ok(())`
    fn decrypt(&self, _data: &mut Vec<u8>) -> crate::Result<()> {
        // No decryption performed
        Ok(())
    }
}

impl Default for PlainBlock {
    fn default() -> Self {
        Self::new()
    }
}
