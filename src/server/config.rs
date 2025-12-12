#[derive(Debug)]
#[allow(unused)]
pub struct Config {
    server_config: ServerConfig,
    crypto_config: CryptoConfig,
}

#[derive(Debug)]
pub struct ServerConfig {
    pub listen_addr: String,
}

#[derive(Debug)]
pub enum CryptoConfig {
    Aes256(String),
    Plain(String),
}
