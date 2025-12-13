use super::Block;

pub struct XorBlock {
    key: Vec<u8>,
}

impl XorBlock {
    pub fn new(key: &[u8]) -> Self {
        if key.is_empty() {
            panic!("XOR key cannot be empty");
        }
        Self {
            key: key.to_vec(),
        }
    }

    pub fn from_string(s: &str) -> Self {
        let key = s.as_bytes();
        if key.is_empty() {
            panic!("XOR key cannot be empty");
        }
        Self {
            key: key.to_vec(),
        }
    }

    fn xor_data(&self, data: &mut [u8]) {
        let key_len = self.key.len();
        for (i, byte) in data.iter_mut().enumerate() {
            *byte ^= self.key[i % key_len];
        }
    }
}

impl Block for XorBlock {
    fn encrypt(&self, data: &mut Vec<u8>) -> crate::Result<()> {
        self.xor_data(data);
        Ok(())
    }

    fn decrypt(&self, data: &mut Vec<u8>) -> crate::Result<()> {
        // XOR encryption is symmetric: decrypt is the same as encrypt
        self.xor_data(data);
        Ok(())
    }
}