use crate::crypto::Block;
use bytes::BytesMut;

pub struct Plain {}

impl Plain {
    pub fn new() -> Self {
        Self {}
    }
}

impl Block for Plain {
    fn encrypt(&self, _data: &mut Vec<u8>) -> crate::Result<()> {
        Ok(())
    }

    fn decrypt(&self, _data: &mut Vec<u8>) -> crate::Result<()> {
        Ok(())
    }
}
