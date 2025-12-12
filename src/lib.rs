pub mod codec;
pub mod server;
pub mod crypto;
pub mod client;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

