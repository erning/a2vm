# A2VM — Apple II / II+ Emulator

A2VM is a Rust Apple II emulator with both terminal (TUI) and native window (GUI) frontends.

It includes a full NMOS 6502 core, Disk II support, TEXT/GR/HGR video, speaker audio, optional mechanical disk noise, and an embedded default Apple II+ ROM.

## Features

- Full 6502 core with 13 addressing modes, cycle accounting, BCD behavior, and key NMOS quirks
- Disk II emulation with `.dsk` loading, nibblized tracks, and optional DOS 3.3 RWTS fast-disk trap
- Video pipeline for TEXT / Lo-Res / Hi-Res with GUI color and monochrome scanline modes
- Speaker synthesis from `$C030` toggles, plus optional mechanical disk noise
- Shared frontend runtime (`EmulatorRunner`) for TUI and GUI behavior consistency
- Embedded Apple II+ ROM fallback (`--rom` optional)

## Quick Start

### TUI

```bash
# Build (audio enabled)
cargo build --release -p a2vm-tui

# Build without audio
cargo build --release -p a2vm-tui --no-default-features

# Run with embedded ROM
cargo run -p a2vm-tui

# Run with disk
cargo run -p a2vm-tui -- --disk "disks/Apple DOS 3.3 January 1983.dsk"

# Fast-disk + mechanical noise
cargo run -p a2vm-tui -- --disk "disks/Apple DOS 3.3 January 1983.dsk" --fast-disk --noise
```

### GUI

```bash
# Build (audio enabled)
cargo build --release -p a2vm-gui

# Build without audio
cargo build --release -p a2vm-gui --no-default-features

# Run with embedded ROM
cargo run -p a2vm-gui

# Run with disk + scanlines
cargo run -p a2vm-gui -- --disk "disks/Apple DOS 3.3 January 1983.dsk" --color-mode mono-scanlines

# Two disks + fast-disk + noise
cargo run -p a2vm-gui -- --disk "disks/Apple DOS 3.3 January 1983.dsk" --disk "disks/Programma.dsk" --fast-disk --noise
```

## CLI Options

Shared options:

```text
a2vm-tui|a2vm-gui [options]

  --rom <FILE>        Apple II/II+ ROM (12K or 20K). Optional; embedded ROM used by default.
  --disk <FILE>       .dsk disk image (143360 bytes), up to two times
  --fast-disk         Enable DOS 3.3 RWTS trap ($B7B5) for all drives
  --noise             Enable realistic mechanical noise simulation
  -h, --help          Show help
```

GUI-only:

```text
a2vm-gui ... [--color-mode <mode>]

  --color-mode <mode> Display mode: 'color' (default), 'mono', 'mono-scanlines'
```

Notes:

- `--rom` is optional (embedded Apple II+ ROM is used when omitted)
- `--disk` accepts 0/1/2 values (drive 1 then drive 2)
- `--fast-disk` applies globally to mounted drives
- `--noise` is only meaningful when audio feature is enabled

## Controls

Both frontends share these controls:

| Key | Action |
|-----|--------|
| `Ctrl+Q` | Quit |
| `Ctrl+R` | Reset |
| `Ctrl+T` | Turbo toggle (x4) |
| Arrow keys | Apple II directional keys |
| Printable keys | ASCII input (Apple II mapping) |

## Project Structure

```text
a2vm/
├── a2vm-core/     # CPU + machine + bus + video + audio + disk
├── a2vm-oxide/    # Shared CLI, noise, and shared frontend runtime (EmulatorRunner)
│   └── assets/    # Embedded ROM and WAV assets
├── a2vm-tui/      # Ratatui/crossterm frontend
├── a2vm-gui/      # winit/pixels frontend
└── docs/          # Architecture docs
```

## Testing

```bash
# Full workspace tests
cargo test

# Core tests only
cargo test -p a2vm-core

# Klaus Dormann functional test
cargo test klaus_dormann

# No-audio build checks
cargo build --no-default-features -p a2vm-tui
cargo build --no-default-features -p a2vm-gui
```

## Architecture Notes

- `a2vm-core` owns emulation behavior (`AppleII`, CPU, disk, video, audio)
- `a2vm-oxide::runner::EmulatorRunner` centralizes cycle accumulation, turbo, audio, noise, and drive flush-on-drop
- TUI and GUI are thin shells around shared runtime + input/rendering

More details: `docs/architecture.md`.

## License

MIT
