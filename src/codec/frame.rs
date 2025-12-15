pub(crate) use crate::codec::errors::FrameError;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

pub(crate) enum FrameType {
    Handshake = 1,
    KeepAlive = 2,
    Data = 3,
    HandshakeReply = 4,
}

impl TryFrom<u8> for FrameType {
    type Error = FrameError;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0x01 => Ok(FrameType::Handshake),
            0x02 => Ok(FrameType::KeepAlive),
            0x03 => Ok(FrameType::Data),
            0x04 => Ok(FrameType::HandshakeReply),
            _ => Err(FrameError::Invalid),
        }
    }
}

pub(crate) const HDR_LEN: usize = 8;

#[derive(Debug)]
pub enum Frame {
    Handshake(HandshakeFrame),
    HandshakeReply(HandshakeReplyFrame),
    KeepAlive(KeepAliveFrame),
    Data(DataFrame),
}

impl Display for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Frame::Handshake(frame) => write!(f, "handshake with {}", frame.identity),
            Frame::HandshakeReply(frame) => write!(f, "handshake reply with {} others", frame.others.len()),
            Frame::KeepAlive(_frame) => write!(f, "keepalive"),
            Frame::Data(frame) => write!(f, "data with payload size {}", frame.payload.len()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeFrame {
    pub identity: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeReplyFrame {
    pub private_ip: String,
    pub mask: String,
    pub gateway: String,
    pub others: Vec<RouteItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RouteItem {
    pub identity: String,
    pub private_ip: String,
    pub ciders: Vec<String>,
}

#[derive(Debug)] #[derive(Deserialize)]
pub struct KeepAliveFrame {
}

#[derive(Debug)] #[derive(Deserialize)]
pub struct DataFrame {
    pub payload: Vec<u8>,
}

impl DataFrame {
    pub fn invalid(&self) -> bool {
        self.payload.len() < 20
    }

    pub fn version(&self) -> i32 {
        (self.payload[0] >> 4) as i32
    }

    pub fn dst(&self) -> String {
        format!("{}.{}.{}.{}", self.payload[16], self.payload[17], self.payload[18], self.payload[19])
    }

    pub fn src(&self) -> String {
        format!("{}.{}.{}.{}", self.payload[12], self.payload[13], self.payload[14], self.payload[15])
    }
}
