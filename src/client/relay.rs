use crate::client::P2P_UDP_PORT;
use crate::client::Args;
use crate::client::prettylog::log_handshake_success;
use crate::codec::frame::{Frame, HandshakeFrame, HandshakeReplyFrame, KeepAliveFrame};
use crate::crypto::Block;
use crate::network::{create_connection, Connection, ConnectionConfig, TCPConnectionConfig};
use crate::utils;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};

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
    pub port: u16,
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
        
        // IPv6 update interval (check every 5 minutes)
        let mut ipv6_update_ticker = interval(Duration::from_secs(300));
        ipv6_update_ticker.tick().await; // Skip first immediate tick
        
        let mut current_ipv6 = self.cfg.ipv6.clone();
        
        loop {
            tokio::select! {
                _ = keepalive_ticker.tick() => {
                    let keepalive_frame = Frame::KeepAlive(KeepAliveFrame {
                        identity: self.cfg.identity.clone(),
                        ipv6: current_ipv6.clone(),
                        port: self.cfg.port,
                    });
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
                
                // Periodic IPv6 address update check
                _ = ipv6_update_ticker.tick() => {
                    if let Some(new_ipv6) = utils::get_ipv6() {
                        tracing::info!("IPv6 address updated: {} -> {}", current_ipv6, new_ipv6);
                        current_ipv6 = new_ipv6.clone();
                        self.cfg.ipv6 = new_ipv6;
                    } else {
                        tracing::debug!("Failed to retrieve IPv6 address during update check");
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
                                    keepalive_wait = keepalive_wait.saturating_sub(1);
                                }
                                Frame::Data(data) => {
                                    if let Err(e) =  self.inbound_tx.send(Frame::Data(data)).await {
                                        tracing::error!("server => device inbound: {}", e);
                                        break;
                                    }
                                }
                                Frame::PeerUpdate(data) => {
                                    if let Err(e) =  self.inbound_tx.send(Frame::PeerUpdate(data)).await {
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
            Err(e) => Err(e)
        }
    }

    async fn handshake(&self, conn: &mut Box<dyn Connection>) -> crate::Result<HandshakeReplyFrame> {
        conn.write_frame(Frame::Handshake(HandshakeFrame {
            identity: self.cfg.identity.clone(),
            ipv6: self.cfg.ipv6.clone(),
            port: self.cfg.port,
        }))
        .await?;

        let frame = conn.read_frame().await?;
        if let Frame::HandshakeReply(frame) = frame {
            return Ok(frame);
        }

        Err("invalid frame".into())
    }
}

#[derive(Clone)]
#[derive(Debug)]
pub struct RelayStatus {
    pub rx_error: u64,
    pub rx_frame: u64,
    pub tx_frame: u64,
    pub tx_error: u64,
}

impl Default for RelayStatus {
    fn default() -> Self {
        Self{
            rx_error: 0,
            tx_error: 0,
            rx_frame: 0,
            tx_frame: 0,
        }
    }
}

pub struct RelayHandler {
    outbound_tx: Option<mpsc::Sender<Frame>>,
    inbound_rx: mpsc::Receiver<Frame>,
    inbound_tx: mpsc::Sender<Frame>,
    block: Arc<Box<dyn Block>>,
    metrics: RelayStatus,
}

impl RelayHandler {
    pub fn new(block: Arc<Box<dyn Block>>) -> RelayHandler {
        let (inbound_tx, inbound_rx) = mpsc::channel(10);
        RelayHandler {
            outbound_tx: None,
            inbound_rx,
            inbound_tx,
            block,
            metrics: Default::default(),
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
        self.metrics.tx_frame += 1;
        let outbound_tx = match self.outbound_tx.clone() {
            Some(tx) => tx,
            None => {
                self.metrics.tx_error += 1;
                return Err("relay connection disconnect".into())}
        };

        let result = outbound_tx.send(frame).await;
        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                self.metrics.tx_error += 1;
                Err(format!("device=> server fail {:?}", e).into())
            },
        }
    }

    pub async fn recv_frame(&mut self) -> crate::Result<Frame> {
        let result = self.inbound_rx.recv().await;
        match result {
            Some(frame) => {
                self.metrics.rx_frame += 1;
                Ok(frame)
            },
            None => {
                self.metrics.rx_error += 1;
                Err("server => device fail for closed channel".into())
            },
        }
    }

    pub fn get_status(&self) -> RelayStatus {
        self.metrics.clone()
    }
}

pub async fn new_relay_handler(args: &Args, block: Arc<Box<dyn Block>>)->crate::Result<(RelayHandler, HandshakeReplyFrame)> {
    let ipv6 = utils::get_ipv6().unwrap_or("".to_string());
    let client_config = RelayClientConfig {
        server_addr: args.server.clone(),
        keepalive_interval: Duration::from_secs(args.keepalive_interval),
        outbound_buffer_size: OUTBOUND_BUFFER_SIZE,
        keep_alive_thresh: args.keepalive_threshold,
        identity: args.identity.clone(),
        ipv6,
        port: P2P_UDP_PORT,
    };

    let mut handler = RelayHandler::new(block);
    let (config_ready_tx, mut config_ready_rx) = mpsc::channel(CONFIG_CHANNEL_SIZE);
    handler.run_client(client_config, config_ready_tx);

    let device_config = config_ready_rx
        .recv()
        .await
        .ok_or("Failed to receive device config from server")?;

    log_handshake_success(&device_config);

    Ok((handler, device_config))
}