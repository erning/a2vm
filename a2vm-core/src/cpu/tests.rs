use crate::memory::FlatMemory;

use super::status::{C, N, Z};
use super::Cpu;

fn run_steps(cpu: &mut Cpu, mem: &mut FlatMemory, count: usize) {
    for _ in 0..count {
        cpu.step(mem);
    }
}

#[test]
fn adc_decimal_sets_nz_from_binary_result() {
    let mut mem = FlatMemory::new();
    mem.data[0x0000] = 0xF8;
    mem.data[0x0001] = 0x18;
    mem.data[0x0002] = 0xA9;
    mem.data[0x0003] = 0x50;
    mem.data[0x0004] = 0x69;
    mem.data[0x0005] = 0x50;

    let mut cpu = Cpu::new();
    cpu.pc = 0x0000;
    run_steps(&mut cpu, &mut mem, 4);

    assert_eq!(cpu.a, 0x00);
    assert!(cpu.p.get(C));
    assert!(cpu.p.get(N));
    assert!(!cpu.p.get(Z));
}

#[test]
fn sbc_decimal_sets_nz_from_binary_result() {
    let mut mem = FlatMemory::new();
    mem.data[0x0000] = 0xF8;
    mem.data[0x0001] = 0x38;
    mem.data[0x0002] = 0xA9;
    mem.data[0x0003] = 0x00;
    mem.data[0x0004] = 0xE9;
    mem.data[0x0005] = 0x01;

    let mut cpu = Cpu::new();
    cpu.pc = 0x0000;
    run_steps(&mut cpu, &mut mem, 4);

    assert_eq!(cpu.a, 0x99);
    assert!(!cpu.p.get(C));
    assert!(cpu.p.get(N));
    assert!(!cpu.p.get(Z));
}

#[test]
fn asl_memory_sets_carry_and_result() {
    let mut mem = FlatMemory::new();
    mem.data[0x0000] = 0x06;
    mem.data[0x0001] = 0x10;
    mem.data[0x0010] = 0x81;

    let mut cpu = Cpu::new();
    cpu.pc = 0x0000;
    cpu.step(&mut mem);

    assert_eq!(mem.data[0x0010], 0x02);
    assert!(cpu.p.get(C));
    assert!(!cpu.p.get(N));
    assert!(!cpu.p.get(Z));
}

#[test]
fn beq_cross_page_adds_extra_cycle() {
    let mut mem = FlatMemory::new();
    mem.data[0x00FE] = 0xF0;
    mem.data[0x00FF] = 0xFE;

    let mut cpu = Cpu::new();
    cpu.pc = 0x00FE;
    cpu.p.set(Z, true);
    let cycles = cpu.step(&mut mem);

    assert_eq!(cycles, 4);
    assert_eq!(cpu.pc, 0x00FE);
}

#[test]
fn jmp_indirect_wraps_high_byte_within_page() {
    let mut mem = FlatMemory::new();
    mem.data[0x0200] = 0x6C;
    mem.data[0x0201] = 0xFF;
    mem.data[0x0202] = 0x10;
    mem.data[0x10FF] = 0x34;
    mem.data[0x1000] = 0x12;
    mem.data[0x1100] = 0x99;

    let mut cpu = Cpu::new();
    cpu.pc = 0x0200;
    cpu.step(&mut mem);

    assert_eq!(cpu.pc, 0x1234);
}

// Illegal opcodes tests

#[test]
fn lax_loads_a_and_x_and_sets_flags() {
    let mut mem = FlatMemory::new();
    mem.data[0x0000] = 0xA7;
    mem.data[0x0001] = 0x10;
    mem.data[0x0010] = 0x42;

    let mut cpu = Cpu::new();
    cpu.pc = 0x0000;
    cpu.step(&mut mem);

    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.x, 0x42);
    assert!(!cpu.p.get(N));
    assert!(!cpu.p.get(Z));
}

#[test]
fn lax_sets_negative_flag() {
    let mut mem = FlatMemory::new();
    mem.data[0x0000] = 0xA7;
    mem.data[0x0001] = 0x10;
    mem.data[0x0010] = 0x80;

    let mut cpu = Cpu::new();
    cpu.pc = 0x0000;
    cpu.step(&mut mem);

    assert_eq!(cpu.a, 0x80);
    assert_eq!(cpu.x, 0x80);
    assert!(cpu.p.get(N));
    assert!(!cpu.p.get(Z));
}

#[test]
fn sax_stores_a_and_x() {
    let mut mem = FlatMemory::new();
    mem.data[0x0000] = 0x87; // SAX ZeroPage
    mem.data[0x0001] = 0x10;

    let mut cpu = Cpu::new();
    cpu.pc = 0x0000;
    cpu.a = 0xFF;
    cpu.x = 0x0F;
    cpu.step(&mut mem);

    assert_eq!(mem.data[0x0010], 0x0F); // A & X = 0xFF & 0x0F = 0x0F
}

