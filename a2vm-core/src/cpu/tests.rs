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
