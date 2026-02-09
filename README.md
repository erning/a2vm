# A2VM — Apple II Emulator

A terminal-based Apple II/II+ emulator written in Rust. Features a complete 6502 CPU implementation, Disk II boot support, TEXT/GR/HGR rendering, 1-bit speaker audio, and a Braille-based TUI display.

## Features

- **Complete 6502 CPU**: All 56 official opcodes, 13 addressing modes, BCD arithmetic, and accurate cycle counting
- **Video Modes**: TEXT (40×24), Lo-Res Graphics (40×48), Hi-Res Graphics (280×192), and Mixed mode
- **Disk II Emulation**: Loads DOS 3.3 `.dsk` images and boots from slot 6
- **Speaker Audio (M6)**: `$C030` toggle timestamps synthesized to PCM and played through the TUI frontend
- **Terminal UI**: Renders Apple II display using Braille characters (140×48) with ratatui
- **Keyboard Input**: Full ASCII keyboard support with Apple II key mapping
- **ROM Support**: Loads Apple II/II+ ROM files (12K/20K)

## Quick Start

```bash
# Build the project
cargo build --release

# Run with an Apple II ROM
./target/release/a2vm-tui path/to/rom.bin

# Run with ROM + DOS 3.3 disk
./target/release/a2vm-tui roms/apple2p.rom "disks/Apple DOS 3.3 January 1983.dsk"
```

## Controls

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
├── a2vm-core/     # Rust core library (CPU, memory, video)
└── a2vm-tui/      # Terminal UI frontend
```

## Testing

```bash
# Run CPU functional test (Klaus Dormann)
cargo test klaus_dormann
```

## Architecture

- **Rust Core**: 6502 CPU with Bus trait abstraction, AppleII machine emulation, Disk II, and speaker synthesis
- **Video**: Unified 280×192 bitmap renderer with mode-specific pipelines
- **Audio**: `$C030` speaker toggles converted to PCM in `a2vm-core`, played by `rodio` in `a2vm-tui`
- **TUI**: Braille encoding (2×4 dots per char) for terminal display and runtime status telemetry

## License

MIT

## Demo

[![asciicast](https://asciinema.org/a/pkGK4iQGW7P6XFMT.svg)](https://asciinema.org/a/pkGK4iQGW7P6XFMT)
