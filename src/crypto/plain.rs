use crate::crypto::Block;

pub struct Plain {}

impl Plain {
    pub fn new() -> Self {
        Self {}
    }
}

impl Block for Plain {
    fn encrypt(&mut self, _data: &mut Vec<u8>) -> crate::Result<()> {
        Ok(())
    }

    fn decrypt(&mut self, _data: &mut Vec<u8>) -> crate::Result<()> {
        Ok(())
    }
}
