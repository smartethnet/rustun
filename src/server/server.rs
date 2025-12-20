use crate::codec::frame::Frame::HandshakeReply;
use crate::codec::frame::{Frame, HandshakeFrame, HandshakeReplyFrame, RouteItem};
use crate::crypto::Block;
use crate::network::connection_manager::ConnectionManager;
use crate::network::{Connection, ListenerConfig, create_listener};
use crate::network::{ConnectionMeta, TCPListenerConfig};
use crate::server::client_manager::ClientManager;
use crate::server::config::ServerConfig;
use std::sync::Arc;
use tokio::sync::mpsc;

const OUTBOUND_BUFFER_SIZE: usize = 1000;

pub struct Server {
    server_config: ServerConfig,
    connection_manager: Arc<ConnectionManager>,
    client_manager: Arc<ClientManager>,
    block: Arc<Box<dyn Block>>,
}

impl Server {
    pub fn new(
        server_config: ServerConfig,
        client_manager: Arc<ClientManager>,
        block: Arc<Box<dyn Block>>,
    ) -> Self {
        Server {
            server_config,
            connection_manager: Arc::new(ConnectionManager::new()),
            client_manager,
            block,
        }
    }
}

impl Server {
    pub async fn run(&mut self) -> crate::Result<()> {
        // only for tcp now, may support multi listener type
        let listener_config = ListenerConfig::TCP(TCPListenerConfig {
            listen_addr: self.server_config.listen_addr.clone(),
        });
        let listener = create_listener(listener_config, self.block.clone());

        let mut listener = match listener {
            Ok(listener) => listener,
            Err(err) => {
                return Err(err.into());
            }
        };

        let mut on_conn_rx = listener.subscribe_on_conn().await?;
        tokio::spawn(async move {
            let err = listener.listen_and_serve().await;
            if err.is_err() {
                tracing::error!("Server listening error: {:?}", err);
            }
        });

        loop {
            tokio::select! {
                conn = on_conn_rx.recv() => {
                    if let Some(conn) = conn {
                        let _ = self.handle_conn(conn);
                    }
                }
            }
        }
    }

    fn handle_conn(&self, mut conn: Box<dyn Connection>) -> crate::Result<()> {
        let peer_addr = conn.peer_addr().unwrap();
        tracing::debug!("new connection from {}", conn.peer_addr().unwrap());

        let mut handler = Handler::new(
            self.connection_manager.clone(),
            self.client_manager.clone(),
            conn,
        );
        tokio::task::spawn(async move {
            let e = handler.run().await;
            tracing::debug!("client {:?} handler stop with {:?}", peer_addr, e);
        });
        Ok(())
    }
}

pub struct Handler {
    connection_manager: Arc<ConnectionManager>,
    client_manager: Arc<ClientManager>,
    conn: Box<dyn Connection>,
    outbound_tx: mpsc::Sender<Frame>,
    outbound_rx: mpsc::Receiver<Frame>,
    cluster: Option<String>,
}

impl Handler {
    pub fn new(
        connection_manager: Arc<ConnectionManager>,
        client_manager: Arc<ClientManager>,
        conn: Box<dyn Connection>,
    ) -> Handler {
        let (tx, rx) = mpsc::channel(OUTBOUND_BUFFER_SIZE);
        Self {
            connection_manager,
            client_manager,
            conn,
            outbound_rx: rx,
            outbound_tx: tx,
            cluster: None,
        }
    }

