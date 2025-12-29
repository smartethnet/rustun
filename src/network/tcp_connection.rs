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
use tokio::time::timeout;

/// Default timeout for read operations
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(20);
/// Default timeout for write operations
const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// TCP connection wrapper with frame parsing and encryption
///
/// Handles reading/writing frames over TCP with buffering and encryption.
pub struct TcpConnection {
    /// Underlying TCP socket
    socket: TcpStream,
    /// Write operation timeout
    write_timeout: Duration,
    /// Read operation timeout
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

    /// Set read timeout duration
    ///
    /// # Arguments
    /// - `timeout` - Duration for read operations
    pub fn set_read_timeout(&mut self, timeout: Duration) {
        self.read_timeout = timeout;
    }

    /// Set write timeout duration
    ///
    /// # Arguments
    /// - `timeout` - Duration for write operations
    pub fn set_write_timeout(&mut self, timeout: Duration) {
        self.write_timeout = timeout;
    }

    /// Get current read timeout
    pub fn read_timeout(&self) -> Duration {
        self.read_timeout
    }

    /// Get current write timeout
    pub fn write_timeout(&self) -> Duration {
        self.write_timeout
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
            Err(e) => Err(e),
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
    /// - `Err` - Connection error, EOF, parse error, or timeout
    async fn read_frame(&mut self) -> crate::Result<Frame> {
        loop {
            if let Ok(frame) = self.parse_frame() {
                if let Some(frame) = frame {
                    return Ok(frame);
                }
            }

            // Read with timeout
            let read_result = timeout(
                self.read_timeout,
                self.socket.read_buf(&mut self.input_stream)
            ).await;

            match read_result {
                Ok(Ok(0)) => {
                    return if self.input_stream.is_empty() {
                        Err("EOF".into())
                    } else {
                        Err("connection reset by peer".into())
                    };
                }
                Ok(Ok(_n)) => {
                    // Successfully read n bytes, continue loop to parse
                }
                Ok(Err(e)) => {
                    return Err(e.into());
                }
                Err(_) => {
                    return Err("read timeout".into());
                }
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
    /// - `Err` - Marshal error, write error, or timeout
    async fn write_frame(&mut self, frame: Frame) -> crate::Result<()> {
        let result = Parser::marshal(frame, self.block.as_ref());
        let buf = match result {
            Ok(buf) => buf,
            Err(e) => {
                return Err(e);
            }
        };

        // Write with timeout
        let write_result = timeout(
            self.write_timeout,
            async {
                self.socket.write_all(buf.as_slice()).await?;
                self.socket.flush().await?;
                Ok::<(), std::io::Error>(())
            }
        ).await;

        match write_result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e.into()),
            Err(_) => Err("write timeout".into()),
        }
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
