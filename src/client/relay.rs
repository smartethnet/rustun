use crate::client::Args;
use crate::client::http::SelfInfo;
use crate::client::prettylog::log_handshake_success;
use crate::codec::frame::{Frame, HandshakeFrame, HandshakeReplyFrame, KeepAliveFrame};
use crate::crypto::Block;
use crate::network::{ConnManage, ConnectionConfig, TCPConnectionConfig, create_connection};
use crate::utils::{self, StunAddr};
use std::net::{Ipv6Addr, SocketAddr};
use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};

const CHANNEL_BUFFER_SIZE: usize = 1000;
const CONFIG_CHANNEL_SIZE: usize = 10;

#[derive(Clone)]
pub struct RelayClientConfig {
    pub server_addr: String,
    pub keepalive_interval: Duration,
    pub outbound_buffer_size: usize,
    pub keep_alive_thresh: u8,
    pub identity: String,
    pub ipv6: Option<Ipv6Addr>,
    pub port: u16,
    pub stun: Option<StunAddr>,
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

    pub async fn run(&mut self, mut conn: Box<dyn ConnManage>) -> anyhow::Result<()> {
        let mut keepalive_ticker = interval(self.cfg.keepalive_interval);
        let mut keepalive_wait: u8 = 0;

        // IPv6 update interval (check every 5 minutes)
        let mut ipv6_update_ticker = interval(Duration::from_secs(300));
        ipv6_update_ticker.tick().await; // Skip first immediate tick

        let mut current_ipv6: Option<Ipv6Addr> = self.cfg.ipv6;
        let stun = self.cfg.stun.clone();

        let mut last_active = Instant::now();
        let timeout_secs =
            (self.cfg.keep_alive_thresh - 1) as u64 * self.cfg.keepalive_interval.as_secs();
        loop {
            tokio::select! {
                _ = keepalive_ticker.tick() => {
                    if let ControlFlow::Break(_) = self
                        .keep_alive(
                            &mut conn,
                            &mut keepalive_wait,
                            current_ipv6.map(|ipv6| SocketAddr::new(ipv6.into(), self.cfg.port)),
                            stun.as_ref(),
                            last_active,
                            timeout_secs,
                        )
                        .await
                    {
                        break;
                    }
                }

                // Periodic IPv6 address update check
                _ = ipv6_update_ticker.tick() => {
                    tracing::debug!("ipv6 update tick");
                    if let Some(new_ipv6) = utils::get_ipv6().await {
                        let curr_display = match current_ipv6 {
                            None => "None".to_string(),
                            Some(ipv6) => ipv6.to_string(),
                        };
                        tracing::info!("IPv6 address updated: {curr_display} -> {new_ipv6}");
                        current_ipv6 = Some(new_ipv6);
                    } else {
                        tracing::debug!("Failed to retrieve IPv6 address during update check");
                    }
                    // TODO：get stun port
                }

                // inbound
                result = conn.read_frame() => {
                    if let ControlFlow::Break(_) = self.read_frame(&mut keepalive_wait, &mut last_active, result).await {
                        break;
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
                        tracing::error!("device => server write frame: {e}");
                    }
                    tracing::debug!("send to server cost {}", now.elapsed().as_millis());
                }
            }
        }

        tracing::debug!("client disconnected");
        let _ = conn.close().await;
        Ok(())
    }

    async fn read_frame(
        &mut self,
        keepalive_wait: &mut u8,
        last_active: &mut Instant,
        result: anyhow::Result<Frame>,
    ) -> ControlFlow<()> {
        *last_active = Instant::now();
        match result {
            Ok(frame) => {
                tracing::debug!("received frame {frame}");
                let beg = Instant::now();
                match frame {
                    Frame::KeepAlive(keepalive) => {
                        *keepalive_wait = keepalive_wait.saturating_sub(1);

                        tracing::debug!("Received keepalive from server");
                        if let Err(e) = self.inbound_tx.send(Frame::KeepAlive(keepalive)).await {
                            tracing::error!("Failed to forward keepalive: {e}");
                            return ControlFlow::Break(());
                        }
                    }
                    Frame::Data(data) => {
                        if let Err(e) = self.inbound_tx.send(Frame::Data(data)).await {
                            tracing::error!("server => device inbound: {e}");
                            return ControlFlow::Break(());
                        }
                    }
                    _ => {}
                }
                tracing::debug!("handle frame cost {}", beg.elapsed().as_millis());
            }
            Err(e) => {
                tracing::error!("Read error: {e}");
                return ControlFlow::Break(());
            }
        }
        ControlFlow::Continue(())
    }

