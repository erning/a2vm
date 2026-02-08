/// 6502 processor status register (P) bit manipulation.
#[derive(Clone, Copy, Debug)]
pub struct Status(pub u8);

// Flag bit positions
pub const C: u8 = 0; // Carry
pub const Z: u8 = 1; // Zero
pub const I: u8 = 2; // Interrupt Disable
pub const D: u8 = 3; // Decimal
pub const B: u8 = 4; // Break
pub const U: u8 = 5; // Unused (always 1)
pub const V: u8 = 6; // Overflow
pub const N: u8 = 7; // Negative

impl Status {
    pub fn new() -> Self {
        // U always set
        Self(1 << U)
    }

    #[inline]
    pub fn get(&self, flag: u8) -> bool {
        (self.0 >> flag) & 1 != 0
    }

    #[inline]
    pub fn set(&mut self, flag: u8, val: bool) {
        if val {
            self.0 |= 1 << flag;
        } else {
            self.0 &= !(1 << flag);
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
        let mut val = self.0 | (1 << U);
        if brk {
            val |= 1 << B;
        } else {
            val &= !(1 << B);
        }
        val
    }

    /// Restore from stack (PLP/RTI). B and U are not affected in the actual register.
    #[inline]
    pub fn from_pull_byte(&mut self, val: u8) {
        self.0 = (val | (1 << U)) & !(1 << B);
    }
}

impl Default for Status {
    fn default() -> Self {
        Self::new()
    }
}
