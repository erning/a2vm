# A2VM Knowledge Base

**Apple II/II+ Emulator** — Rust core with TUI and GUI frontends.

## Quick Reference

| Task | Location | Notes |
|------|----------|-------|
| CPU implementation | `a2vm-core/src/cpu/` | 6502 with 13 addressing modes |
| Bus + soft switches | `a2vm-core/src/bus.rs`, `a2vm-core/src/machine.rs` | Keyboard, display, speaker, disk I/O |
| Disk II | `a2vm-core/src/disk.rs` | `.dsk` load, nibblized tracks, RWTS fast-disk trap |
| Speaker audio | `a2vm-core/src/audio.rs` | `$C030` toggles -> PCM samples |
| Keyboard mapping | `a2vm-core/src/keyboard.rs` | `AppleKey` enum, ASCII/control translation |
| Shared timing | `a2vm-core/src/timing.rs` | `CPU_HZ` constant |
| Video renderer | `a2vm-core/src/video/` | TEXT/GR/HGR render + RGBA output |
| Shared CLI args | `a2vm-oxide/src/cli.rs` | `SharedArgs`, embedded default ROM |
| Shared frontend runtime | `a2vm-oxide/src/runner.rs` | `EmulatorRunner` for emulation/audio/noise/turbo |
| Mechanical noise | `a2vm-oxide/src/noise.rs` | Disk motor/seek event tracking |
| TUI runtime | `a2vm-tui/src/main.rs`, `a2vm-tui/src/cli.rs` | Braille display (140×48), terminal controls |
| GUI runtime | `a2vm-gui/src/main.rs`, `a2vm-gui/src/cli.rs` | Native window (280×192), color-mode options |

## Project Structure

详细目录结构见 [docs/architecture.md](docs/architecture.md)。

## Key Conventions

**CPU-Bus Pattern:** `AppleII` owns `Cpu` and `BusState` directly. CPU executes against mutable bus.

**Frontend Runtime Pattern:** TUI and GUI should delegate emulation loop concerns (cycle accumulation, turbo, audio, mechanical noise, flush-on-drop) to `a2vm_oxide::runner::EmulatorRunner`.

**ROM Support:** Accepted ROM sizes are 12K (`0x3000`) and 20K (`0x5000`). Default ROM is embedded in `a2vm-oxide`.

**Soft Switches:**
- `$C010`: clears keyboard strobe
- `$C030`: toggles speaker latch (sound)
- `$C050-$C057`: display mode control
- `$C0E0-$C0EF`: Disk II controller

**Audio System:**
- Speaker: `machine.rs` records `$C030` toggle timestamps; `audio.rs` renders PCM; frontends play via `rodio`
- Mechanical: `a2vm-oxide::noise` tracks motor/track state and emits `MotorStart`/`TrackSeek`/`MotorStop`

## 6502 Traps (NMOS)

| Issue | Location | Details |
|-------|----------|---------|
| BCD flags | `adc_bcd()`, `sbc_bcd()` | N/Z from binary result, not BCD |
| JMP indirect | `resolve()` | JMP `($xxFF)` reads high byte from `$xx00` |
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

# Run TUI with embedded ROM
cargo run -p a2vm-tui

# Run TUI with custom ROM + disk
cargo run -p a2vm-tui -- --rom roms/apple2p.rom --disk "disks/Apple DOS 3.3 January 1983.dsk"

# TUI with fast-disk + mechanical noise
cargo run -p a2vm-tui -- --disk "disks/Apple DOS 3.3 January 1983.dsk" --fast-disk --noise

# GUI --------------------------------------------------------

# Build GUI release (with audio)
cargo build --release -p a2vm-gui

# Build GUI without audio
cargo build --release -p a2vm-gui --no-default-features

# Run GUI with embedded ROM
cargo run -p a2vm-gui

# GUI with disk + monochrome scanlines
cargo run -p a2vm-gui -- --disk "disks/Apple DOS 3.3 January 1983.dsk" --color-mode mono-scanlines

# GUI with mechanical noise
cargo run -p a2vm-gui -- --disk "disks/Apple DOS 3.3 January 1983.dsk" --noise
```

### CLI Reference

Shared options:

```text
a2vm-tui|a2vm-gui [options]

  --rom <FILE>        Apple II/II+ ROM (12K or 20K). Optional; uses embedded ROM if not specified.
  --disk <FILE>       .dsk disk image (143360 bytes), up to two times
  --fast-disk         Enable DOS 3.3 RWTS trap ($B7B5) for all drives
  --noise             Enable realistic mechanical noise simulation
  -h, --help          Show this help
```

GUI-only option:

```text
a2vm-gui ... [--color-mode <mode>]

  --color-mode <mode> Display mode: 'color' (default), 'mono', 'mono-scanlines'
```

- `--rom` is optional; embedded Apple II+ ROM is used by default
- `--disk` can be provided zero, one, or two times (drive 1 then drive 2)
- `--fast-disk` works best with DOS 3.3 formatted disks
- `--noise` requires audio-enabled build (`default` feature set)

## Testing

Uses [Klaus Dormann's 6502 functional test](https://github.com/Klaus2m5/6502_65C02_functional_tests). Binary in `a2vm-core/tests/data/`. Test passes if CPU runs to completion at address `$3469`.
