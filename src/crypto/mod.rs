pub mod aes256;
pub mod plain;
pub mod xor;

use serde::{Deserialize, Serialize};
use crate::crypto::aes256::Aes256Block;
use crate::crypto::plain::PlainBlock;
use crate::crypto::xor::XorBlock;

pub trait Block: Send + Sync {
    fn encrypt(&self, data: &mut Vec<u8>) -> crate::Result<()>;
    fn decrypt(&self, data: &mut Vec<u8>) -> crate::Result<()>;
}

pub fn new_block(cfg: &CryptoConfig) -> Box<dyn Block> {
    match cfg {
        CryptoConfig::Aes256(aes) => {
            Box::new(Aes256Block::from_string(aes.as_str()))
        }
        CryptoConfig::Xor(xor) => {
            Box::new(XorBlock::from_string(xor.as_str()))
        }
        CryptoConfig::Plain => {
            Box::new(PlainBlock::new())
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CryptoConfig {
    Aes256(String),
    Plain,
    Xor(String),
}
