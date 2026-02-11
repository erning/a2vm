use a2vm_core::cpu::Cpu;
use a2vm_core::memory::FlatMemory;

const SUCCESS_TRAP: u16 = 0x3469;
const MAX_CYCLES: u64 = 100_000_000;

#[test]
fn klaus_dormann_functional_test() {
    let bin = include_bytes!("data/6502_functional_test.bin");

    let mut mem = FlatMemory::new();
    mem.data[..bin.len()].copy_from_slice(bin);

    let mut cpu = Cpu::new();
    cpu.set_pc(0x0400);

    let mut prev_pc = cpu.pc();
    let mut same_pc_count = 0u32;

    loop {
        cpu.step(&mut mem);

        if cpu.pc() == prev_pc {
            same_pc_count += 1;
            if same_pc_count > 2 {
                // CPU is stuck in a trap (tight loop to self)
                break;
            }
        } else {
            same_pc_count = 0;
            prev_pc = cpu.pc();
        }

        if cpu.cycles() > MAX_CYCLES {
            panic!(
                "Test did not complete within {} cycles. PC={:#06X}",
                MAX_CYCLES,
                cpu.pc()
            );
        }
    }

    assert_eq!(
        cpu.pc(),
        SUCCESS_TRAP,
        "Test FAILED: trapped at PC={:#06X}, expected success trap at {:#06X}",
        cpu.pc(),
        SUCCESS_TRAP
    );

    eprintln!(
        "Klaus Dormann 6502 functional test PASSED! Cycles: {}",
        cpu.cycles()
    );
}
