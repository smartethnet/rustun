use tokio::time::Duration;
use tokio::time;
use tokio::net::{TcpStream};
use std::sync::Arc;
use tokio::net::TcpListener;
use crate::crypto::Block;
use crate::server::handler::ConnectionHandler;
use crate::server::route::{RouteManager};

pub struct Server {
    addr: String,
    routes: Arc<RouteManager>,
    listener: Option<TcpListener>,
    #[allow(unused)]
    block: Arc<Box<dyn Block>>,
}

impl Server {
    pub fn new(addr: String, block: Arc<Box<dyn Block>>) -> Self {
        Server {
            addr,
            routes: Arc::new(RouteManager::new()),
            listener: None,
            block,
        }
    }
}

impl Server {
    pub async fn listen_and_serve(&mut self) -> crate::Result<()> {
        let listener = TcpListener::bind(self.addr.clone()).await?;
        tracing::info!("Server started at {}", self.addr);
        self.listener = Some(listener);
        loop {
            tokio::select! {
                socket = self.accept() => {
                    match socket {
                        Ok(socket) => {
                            self.handle_conn(socket)
                        },
                        Err(e) => {
                            return Err(e.into());
                        }
                    };
                }
            }
        }
    }

    async fn accept(&mut self) -> crate::Result<TcpStream> {
        let mut backoff = 1;

        loop {
            match self.listener.as_ref().unwrap().accept().await {
                Ok((socket, _)) => return Ok(socket),
                Err(err) => {
                    if backoff > 64 {
                        return Err(err.into());
                    }
                }
            }

            time::sleep(Duration::from_secs(backoff)).await;
            backoff *= 2;
        }
    }

    fn handle_conn(&self, socket: TcpStream) {
        let peer_addr = socket.peer_addr().unwrap();
        tracing::info!("new connection from {}", socket.peer_addr().unwrap());

        let mut handler = ConnectionHandler::new(self.routes.clone(), socket, self.block.clone());
        tokio::task::spawn(async move {
            let e = handler.run().await;
            tracing::info!("client {:?} handler stop with {:?}", peer_addr, e);
        });
    }
}
