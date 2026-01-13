//! Frame parser and serializer
//!
//! This module handles the serialization (marshaling) and deserialization (unmarshaling)
//! of VPN protocol frames. It manages the frame header format, payload encryption/decryption,
//! and JSON serialization of frame data.

use crate::codec::frame::*;
use crate::crypto::Block;
use anyhow::Context;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Protocol magic number for frame validation
const MAGIC: u32 = 0x91929394;
/// Protocol version
const VERSION: u8 = 0x01;

pub struct Parser;

impl Parser {
    /// Unmarshals (deserializes) a frame from raw bytes
    ///
    /// Parses the frame header, validates it, extracts and decrypts the payload,
    /// and deserializes it into the appropriate Frame variant.
    ///
    /// # Arguments
    /// * `buf` - Raw byte buffer containing the frame
    /// * `block` - Cipher block for payload decryption
    ///
    /// # Returns
    /// * `Ok((Frame, usize))` - Parsed frame and total bytes consumed
    /// * `Err` - If frame is invalid, too short, or decryption/parsing fails
    pub fn unmarshal(buf: &[u8], block: &Box<dyn Block>) -> crate::Result<(Frame, usize)> {
        if buf.len() < HDR_LEN {
            return Err(FrameError::TooShort.into());
        }

        let magic = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let version = buf[4];
        let cmd = buf[5];
        let payload_size = u16::from_be_bytes([buf[6], buf[7]]);

        if !Self::validate(magic, version, payload_size, buf) {
            tracing::debug!("validate header fail: magic = {} version={} payload_size={} buf size={}", magic, version, payload_size, buf.len());
            return Err(FrameError::Invalid.into());
        }

        let total_len = HDR_LEN + payload_size as usize;
        let payload = &mut buf[HDR_LEN..total_len].to_vec();

        let frame_type = FrameType::try_from(cmd)?;
        match frame_type {
            FrameType::Handshake => {
                let hs: HandshakeFrame = Self::decrypt_and_deserialize(payload, block)?;
                Ok((Frame::Handshake(hs), total_len))
            }

            FrameType::HandshakeReply => {
                let reply: HandshakeReplyFrame = Self::decrypt_and_deserialize(payload, block)?;
                Ok((Frame::HandshakeReply(reply), total_len))
            }

            FrameType::KeepAlive => {
                let keepalive: KeepAliveFrame = Self::decrypt_and_deserialize(payload, block)?;
                Ok((Frame::KeepAlive(keepalive), total_len))
            }

            FrameType::Data => {
                block
                    .decrypt(payload)
                    .map_err(FrameError::DecryptionFailed)?;
                Ok((
                    Frame::Data(DataFrame {
                        payload: payload.to_vec(),
                    }),
                    total_len,
                ))
            }

            FrameType::ProbeIPv6 => {
                let probe: ProbeIPv6Frame = Self::decrypt_and_deserialize(payload, block)?;
                Ok((Frame::ProbeIPv6(probe), total_len))
            }

            FrameType::ProbeHolePunch => {
                let probe: ProbeHolePunchFrame = Self::decrypt_and_deserialize(payload, block)?;
                Ok((Frame::ProbeHolePunch(probe), total_len))
            }
        }
    }

    /// Validates frame header
    ///
    /// Checks magic number, version, and ensures complete frame is in buffer.
    ///
    /// # Arguments
    /// * `magic` - Magic number from header (should be 0x91929394)
    /// * `version` - Protocol version (should be 0x01)
    /// * `payload_size` - Payload length from header
    /// * `buf` - Complete buffer to verify size
    fn validate(magic: u32, version: u8, payload_size: u16, buf: &[u8]) -> bool {
        magic == MAGIC && version == VERSION && (payload_size as usize + HDR_LEN) <= buf.len()
    }

    /// Decrypts and deserializes JSON payload
    ///
    /// Helper function to decrypt a payload and deserialize it from JSON.
    /// Used for Handshake and HandshakeReply frames.
    ///
    /// # Arguments
    /// * `payload` - Encrypted payload bytes
    /// * `block` - Cipher block for decryption
    ///
    /// # Returns
    /// Deserialized frame data of type T
    fn decrypt_and_deserialize<T: DeserializeOwned>(
        payload: &mut Vec<u8>,
        block: &Box<dyn Block>,
    ) -> crate::Result<T> {
        block
            .decrypt(payload)
            .map_err(FrameError::DecryptionFailed)?;
        serde_json::from_slice(payload).map_err(|_| FrameError::Invalid.into())
    }

