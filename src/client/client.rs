use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use crate::codec::frame::{Frame, HandshakeFrame, KeepAliveFrame};
use crate::crypto::Block;
use crate::server::connection::{Connection, TcpConnection};

#[derive(Clone)]
pub struct ClientConfig {
    pub server_addr: String,
    pub private_ip: String,
    pub cidr: Vec<String>,
    pub keepalive_interval: Duration,
    pub outbound_buffer_size: usize,
    pub keep_alive_thresh: u8,
}

pub struct Client {
    cfg: ClientConfig,
    outbound_rx: mpsc::Receiver<Frame>,
    inbound_tx: mpsc::Sender<Frame>,
    block: Arc<Box<dyn Block>>,
}

impl Client {
    pub fn new(cfg: ClientConfig,
               outbound_rx: mpsc::Receiver<Frame>,
               inbound_tx: mpsc::Sender<Frame>,
               block: Arc<Box<dyn Block>>) -> Self {
        Self { cfg, outbound_rx, inbound_tx, block }
    }

    pub async fn run_loop(&mut self)  {
        loop {
            let result = self.run().await;
            tracing::warn!("run client fail {:?}, reconnecting", result);
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    pub async fn run(&mut self) -> crate::Result<()> {
        let socket = TcpStream::connect(&self.cfg.server_addr).await?;
        let mut conn = TcpConnection::new(socket, self.block.clone());
        tracing::info!("Connected to server {}", self.cfg.server_addr);

        // send handshake
        conn.write_frame(Frame::Handshake(HandshakeFrame{
            private_ip: self.cfg.private_ip.clone(),
            ciders: self.cfg.cidr.clone(),
        })).await?;

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
                            tracing::info!("received frame {}", frame);
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
                            tracing::info!("handle frame cost {}", beg.elapsed().as_millis());
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
                    tracing::info!("send to server cost {}", now.elapsed().as_millis());
                }
            }
        }

        tracing::info!("client disconnected");
        let _ = conn.close().await;
        Ok(())
    }
}

pub struct ClientHandler {
    cfg: ClientConfig,
    outbound_tx: Option<mpsc::Sender<Frame>>,
    inbound_rx: Option<mpsc::Receiver<Frame>>,
    block: Arc<Box<dyn Block>>,
}

impl ClientHandler {
    pub fn new(cfg: ClientConfig, block: Arc<Box<dyn Block>>) -> ClientHandler {
        ClientHandler {
            cfg,
            outbound_tx: None,
            inbound_rx: None,
            block,
        }
    }

    pub fn run_client(&mut self){
        let (outbound_tx, outbound_rx) = mpsc::channel(self.cfg.outbound_buffer_size);
        let (inbound_tx, inbound_rx) = mpsc::channel(self.cfg.outbound_buffer_size);
        let mut client = Client::new(self.cfg.clone(), outbound_rx, inbound_tx, self.block.clone());
        tokio::spawn(async move {
            client.run_loop().await;
        });
        self.outbound_tx = Some(outbound_tx);
        self.inbound_rx = Some(inbound_rx);
    }

    pub async fn send_frame(&mut self, frame: Frame) ->crate::Result<()> {
        let outbound_tx = match self.outbound_tx {
            Some(ref tx) => tx,
            None => {
                return Err("device => server channel closed".into())
            },
        };

        let result = outbound_tx.send(frame).await;
        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                Err(format!("device=> server fail {:?}", e).into())
            },
        }

    }

    pub async fn recv_frame(&mut self) ->crate::Result<Frame> {
        let inbound_rx = match self.inbound_rx {
            Some(ref mut rx) => rx,
            None => return {
                Err("server => device channel closed".into())
            },
        };

        let result = inbound_rx.recv().await;
        match result {
            Some(frame) => {
                Ok(frame)
            },
            None => {
                Err("server => device fail for closed channel".into())
            }
        }
    }
}
