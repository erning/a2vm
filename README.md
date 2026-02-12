# A2VM — Apple II Emulator

An Apple II/II+ emulator written in Rust featuring both terminal (TUI) and graphical (GUI) frontends. Includes a complete 6502 CPU implementation, Disk II boot support, TEXT/GR/HGR rendering with authentic color, 1-bit speaker audio, mechanical disk noise, and both Braille-based terminal display and native GPU-accelerated GUI.

## Features

- **Complete 6502 CPU**: All 56 official opcodes, 13 addressing modes, BCD arithmetic, and accurate cycle counting
- **Video Modes**: TEXT (40×24), Lo-Res Graphics (40×48), Hi-Res Graphics (280×192), and Mixed mode
- **Disk II Emulation**: Loads DOS 3.3 `.dsk` images and boots from slot 6, with optional fast-disk mode
- **Speaker Audio**: `$C030` toggle timestamps synthesized to PCM and played through frontend
- **Mechanical Noise**: Optional realistic disk drive sounds (motor, seek) via `--noise` flag
- **Two Frontends**:
  - **TUI**: Terminal UI with Braille characters (140×48) using ratatui
  - **GUI**: Native GPU-accelerated window with authentic Apple II colors plus monochrome CRT modes (`color`, `mono`, `mono-scanlines`)
- **Keyboard Input**: Full ASCII keyboard support with Apple II key mapping
- **Clap-based CLI**: Optional `--rom` with embedded Apple II+ ROM as default
- **ROM Support**: Loads Apple II/II+ ROM files (12K/20K), with embedded default

## Quick Start

### TUI (Terminal UI)

```bash
# Build the TUI (with audio support, requires ALSA on Linux)
cargo build --release -p a2vm-tui

# Build without audio (if audio libraries are not available)
cargo build --release -p a2vm-tui --no-default-features

# Run with embedded ROM (no external files needed)
cargo run -p a2vm-tui

# Run with a disk image
cargo run -p a2vm-tui -- --disk "disks/Apple DOS 3.3 January 1983.dsk"

# Fast-disk mode (global switch for all mounted drives)
cargo run -p a2vm-tui -- --disk "disks/Apple DOS 3.3 January 1983.dsk" --fast-disk

# With mechanical noise simulation
cargo run -p a2vm-tui -- --disk "disks/Apple DOS 3.3 January 1983.dsk" --noise

# Mount two disks (drive 1 then drive 2)
cargo run -p a2vm-tui -- --disk "disks/Apple DOS 3.3 January 1983.dsk" --disk "disks/Programma.dsk"

# Run with custom ROM (optional)
cargo run -p a2vm-tui -- --rom custom.rom --disk "disks/Apple DOS 3.3 January 1983.dsk"
```

### GUI (Graphical UI)

```bash
# Build the GUI (with audio support)
cargo build --release -p a2vm-gui

# Build without audio
cargo build --release -p a2vm-gui --no-default-features

# Run with embedded ROM (no external files needed)
cargo run -p a2vm-gui

# Run with monochrome scanline mode
cargo run -p a2vm-gui -- --color-mode mono-scanlines

# Run with disk
cargo run -p a2vm-gui -- --disk "disks/Apple DOS 3.3 January 1983.dsk"

# Run with two disks + global fast-disk + mechanical noise
cargo run -p a2vm-gui -- --disk "disks/Apple DOS 3.3 January 1983.dsk" --disk "disks/Programma.dsk" --fast-disk --noise
```

### Command Line Options

Shared options:

```
a2vm-tui|a2vm-gui [options]

Options:
  --rom <file>        Apple II/II+ ROM file (12K or 20K). Optional; uses embedded ROM if not specified.
  --disk <file>       .dsk disk image (143360 bytes), may be passed up to two times
  --fast-disk         Enable DOS 3.3 RWTS fast path for all mounted drives
  --noise             Enable realistic mechanical noise simulation
  -h, --help          Show this help
```

GUI-only option:

```
a2vm-gui ... [--color-mode <mode>]

  --color-mode <mode> Display mode: 'color' (default), 'mono', 'mono-scanlines'
```

- `--rom` is optional; embedded Apple II+ ROM is used by default
- `--disk` can be omitted, provided once, or provided twice (first maps to drive 1, second to drive 2)
- `--fast-disk` is a global switch that applies to all mounted drives
- `--fast-disk` traps the DOS 3.3 RWTS entry point (`$B7B5`) and copies sector data directly from raw `.dsk` images, skipping nibble-level emulation. Best used with DOS 3.3 formatted disks
- `--noise` plays the embedded `move_arm.wav` sound in a loop while the disk motor is running

## Controls

Both frontends use the same keyboard controls:

| Key | Action |
|-----|--------|
| `Ctrl+Q` | Quit |
| `Ctrl+R` | Reset |
| `Ctrl+T` | Toggle turbo (x4) |
| Arrow Keys | Apple II arrow keys |
| Regular Keys | ASCII input (auto uppercase) |

## Project Structure

```
a2vm/
├── a2vm-core/     # Core emulation library (CPU, memory, video, audio, disk)
├── a2vm-oxide/    # Shared frontend resources (mechanical noise, embedded ROM & assets)
│   └── assets/    # Embedded assets (ROM, WAV files)
├── a2vm-tui/      # Terminal UI frontend (Braille display)
├── a2vm-gui/      # Graphical UI frontend (pixels + winit)
└── docs/          # Documentation
```

## Testing

```bash
# Run CPU functional test (Klaus Dormann)
cargo test klaus_dormann
```

## Architecture

- **Rust Core**: 6502 CPU with Bus trait abstraction, AppleII machine emulation, Disk II, and speaker synthesis
- **Video**: Unified 280×192 bitmap renderer with mode-specific pipelines; RGBA output for GUI with authentic Apple II colors
- **Audio** (optional): `$C030` speaker toggles converted to PCM in `a2vm-core`, played by `rodio`. Disable with `--no-default-features` if rodio/ALSA is unavailable
- **Mechanical Noise**: `a2vm-oxide` provides disk motor/seek sound simulation via embedded WAV assets
- **TUI**: Braille encoding (2×4 dots per char) for terminal display and runtime status telemetry
- **GUI**: Native window using `pixels` (wgpu) + `winit` with 280×192 resolution, 3× default scaling, status output in console, and `color`/`mono`/`mono-scanlines` display modes

## License

MIT

## Demo

[![asciicast](https://asciinema.org/a/pkGK4iQGW7P6XFMT.svg)](https://asciinema.org/a/pkGK4iQGW7P6XFMT)

![Image](https://github.com/user-attachments/assets/c1f33259-a0df-402d-9844-effebaa4fba2)
