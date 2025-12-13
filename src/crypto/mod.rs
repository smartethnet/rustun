pub mod aes256;
pub mod plain;

use bytes::BytesMut;

pub trait Block: Send + Sync {
    fn encrypt(&self, data: &mut Vec<u8>) -> crate::Result<()>;
    fn decrypt(&self, data: &mut Vec<u8>) -> crate::Result<()>;
}