#[test]
fn sax_indirect_x_stores_a_and_x() {
    let mut mem = FlatMemory::new();
    // SAX (zp, X) - 0x83
    mem.data[0x0000] = 0x83;
    mem.data[0x0001] = 0x20; // zero page base
    mem.data[0x0025] = 0x34; // lo byte of target addr (0x20 + 0x05 = 0x25)
    mem.data[0x0026] = 0x12; // hi byte of target addr

    let mut cpu = Cpu::new();
    cpu.pc = 0x0000;
    cpu.x = 0x05; // X index
    cpu.a = 0xAA;
    cpu.step(&mut mem);

    assert_eq!(mem.data[0x1234], 0x00); // A & X = 0xAA & 0x05 = 0x00
}

#[test]
fn sax_absolute_stores_a_and_x() {
    let mut mem = FlatMemory::new();
    // SAX $1234 - 0x8F
    mem.data[0x0000] = 0x8F;
    mem.data[0x0001] = 0x34; // lo byte
    mem.data[0x0002] = 0x12; // hi byte

    let mut cpu = Cpu::new();
    cpu.pc = 0x0000;
    cpu.a = 0xF0;
    cpu.x = 0x0F;
    cpu.step(&mut mem);

    assert_eq!(mem.data[0x1234], 0x00); // A & X = 0xF0 & 0x0F = 0x00
}

#[test]
fn dcp_decrements_and_compares() {
    let mut mem = FlatMemory::new();
    mem.data[0x0000] = 0xC7;
    mem.data[0x0001] = 0x10;
    mem.data[0x0010] = 0x05;

    let mut cpu = Cpu::new();
    cpu.pc = 0x0000;
    cpu.a = 0x04;
    cpu.step(&mut mem);

    assert_eq!(mem.data[0x0010], 0x04);
    assert!(cpu.p.get(C));
    assert!(cpu.p.get(Z));
    assert!(!cpu.p.get(N));
}

#[test]
fn isc_increments_and_sbc() {
    let mut mem = FlatMemory::new();
    mem.data[0x0000] = 0xE7;
    mem.data[0x0001] = 0x10;
    mem.data[0x0010] = 0x05;

    let mut cpu = Cpu::new();
    cpu.pc = 0x0000;
    cpu.p.set(C, true);
    cpu.a = 0x10;
    cpu.step(&mut mem);

    assert_eq!(mem.data[0x0010], 0x06);
    assert_eq!(cpu.a, 0x0A);
}

#[test]
fn slo_shifts_left_and_ora() {
    let mut mem = FlatMemory::new();
    mem.data[0x0000] = 0x07;
    mem.data[0x0001] = 0x10;
    mem.data[0x0010] = 0x81;

    let mut cpu = Cpu::new();
    cpu.pc = 0x0000;
    cpu.a = 0x01;
    cpu.step(&mut mem);

    assert_eq!(mem.data[0x0010], 0x02);
    assert_eq!(cpu.a, 0x03);
    assert!(cpu.p.get(C));
    assert!(!cpu.p.get(N));
}

#[test]
fn rla_rotates_left_and_ands() {
    let mut mem = FlatMemory::new();
    mem.data[0x0000] = 0x27;
    mem.data[0x0001] = 0x10;
    mem.data[0x0010] = 0x81;

    let mut cpu = Cpu::new();
    cpu.pc = 0x0000;
    cpu.p.set(C, false);
    cpu.a = 0xFF;
    cpu.step(&mut mem);

    assert_eq!(mem.data[0x0010], 0x02);
    assert_eq!(cpu.a, 0x02);
    assert!(cpu.p.get(C));
    assert!(!cpu.p.get(N));
}

#[test]
fn rra_rotates_right_and_adc() {
    let mut mem = FlatMemory::new();
    mem.data[0x0000] = 0x67;
    mem.data[0x0001] = 0x10;
    mem.data[0x0010] = 0x82;

    let mut cpu = Cpu::new();
    cpu.pc = 0x0000;
    cpu.p.set(C, true);
    cpu.a = 0x10;
    cpu.step(&mut mem);

    // 0x82 ROR with carry_in=1: (0x82 >> 1) | 0x80 = 0x41 | 0x80 = 0xC1
    assert_eq!(mem.data[0x0010], 0xC1);
    // ADC: 0x10 + 0xC1 + 0(new carry from ROR) = 0xD1
    assert_eq!(cpu.a, 0xD1);
    assert!(!cpu.p.get(C));
}

#[test]
fn sre_shifts_right_and_eors() {
    let mut mem = FlatMemory::new();
    mem.data[0x0000] = 0x47;
    mem.data[0x0001] = 0x10;
    mem.data[0x0010] = 0x82;

    let mut cpu = Cpu::new();
    cpu.pc = 0x0000;
    cpu.a = 0xFF;
    cpu.step(&mut mem);

    assert_eq!(mem.data[0x0010], 0x41);
    assert_eq!(cpu.a, 0xBE);
    assert!(!cpu.p.get(C));
}
