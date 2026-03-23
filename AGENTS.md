# A2VM Knowledge Base

**Apple II/II+ Emulator** — Rust core with TUI and GUI frontends.

## Quick Reference

| Task | Location | Notes |
|------|----------|-------|
| CPU implementation | `a2vm-core/src/cpu/` | 6502 with 13 addressing modes |
| Bus + soft switches | `a2vm-core/src/bus.rs`, `a2vm-core/src/machine/` | Keyboard, display, speaker, disk I/O |
| Disk II | `a2vm-core/src/disk.rs` | `.dsk` load, nibblized tracks, RWTS fast-disk trap |
| Speaker audio | `a2vm-core/src/audio.rs` | `$C030` toggles -> PCM samples |
| Keyboard mapping | `a2vm-core/src/keyboard.rs` | `AppleKey` enum, ASCII/control translation |
| Shared timing | `a2vm-core/src/timing.rs` | `CPU_HZ` constant |
| Video renderer | `a2vm-core/src/video/` | TEXT/GR/HGR render + RGBA output |
| Shared CLI args | `a2vm-oxide/src/cli.rs` | `SharedArgs`, embedded default ROM |
| Shared frontend runtime | `a2vm-oxide/src/runner.rs` | `EmulatorRunner` for emulation/audio/noise/turbo |
| Mechanical noise | `a2vm-oxide/src/noise.rs` | Disk motor/seek event tracking |
| FFI C library | `a2vm-ffi/src/lib.rs` | C-compatible staticlib wrapping EmulatorRunner |
| TUI runtime | `a2vm-tui/src/main.rs`, `a2vm-tui/src/cli.rs` | Braille display (140×48), terminal controls |
| GUI runtime (cross-platform) | `a2vm-gui/src/main.rs`, `a2vm-gui/src/cli.rs` | winit/pixels (to be replaced by native frontends) |
| macOS native frontend | `a2vm-macos/` | Swift + AppKit + Metal, CRT effects |
| Web frontend (wasm) | `a2vm-web/src/lib.rs`, `a2vm-web/www/` | wasm-bindgen + WebGPU, CRT effects |

## Project Structure

详细目录结构见 [docs/architecture.md](docs/architecture.md)。

## Key Conventions

**CPU-Bus Pattern:** `AppleII` owns `Cpu` and `BusState` directly. CPU executes against mutable bus.

**Frontend Runtime Pattern:** TUI and GUI should delegate emulation loop concerns (cycle accumulation, turbo, audio, mechanical noise, flush-on-drop) to `a2vm_oxide::runner::EmulatorRunner`.

**Native Frontend Pattern:** Platform-native frontends (macOS, etc.) link `a2vm-ffi` (C static library) rather than using Rust crates directly. `a2vm-gui` (winit/pixels) is the cross-platform fallback, to be replaced by native frontends per platform.

**macOS Build:** `make macos-app` builds `a2vm-ffi` (Rust) + Metal shaders + Swift app → `a2vm-macos/build/A2VM.app`. Uses `swiftc` directly (no SPM/Xcode).

**ROM Support:** Accepted ROM sizes are 12K (`0x3000`) and 20K (`0x5000`). Default ROM is embedded in `a2vm-oxide`.

**Soft Switches:**
- `$C010`: clears keyboard strobe
- `$C030`: toggles speaker latch (sound)
- `$C050-$C057`: display mode control
- `$C0E0-$C0EF`: Disk II controller

**Audio System:**
- Speaker: `machine/` records `$C030` toggle timestamps; `audio.rs` renders PCM; frontends play via `rodio`
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

# GUI (cross-platform, winit/pixels) -------------------------

# Build GUI release (with audio)
cargo build --release -p a2vm-gui

# Run GUI with embedded ROM
cargo run -p a2vm-gui

# GUI with disk + monochrome scanlines
cargo run -p a2vm-gui -- --disk "disks/Apple DOS 3.3 January 1983.dsk" --color-mode mono-scanlines

# macOS native (Swift + Metal) --------------------------------

# Build .app bundle
make macos-app

# Build and launch
make run-app

# Web (wasm + WebGPU) ----------------------------------------

# Build wasm package
wasm-pack build --target web a2vm-web

# Serve locally
cd a2vm-web && python3 -m http.server 8080
# Open http://localhost:8080/www/
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
