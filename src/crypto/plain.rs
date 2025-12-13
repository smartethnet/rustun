use crate::crypto::Block;

pub struct PlainBlock {}

impl PlainBlock {
    pub fn new() -> Self {
        Self {}
    }
}

impl Block for PlainBlock {
    fn encrypt(&self, _data: &mut Vec<u8>) -> crate::Result<()> {
        Ok(())
    }

    fn decrypt(&self, _data: &mut Vec<u8>) -> crate::Result<()> {
        Ok(())
    }
}