    async fn keep_alive(
        &mut self,
        conn: &mut Box<dyn ConnManage + 'static>,
        keepalive_wait: &mut u8,
        current_ipv6: Option<SocketAddr>,
        stun: Option<&StunAddr>,
        last_active: Instant,
        timeout_secs: u64,
    ) -> ControlFlow<()> {
        if last_active.elapsed().as_secs() > timeout_secs {
            tracing::warn!("keepalive threshold {:?} exceeded", last_active.elapsed());
            return ControlFlow::Break(());
        }
        tracing::debug!("sending keepalive frame");
        let keepalive_frame = Frame::KeepAlive(KeepAliveFrame {
            name: "".to_string(),
            identity: self.cfg.identity.clone(),
            ipv6: current_ipv6
                .map(|ipv6| ipv6.ip().to_string())
                .unwrap_or_default(),
            port: current_ipv6.map(|ipv6| ipv6.port()).unwrap_or_default(),
            #[allow(clippy::unwrap_or_default)]
            stun_ip: stun
                .as_ref()
                .map(|stun| stun.ip.clone())
                .unwrap_or(String::new()),
            stun_port: stun.as_ref().map(|stun| stun.port).unwrap_or(0),
            peer_details: vec![], // Client doesn't need to send peer info
        });

        match conn.write_frame(keepalive_frame).await {
            Ok(_) => {
                *keepalive_wait = 0;
            }
            Err(e) => {
                tracing::error!("Failed to send keepalive: {e}");
                *keepalive_wait += 1;
                if *keepalive_wait > self.cfg.keep_alive_thresh {
                    tracing::error!("keepalive max retry, close connection");
                    return ControlFlow::Break(());
                }
            }
        }
        ControlFlow::Continue(())
    }

    async fn connect(&self) -> anyhow::Result<Box<dyn ConnManage>> {
        let conn = create_connection(
            ConnectionConfig::TCP(TCPConnectionConfig {
                server_addr: self.cfg.server_addr.clone(),
            }),
            self.block.clone(),
        )
        .await;
        match conn {
            Ok(conn) => Ok(conn),
            Err(e) => Err(e),
        }
    }

    async fn handshake(
        &self,
        conn: &mut Box<dyn ConnManage>,
    ) -> anyhow::Result<HandshakeReplyFrame> {
        conn.write_frame(Frame::Handshake(HandshakeFrame {
            identity: self.cfg.identity.clone(),
        }))
        .await?;

        let frame = conn.read_frame().await?;
        if let Frame::HandshakeReply(frame) = frame {
            return Ok(frame);
        }

        Err(anyhow::anyhow!("invalid frame"))
    }
}

#[derive(Clone, Debug, Default)]
pub struct RelayStatus {
    pub rx_error: u64,
    pub rx_frame: u64,
    pub tx_frame: u64,
    pub tx_error: u64,
}

pub struct RelayHandler {
    outbound_tx: Option<mpsc::Sender<Frame>>,
    inbound_rx: mpsc::Receiver<Frame>,
    inbound_tx: mpsc::Sender<Frame>,
    block: Arc<Box<dyn Block>>,
    metrics: RelayStatus,
    // Self information
    config: Option<RelayClientConfig>,
    handshake_reply: Arc<RwLock<Option<HandshakeReplyFrame>>>,
}

