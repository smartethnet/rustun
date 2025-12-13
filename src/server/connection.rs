use crate::crypto::Block;
use std::fmt::Display;
use std::sync::Arc;
use crate::codec::frame::Frame;
use crate::codec::parser::Parser;
use async_trait::async_trait;
use bytes::{Buf, BytesMut};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use std::net::IpAddr;
use ipnet::IpNet;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct ConnectionMeta {
    pub identity: String,
    pub private_ip: String,
    pub ciders: Vec<String>,
    pub(crate) outbound_tx: mpsc::Sender<Frame>,
}

impl PartialEq<ConnectionMeta> for &ConnectionMeta {
    fn eq(&self, other: &ConnectionMeta) -> bool {
        self.identity == other.identity
    }
}

impl ConnectionMeta {
    pub fn match_dst(&self, dst: String) -> bool {
        if self.private_ip == dst {
            return true;
        }

        let dst_ip = match dst.parse::<IpAddr>() {
            Ok(ip) => ip,
            Err(_) => return false, 
        };

        for cidr in &self.ciders {
            if let Ok(network) = cidr.parse::<IpNet>() {
                if network.contains(&dst_ip) {
                    return true;
                }
            }
        }

        false
    }
}

impl Display for ConnectionMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "private ip {}", self.private_ip)
    }
}

pub struct TcpConnection {
    socket: TcpStream,
    #[allow(dead_code)]
    write_timeout: Duration,
    #[allow(dead_code)]
    read_timeout: Duration,
    input_stream: BytesMut,
    #[allow(unused)]
    block: Arc<Box<dyn Block>>
}

impl TcpConnection {
    pub fn new(socket: TcpStream, block: Arc<Box<dyn Block>>) -> Self {
        Self {
            socket,
            write_timeout: DEFAULT_WRITE_TIMEOUT,
            read_timeout: DEFAULT_READ_TIMEOUT,
            input_stream: BytesMut::with_capacity(4096),
            block
        }
    }
}

impl TcpConnection {
    fn parse_frame(&mut self) -> crate::Result<Option<Frame>> {
        let result = Parser::unmarshal(self.input_stream.as_ref(), self.block.as_ref());
        match result {
            Ok((frame, total_len)) => {
                self.input_stream.advance(total_len);
                Ok(Some(frame))
            },
            Err(e) => {

                Err(e.into())
            }
        }
    }
}

#[async_trait]
impl Connection for TcpConnection {
    async fn read_frame(&mut self) -> crate::Result<Frame> {
        loop {
            // TODO: decrypt
            if let Ok(frame) = self.parse_frame() {
                if let Some(frame) = frame {
                    return Ok(frame);
                }
            }

            // TODO: set read timeout
            if 0 == self.socket.read_buf(&mut self.input_stream).await? {
                return if self.input_stream.is_empty() {
                    Err("EOF".into())
                } else {
                    Err("connection reset by peer".into())
                }
            }

        }
    }

    async fn write_frame(&mut self, frame: Frame) -> crate::Result<()> {
        // TODO: encrypt
        let result = Parser::marshal(frame, self.block.as_ref());
        let buf = match result {
            Ok(buf) => buf,
            Err(e) => {
                return Err(e.into());
            }
        };

        // TODO: set write timeout
        if let Err(e) = self.socket.write_all(buf.as_slice()).await {
            return Err(e.into());
        }
        if let Err(e) = self.socket.flush().await {
            return Err(e.into())
        }
        Ok(())
    }

    async fn close(&mut self)  {
        let _ = self.socket.shutdown().await;
    }
}

#[async_trait]
pub trait Connection: Send + Sync {
    async fn read_frame(&mut self) -> crate::Result<Frame>;
    async fn write_frame(&mut self, frame: Frame) -> crate::Result<()>;
    async fn close(&mut self);
}