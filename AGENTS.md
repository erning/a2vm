# A2VM Knowledge Base

**Apple II/II+ Emulator** — Rust core with TUI and GUI frontends.

## Quick Reference

| Task | Location | Notes |
|------|----------|-------|
| CPU implementation | `a2vm-core/src/cpu/` | 6502 with 13 addressing modes |
| Bus + soft switches | `a2vm-core/src/bus.rs`, `a2vm-core/src/machine.rs` | Keyboard, display, speaker, disk I/O |
| Disk II | `a2vm-core/src/disk.rs` | `.dsk` load, nibblized track reads, fast-disk trap |
| Speaker audio | `a2vm-core/src/audio.rs` | `$C030` toggles -> PCM samples |
| Keyboard mapping | `a2vm-core/src/keyboard.rs` | `AppleKey` enum, ASCII translation |
| Shared timing | `a2vm-core/src/timing.rs` | `CPU_HZ` for cycle timing |
| Video renderer | `a2vm-core/src/video.rs` | TEXT/GR/HGR bitmap pipeline + RGBA output |
| Mechanical noise | `a2vm-oxide/src/noise.rs` | Disk II motor/seek sound events |
| TUI runtime | `a2vm-tui/src/main.rs`, `a2vm-tui/src/cli.rs` | Braille display (140×48), `TuiApp` struct |
| GUI runtime | `a2vm-gui/src/main.rs`, `a2vm-gui/src/cli.rs` | Native window (280×192), `App` struct |

## Project Structure

```
a2vm/
├── Cargo.toml              # Workspace root (4 crates)
├── assets/                 # Audio assets
│   ├── move_arm.wav        # Disk stepper motor (embedded in a2vm-oxide)
│   ├── disk_insertion.wav
│   ├── disk_removal.wav
│   ├── pop_on.wav
│   └── pop_off.wav
├── a2vm-core/              # Core emulation library
│   ├── src/
│   │   ├── lib.rs          # Exports: audio, bus, cpu, disk, error, keyboard, machine, memory, timing, video
│   │   ├── audio.rs        # Speaker synthesis (toggle timestamps -> PCM)
│   │   ├── bus.rs          # Bus trait for CPU-device communication
│   │   ├── disk.rs         # Disk II controller + RWTS trap
│   │   ├── error.rs        # Error types
│   │   ├── keyboard.rs     # Apple II key mapping
│   │   ├── machine.rs      # AppleII system + BusState
│   │   ├── memory.rs       # FlatMemory for CPU tests
│   │   ├── timing.rs       # CPU_HZ constant
│   │   ├── video.rs        # TEXT/GR/HGR renderer
│   │   └── cpu/            # 6502 implementation
│   │       ├── mod.rs      # Cpu struct, step(), interrupts
│   │       ├── opcodes.rs  # Mnemonic enum, OPCODES table
│   │       ├── addressing.rs # 13 addressing modes
│   │       ├── disasm.rs   # Disassembler
│   │       ├── status.rs   # Status register (P)
│   │       └── tests.rs    # BCD arithmetic tests
│   └── tests/
│       └── klaus_dormann.rs # 6502 functional test
├── a2vm-oxide/             # Shared frontend resources
│   ├── src/
│   │   ├── lib.rs          # Exports: noise
│   │   └── noise.rs        # DiskMechTracker, MechanicalEvent, MOVE_ARM_WAV
├── a2vm-tui/               # Terminal frontend
│   ├── src/
│   │   ├── main.rs         # TuiApp struct, Braille display
│   │   └── cli.rs          # Clap CLI definition
├── a2vm-gui/               # Graphical frontend
│   ├── src/
│   │   ├── main.rs         # App struct, pixels+wgpu
│   │   └── cli.rs          # Clap CLI definition
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

**Audio System:**
- Speaker: `machine.rs` records toggles at `$C030`; `audio.rs` converts to PCM; frontends play via `rodio`
- Mechanical: `a2vm-oxide::noise` tracks motor/track state; emits `MotorStart`/`TrackSeek`/`MotorStop` events

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

# Build TUI release (with audio)
cargo build --release -p a2vm-tui

# Build TUI without audio
cargo build --release -p a2vm-tui --no-default-features

# Run TUI with disk
cargo run -p a2vm-tui -- --rom roms/apple2p.rom --disk "disks/Apple DOS 3.3 January 1983.dsk"

# TUI with fast-disk + mechanical noise
cargo run -p a2vm-tui -- --rom roms/apple2p.rom --disk "disks/Apple DOS 3.3 January 1983.dsk" --fast-disk --noise

# GUI --------------------------------------------------------

# Build GUI release (with audio)
cargo build --release -p a2vm-gui

# Build GUI without audio
cargo build --release -p a2vm-gui --no-default-features

# Run GUI
cargo run -p a2vm-gui -- --rom roms/apple2p.rom

# GUI with disk + monochrome scanlines
cargo run -p a2vm-gui -- --rom roms/apple2p.rom --disk "disks/Apple DOS 3.3 January 1983.dsk" --color-mode mono-scanlines

# GUI with mechanical noise
cargo run -p a2vm-gui -- --rom roms/apple2p.rom --disk "disks/Apple DOS 3.3 January 1983.dsk" --noise
```

### CLI Reference

Shared options:

```
a2vm-tui|a2vm-gui --rom <file> [options]

  --rom <file>        Apple II/II+ ROM (12K or 20K) [env: A2VM_ROM]
  --disk <file>       .dsk disk image (143360 bytes), up to two times
  --fast-disk         Enable DOS 3.3 RWTS trap ($B7B5) for all drives
  --noise             Enable realistic mechanical noise simulation
  -h, --help          Show this help
```

GUI-only option:

```
a2vm-gui ... [--color-mode <mode>]

  --color-mode <mode> Display mode: 'color' (default), 'mono', 'mono-scanlines'
```

- `--rom` required; falls back to `A2VM_ROM` env var
- `--disk` can be provided zero, one, or two times (drive 1 then drive 2)
- `--fast-disk` works best with DOS 3.3 formatted disks
- `--noise` plays `move_arm.wav` looped during disk motor activity

## Testing

Uses [Klaus Dormann's 6502 functional test](https://github.com/Klaus2m5/6502_65C02_functional_tests). Binary in `a2vm-core/tests/data/`. Test passes if CPU runs to completion at address `$3469`.