impl RelayHandler {
    pub fn new(block: Arc<Box<dyn Block>>) -> RelayHandler {
        let (inbound_tx, inbound_rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        RelayHandler {
            outbound_tx: None,
            inbound_rx,
            inbound_tx,
            block,
            metrics: Default::default(),
            config: None,
            handshake_reply: Arc::new(RwLock::new(None)),
        }
    }

    /// Get self information
    pub async fn get_self_info(&self) -> Option<SelfInfo> {
        let reply_guard = self.handshake_reply.read().unwrap();
        match (&self.config, reply_guard.as_ref()) {
            (Some(cfg), Some(reply)) => Some(SelfInfo {
                identity: cfg.identity.clone(),
                private_ip: reply.private_ip.clone(),
                mask: reply.mask.clone(),
                gateway: reply.gateway.clone(),
                ciders: reply.ciders.clone(),
                ipv6: cfg.ipv6.map(|ipv6| ipv6.to_string()).unwrap_or_default(),
                port: cfg.port,
                stun_ip: cfg
                    .stun
                    .as_ref()
                    .map(|stun| stun.ip.clone())
                    .unwrap_or(String::new()),
                stun_port: cfg.stun.as_ref().map(|stun| stun.port).unwrap_or(0),
            }),
            _ => None,
        }
    }

    pub fn run_client(
        &mut self,
        cfg: RelayClientConfig,
        on_ready: mpsc::Sender<HandshakeReplyFrame>,
    ) {
        // Store config
        self.config = Some(cfg.clone());

        let (outbound_tx, outbound_rx) = mpsc::channel(cfg.outbound_buffer_size);
        let mut client = RelayClient::new(
            cfg.clone(),
            outbound_rx,
            self.inbound_tx.clone(),
            self.block.clone(),
        );
        self.outbound_tx = Some(outbound_tx);

        // Store handshake reply when received
        let handshake_reply = self.handshake_reply.clone();

        tokio::spawn(async move {
            loop {
                run_client_session(&on_ready, &mut client, &handshake_reply).await;
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }

    pub fn get_outbound_tx(&self) -> Option<mpsc::Sender<Frame>> {
        self.outbound_tx.clone()
    }

    pub async fn send_frame(outbound_tx: mpsc::Sender<Frame>, frame: Frame) -> anyhow::Result<()> {
        // self.metrics.tx_frame += 1;
        let result = outbound_tx.send(frame).await;
        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                // self.metrics.tx_error += 1;
                Err(anyhow::anyhow!("device=> server fail {e:?}"))
            }
        }
    }

    pub async fn recv_frame(&mut self) -> anyhow::Result<Frame> {
        let result = self.inbound_rx.recv().await;
        match result {
            Some(frame) => {
                self.metrics.rx_frame += 1;
                Ok(frame)
            }
            None => {
                self.metrics.rx_error += 1;
                Err(anyhow::anyhow!("server => device fail for closed channel"))
            }
        }
    }

    pub fn get_status(&self) -> RelayStatus {
        self.metrics.clone()
    }
}

async fn run_client_session(
    on_ready: &mpsc::Sender<HandshakeReplyFrame>,
    client: &mut RelayClient,
    handshake_reply: &Arc<RwLock<Option<HandshakeReplyFrame>>>,
) {
    let mut conn = match client.connect().await {
        Ok(socket) => socket,
        Err(e) => {
            tracing::error!("connect error: {e}");
            return;
        }
    };

    let frame = match client.handshake(&mut conn).await {
        Ok(frame) => frame,
        Err(e) => {
            tracing::warn!("handshake fail {e:?}, reconnecting");
            return;
        }
    };

    tracing::info!("Handshake complete with {} peers", frame.peer_details.len());

    // Store handshake reply in handler
    {
        let mut guard = handshake_reply.write().unwrap();
        *guard = Some(frame.clone());
    }

    if let Err(e) = on_ready.send(frame.clone()).await {
        tracing::error!("on ready send fail: {e}");
    }

    let result = client.run(conn).await;

    tracing::warn!("run client fail {result:?}, reconnecting");
}

pub async fn new_relay_handler(
    args: &Args,
    block: Arc<Box<dyn Block>>,
    ipv6: Option<Ipv6Addr>,
    port: u16,
    stun: Option<StunAddr>,
) -> anyhow::Result<(RelayHandler, HandshakeReplyFrame)> {
    let client_config = RelayClientConfig {
        server_addr: args.server.clone(),
        keepalive_interval: Duration::from_secs(args.keepalive_interval),
        outbound_buffer_size: CHANNEL_BUFFER_SIZE,
        keep_alive_thresh: args.keepalive_threshold,
        identity: args.identity.clone(),
        ipv6,
        port,
        stun,
    };

    let mut handler = RelayHandler::new(block);
    let (config_ready_tx, mut config_ready_rx) = mpsc::channel(CONFIG_CHANNEL_SIZE);
    handler.run_client(client_config, config_ready_tx);

    let device_config = config_ready_rx
        .recv()
        .await
        .ok_or_else(|| anyhow::anyhow!("Failed to receive device config from server"))?;

    log_handshake_success(&device_config);

    Ok((handler, device_config))
}
