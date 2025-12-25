use crate::crypto::Block;
use crate::network::tcp_connection::TcpConnection;
use crate::network::{Connection, Listener};
use async_trait::async_trait;
use std::io::ErrorKind;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;

/// Default queue size for new connection channel
const DEFAULT_ON_CONNECTION_QUEUE: usize = 1024;

/// TCP listener implementation
///
/// Handles TCP connection acceptance with exponential backoff retry logic.
pub struct TCPListener {
    /// Address to bind to
    addr: String,
    /// Underlying tokio TCP listener
    listener: Option<TcpListener>,
    /// Channel sender for broadcasting new connections
    on_conn_tx: Option<mpsc::Sender<Box<dyn Connection>>>,
    /// Crypto Block
    block: Arc<Box<dyn Block>>,
}

impl TCPListener {
    /// Create a new TCP listener
    ///
    /// # Arguments
    /// - `addr` - Address to bind (e.g., "0.0.0.0:8080")
    /// - `block` - Crypto block
    pub fn new(addr: String, block: Arc<Box<dyn Block>>) -> Self {
        TCPListener {
            addr,
            listener: None,
            on_conn_tx: None,
            block,
        }
    }

    /// Accept a new TCP connection with exponential backoff
    ///
    /// Retries on transient errors with backoff starting at 1s, doubling
    /// up to 64s before giving up. Only retries on temporary errors like
    /// too many open files.
    ///
    /// # Returns
    /// - `Ok(TcpStream)` - Accepted connection
    /// - `Err` - Fatal accept error or retries exhausted
    async fn accept(&mut self) -> crate::Result<TcpStream> {
        let listener = self.listener.as_ref().ok_or_else(|| {
            std::io::Error::new(ErrorKind::NotConnected, "listener not initialized")
        })?;

        let mut backoff = 1;

        loop {
            match listener.accept().await {
                Ok((socket, _)) => return Ok(socket),
                Err(err) => {
                    // Only retry on transient errors
                    match err.kind() {
                        ErrorKind::ConnectionAborted
                        | ErrorKind::ConnectionReset
                        | ErrorKind::WouldBlock => {
                            if backoff > 64 {
                                tracing::error!("Accept retry exhausted: {}", err);
                                return Err(err.into());
                            }
                            tracing::warn!("Accept failed, retrying in {}s: {}", backoff, err);
                            tokio::time::sleep(Duration::from_secs(backoff)).await;
                            backoff *= 2;
                        }
                        _ => {
                            tracing::error!("Fatal accept error: {}", err);
                            return Err(err.into());
                        }
                    }
                }
            }
        }
    }
}

#[async_trait]
impl Listener for TCPListener {
    /// Bind to address and start accepting connections
    ///
    /// Runs in a loop, accepting connections and sending them to subscribers
    /// via the channel. Continues accepting even if sending fails.
    async fn listen_and_serve(&mut self) -> crate::Result<()> {
        let listener = TcpListener::bind(self.addr.clone()).await?;
        tracing::info!("Server listening on {}", self.addr);
        self.listener = Some(listener);

        loop {
            tokio::select! {
                socket = self.accept() => {
                    match socket {
                        Ok(socket) => {
                            let conn = TcpConnection::new(socket, self.block.clone());
                            if let Some(tx) = &self.on_conn_tx
                                && let Err(e) = tx.send(Box::new(conn)).await {
                                tracing::warn!("Failed to send new connection: {}", e);
                            }
                        },
                        Err(e) => {
                            tracing::error!("Accept error: {}", e);
                            return Err(e);
                        }
                    };
                }
            }
        }
    }

    /// Create a channel for receiving new connections
    ///
    /// # Returns
    /// - `Ok(Receiver)` - Channel receiver for new connections
    async fn subscribe_on_conn(&mut self) -> crate::Result<Receiver<Box<dyn Connection>>> {
        let (tx, rx) = mpsc::channel::<Box<dyn Connection>>(DEFAULT_ON_CONNECTION_QUEUE);
        self.on_conn_tx = Some(tx);
        Ok(rx)
    }

    /// Close the listener and clean up resources
    async fn close(&mut self) -> crate::Result<()> {
        if let Some(listener) = self.listener.take() {
            drop(listener);
            tracing::info!("TCP listener closed");
        }
        self.on_conn_tx = None;
        Ok(())
    }
}
