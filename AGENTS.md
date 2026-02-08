# A2VM Knowledge Base

**Apple II Emulator** — Rust core + Swift frontend architecture.

## Quick Reference

| Task | Location | Notes |
|------|----------|-------|
| CPU implementation | `a2vm-core/src/cpu/` | 6502 with 13 addressing modes |
| Bus trait | `a2vm-core/src/bus.rs` | Hardware abstraction layer |
| Memory | `a2vm-core/src/memory.rs` | FlatMemory for tests |
| Architecture docs | `docs/architecture.md` | Full design spec |
| CPU tests | `a2vm-core/tests/` | Klaus Dormann functional test |

## Project Structure

```
a2vm/
├── Cargo.toml              # Workspace root
├── a2vm-core/              # Rust core library
│   ├── src/
│   │   ├── lib.rs          # Exports: bus, cpu, memory
│   │   ├── bus.rs          # Bus trait (read/write)
│   │   ├── memory.rs       # FlatMemory impl
│   │   └── cpu/            # 6502 implementation
│   │       ├── mod.rs      # Cpu struct, step(), execute()
│   │       ├── opcodes.rs  # 256-entry opcode table
│   │       ├── addressing.rs
│   │       └── status.rs   # Status register flags
│   └── tests/
│       └── klaus_dormann.rs
└── docs/
    ├── architecture.md     # System design
    └── milestones.md       # Development roadmap
```

## Key Conventions

**CPU-Bus Pattern:** `AppleII` owns both CPU and Bus impl. Use `std::mem::replace` to temporarily extract CPU during execution to avoid borrow conflicts.

**Opcode Table:** Static 256-entry array indexed by opcode byte. Each entry: `{mnemonic, mode, cycles, page_penalty}`.

**Status Register:** Bit flags accessed via constants `C, Z, I, D, B, U, V, N`. Use `set_nz()` for N/Z flag updates.

## 6502 Traps (NMOS)

| Issue | Location | Details |
|-------|----------|---------|
| BCD flags | `adc_bcd()`, `sbc_bcd()` | N/Z from binary result, not BCD |
| JMP indirect | `resolve()` | Page wrap bug: JMP ($xxFF) reads high byte from $xx00 |
| BRK | `execute()` | Pushes PC+2, sets B=1 |
| RTI vs RTS | `execute()` | RTI restores exact PC; RTS adds 1 |

## Commands

```bash
# Run tests
cargo test

# Run CPU functional test
cargo test klaus_dormann

# Build
cargo build --release
```

## Testing

Uses [Klaus Dormann's 6502 functional test](https://github.com/Klaus2m5/6502_65C02_functional_tests). Binary in `a2vm-core/tests/data/`. Test passes if CPU runs to completion at address $3399.

## FFI (Future)

C ABI for Swift frontend:
- `a2vm_create()` / `a2vm_destroy()`
- `a2vm_run_frame()` — 17030 cycles (1 video frame)
- `a2vm_key_down/up()` — keyboard input
- `a2vm_get_cpu_state()` — debug inspection