    /// Serializes and encrypts JSON payload
    ///
    /// Helper function to serialize data to JSON and encrypt it.
    /// Used for Handshake and HandshakeReply frames.
    ///
    /// # Arguments
    /// * `data` - Data to serialize
    /// * `block` - Cipher block for encryption
    /// * `context_msg` - Error context message
    ///
    /// # Returns
    /// Encrypted payload bytes
    fn serialize_and_encrypt<T: Serialize>(
        data: &T,
        block: &Box<dyn Block>,
        context_msg: &str,
    ) -> crate::Result<Vec<u8>> {
        let msg = context_msg.to_string();
        let json = serde_json::to_string(data).with_context(|| msg)?;
        let mut payload = json.as_bytes().to_vec();
        block.encrypt(&mut payload)?;
        Ok(payload)
    }

    /// Builds a frame header
    ///
    /// Creates the 8-byte frame header with magic, version, frame type, and payload length.
    ///
    /// # Arguments
    /// * `frame_type` - Type of frame
    /// * `payload_len` - Length of payload in bytes
    ///
    /// # Returns
    /// Header bytes (8 bytes total)
    fn build_header(frame_type: FrameType, payload_len: u16) -> Vec<u8> {
        let mut buf = Vec::with_capacity(HDR_LEN + payload_len as usize);
        buf.extend_from_slice(&MAGIC.to_be_bytes());
        buf.push(VERSION);
        buf.push(frame_type as u8);
        buf.extend_from_slice(&payload_len.to_be_bytes());
        buf
    }

    /// Marshals (serializes) a frame into raw bytes
    ///
    /// Serializes the frame data to JSON, encrypts the payload, and builds
    /// the frame header with the complete frame structure.
    ///
    /// # Arguments
    /// * `frame` - Frame to serialize
    /// * `block` - Cipher block for payload encryption
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - Complete frame bytes (header + encrypted payload)
    /// * `Err` - If serialization or encryption fails
    pub fn marshal(frame: Frame, block: &Box<dyn Block>) -> crate::Result<Vec<u8>> {
        match frame {
            Frame::Handshake(hs) => {
                let payload =
                    Self::serialize_and_encrypt(&hs, block, "failed to marshal handshake")?;
                let mut buf = Self::build_header(FrameType::Handshake, payload.len() as u16);
                buf.extend_from_slice(&payload);
                Ok(buf)
            }

            Frame::HandshakeReply(reply) => {
                let payload = Self::serialize_and_encrypt(
                    &reply,
                    block,
                    "failed to marshal handshake reply",
                )?;
                let mut buf = Self::build_header(FrameType::HandshakeReply, payload.len() as u16);
                buf.extend_from_slice(&payload);
                Ok(buf)
            }

            Frame::KeepAlive(keepalive) => {
                let payload = Self::serialize_and_encrypt(
                    &keepalive,
                    block,
                    "failed to marshal keepalive",
                )?;
                let mut buf = Self::build_header(FrameType::KeepAlive, payload.len() as u16);
                buf.extend_from_slice(&payload);
                Ok(buf)
            }

            Frame::Data(mut data) => {
                block.encrypt(&mut data.payload)?;
                let mut buf = Self::build_header(FrameType::Data, data.payload.len() as u16);
                buf.extend_from_slice(&data.payload);
                Ok(buf)
            }

            Frame::ProbeIPv6(frame) => {
                let payload = Self::serialize_and_encrypt(
                    &frame,
                    block,
                    "failed to marshal probe ipv6",
                )?;
                let mut buf = Self::build_header(FrameType::ProbeIPv6, payload.len() as u16);
                buf.extend_from_slice(&payload);
                Ok(buf)
            }

            Frame::ProbeHolePunch(frame) => {
                let payload = Self::serialize_and_encrypt(
                    &frame,
                    block,
                    "failed to marshal probe hole punch",
                )?;
                let mut buf = Self::build_header(FrameType::ProbeHolePunch, payload.len() as u16);
                buf.extend_from_slice(&payload);
                Ok(buf)
            }
        }
    }
}
