# A2VM — Apple II Emulator

A terminal-based Apple II emulator written in Rust. Features a complete 6502 CPU implementation, video rendering with TEXT/GR/HGR modes, and a Braille-based TUI display.

## Features

- **Complete 6502 CPU**: All 56 official opcodes, 13 addressing modes, BCD arithmetic, and accurate cycle counting
- **Video Modes**: TEXT (40×24), Lo-Res Graphics (40×48), Hi-Res Graphics (280×192), and Mixed mode
- **Terminal UI**: Renders Apple II display using Braille characters (140×48) with ratatui
- **Keyboard Input**: Full ASCII keyboard support with Apple II key mapping
- **ROM Support**: Loads Apple II/II+ ROM files (12K/20K)

## Quick Start

```bash
# Build the project
cargo build --release

# Run with an Apple II ROM
./target/release/a2vm-tui path/to/rom.bin
```

## Controls

| Key | Action |
|-----|--------|
| `Ctrl+Q` / `Ctrl+C` | Quit |
| `Ctrl+R` | Reset |
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

- **Rust Core**: 6502 CPU with Bus trait abstraction, FlatMemory for testing, AppleII machine emulation
- **Video**: Unified 280×192 bitmap renderer with mode-specific pipelines
- **TUI**: Braille encoding (2×4 dots per char) for terminal display

## License

MIT

## Demo

[![asciicast](https://asciinema.org/a/a2wSxJYbQWPZlZio.svg)](https://asciinema.org/a/a2wSxJYbQWPZlZio)
