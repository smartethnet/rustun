use crate::codec::frame::{Frame, HandshakeFrame, HandshakeReplyFrame, KeepAliveFrame};
use crate::crypto::Block;
use crate::network::{create_connection, Connection, ConnectionConfig, TCPConnectionConfig};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};
use crate::client::main::Args;
use crate::client::prettylog::log_handshake_success;
use crate::utils;

const OUTBOUND_BUFFER_SIZE: usize = 1000;
const CONFIG_CHANNEL_SIZE: usize = 10;

#[derive(Clone)]
pub struct RelayClientConfig {
    pub server_addr: String,
    pub keepalive_interval: Duration,
    pub outbound_buffer_size: usize,
    pub keep_alive_thresh: u8,
    pub identity: String,
    pub ipv6: String,
}

pub struct RelayClient {
    cfg: RelayClientConfig,
    outbound_rx: mpsc::Receiver<Frame>,
    inbound_tx: mpsc::Sender<Frame>,
    block: Arc<Box<dyn Block>>,
}

impl RelayClient {
    pub fn new(
        cfg: RelayClientConfig,
        outbound_rx: mpsc::Receiver<Frame>,
        inbound_tx: mpsc::Sender<Frame>,
        block: Arc<Box<dyn Block>>,
    ) -> Self {
        Self {
            cfg,
            outbound_rx,
            inbound_tx,
            block,
        }
    }

    pub async fn run(&mut self, mut conn: Box<dyn Connection>) -> crate::Result<()> {
        let mut keepalive_ticker = interval(self.cfg.keepalive_interval);
        let mut keepalive_wait: u8 = 0;
        loop {
            tokio::select! {
                _ = keepalive_ticker.tick() => {
                    let keepalive_frame = Frame::KeepAlive(KeepAliveFrame {});
                    match conn.write_frame(keepalive_frame).await {
                        Ok(_) => {
                            keepalive_wait = 0;
                        }
                        Err(e) => {
                            tracing::error!("Failed to send keepalive: {}", e);
                            keepalive_wait+=1;
                            if keepalive_wait > self.cfg.keep_alive_thresh {
                                tracing::error!("keepalive max retry, close connection");
                                break;
                            }
                        }
                    }
                }
                // inbound
                result = conn.read_frame() => {
                    match result {
                        Ok(frame) => {
                            tracing::debug!("received frame {}", frame);
                            let beg = Instant::now();
                            match frame {
                                Frame::KeepAlive(_) => {
                                    if keepalive_wait > 0 {
                                        keepalive_wait-=1;
                                    }
                                }
                                Frame::Data(data) => {
                                    if let Err(e) =  self.inbound_tx.send(Frame::Data(data)).await {
                                        tracing::error!("server => device inbound: {}", e);
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            tracing::debug!("handle frame cost {}", beg.elapsed().as_millis());
                        }
                        Err(e) => {
                            tracing::error!("Read error: {}", e);
                            break;
                        }
                    }
                }
                // outbound
                frame = self.outbound_rx.recv() => {
                    if frame.is_none() {
                        tracing::error!("device => server outbound closed");
                        break;
                    }

                    let now = Instant::now();
                    if let Err(e) = conn.write_frame(frame.unwrap()).await {
                        tracing::error!("device => server write frame: {}", e);
                    }
                    tracing::debug!("send to server cost {}", now.elapsed().as_millis());
                }
            }
        }

        tracing::debug!("client disconnected");
        let _ = conn.close().await;
        Ok(())
    }

    async fn connect(&self) -> crate::Result<Box<dyn Connection>> {
        let conn = create_connection(ConnectionConfig::TCP(TCPConnectionConfig {
            server_addr: self.cfg.server_addr.clone(),
        }), self.block.clone()).await;
        match conn {
            Ok(conn) => Ok(conn),
            Err(e) => Err(e.into())
        }
    }

    async fn handshake(&self, conn: &mut Box<dyn Connection>) -> crate::Result<HandshakeReplyFrame> {
        conn.write_frame(Frame::Handshake(HandshakeFrame {
            identity: self.cfg.identity.clone(),
            ipv6: self.cfg.ipv6.clone(),
            port: 51258,
        }))
        .await?;

        let frame = conn.read_frame().await?;
        if let Frame::HandshakeReply(frame) = frame {
            return Ok(frame);
        }

        Err("invalid frame".into())
    }
}

pub struct ClientHandler {
    outbound_tx: Option<mpsc::Sender<Frame>>,
    inbound_rx: mpsc::Receiver<Frame>,
    inbound_tx: mpsc::Sender<Frame>,
    block: Arc<Box<dyn Block>>,
}

impl ClientHandler {
    pub fn new(block: Arc<Box<dyn Block>>) -> ClientHandler {
        let (inbound_tx, inbound_rx) = mpsc::channel(10);
        ClientHandler {
            outbound_tx: None,
            inbound_rx,
            inbound_tx,
            block,
        }
    }

    pub fn run_client(&mut self, cfg: RelayClientConfig,
                      on_ready: mpsc::Sender<HandshakeReplyFrame>) {
        let (outbound_tx, outbound_rx) = mpsc::channel(cfg.outbound_buffer_size);
        let mut client = RelayClient::new(
            cfg.clone(),
            outbound_rx,
            self.inbound_tx.clone(),
            self.block.clone(),
        );
        self.outbound_tx = Some(outbound_tx);

        tokio::spawn(async move {
            loop {
                let mut conn = match client.connect().await {
                    Ok(socket) => socket,
                    Err(e) => {
                        tracing::error!("connect error: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                };

                let frame = match client.handshake(&mut conn).await {
                    Ok(frame) => frame,
                    Err(e) => {
                        tracing::warn!("handshake fail {:?}, reconnecting", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                };
                if let Err(e) = on_ready.send(frame.clone()).await {
                    tracing::error!("on ready send fail: {}", e);
                }

                let result = client.run(conn).await;

                tracing::warn!("run client fail {:?}, reconnecting", result);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }

    pub async fn send_frame(&mut self, frame: Frame) -> crate::Result<()> {
        let outbound_tx = match self.outbound_tx.clone() {
            Some(tx) => tx,
            None => {return Err("relay connection disconnect".into())}
        };

        let result = outbound_tx.send(frame).await;
        match result {
            Ok(()) => Ok(()),
            Err(e) => Err(format!("device=> server fail {:?}", e).into()),
        }
    }

    pub async fn recv_frame(&mut self) -> crate::Result<Frame> {
        let result = self.inbound_rx.recv().await;
        match result {
            Some(frame) => Ok(frame),
            None => Err("server => device fail for closed channel".into()),
        }
    }
}

pub async fn new_relay_handler(args: &Args, block: Arc<Box<dyn Block>>)->crate::Result<(ClientHandler, HandshakeReplyFrame)> {
    let ipv6 = utils::get_ipv6().unwrap_or("".to_string());
    let client_config = RelayClientConfig {
        server_addr: args.server.clone(),
        keepalive_interval: Duration::from_secs(args.keepalive_interval),
        outbound_buffer_size: OUTBOUND_BUFFER_SIZE,
        keep_alive_thresh: args.keepalive_threshold,
        identity: args.identity.clone(),
        ipv6,
    };

    let mut handler = ClientHandler::new(block);
    let (config_ready_tx, mut config_ready_rx) = mpsc::channel(CONFIG_CHANNEL_SIZE);
    handler.run_client(client_config, config_ready_tx);

    let device_config = config_ready_rx
        .recv()
        .await
        .ok_or("Failed to receive device config from server")?;

    log_handshake_success(&device_config);

    Ok((handler, device_config))
}