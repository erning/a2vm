use crate::bus::Bus;

/// Flat 64K RAM — used for standalone CPU testing (e.g., Klaus Dormann test suite).
pub struct FlatMemory {
    data: Box<[u8; 65536]>,
}

impl FlatMemory {
    pub fn new() -> Self {
        Self {
            data: Box::new([0u8; 65536]),
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data[..]
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data[..]
    }
}

impl Default for FlatMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Index<usize> for FlatMemory {
    type Output = u8;
    fn index(&self, index: usize) -> &u8 {
        &self.data[index]
    }
}

impl std::ops::IndexMut<usize> for FlatMemory {
    fn index_mut(&mut self, index: usize) -> &mut u8 {
        &mut self.data[index]
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
