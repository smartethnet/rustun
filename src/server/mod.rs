use async_trait::async_trait;
use crate::codec::frame::Frame;

pub mod main;
pub mod connection;
pub mod config;
mod client_manager;
mod connection_manager;
mod server;


#[async_trait]
pub trait Connection: Send + Sync {
    async fn read_frame(&mut self) -> crate::Result<Frame>;
    async fn write_frame(&mut self, frame: Frame) -> crate::Result<()>;
    async fn close(&mut self);
}