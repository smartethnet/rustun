use crate::codec::frame::*;
use crate::crypto::Block;
use anyhow::Context;

pub struct Parser;

impl Parser {
    pub fn unmarshal(buf: &[u8], block: &Box<dyn Block>) -> crate::Result<(Frame, usize)> {
        if buf.len() < HDR_LEN {
            return Err(FrameError::TooShort.into());
        }

        let magic = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let version = buf[4];
        let cmd = buf[5];
        let payload_size = u16::from_be_bytes([buf[6], buf[7]]);

        if !Parser::validate(magic, version, payload_size, buf) {
            return Err(FrameError::Invalid.into());
        }

        let total_len = HDR_LEN + payload_size as usize;
        let payload = &mut buf[HDR_LEN..total_len].to_vec();


        let frame_type = FrameType::try_from(cmd)?;
        match frame_type {
            FrameType::Handshake => {
                block.decrypt(payload).map_err(FrameError::DecryptionFailed)?;
                let hs: HandshakeFrame = serde_json::from_slice(payload)
                    .map_err(|_| FrameError::Invalid)?;
                Ok((Frame::Handshake(hs), total_len))
            }

            FrameType::HandshakeReply => {
                block.decrypt(payload).map_err(FrameError::DecryptionFailed)?;
                let reply: HandshakeReplyFrame = serde_json::from_slice(payload)
                    .map_err(|_| FrameError::Invalid)?;
                Ok((Frame::HandshakeReply(reply), total_len))
            }

            FrameType::KeepAlive => {
                Ok((Frame::KeepAlive(KeepAliveFrame {}), total_len))
            }

            FrameType::Data => {
                block.decrypt(payload).map_err(FrameError::DecryptionFailed)?;
                Ok((Frame::Data(DataFrame { payload: payload.to_vec() }), total_len))
            }
        }
    }

    fn validate(magic: u32, version: u8, payload_size: u16, buf: &[u8]) -> bool {
        if magic != 0x91929394 {
            return false;
        }

        if version != 0x01 {
            return false;
        }

        if payload_size + HDR_LEN as u16 > buf.len() as u16 {
            return false;
        }
        true
    }

    pub fn marshal(frame: Frame, block: &Box<dyn Block>) -> crate::Result<Vec<u8>> {
        match frame {
            Frame::Handshake(hs) => {
                let payload = serde_json::to_string(&hs).with_context(|| "failed to marshal handshake")?;
                let mut payload = payload.as_bytes().to_vec();
                if let Err(e) =  block.encrypt(&mut payload) {
                    return Err(e.into());
                };

                let mut buf = Vec::with_capacity(HDR_LEN);
                // magic: 0x91929394
                buf.extend_from_slice(&0x91929394u32.to_be_bytes());
                // version: 0x01
                buf.push(0x01);
                // cmd
                buf.push(FrameType::Handshake as u8);
                // payload_size
                let payload_length = payload.len() as u16;
                buf.extend_from_slice(&(payload_length.to_be_bytes()));
                // payload
                buf.extend_from_slice(&payload);
                Ok(buf)
            }
            Frame::HandshakeReply(reply) => {
                let payload = serde_json::to_string(&reply).with_context(|| "failed to marshal handshake reply")?;
                let mut payload = payload.as_bytes().to_vec();
                if let Err(e) = block.encrypt(&mut payload) {
                    return Err(e.into());
                };

                let mut buf = Vec::with_capacity(HDR_LEN);
                // magic: 0x91929394
                buf.extend_from_slice(&0x91929394u32.to_be_bytes());
                // version: 0x01
                buf.push(0x01);
                // cmd
                buf.push(FrameType::HandshakeReply as u8);
                // payload_size
                let payload_length = payload.len() as u16;
                buf.extend_from_slice(&(payload_length.to_be_bytes()));
                // payload
                buf.extend_from_slice(&payload);
                Ok(buf)
            }
            Frame::KeepAlive(_kf) => {
                let mut buf = Vec::with_capacity(HDR_LEN);
                // magic: 0x91929394
                buf.extend_from_slice(&0x91929394u32.to_be_bytes());
                // version: 0x01
                buf.push(0x01);
                // cmd: KeepAlive = 2
                buf.push(FrameType::KeepAlive as u8);
                // payload_size: 0
                buf.extend_from_slice(&0u16.to_be_bytes());
                Ok(buf)
            }
            Frame::Data(mut data) => {
                let payload = data.payload.as_mut();
                if let Err(e) = block.encrypt(payload) {
                    return Err(e.into());
                };

                let mut buf = Vec::with_capacity(HDR_LEN);
                // magic: 0x91929394
                buf.extend_from_slice(&0x91929394u32.to_be_bytes());
                // version: 0x01
                buf.push(0x01);
                // cmd: data = 2
                buf.push(FrameType::Data as u8);
                // payload_size: 0
                let payload_length = payload.len() as u16;
                buf.extend_from_slice(&payload_length.to_be_bytes());
                buf.extend_from_slice(&payload);
                Ok(buf)
            }
        }
    }

}

