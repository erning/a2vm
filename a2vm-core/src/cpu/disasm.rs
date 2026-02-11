use crate::bus::Bus;

use super::addressing::AddrMode;
use super::opcodes::OPCODES;

pub fn disasm(bus: &impl Bus, pc: u16) -> (String, u8) {
    let opcode = bus.peek(pc);
    let info = &OPCODES[opcode as usize];
    let lo = bus.peek(pc.wrapping_add(1));
    let hi = bus.peek(pc.wrapping_add(2));

    let (operand, size) = match info.mode {
        AddrMode::Implied => (String::new(), 1),
        AddrMode::Accumulator => ("A".to_string(), 1),
        AddrMode::Immediate => (format!("#${lo:02X}"), 2),
        AddrMode::ZeroPage => (format!("${lo:02X}"), 2),
        AddrMode::ZeroPageX => (format!("${lo:02X},X"), 2),
        AddrMode::ZeroPageY => (format!("${lo:02X},Y"), 2),
        AddrMode::Absolute => (format!("${:04X}", u16::from_le_bytes([lo, hi])), 3),
        AddrMode::AbsoluteX => (format!("${:04X},X", u16::from_le_bytes([lo, hi])), 3),
        AddrMode::AbsoluteY => (format!("${:04X},Y", u16::from_le_bytes([lo, hi])), 3),
        AddrMode::Indirect => (format!("(${:04X})", u16::from_le_bytes([lo, hi])), 3),
        AddrMode::IndirectX => (format!("(${lo:02X},X)"), 2),
        AddrMode::IndirectY => (format!("(${lo:02X}),Y"), 2),
        AddrMode::Relative => {
            let offset = lo as i8 as i32;
            let target = (pc as i32 + 2 + offset) as u16;
            (format!("${target:04X}"), 2)
        }
    };

    let mnemonic = format!("{:?}", info.mnemonic);
    if operand.is_empty() {
        (mnemonic, size)
    } else {
        (format!("{mnemonic} {operand}"), size)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use crate::bus::Bus;
    use crate::memory::FlatMemory;

    use super::disasm;

    struct PeekOnlyBus {
        data: [u8; 65536],
        read_count: Cell<u32>,
    }

    impl PeekOnlyBus {
        fn new() -> Self {
            Self {
                data: [0; 65536],
                read_count: Cell::new(0),
            }
        }
    }

    impl Bus for PeekOnlyBus {
        fn read(&mut self, addr: u16) -> u8 {
            self.read_count.set(self.read_count.get() + 1);
            self.data[addr as usize]
        }

        fn write(&mut self, addr: u16, val: u8) {
            self.data[addr as usize] = val;
        }

        fn peek(&self, addr: u16) -> u8 {
            self.data[addr as usize]
        }
    }

    #[test]
    fn formats_immediate() {
        let mut mem = FlatMemory::new();
        mem.data[0x400] = 0xA9;
        mem.data[0x401] = 0x7F;

        let (line, size) = disasm(&mem, 0x400);
        assert_eq!(line, "LDA #$7F");
        assert_eq!(size, 2);
    }

    #[test]
    fn formats_absolute_indexed() {
        let mut mem = FlatMemory::new();
        mem.data[0x200] = 0xBD;
        mem.data[0x201] = 0x34;
        mem.data[0x202] = 0x12;

        let (line, size) = disasm(&mem, 0x200);
        assert_eq!(line, "LDA $1234,X");
        assert_eq!(size, 3);
    }

    #[test]
    fn formats_relative_target() {
        let mut mem = FlatMemory::new();
        mem.data[0x1000] = 0xD0;
        mem.data[0x1001] = 0xFE;

        let (line, size) = disasm(&mem, 0x1000);
        assert_eq!(line, "BNE $1000");
        assert_eq!(size, 2);
    }

    #[test]
    fn uses_peek_without_side_effect_reads() {
        let mut bus = PeekOnlyBus::new();
        bus.data[0x0000] = 0xEA;

        let (line, size) = disasm(&bus, 0x0000);
        assert_eq!(line, "NOP");
        assert_eq!(size, 1);
        assert_eq!(bus.read_count.get(), 0);
    }
}
