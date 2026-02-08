use super::addressing::AddrMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mnemonic {
    ADC, AND, ASL, BCC, BCS, BEQ, BIT, BMI,
    BNE, BPL, BRK, BVC, BVS, CLC, CLD, CLI,
    CLV, CMP, CPX, CPY, DEC, DEX, DEY, EOR,
    INC, INX, INY, JMP, JSR, LDA, LDX, LDY,
    LSR, NOP, ORA, PHA, PHP, PLA, PLP, ROL,
    ROR, RTI, RTS, SBC, SEC, SED, SEI, STA,
    STX, STY, TAX, TAY, TSX, TXA, TXS, TYA,
    ILL,
}

#[derive(Clone, Copy, Debug)]
pub struct OpcodeInfo {
    pub mnemonic: Mnemonic,
    pub mode: AddrMode,
    pub cycles: u32,
    pub page_penalty: bool,
}

use AddrMode::*;
use Mnemonic::*;

const fn op(mnemonic: Mnemonic, mode: AddrMode, cycles: u32, page_penalty: bool) -> OpcodeInfo {
    OpcodeInfo { mnemonic, mode, cycles, page_penalty }
}

const fn ill() -> OpcodeInfo {
    op(ILL, Implied, 2, false)
}

pub static OPCODES: [OpcodeInfo; 256] = [
    // 0x00
    op(BRK, Implied,    7, false), // 00
    op(ORA, IndirectX,  6, false), // 01
    ill(),                         // 02
    ill(),                         // 03
    ill(),                         // 04
    op(ORA, ZeroPage,   3, false), // 05
    op(ASL, ZeroPage,   5, false), // 06
    ill(),                         // 07
    op(PHP, Implied,    3, false), // 08
    op(ORA, Immediate,  2, false), // 09
    op(ASL, Accumulator,2, false), // 0A
    ill(),                         // 0B
    ill(),                         // 0C
    op(ORA, Absolute,   4, false), // 0D
    op(ASL, Absolute,   6, false), // 0E
    ill(),                         // 0F

    // 0x10
    op(BPL, Relative,   2, false), // 10
    op(ORA, IndirectY,  5, true),  // 11
    ill(),                         // 12
    ill(),                         // 13
    ill(),                         // 14
    op(ORA, ZeroPageX,  4, false), // 15
    op(ASL, ZeroPageX,  6, false), // 16
    ill(),                         // 17
    op(CLC, Implied,    2, false), // 18
    op(ORA, AbsoluteY,  4, true),  // 19
    ill(),                         // 1A
    ill(),                         // 1B
    ill(),                         // 1C
    op(ORA, AbsoluteX,  4, true),  // 1D
    op(ASL, AbsoluteX,  7, false), // 1E
    ill(),                         // 1F

    // 0x20
    op(JSR, Absolute,   6, false), // 20
    op(AND, IndirectX,  6, false), // 21
    ill(),                         // 22
    ill(),                         // 23
    op(BIT, ZeroPage,   3, false), // 24
    op(AND, ZeroPage,   3, false), // 25
    op(ROL, ZeroPage,   5, false), // 26
    ill(),                         // 27
    op(PLP, Implied,    4, false), // 28
    op(AND, Immediate,  2, false), // 29
    op(ROL, Accumulator,2, false), // 2A
    ill(),                         // 2B
    op(BIT, Absolute,   4, false), // 2C
    op(AND, Absolute,   4, false), // 2D
    op(ROL, Absolute,   6, false), // 2E
    ill(),                         // 2F

    // 0x30
    op(BMI, Relative,   2, false), // 30
    op(AND, IndirectY,  5, true),  // 31
    ill(),                         // 32
    ill(),                         // 33
    ill(),                         // 34
    op(AND, ZeroPageX,  4, false), // 35
    op(ROL, ZeroPageX,  6, false), // 36
    ill(),                         // 37
    op(SEC, Implied,    2, false), // 38
    op(AND, AbsoluteY,  4, true),  // 39
    ill(),                         // 3A
    ill(),                         // 3B
    ill(),                         // 3C
    op(AND, AbsoluteX,  4, true),  // 3D
    op(ROL, AbsoluteX,  7, false), // 3E
    ill(),                         // 3F

    // 0x40
    op(RTI, Implied,    6, false), // 40
    op(EOR, IndirectX,  6, false), // 41
    ill(),                         // 42
    ill(),                         // 43
    ill(),                         // 44
    op(EOR, ZeroPage,   3, false), // 45
    op(LSR, ZeroPage,   5, false), // 46
    ill(),                         // 47
    op(PHA, Implied,    3, false), // 48
    op(EOR, Immediate,  2, false), // 49
    op(LSR, Accumulator,2, false), // 4A
    ill(),                         // 4B
    op(JMP, Absolute,   3, false), // 4C
    op(EOR, Absolute,   4, false), // 4D
    op(LSR, Absolute,   6, false), // 4E
    ill(),                         // 4F

    // 0x50
    op(BVC, Relative,   2, false), // 50
    op(EOR, IndirectY,  5, true),  // 51
    ill(),                         // 52
    ill(),                         // 53
    ill(),                         // 54
    op(EOR, ZeroPageX,  4, false), // 55
    op(LSR, ZeroPageX,  6, false), // 56
    ill(),                         // 57
    op(CLI, Implied,    2, false), // 58
    op(EOR, AbsoluteY,  4, true),  // 59
    ill(),                         // 5A
    ill(),                         // 5B
    ill(),                         // 5C
    op(EOR, AbsoluteX,  4, true),  // 5D
    op(LSR, AbsoluteX,  7, false), // 5E
    ill(),                         // 5F

    // 0x60
    op(RTS, Implied,    6, false), // 60
    op(ADC, IndirectX,  6, false), // 61
    ill(),                         // 62
    ill(),                         // 63
    ill(),                         // 64
    op(ADC, ZeroPage,   3, false), // 65
    op(ROR, ZeroPage,   5, false), // 66
    ill(),                         // 67
    op(PLA, Implied,    4, false), // 68
    op(ADC, Immediate,  2, false), // 69
    op(ROR, Accumulator,2, false), // 6A
    ill(),                         // 6B
    op(JMP, Indirect,   5, false), // 6C
    op(ADC, Absolute,   4, false), // 6D
    op(ROR, Absolute,   6, false), // 6E
    ill(),                         // 6F

    // 0x70
    op(BVS, Relative,   2, false), // 70
    op(ADC, IndirectY,  5, true),  // 71
    ill(),                         // 72
    ill(),                         // 73
    ill(),                         // 74
    op(ADC, ZeroPageX,  4, false), // 75
    op(ROR, ZeroPageX,  6, false), // 76
    ill(),                         // 77
    op(SEI, Implied,    2, false), // 78
    op(ADC, AbsoluteY,  4, true),  // 79
    ill(),                         // 7A
    ill(),                         // 7B
    ill(),                         // 7C
    op(ADC, AbsoluteX,  4, true),  // 7D
    op(ROR, AbsoluteX,  7, false), // 7E
    ill(),                         // 7F

    // 0x80
    ill(),                         // 80
    op(STA, IndirectX,  6, false), // 81
    ill(),                         // 82
    ill(),                         // 83
    op(STY, ZeroPage,   3, false), // 84
    op(STA, ZeroPage,   3, false), // 85
    op(STX, ZeroPage,   3, false), // 86
    ill(),                         // 87
    op(DEY, Implied,    2, false), // 88
    ill(),                         // 89
    op(TXA, Implied,    2, false), // 8A
    ill(),                         // 8B
    op(STY, Absolute,   4, false), // 8C
    op(STA, Absolute,   4, false), // 8D
    op(STX, Absolute,   4, false), // 8E
    ill(),                         // 8F

    // 0x90
    op(BCC, Relative,   2, false), // 90
    op(STA, IndirectY,  6, false), // 91
    ill(),                         // 92
    ill(),                         // 93
    op(STY, ZeroPageX,  4, false), // 94
    op(STA, ZeroPageX,  4, false), // 95
    op(STX, ZeroPageY,  4, false), // 96
    ill(),                         // 97
    op(TYA, Implied,    2, false), // 98
    op(STA, AbsoluteY,  5, false), // 99
    op(TXS, Implied,    2, false), // 9A
    ill(),                         // 9B
    ill(),                         // 9C
    op(STA, AbsoluteX,  5, false), // 9D
    ill(),                         // 9E
    ill(),                         // 9F

    // 0xA0
    op(LDY, Immediate,  2, false), // A0
    op(LDA, IndirectX,  6, false), // A1
    op(LDX, Immediate,  2, false), // A2
    ill(),                         // A3
    op(LDY, ZeroPage,   3, false), // A4
    op(LDA, ZeroPage,   3, false), // A5
    op(LDX, ZeroPage,   3, false), // A6
    ill(),                         // A7
    op(TAY, Implied,    2, false), // A8
    op(LDA, Immediate,  2, false), // A9
    op(TAX, Implied,    2, false), // AA
    ill(),                         // AB
    op(LDY, Absolute,   4, false), // AC
    op(LDA, Absolute,   4, false), // AD
    op(LDX, Absolute,   4, false), // AE
    ill(),                         // AF

    // 0xB0
    op(BCS, Relative,   2, false), // B0
    op(LDA, IndirectY,  5, true),  // B1
    ill(),                         // B2
    ill(),                         // B3
    op(LDY, ZeroPageX,  4, false), // B4
    op(LDA, ZeroPageX,  4, false), // B5
    op(LDX, ZeroPageY,  4, false), // B6
    ill(),                         // B7
    op(CLV, Implied,    2, false), // B8
    op(LDA, AbsoluteY,  4, true),  // B9
    op(TSX, Implied,    2, false), // BA
    ill(),                         // BB
    op(LDY, AbsoluteX,  4, true),  // BC
    op(LDA, AbsoluteX,  4, true),  // BD
    op(LDX, AbsoluteY,  4, true),  // BE
    ill(),                         // BF

    // 0xC0
    op(CPY, Immediate,  2, false), // C0
    op(CMP, IndirectX,  6, false), // C1
    ill(),                         // C2
    ill(),                         // C3
    op(CPY, ZeroPage,   3, false), // C4
    op(CMP, ZeroPage,   3, false), // C5
    op(DEC, ZeroPage,   5, false), // C6
    ill(),                         // C7
    op(INY, Implied,    2, false), // C8
    op(CMP, Immediate,  2, false), // C9
    op(DEX, Implied,    2, false), // CA
    ill(),                         // CB
    op(CPY, Absolute,   4, false), // CC
    op(CMP, Absolute,   4, false), // CD
    op(DEC, Absolute,   6, false), // CE
    ill(),                         // CF

    // 0xD0
    op(BNE, Relative,   2, false), // D0
    op(CMP, IndirectY,  5, true),  // D1
    ill(),                         // D2
    ill(),                         // D3
    ill(),                         // D4
    op(CMP, ZeroPageX,  4, false), // D5
    op(DEC, ZeroPageX,  6, false), // D6
    ill(),                         // D7
    op(CLD, Implied,    2, false), // D8
    op(CMP, AbsoluteY,  4, true),  // D9
    ill(),                         // DA
    ill(),                         // DB
    ill(),                         // DC
    op(CMP, AbsoluteX,  4, true),  // DD
    op(DEC, AbsoluteX,  7, false), // DE
    ill(),                         // DF

    // 0xE0
    op(CPX, Immediate,  2, false), // E0
    op(SBC, IndirectX,  6, false), // E1
    ill(),                         // E2
    ill(),                         // E3
    op(CPX, ZeroPage,   3, false), // E4
    op(SBC, ZeroPage,   3, false), // E5
    op(INC, ZeroPage,   5, false), // E6
    ill(),                         // E7
    op(INX, Implied,    2, false), // E8
    op(SBC, Immediate,  2, false), // E9
    op(NOP, Implied,    2, false), // EA
    ill(),                         // EB
    op(CPX, Absolute,   4, false), // EC
    op(SBC, Absolute,   4, false), // ED
    op(INC, Absolute,   6, false), // EE
    ill(),                         // EF

    // 0xF0
    op(BEQ, Relative,   2, false), // F0
    op(SBC, IndirectY,  5, true),  // F1
    ill(),                         // F2
    ill(),                         // F3
    ill(),                         // F4
    op(SBC, ZeroPageX,  4, false), // F5
    op(INC, ZeroPageX,  6, false), // F6
    ill(),                         // F7
    op(SED, Implied,    2, false), // F8
    op(SBC, AbsoluteY,  4, true),  // F9
    ill(),                         // FA
    ill(),                         // FB
    ill(),                         // FC
    op(SBC, AbsoluteX,  4, true),  // FD
    op(INC, AbsoluteX,  7, false), // FE
    ill(),                         // FF
];
