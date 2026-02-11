/// 6502 processor status register (P) bit manipulation.
#[derive(Clone, Copy, Debug)]
pub struct Status(pub u8);

// Flag bit positions
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    C = 0,
    Z = 1,
    I = 2,
    D = 3,
    B = 4,
    U = 5,
    V = 6,
    N = 7,
}

pub const C: Flag = Flag::C;
pub const Z: Flag = Flag::Z;
pub const I: Flag = Flag::I;
pub const D: Flag = Flag::D;
pub const B: Flag = Flag::B;
pub const U: Flag = Flag::U;
pub const V: Flag = Flag::V;
pub const N: Flag = Flag::N;

impl Status {
    pub fn new() -> Self {
        // U always set
        Self(1 << (U as u8))
    }

    #[inline]
    pub fn get(&self, flag: Flag) -> bool {
        (self.0 >> (flag as u8)) & 1 != 0
    }

    #[inline]
    pub fn set(&mut self, flag: Flag, val: bool) {
        let mask = 1 << (flag as u8);
        if val {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    /// Set N and Z flags based on a value.
    #[inline]
    pub fn set_nz(&mut self, val: u8) {
        self.set(N, val & 0x80 != 0);
        self.set(Z, val == 0);
    }

    /// Byte pushed to stack by PHP/BRK.
    /// B is set for BRK (brk=true) and PHP (brk=true for PHP too — PHP always pushes B=1).
    /// U is always 1.
    #[inline]
    pub fn to_push_byte(&self, brk: bool) -> u8 {
        let mut val = self.0 | (1 << (U as u8));
        if brk {
            val |= 1 << (B as u8);
        } else {
            val &= !(1 << (B as u8));
        }
        val
    }

    /// Restore from stack (PLP/RTI). B and U are not affected in the actual register.
    #[inline]
    pub fn from_pull_byte(&mut self, val: u8) {
        self.0 = (val | (1 << (U as u8))) & !(1 << (B as u8));
    }
}

impl Default for Status {
    fn default() -> Self {
        Self::new()
    }
}
