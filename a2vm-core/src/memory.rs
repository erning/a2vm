use crate::bus::Bus;

/// Flat 64K RAM — used for standalone CPU testing (e.g., Klaus Dormann test suite).
pub struct FlatMemory {
    pub data: Box<[u8; 65536]>,
}

impl FlatMemory {
    pub fn new() -> Self {
        Self {
            data: Box::new([0u8; 65536]),
        }
    }
}

impl Default for FlatMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus for FlatMemory {
    fn read(&mut self, addr: u16) -> u8 {
        self.data[addr as usize]
    }

    fn write(&mut self, addr: u16, val: u8) {
        self.data[addr as usize] = val;
    }

    fn peek(&self, addr: u16) -> u8 {
        self.data[addr as usize]
    }
}
