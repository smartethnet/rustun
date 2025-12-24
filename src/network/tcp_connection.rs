use crate::codec::frame::Frame;
use crate::codec::parser::Parser;
use crate::crypto::Block;
use crate::crypto::plain::PlainBlock;
use crate::network::Connection;
use async_trait::async_trait;
use bytes::{Buf, BytesMut};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Default timeout for read operations
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
/// Default timeout for write operations
const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_secs(3);

/// TCP connection wrapper with frame parsing and encryption
///
/// Handles reading/writing frames over TCP with buffering and encryption.
pub struct TcpConnection {
    /// Underlying TCP socket
    socket: TcpStream,
    /// Write operation timeout
    #[allow(dead_code)]
    write_timeout: Duration,
    /// Read operation timeout
    #[allow(dead_code)]
    read_timeout: Duration,
    /// Input buffer for incomplete frames
    input_stream: BytesMut,
    /// Crypto block for encryption/decryption
    block: Arc<Box<dyn Block>>,
}

impl TcpConnection {
    /// Create a new TCP connection with encryption
    ///
    /// # Arguments
    /// - `socket` - Established TCP stream
    /// - `block` - Crypto block for encryption/decryption
    pub fn new(socket: TcpStream, block: Arc<Box<dyn Block>>) -> Self {
        Self {
            socket,
            write_timeout: DEFAULT_WRITE_TIMEOUT,
            read_timeout: DEFAULT_READ_TIMEOUT,
            input_stream: BytesMut::with_capacity(4096),
            block,
        }
    }

    /// Create a TCP connection from socket with no encryption
    ///
    /// Uses PlainBlock for passthrough mode (no encryption).
    ///
    /// # Arguments
    /// - `socket` - Established TCP stream
    pub fn from_socket(socket: TcpStream) -> Self {
        Self {
            socket,
            write_timeout: DEFAULT_WRITE_TIMEOUT,
            read_timeout: DEFAULT_READ_TIMEOUT,
            input_stream: BytesMut::with_capacity(4096),
            block: Arc::new(Box::new(PlainBlock::new())),
        }
    }

    /// Parse a complete frame from the input buffer
    ///
    /// Attempts to parse a frame from buffered data. If successful,
    /// advances the buffer by the consumed bytes.
    ///
    /// # Returns
    /// - `Ok(Some(Frame))` - Successfully parsed frame
    /// - `Ok(None)` - Incomplete data, need more bytes
    /// - `Err` - Parse error (invalid frame format)
    fn parse_frame(&mut self) -> crate::Result<Option<Frame>> {
        let result = Parser::unmarshal(self.input_stream.as_ref(), self.block.as_ref());
        match result {
            Ok((frame, total_len)) => {
                self.input_stream.advance(total_len);
                Ok(Some(frame))
            }
            Err(e) => Err(e.into()),
        }
    }
}

#[async_trait]
impl Connection for TcpConnection {
    /// Read a complete frame from the connection
    ///
    /// Reads data from the socket into a buffer and attempts to parse
    /// complete frames. Blocks until a frame is available or error occurs.
    ///
    /// # Returns
    /// - `Ok(Frame)` - Successfully received frame
    /// - `Err` - Connection error, EOF, or parse error
    async fn read_frame(&mut self) -> crate::Result<Frame> {
        loop {
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
                };
            }
        }
    }

    /// Write a frame to the connection
    ///
    /// Marshals the frame with encryption and sends it over the socket.
    ///
    /// # Arguments
    /// - `frame` - Frame to send
    ///
    /// # Returns
    /// - `Ok(())` - Frame sent successfully
    /// - `Err` - Marshal error or write error
    async fn write_frame(&mut self, frame: Frame) -> crate::Result<()> {
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
            return Err(e.into());
        }
        Ok(())
    }

    /// Close the connection gracefully
    async fn close(&mut self) {
        let _ = self.socket.shutdown().await;
    }

    /// Get the peer's socket address
    fn peer_addr(&mut self) -> io::Result<SocketAddr> {
        self.socket.peer_addr()
    }
}