    pub async fn run(&mut self) -> crate::Result<()> {
        // handshake
        let hs = match self.handle_handshake().await {
            Ok(hs) => hs,
            Err(e) => return Err(e),
        };

        // validate client identity
        let client_config = match self.client_manager.get_client(&hs.identity) {
            Some(c) => c,
            None => {
                tracing::debug!("{} unauthorized", hs.identity);
                return Ok(());
            }
        };

        // reply handshake with other clients info
        let others = self
            .client_manager
            .get_cluster_clients_exclude(&hs.identity);
        let route_items: Vec<RouteItem> = others
            .iter()
            .map(|client| RouteItem {
                identity: client.identity.clone(),
                private_ip: client.private_ip.clone(),
                ciders: client.ciders.clone(),
            })
            .collect();

        self.conn
            .write_frame(HandshakeReply(HandshakeReplyFrame {
                private_ip: client_config.private_ip.clone(),
                mask: client_config.mask.clone(),
                gateway: client_config.gateway.clone(),
                others: route_items,
                ipv6: hs.ipv6,
            }))
            .await?;

        let meta = ConnectionMeta {
            cluster: client_config.cluster.clone(),
            identity: client_config.identity.clone(),
            private_ip: client_config.private_ip.clone(),
            mask: client_config.mask.clone(),
            gateway: client_config.gateway.clone(),
            ciders: client_config.ciders.clone(),
            outbound_tx: self.outbound_tx.clone(),
        };
        tracing::debug!("handshake completed with {:?}", meta);

        // Store cluster for routing
        self.cluster = Some(client_config.cluster.clone());
        self.connection_manager.add_connection(meta);

        loop {
            tokio::select! {
                // read frame
                result = self.conn.read_frame() => {
                    match result {
                        Ok(frame) => {
                            tracing::debug!("received frame: {}", frame);
                            self.handle_frame(frame).await;
                        }
                        Err(e) => {
                            tracing::error!("read {} failed: {:?}", hs.identity, e);
                            break;
                        }
                    }
                }

                // write frame
                frame = self.outbound_rx.recv() => {
                    if let Some(frame) = frame {
                        tracing::debug!("send frame {}", frame);
                        if let Err(e) = self.conn.write_frame(frame).await {
                            tracing::debug!("connection closed with {:?}", e);
                            break;
                        };
                    }
                }
            }
        }

        tracing::debug!("delete client {}", hs.identity);
        self.connection_manager.del_connection(hs.identity);
        Ok(())
    }

    async fn handle_handshake(&mut self) -> crate::Result<HandshakeFrame> {
        let frame = self.conn.read_frame().await;
        match frame {
            Ok(frame) => {
                tracing::debug!("handshake: {}", frame);
                if let Frame::Handshake(handshake) = frame {
                    Ok(handshake)
                } else {
                    Err("unexpected frame type when handshaking".into())
                }
            }
            Err(e) => Err(e),
        }
    }

    async fn handle_frame(&mut self, frame: Frame) {
        match frame {
            Frame::KeepAlive(frame) => {
                tracing::debug!("on keepalive");
                if let Err(e) = self.outbound_tx.send(Frame::KeepAlive(frame)).await {
                    tracing::error!("reply keepalive frame failed with {:?}", e);
                }
            }

            Frame::Data(frame) => {
                if frame.invalid() {
                    tracing::warn!("receive invalid ip packet");
                    return;
                }

                if frame.version() != 4 {
                    tracing::warn!("receive invalid ipv4 packet");
                    return;
                }
                tracing::debug!("on data: {} => {}", frame.src(), frame.dst());

                // route within cluster (tenant isolation)
                let dst_ip = frame.dst();
                let cluster = match &self.cluster {
                    Some(c) => c,
                    None => {
                        tracing::error!("cluster not set");
                        return;
                    }
                };

                let dst_client = self.connection_manager.get_connection(cluster, &dst_ip);
                if let Some(dst_client) = dst_client {
                    let result = dst_client.outbound_tx.send(Frame::Data(frame)).await;
                    if result.is_err() {
                        tracing::warn!("dst client {} not online", dst_ip);
                    }
                } else {
                    tracing::warn!("no route to {} in cluster {}", dst_ip, cluster);
                }
            }
            _ => {
                tracing::warn!("unknown frame: {:?}", frame);
            }
        }
    }
}
