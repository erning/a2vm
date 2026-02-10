# A2VM — Apple II Emulator

An Apple II/II+ emulator written in Rust featuring both terminal (TUI) and graphical (GUI) frontends. Includes a complete 6502 CPU implementation, Disk II boot support, TEXT/GR/HGR rendering with authentic color, 1-bit speaker audio, and both Braille-based terminal display and native GPU-accelerated GUI.

## Features

- **Complete 6502 CPU**: All 56 official opcodes, 13 addressing modes, BCD arithmetic, and accurate cycle counting
- **Video Modes**: TEXT (40×24), Lo-Res Graphics (40×48), Hi-Res Graphics (280×192), and Mixed mode
- **Disk II Emulation**: Loads DOS 3.3 `.dsk` images and boots from slot 6
- **Speaker Audio (M6)**: `$C030` toggle timestamps synthesized to PCM and played through frontend
- **Two Frontends**:
  - **TUI**: Terminal UI with Braille characters (140×48) using ratatui
  - **GUI**: Native GPU-accelerated window with authentic Apple II colors plus monochrome CRT modes (`color`, `mono`, `mono-scanlines`)
- **Keyboard Input**: Full ASCII keyboard support with Apple II key mapping
- **ROM Support**: Loads Apple II/II+ ROM files (12K/20K)

## Quick Start

### TUI (Terminal UI)

```bash
# Build the TUI (with audio support, requires ALSA on Linux)
cargo build --release -p a2vm-tui

# Build without audio (if audio libraries are not available)
cargo build --release -p a2vm-tui --no-default-features

# Run with ROM only (enter Monitor)
./target/release/a2vm-tui --rom roms/apple2p.rom

# Run with a disk image
./target/release/a2vm-tui --rom roms/apple2p.rom --disk "disks/Apple DOS 3.3 January 1983.dsk"

# Fast-disk mode (DOS 3.3 RWTS trap for instant sector reads)
./target/release/a2vm-tui --rom roms/apple2p.rom --fast-disk "disks/Apple DOS 3.3 January 1983.dsk"

# Use A2VM_ROM environment variable to avoid typing --rom every time
export A2VM_ROM=roms/apple2p.rom
./target/release/a2vm-tui --disk "disks/Apple DOS 3.3 January 1983.dsk"
```

### GUI (Graphical UI)

```bash
# Build the GUI (with audio support)
cargo build --release -p a2vm-gui

# Build without audio
cargo build --release -p a2vm-gui --no-default-features

# Run with ROM
./target/release/a2vm-gui --rom roms/apple2p.rom

# Run with monochrome scanline mode
./target/release/a2vm-gui --rom roms/apple2p.rom --color-mode mono-scanlines

# Run with disk
cargo run -p a2vm-gui -- --rom roms/apple2p.rom --disk "disks/Apple DOS 3.3 January 1983.dsk"
```

### Command Line Options

Shared options:

```
a2vm-tui|a2vm-gui --rom <file> [--disk <file> | --fast-disk <file>]

Options:
  --rom <file>        Apple II/II+ ROM (12K or 20K) [env: A2VM_ROM]
  --disk <file>       .dsk disk image (143360 bytes)
  --fast-disk <file>  .dsk image with DOS 3.3 RWTS trap for instant sector reads
  -h, --help          Show this help
```

GUI-only option:

```
a2vm-gui ... [--color-mode <mode>]

  --color-mode <mode> Display mode: 'color' (default), 'mono', 'mono-scanlines'
```

- `--rom` is required; falls back to the `A2VM_ROM` environment variable
- `--disk` and `--fast-disk` are mutually exclusive
- `--fast-disk` traps the DOS 3.3 RWTS entry point (`$B7B5`) and copies sector data directly from the raw `.dsk` image, skipping nibble-level emulation. Only works with DOS 3.3 formatted disks

## Controls

Both frontends use the same keyboard controls:

| Key | Action |
|-----|--------|
| `Ctrl+Q` / `Ctrl+C` | Quit |
| `Ctrl+R` | Reset |
| `Ctrl+T` | Toggle turbo (x4) |
| Arrow Keys | Apple II arrow keys |
| Regular Keys | ASCII input (auto uppercase) |

## Project Structure

```
a2vm/
├── a2vm-core/     # Rust core library (CPU, memory, video, audio)
├── a2vm-tui/      # Terminal UI frontend (Braille display)
├── a2vm-gui/      # Graphical UI frontend (pixels + winit)
└── docs/          # Documentation (architecture, milestones, GUI plan)
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
- **TUI**: Braille encoding (2×4 dots per char) for terminal display and runtime status telemetry
- **GUI**: Native window using `pixels` (wgpu) + `winit` with 280×192 resolution, 3× default scaling, status output in console, and `color`/`mono`/`mono-scanlines` display modes

## License

MIT

## Demo

[![asciicast](https://asciinema.org/a/pkGK4iQGW7P6XFMT.svg)](https://asciinema.org/a/pkGK4iQGW7P6XFMT)

![Image](https://github.com/user-attachments/assets/c1f33259-a0df-402d-9844-effebaa4fba2)
