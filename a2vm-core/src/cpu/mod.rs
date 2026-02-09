pub mod addressing;
pub mod opcodes;
pub mod status;

use crate::bus::Bus;
use addressing::{AddrMode, Operand, Resolved};
use opcodes::{Mnemonic, OPCODES};
use status::{Status, C, D, I, N, V, Z};

pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    pub p: Status,
    pub cycles: u64,
    pub irq_pending: bool,
    pub nmi_pending: bool,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFD,
            pc: 0,
            p: Status::new(),
            cycles: 0,
            irq_pending: false,
            nmi_pending: false,
        }
    }

    pub fn reset(&mut self, bus: &mut dyn Bus) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFD;
        self.p = Status::new();
        self.p.set(I, true);
        self.pc = bus.read_word(0xFFFC);
        self.irq_pending = false;
        self.nmi_pending = false;
    }

    /// Execute one instruction, return cycles consumed.
    pub fn step(&mut self, bus: &mut dyn Bus) -> u32 {
        bus.set_cycle(self.cycles);

        // Handle interrupts
        if self.nmi_pending {
            self.nmi_pending = false;
            return self.handle_nmi(bus);
        }
        if self.irq_pending && !self.p.get(I) {
            return self.handle_irq(bus);
        }

        let opcode = self.fetch(bus);
        let info = &OPCODES[opcode as usize];
        let resolved = self.resolve(info.mode, bus);

        let mut cycles = info.cycles;
        if info.page_penalty && resolved.page_crossed {
            cycles += 1;
        }

        cycles += self.execute(info.mnemonic, info.mode, &resolved, bus);

        self.cycles += cycles as u64;
        cycles
    }

    /// Run for at least `target_cycles` cycles. Returns total cycles executed.
    pub fn run(&mut self, bus: &mut dyn Bus, target_cycles: u64) -> u64 {
        let start = self.cycles;
        while self.cycles - start < target_cycles {
            self.step(bus);
        }
        self.cycles - start
    }

    // -- Fetch --

    fn fetch(&mut self, bus: &mut dyn Bus) -> u8 {
        let val = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    fn fetch_word(&mut self, bus: &mut dyn Bus) -> u16 {
        let lo = self.fetch(bus) as u16;
        let hi = self.fetch(bus) as u16;
        (hi << 8) | lo
    }

    // -- Stack --

    fn push(&mut self, bus: &mut dyn Bus, val: u8) {
        bus.write(0x0100 | self.sp as u16, val);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn push_word(&mut self, bus: &mut dyn Bus, val: u16) {
        self.push(bus, (val >> 8) as u8);
        self.push(bus, val as u8);
    }

    fn pull(&mut self, bus: &mut dyn Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.read(0x0100 | self.sp as u16)
    }

    fn pull_word(&mut self, bus: &mut dyn Bus) -> u16 {
        let lo = self.pull(bus) as u16;
        let hi = self.pull(bus) as u16;
        (hi << 8) | lo
    }

    // -- Addressing mode resolution --

    fn resolve(&mut self, mode: AddrMode, bus: &mut dyn Bus) -> Resolved {
        match mode {
            AddrMode::Implied | AddrMode::Accumulator => Resolved {
                operand: Operand::None,
                page_crossed: false,
            },

            AddrMode::Immediate => {
                let addr = self.pc;
                self.pc = self.pc.wrapping_add(1);
                Resolved {
                    operand: Operand::Address(addr),
                    page_crossed: false,
                }
            }

            AddrMode::ZeroPage => {
                let addr = self.fetch(bus) as u16;
                Resolved {
                    operand: Operand::Address(addr),
                    page_crossed: false,
                }
            }

            AddrMode::ZeroPageX => {
                let base = self.fetch(bus);
                let addr = base.wrapping_add(self.x) as u16;
                Resolved {
                    operand: Operand::Address(addr),
                    page_crossed: false,
                }
            }

            AddrMode::ZeroPageY => {
                let base = self.fetch(bus);
                let addr = base.wrapping_add(self.y) as u16;
                Resolved {
                    operand: Operand::Address(addr),
                    page_crossed: false,
                }
            }

            AddrMode::Absolute => {
                let addr = self.fetch_word(bus);
                Resolved {
                    operand: Operand::Address(addr),
                    page_crossed: false,
                }
            }

            AddrMode::AbsoluteX => {
                let base = self.fetch_word(bus);
                let addr = base.wrapping_add(self.x as u16);
                Resolved {
                    operand: Operand::Address(addr),
                    page_crossed: (base & 0xFF00) != (addr & 0xFF00),
                }
            }

            AddrMode::AbsoluteY => {
                let base = self.fetch_word(bus);
                let addr = base.wrapping_add(self.y as u16);
                Resolved {
                    operand: Operand::Address(addr),
                    page_crossed: (base & 0xFF00) != (addr & 0xFF00),
                }
            }

            AddrMode::Indirect => {
                let ptr = self.fetch_word(bus);
                // NMOS 6502 bug: JMP ($xxFF) wraps within page
                let addr = bus.read_word_page_wrap(ptr);
                Resolved {
                    operand: Operand::Address(addr),
                    page_crossed: false,
                }
            }

            AddrMode::IndirectX => {
                let base = self.fetch(bus);
                let ptr = base.wrapping_add(self.x);
                let addr = bus.read_word_page_wrap(ptr as u16);
                Resolved {
                    operand: Operand::Address(addr),
                    page_crossed: false,
                }
            }

            AddrMode::IndirectY => {
                let ptr = self.fetch(bus) as u16;
                let base = bus.read_word_page_wrap(ptr);
                let addr = base.wrapping_add(self.y as u16);
                Resolved {
                    operand: Operand::Address(addr),
                    page_crossed: (base & 0xFF00) != (addr & 0xFF00),
                }
            }

            AddrMode::Relative => {
                let offset = self.fetch(bus) as i8;
                Resolved {
                    operand: Operand::Byte(offset as u8),
                    page_crossed: false,
                }
            }
        }
    }

    // -- Helpers --

    fn read_operand(&self, resolved: &Resolved, bus: &mut dyn Bus) -> u8 {
        match resolved.operand {
            Operand::Address(addr) => bus.read(addr),
            _ => 0,
        }
    }

    fn addr_of(&self, resolved: &Resolved) -> u16 {
        match resolved.operand {
            Operand::Address(addr) => addr,
            _ => 0,
        }
    }

    // -- Instruction execution --

    /// Returns extra cycles beyond the base (e.g., branch taken penalty).
    fn execute(
        &mut self,
        mnemonic: Mnemonic,
        mode: AddrMode,
        resolved: &Resolved,
        bus: &mut dyn Bus,
    ) -> u32 {
        match mnemonic {
            // -- Load/Store --
            Mnemonic::LDA => {
                self.a = self.read_operand(resolved, bus);
                self.p.set_nz(self.a);
                0
            }
            Mnemonic::LDX => {
                self.x = self.read_operand(resolved, bus);
                self.p.set_nz(self.x);
                0
            }
            Mnemonic::LDY => {
                self.y = self.read_operand(resolved, bus);
                self.p.set_nz(self.y);
                0
            }
            Mnemonic::STA => {
                let addr = self.addr_of(resolved);
                bus.write(addr, self.a);
                0
            }
            Mnemonic::STX => {
                let addr = self.addr_of(resolved);
                bus.write(addr, self.x);
                0
            }
            Mnemonic::STY => {
                let addr = self.addr_of(resolved);
                bus.write(addr, self.y);
                0
            }

            // -- Arithmetic --
            Mnemonic::ADC => {
                let val = self.read_operand(resolved, bus);
                self.adc(val);
                0
            }
            Mnemonic::SBC => {
                let val = self.read_operand(resolved, bus);
                self.sbc(val);
                0
            }

            // -- Compare --
            Mnemonic::CMP => {
                let val = self.read_operand(resolved, bus);
                self.compare(self.a, val);
                0
            }
            Mnemonic::CPX => {
                let val = self.read_operand(resolved, bus);
                self.compare(self.x, val);
                0
            }
            Mnemonic::CPY => {
                let val = self.read_operand(resolved, bus);
                self.compare(self.y, val);
                0
            }

            // -- Logic --
            Mnemonic::AND => {
                self.a &= self.read_operand(resolved, bus);
                self.p.set_nz(self.a);
                0
            }
            Mnemonic::ORA => {
                self.a |= self.read_operand(resolved, bus);
                self.p.set_nz(self.a);
                0
            }
            Mnemonic::EOR => {
                self.a ^= self.read_operand(resolved, bus);
                self.p.set_nz(self.a);
                0
            }
            Mnemonic::BIT => {
                let val = self.read_operand(resolved, bus);
                self.p.set(Z, (self.a & val) == 0);
                self.p.set(N, val & 0x80 != 0);
                self.p.set(V, val & 0x40 != 0);
                0
            }

            // -- Shift/Rotate --
            Mnemonic::ASL => {
                if mode == AddrMode::Accumulator {
                    self.p.set(C, self.a & 0x80 != 0);
                    self.a <<= 1;
                    self.p.set_nz(self.a);
                } else {
                    let addr = self.addr_of(resolved);
                    let mut val = bus.read(addr);
                    self.p.set(C, val & 0x80 != 0);
                    val <<= 1;
                    bus.write(addr, val);
                    self.p.set_nz(val);
                }
                0
            }
            Mnemonic::LSR => {
                if mode == AddrMode::Accumulator {
                    self.p.set(C, self.a & 0x01 != 0);
                    self.a >>= 1;
                    self.p.set_nz(self.a);
                } else {
                    let addr = self.addr_of(resolved);
                    let mut val = bus.read(addr);
                    self.p.set(C, val & 0x01 != 0);
                    val >>= 1;
                    bus.write(addr, val);
                    self.p.set_nz(val);
                }
                0
            }
            Mnemonic::ROL => {
                let carry_in = self.p.get(C) as u8;
                if mode == AddrMode::Accumulator {
                    self.p.set(C, self.a & 0x80 != 0);
                    self.a = (self.a << 1) | carry_in;
                    self.p.set_nz(self.a);
                } else {
                    let addr = self.addr_of(resolved);
                    let mut val = bus.read(addr);
                    self.p.set(C, val & 0x80 != 0);
                    val = (val << 1) | carry_in;
                    bus.write(addr, val);
                    self.p.set_nz(val);
                }
                0
            }
            Mnemonic::ROR => {
                let carry_in = if self.p.get(C) { 0x80u8 } else { 0 };
                if mode == AddrMode::Accumulator {
                    self.p.set(C, self.a & 0x01 != 0);
                    self.a = (self.a >> 1) | carry_in;
                    self.p.set_nz(self.a);
                } else {
                    let addr = self.addr_of(resolved);
                    let mut val = bus.read(addr);
                    self.p.set(C, val & 0x01 != 0);
                    val = (val >> 1) | carry_in;
                    bus.write(addr, val);
                    self.p.set_nz(val);
                }
                0
            }

            // -- Inc/Dec --
            Mnemonic::INC => {
                let addr = self.addr_of(resolved);
                let val = bus.read(addr).wrapping_add(1);
                bus.write(addr, val);
                self.p.set_nz(val);
                0
            }
            Mnemonic::DEC => {
                let addr = self.addr_of(resolved);
                let val = bus.read(addr).wrapping_sub(1);
                bus.write(addr, val);
                self.p.set_nz(val);
                0
            }
            Mnemonic::INX => {
                self.x = self.x.wrapping_add(1);
                self.p.set_nz(self.x);
                0
            }
            Mnemonic::DEX => {
                self.x = self.x.wrapping_sub(1);
                self.p.set_nz(self.x);
                0
            }
            Mnemonic::INY => {
                self.y = self.y.wrapping_add(1);
                self.p.set_nz(self.y);
                0
            }
            Mnemonic::DEY => {
                self.y = self.y.wrapping_sub(1);
                self.p.set_nz(self.y);
                0
            }

            // -- Transfer --
            Mnemonic::TAX => {
                self.x = self.a;
                self.p.set_nz(self.x);
                0
            }
            Mnemonic::TAY => {
                self.y = self.a;
                self.p.set_nz(self.y);
                0
            }
            Mnemonic::TXA => {
                self.a = self.x;
                self.p.set_nz(self.a);
                0
            }
            Mnemonic::TYA => {
                self.a = self.y;
                self.p.set_nz(self.a);
                0
            }
            Mnemonic::TSX => {
                self.x = self.sp;
                self.p.set_nz(self.x);
                0
            }
            Mnemonic::TXS => {
                self.sp = self.x;
                // TXS does NOT affect flags
                0
            }

            // -- Stack --
            Mnemonic::PHA => {
                self.push(bus, self.a);
                0
            }
            Mnemonic::PHP => {
                // PHP always pushes with B=1 and U=1
                let val = self.p.to_push_byte(true);
                self.push(bus, val);
                0
            }
            Mnemonic::PLA => {
                self.a = self.pull(bus);
                self.p.set_nz(self.a);
                0
            }
            Mnemonic::PLP => {
                let val = self.pull(bus);
                self.p.from_pull_byte(val);
                0
            }

            // -- Branch --
            Mnemonic::BCC => self.branch(!self.p.get(C), resolved),
            Mnemonic::BCS => self.branch(self.p.get(C), resolved),
            Mnemonic::BEQ => self.branch(self.p.get(Z), resolved),
            Mnemonic::BNE => self.branch(!self.p.get(Z), resolved),
            Mnemonic::BMI => self.branch(self.p.get(N), resolved),
            Mnemonic::BPL => self.branch(!self.p.get(N), resolved),
            Mnemonic::BVC => self.branch(!self.p.get(V), resolved),
            Mnemonic::BVS => self.branch(self.p.get(V), resolved),

            // -- Jump/Call --
            Mnemonic::JMP => {
                self.pc = self.addr_of(resolved);
                0
            }
            Mnemonic::JSR => {
                // JSR pushes PC-1 (address of last byte of JSR instruction)
                let ret = self.pc.wrapping_sub(1);
                self.push_word(bus, ret);
                self.pc = self.addr_of(resolved);
                0
            }
            Mnemonic::RTS => {
                let addr = self.pull_word(bus);
                self.pc = addr.wrapping_add(1);
                0
            }
            Mnemonic::RTI => {
                let val = self.pull(bus);
                self.p.from_pull_byte(val);
                self.pc = self.pull_word(bus);
                0
            }

            // -- Interrupt --
            Mnemonic::BRK => {
                // BRK pushes PC+1 (so the byte after BRK opcode+padding is skipped)
                let ret = self.pc.wrapping_add(1);
                self.push_word(bus, ret);
                self.push(bus, self.p.to_push_byte(true));
                self.p.set(I, true);
                self.pc = bus.read_word(0xFFFE);
                0
            }

            // -- Flags --
            Mnemonic::CLC => {
                self.p.set(C, false);
                0
            }
            Mnemonic::SEC => {
                self.p.set(C, true);
                0
            }
            Mnemonic::CLD => {
                self.p.set(D, false);
                0
            }
            Mnemonic::SED => {
                self.p.set(D, true);
                0
            }
            Mnemonic::CLI => {
                self.p.set(I, false);
                0
            }
            Mnemonic::SEI => {
                self.p.set(I, true);
                0
            }
            Mnemonic::CLV => {
                self.p.set(V, false);
                0
            }

            // -- No-op / Illegal --
            Mnemonic::NOP => 0,
            Mnemonic::ILL => {
                // Treat as NOP for now
                0
            }
        }
    }

    // -- ALU helpers --

    fn adc(&mut self, val: u8) {
        if self.p.get(D) {
            self.adc_bcd(val);
        } else {
            self.adc_binary(val);
        }
    }

    fn adc_binary(&mut self, val: u8) {
        let a = self.a as u16;
        let v = val as u16;
        let c = self.p.get(C) as u16;
        let sum = a + v + c;

        // Overflow: set if sign of result differs from both inputs
        let overflow = (!(self.a ^ val) & (self.a ^ sum as u8)) & 0x80 != 0;
        self.p.set(V, overflow);
        self.p.set(C, sum > 0xFF);

        self.a = sum as u8;
        self.p.set_nz(self.a);
    }

    fn adc_bcd(&mut self, val: u8) {
        let a = self.a as u16;
        let v = val as u16;
        let c = self.p.get(C) as u16;

        // Binary addition for N, Z, V flags (NMOS 6502 behavior)
        let bin_sum = a + v + c;

        // BCD low nibble
        let mut lo = (a & 0x0F) + (v & 0x0F) + c;
        if lo > 9 {
            lo += 6;
        }

        // BCD high nibble
        let mut hi = (a >> 4) + (v >> 4) + if lo > 0x0F { 1 } else { 0 };

        // NMOS 6502: V is based on intermediate (after lo fixup, before hi fixup)
        let intermediate = ((hi << 4) | (lo & 0x0F)) as u8;
        let overflow = (!(self.a ^ val) & (self.a ^ intermediate)) & 0x80 != 0;
        self.p.set(V, overflow);

        if hi > 9 {
            hi += 6;
        }

        self.p.set(C, hi > 0x0F);

        self.a = ((hi << 4) | (lo & 0x0F)) as u8;

        // NMOS 6502: N and Z are based on binary result, not BCD result
        self.p.set(N, bin_sum as u8 & 0x80 != 0);
        self.p.set(Z, (bin_sum as u8) == 0);
    }

    fn sbc(&mut self, val: u8) {
        if self.p.get(D) {
            self.sbc_bcd(val);
        } else {
            // SBC is ADC with complement
            self.adc_binary(!val);
        }
    }

    fn sbc_bcd(&mut self, val: u8) {
        let a = self.a as i16;
        let v = val as i16;
        let c = self.p.get(C) as i16;

        // Binary result for N, Z, V flags (NMOS 6502 behavior)
        let bin_sum = (a as u16)
            .wrapping_add((!val) as u16)
            .wrapping_add(c as u16);

        // V flag from binary subtraction
        let overflow = (!(self.a ^ !val) & (self.a ^ bin_sum as u8)) & 0x80 != 0;
        self.p.set(V, overflow);

        // BCD low nibble
        let mut lo = (a & 0x0F) - (v & 0x0F) + c - 1;
        if lo < 0 {
            lo += 10;
        }
        // BCD high nibble
        let borrow_lo = if (a & 0x0F) + c - 1 < (v & 0x0F) {
            1
        } else {
            0
        };
        let mut hi = (a >> 4) - (v >> 4) - borrow_lo;

        if hi < 0 {
            hi += 10;
        }

        // C flag: no borrow = carry out from binary addition A + ~val + C
        self.p.set(C, bin_sum > 0xFF);

        self.a = ((hi << 4) as u8) | (lo as u8 & 0x0F);

        // NMOS 6502: N and Z based on binary result
        self.p.set(N, bin_sum as u8 & 0x80 != 0);
        self.p.set(Z, (bin_sum as u8) == 0);
    }

    fn compare(&mut self, reg: u8, val: u8) {
        let result = reg.wrapping_sub(val);
        self.p.set(C, reg >= val);
        self.p.set_nz(result);
    }

    fn branch(&mut self, condition: bool, resolved: &Resolved) -> u32 {
        if !condition {
            return 0;
        }
        let offset = match resolved.operand {
            Operand::Byte(b) => b as i8,
            _ => return 0,
        };
        let old_pc = self.pc;
        self.pc = self.pc.wrapping_add(offset as u16);

        // +1 for taken, +1 more if page crossed
        if (old_pc & 0xFF00) != (self.pc & 0xFF00) {
            2
        } else {
            1
        }
    }

    // -- Interrupt handlers --

    fn handle_nmi(&mut self, bus: &mut dyn Bus) -> u32 {
        self.push_word(bus, self.pc);
        self.push(bus, self.p.to_push_byte(false));
        self.p.set(I, true);
        self.pc = bus.read_word(0xFFFA);
        self.cycles += 7;
        7
    }

    fn handle_irq(&mut self, bus: &mut dyn Bus) -> u32 {
        self.push_word(bus, self.pc);
        self.push(bus, self.p.to_push_byte(false));
        self.p.set(I, true);
        self.pc = bus.read_word(0xFFFE);
        self.cycles += 7;
        7
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}
