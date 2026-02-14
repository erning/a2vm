# A2VM - Apple II Emulator Architecture

## Overview

A2VM is a Rust workspace with one emulation core crate, one shared runtime/resources crate, and two frontend binaries.

- `a2vm-core`: CPU, bus/machine integration, disk, video, audio, keyboard
- `a2vm-oxide`: shared CLI, embedded assets, mechanical noise tracker, shared frontend runtime (`EmulatorRunner`)
- `a2vm-tui`: terminal frontend (`ratatui` + `crossterm`)
- `a2vm-gui`: native window frontend (`pixels` + `winit`)

## Workspace Layout

```text
a2vm/
├── Cargo.toml
├── a2vm-core/
│   ├── src/
│   │   ├── lib.rs
│   │   ├── audio.rs
│   │   ├── bus.rs
│   │   ├── disk.rs
│   │   ├── error.rs
│   │   ├── keyboard.rs
│   │   ├── machine/
│   │   │   ├── mod.rs
│   │   │   ├── bus_state.rs
│   │   │   ├── runtime.rs
│   │   │   ├── rwts.rs
│   │   │   └── tests.rs
│   │   ├── memory.rs
│   │   ├── timing.rs
│   │   ├── video/
│   │   │   ├── mod.rs
│   │   │   ├── mode.rs
│   │   │   ├── layout.rs
│   │   │   ├── mono.rs
│   │   │   ├── rgba.rs
│   │   │   ├── overlay.rs
│   │   │   └── tests.rs
│   │   └── cpu/
│   │       ├── mod.rs
│   │       ├── opcodes.rs
│   │       ├── addressing.rs
│   │       ├── disasm.rs
│   │       ├── status.rs
│   │       └── tests.rs
│   └── tests/
│       ├── klaus_dormann.rs
│       └── data/6502_functional_test.bin
├── a2vm-oxide/
│   ├── assets/
│   │   ├── apple2p.rom
│   │   ├── move_arm.wav
│   │   ├── disk_insertion.wav
│   │   ├── disk_removal.wav
│   │   ├── pop_on.wav
│   │   └── pop_off.wav
│   └── src/
│       ├── lib.rs
│       ├── cli.rs
│       ├── noise.rs
│       └── runner.rs
├── a2vm-tui/
│   └── src/
│       ├── main.rs
│       └── cli.rs
├── a2vm-gui/
│   └── src/
│       ├── main.rs
│       └── cli.rs
└── docs/
    └── architecture.md
```

## Runtime Architecture

```text
TUI (ratatui/crossterm)       GUI (winit/pixels)
          |                              |
          +--------------+---------------+
                         |
              a2vm-oxide::EmulatorRunner
                         |
                      AppleII
                         |
            +------------+------------+
            |            |            |
           CPU          Bus      Video/Audio/Disk
```

Both frontends use the same runner and machine APIs, so turbo timing, audio generation, mechanical noise behavior, and disk flushing semantics stay consistent.

## Core Modules (`a2vm-core`)

### `bus.rs`

`Bus` defines CPU-visible memory/I/O access.

- `read(&mut self, addr)` for side-effect reads
- `write(&mut self, addr, val)` for writes
- `peek(&self, addr)` for side-effect-free inspection/disassembly
- `read_word_page_wrap` models NMOS JMP indirect page-wrap behavior

### `cpu/`

- `opcodes.rs`: 256-entry opcode table (official + selected unofficial opcodes)
- `addressing.rs`: 13 addressing modes
- `status.rs`: processor status flag helpers
- `mod.rs`: fetch/decode/execute, interrupts, ALU behavior
- `disasm.rs`: side-effect-free disassembly via `peek`
- `tests.rs`: instruction-level behavior tests

### `machine/`

`AppleII` owns `Cpu`, RAM/ROM, soft-switch state, speaker, and Disk II.

Key behavior:

- `step()` executes one instruction and ticks disk timing
- `run_cycles()` handles normal stepping and optional fast-disk RWTS trap path
- `load_rom_data()` supports 12K/20K ROM layouts
- keyboard latch/strobe behavior at `$C000/$C010`
- speaker toggle at `$C030`
- display soft-switches at `$C050-$C057`
- disk controller I/O at `$C0E0-$C0EF` and slot ROM at `$C600-$C6FF`

### `disk.rs`

Disk II implementation with nibblized track data.

- loads/saves `.dsk` images
- supports raw sector read/write helpers for RWTS trap
- syncs nibble writes back to raw image on motor-off and explicit flush
- exposes `flush_drive` / `flush_all_drives`

### `video/`

Display renderer for TEXT/GR/HGR.

- monochrome bitmap generation
- RGBA conversion for GUI
- color/mono/mono-scanlines variants

### `audio.rs`

Speaker edge-timeline synthesis.

- collects `$C030` toggle cycles
- renders PCM for requested cycle budget
- supports reusable output buffer API

### `keyboard.rs`

Apple II key mapping helpers.

- `AppleKey` enum for printable/control/named keys
- `map_apple_key()` to Apple II ASCII codes

### `error.rs`

Typed error enum for ROM/disk/core I/O operations with error-source chaining.

## Shared Runtime and Resources (`a2vm-oxide`)

### `cli.rs`

Shared CLI arguments and embedded ROM.

- `SharedArgs` includes `--rom`, `--disk`, `--fast-disk`, `--noise`
- `DEFAULT_ROM` embeds Apple II+ ROM
- `rom_data()` returns `Cow<'static, [u8]>`

### `noise.rs`

Mechanical noise event tracker.

- tracks disk motor state and half-track movement
- emits `MotorStart` / `TrackSeek` / `MotorStop`

### `runner.rs`

`EmulatorRunner` is the common frontend runtime.

Responsibilities:

- cycle accumulation using elapsed wall time and `CPU_HZ`
- turbo mode multiplier
- optional speaker audio playback pipeline
- optional mechanical noise playback pipeline
- periodic emulation speed stats (`MHz`)
- automatic disk flush on drop

## Frontends

### `a2vm-tui`

- uses `SharedArgs` + `EmulatorRunner`
- renders 280×192 bitmap as Braille (140×48 effective)
- handles keyboard mapping and status line display
- terminal state protected by RAII guard

### `a2vm-gui`

- uses `SharedArgs` + GUI-only `--color-mode`
- uses `EmulatorRunner` for emulation and runtime state
- renders via `pixels` at 280×192
- processes input through `winit`

## Build/Feature Model

- Workspace defines shared dependency versions in root `Cargo.toml`
- `a2vm-oxide` has optional `audio` feature (default on)
- TUI/GUI `audio` features forward-enable `a2vm-oxide/audio`
- `--no-default-features` disables audio stack for environments without `rodio` backend support

## Testing Strategy

- `cargo test`: full workspace tests
- `cargo test -p a2vm-core`: core behavior + module tests
- `cargo test klaus_dormann`: 6502 functional test
- `cargo build --no-default-features -p a2vm-tui -p a2vm-gui`: no-audio build validation

Core tests cover CPU behavior, disk persistence, ROM loading edge cases, keyboard/speaker semantics, and boot-path smoke checks.
