use crate::crypto::Block;
use std::sync::Arc;
use std::time::{Instant};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use crate::codec::frame::{Frame, HandshakeFrame};
use crate::server::connection::{Connection, ConnectionMeta, TcpConnection};
use crate::server::route::RouteManager;

const OUTBOUND_BUFFER_SIZE: usize = 1000;

pub struct ConnectionHandler {
    route_manager: Arc<RouteManager>,
    conn: Box<dyn Connection>,
    outbound_tx: mpsc::Sender<Frame>,
    outbound_rx: mpsc::Receiver<Frame>,
}

impl ConnectionHandler {
    pub fn new(route_manager: Arc<RouteManager>,
               tcp_stream: TcpStream,
                block: Arc<Box<dyn Block>>) -> ConnectionHandler {
        let (tx, rx) = mpsc::channel(OUTBOUND_BUFFER_SIZE);
        let conn = TcpConnection::new(tcp_stream, block);
        Self{
            route_manager,
            conn: Box::new(conn),
            outbound_rx: rx,
            outbound_tx: tx,
        }
    }

    pub async fn run(&mut self) -> crate::Result<()> {
        // handshake
        let meta = match self.handle_handshake().await {
            Ok(meta) => meta,
            Err(e) => return Err(e),
        };

        let meta = ConnectionMeta {
            private_ip: meta.private_ip,
            ciders: meta.ciders,
            outbound_tx: self.outbound_tx.clone(),
        };
        tracing::debug!("handshake completed with {:?}", meta);

        self.route_manager.add_route(meta.clone());

        loop {
            tokio::select! {
                // read frame
                result = self.conn.read_frame() => {
                    match result {
                        Ok(frame) => {
                            tracing::info!("received frame: {}", frame);
                            let beg = Instant::now();
                            self.handle_frame(frame).await;
                            tracing::info!("handle frame cost {}", beg.elapsed().as_millis());
                        }
                        Err(e) => {
                            tracing::error!("read from client {} failed: {:?}", meta.private_ip, e);
                            break;
                        }
                    }
                }

                // write frame
                frame = self.outbound_rx.recv() => {
                    if let Some(frame) = frame {
                        tracing::info!("send frame {}", frame);
                        if let Err(e) = self.conn.write_frame(frame).await {
                            tracing::debug!("connection closed with {:?}", e);
                            break;
                        };
                    }
                }
            }
        }

        tracing::info!("delete route {:?}", meta.clone());
        self.route_manager.del_route(meta.clone());
        Ok(())
    }

    async fn handle_handshake(&mut self) -> crate::Result<HandshakeFrame> {
        let frame = self.conn.read_frame().await;
        match frame {
            Ok(frame) => {
                tracing::info!("handshake: {}", frame);
                // TODO: authorization
                if let Frame::Handshake(handshake) = frame {
                    Ok(handshake)
                } else {
                    Err("unexpected frame type when handshaking".into())
                }
            }
            Err(e) => {
                Err(e)
            }
        }
    }

    async fn handle_frame(&mut self, frame: Frame){
        match frame {
            Frame::KeepAlive(frame) => {
                tracing::info!("on keepalive");
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
                tracing::info!("on data: {} => {}", frame.src(), frame.dst());

                // route
                let dst_route = self.route_manager.route(frame.dst());
                match dst_route {
                    Some(dst_route) => {
                        let result = dst_route.outbound_tx.
                            send(Frame::Data(frame)).await;

                        if result.is_err() {
                            tracing::warn!("route to {:?} failed {:?}", dst_route, result.err().unwrap());
                        }
                    }
                    None => {
                        tracing::warn!("no route to {:?}", frame.dst());
                    }
                }
            }
            _ => {
                tracing::warn!("unknown frame: {:?}", frame);
            }
        }
    }
}
