# A2VM Knowledge Base

**Apple II/II+ Emulator** — Rust core + terminal frontend.

## Quick Reference

| Task | Location | Notes |
|------|----------|-------|
| CPU implementation | `a2vm-core/src/cpu/` | 6502 with 13 addressing modes |
| Bus + soft switches | `a2vm-core/src/bus.rs`, `a2vm-core/src/machine.rs` | Keyboard, display, speaker, disk I/O |
| Disk II | `a2vm-core/src/disk.rs` | `.dsk` load, nibblized track reads |
| Speaker audio | `a2vm-core/src/audio.rs` | `$C030` toggles -> PCM samples |
| Video renderer | `a2vm-core/src/video.rs` | TEXT/GR/HGR bitmap pipeline |
| TUI runtime | `a2vm-tui/src/main.rs` | Braille display, keyboard, audio playback |

## Project Structure

```
a2vm/
├── Cargo.toml              # Workspace root
├── a2vm-core/              # Rust core library
│   ├── src/
│   │   ├── lib.rs          # Exports: audio, bus, cpu, disk, machine, memory, video
│   │   ├── audio.rs        # Speaker toggle timestamps -> PCM
│   │   ├── bus.rs          # Bus trait and utility helpers
│   │   ├── disk.rs         # Disk II controller
│   │   ├── machine.rs      # AppleII system integration
│   │   ├── memory.rs       # FlatMemory impl for tests
│   │   ├── video.rs        # TEXT/GR/HGR renderer
│   │   └── cpu/            # 6502 implementation
│   │       ├── mod.rs
│   │       ├── opcodes.rs
│   │       ├── addressing.rs
│   │       └── status.rs
│   └── tests/
│       └── klaus_dormann.rs
├── a2vm-tui/               # Terminal frontend
│   └── src/main.rs
└── docs/
    ├── architecture.md
    └── milestones.md
```

## Key Conventions

**CPU-Bus Pattern:** `AppleII` owns both CPU and Bus impl. Use `std::mem::take` to temporarily extract CPU during execution.

**ROM Support:** Only Apple II / Apple II+ ROM sizes are accepted: 12K (`0x3000`) and 20K (`0x5000`).

**Soft Switches:**
- `$C010`: clears keyboard strobe
- `$C030`: toggles speaker latch (sound)
- `$C050-$C057`: display mode control
- `$C0E0-$C0EF`: Disk II controller

**Audio Path:** `machine.rs` records speaker toggles at `$C030`; `audio.rs` converts cycle-timestamped toggles into PCM via `render_until`; `a2vm-tui` plays samples with `rodio`.

## 6502 Traps (NMOS)

| Issue | Location | Details |
|-------|----------|---------|
| BCD flags | `adc_bcd()`, `sbc_bcd()` | N/Z from binary result, not BCD |
| JMP indirect | `resolve()` | Page wrap bug: JMP ($xxFF) reads high byte from $xx00 |
| BRK | `execute()` | Pushes PC+2, sets B=1 |
| RTI vs RTS | `execute()` | RTI restores exact PC; RTS adds 1 |

## Commands

```bash
# Run all tests
cargo test

# Run CPU functional test
cargo test klaus_dormann

# Build release (with audio support)
cargo build --release

# Build without audio (if ALSA/libasound is unavailable)
cargo build --release -p a2vm-tui --no-default-features

# Run Apple II+ with disk
cargo run -p a2vm-tui -- --rom roms/apple2p.rom --disk "disks/Apple DOS 3.3 January 1983.dsk"

# Fast-disk mode (DOS 3.3 only)
cargo run -p a2vm-tui -- --rom roms/apple2p.rom --fast-disk "disks/Apple DOS 3.3 January 1983.dsk"
```

### CLI Reference

```
a2vm-tui --rom <file> [--disk <file> | --fast-disk <file>]

  --rom <file>        Apple II/II+ ROM (12K or 20K) [env: A2VM_ROM]
  --disk <file>       .dsk disk image (143360 bytes)
  --fast-disk <file>  .dsk image with DOS 3.3 RWTS trap ($B7B5) for instant reads
  -h, --help          Show this help
```

- `--rom` required; falls back to `A2VM_ROM` env var
- `--disk` and `--fast-disk` are mutually exclusive
- `--fast-disk` only works with DOS 3.3 formatted disks

## Testing

Uses [Klaus Dormann's 6502 functional test](https://github.com/Klaus2m5/6502_65C02_functional_tests). Binary in `a2vm-core/tests/data/`. Test passes if CPU runs to completion at address `$3399`.
