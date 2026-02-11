# A2VM Knowledge Base

**Apple II/II+ Emulator** — Rust core with TUI and GUI frontends.

## Quick Reference

| Task | Location | Notes |
|------|----------|-------|
| CPU implementation | `a2vm-core/src/cpu/` | 6502 with 13 addressing modes |
| Bus + soft switches | `a2vm-core/src/bus.rs`, `a2vm-core/src/machine.rs` | Keyboard, display, speaker, disk I/O |
| Disk II | `a2vm-core/src/disk.rs` | `.dsk` load, nibblized track reads |
| Speaker audio | `a2vm-core/src/audio.rs` | `$C030` toggles -> PCM samples |
| Shared timing constants | `a2vm-core/src/timing.rs` | `CPU_HZ` for cycle timing |
| Video renderer | `a2vm-core/src/video.rs` | TEXT/GR/HGR bitmap pipeline + RGBA output |
| TUI runtime | `a2vm-tui/src/main.rs`, `a2vm-tui/src/cli.rs` | Braille display (140×48), keyboard, clap CLI |
| GUI runtime | `a2vm-gui/src/main.rs`, `a2vm-gui/src/cli.rs` | Native window (280×192), pixels+wgpu, clap CLI |

## Project Structure

```
a2vm/
├── Cargo.toml              # Workspace root
├── a2vm-core/              # Rust core library
│   ├── src/
│   │   ├── lib.rs          # Exports: audio, bus, cpu, disk, machine, memory, timing, video
│   │   ├── audio.rs        # Speaker toggle timestamps -> PCM
│   │   ├── bus.rs          # Bus trait and utility helpers
│   │   ├── disk.rs         # Disk II controller
│   │   ├── machine.rs      # AppleII system integration
│   │   ├── memory.rs       # FlatMemory impl for tests
│   │   ├── timing.rs       # Shared timing constants (CPU_HZ)
│   │   ├── video.rs        # TEXT/GR/HGR renderer
│   │   └── cpu/            # 6502 implementation
│   │       ├── mod.rs
│   │       ├── opcodes.rs
│   │       ├── addressing.rs
│   │       ├── disasm.rs
│   │       ├── status.rs
│   │       └── tests.rs
│   └── tests/
│       └── klaus_dormann.rs
├── a2vm-tui/               # Terminal frontend
│   └── src/
│       ├── main.rs
│       └── cli.rs
├── a2vm-gui/               # Graphical frontend
│   └── src/
│       ├── main.rs
│       └── cli.rs
└── docs/
    └── architecture.md
```

## Key Conventions

**CPU-Bus Pattern:** `AppleII` owns `Cpu` and `BusState` directly. CPU executes against mutable bus without temporary extraction patterns.

**ROM Support:** Only Apple II / Apple II+ ROM sizes are accepted: 12K (`0x3000`) and 20K (`0x5000`).

**Soft Switches:**
- `$C010`: clears keyboard strobe
- `$C030`: toggles speaker latch (sound)
- `$C050-$C057`: display mode control
- `$C0E0-$C0EF`: Disk II controller

**Audio Path:** `machine.rs` records speaker toggles at `$C030`; `audio.rs` converts cycle-timestamped toggles into PCM; frontends consume via `take_audio_samples_into` and play with `rodio` when enabled.

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

# TUI --------------------------------------------------------

# Build TUI release (with audio support)
cargo build --release -p a2vm-tui

# Build TUI without audio
cargo build --release -p a2vm-tui --no-default-features

# Run TUI with disk
cargo run -p a2vm-tui -- --rom roms/apple2p.rom --disk "disks/Apple DOS 3.3 January 1983.dsk"

# TUI fast-disk mode
cargo run -p a2vm-tui -- --rom roms/apple2p.rom --fast-disk "disks/Apple DOS 3.3 January 1983.dsk"

# GUI --------------------------------------------------------

# Build GUI release (with audio support)
cargo build --release -p a2vm-gui

# Build GUI without audio
cargo build --release -p a2vm-gui --no-default-features

# Run GUI
cargo run -p a2vm-gui -- --rom roms/apple2p.rom

# GUI with disk
cargo run -p a2vm-gui -- --rom roms/apple2p.rom --disk "disks/Apple DOS 3.3 January 1983.dsk"
```

### CLI Reference

Shared options:

```
a2vm-tui|a2vm-gui --rom <file> [--disk <file> | --fast-disk <file>]

  --rom <file>        Apple II/II+ ROM (12K or 20K) [env: A2VM_ROM]
  --disk <file>       .dsk disk image (143360 bytes)
  --fast-disk <file>  .dsk image with DOS 3.3 RWTS trap ($B7B5) for instant reads
  -h, --help          Show this help
```

GUI-only option:

```
a2vm-gui ... [--color-mode <mode>]

  --color-mode <mode> Display mode: 'color' (default), 'mono', 'mono-scanlines'
```

- `--rom` required; falls back to `A2VM_ROM` env var
- `--disk` and `--fast-disk` are mutually exclusive
- `--fast-disk` only works with DOS 3.3 formatted disks

## Testing

Uses [Klaus Dormann's 6502 functional test](https://github.com/Klaus2m5/6502_65C02_functional_tests). Binary in `a2vm-core/tests/data/`. Test passes if CPU runs to completion at address `$3399`.
