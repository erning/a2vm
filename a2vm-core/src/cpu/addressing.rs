/// 13 NMOS 6502 addressing modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddrMode {
    Implied,
    Accumulator,
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndirectX,
    IndirectY,
    Relative,
}

#[derive(Clone, Copy, Debug)]
pub enum Operand {
    None,
    Byte(u8),
    Address(u16),
}

#[derive(Clone, Copy, Debug)]
pub struct Resolved {
    pub operand: Operand,
    pub page_crossed: bool,
}
