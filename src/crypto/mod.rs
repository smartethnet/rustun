pub mod aes256;
pub mod plain;

pub trait Block: Send + Sync {
    fn encrypt(&mut self, data: &mut Vec<u8>) -> crate::Result<()>;
    fn decrypt(&mut self, data: &mut Vec<u8>) -> crate::Result<()>;
}