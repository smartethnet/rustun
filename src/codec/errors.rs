//! Frame parsing and validation errors
//!
//! This module defines error types that can occur during frame parsing,
//! validation, and decryption operations. All errors implement the standard
//! Error trait for proper error propagation and handling.

use std::fmt;
use std::fmt::Display;

/// Frame parsing and processing errors
///
/// Represents various failure modes that can occur when unmarshaling frames
/// from raw byte streams, including incomplete data, invalid format, and
/// cryptographic failures.
#[derive(Debug)]
pub(crate) enum FrameError {
    /// Buffer is too short to contain a complete frame
    ///
    /// Occurs when:
    /// - Buffer length < 8 bytes (minimum header size)
    /// - Buffer length < header_size + payload_size
    ///
    /// This typically indicates the stream was interrupted or the data
    /// is still being received.
    TooShort,

    /// Frame header or payload format is invalid
    ///
    /// Occurs when:
    /// - Magic number is not 0x91929394
    /// - Protocol version is not 0x01
    /// - Frame type is unknown (not 1-4)
    /// - JSON deserialization fails
    ///
    /// This indicates corrupted data or protocol mismatch.
    Invalid,

    /// Payload decryption failed
    ///
    /// Wraps the underlying cryptographic error. This can occur when:
    /// - Authentication tag verification fails (AEAD ciphers)
    /// - Data was tampered with during transmission
    /// - Wrong encryption key is being used
    /// - Payload is too short for the cipher's requirements
    DecryptionFailed(crate::Error),
}

impl std::error::Error for FrameError {}

impl Display for FrameError {
    /// Formats the error for display and logging
    ///
    /// Provides human-readable error messages for debugging and logging purposes.
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FrameError::TooShort => "stream ended early".fmt(fmt),
            FrameError::Invalid => "invalid frame".fmt(fmt),
            FrameError::DecryptionFailed(e) => write!(fmt, "decryption failed: {}", e),
        }
    }
}
