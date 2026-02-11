# A2VM - Apple II Emulator Architecture

## Overview

A2VM is a Rust workspace with one shared emulation core crate and two frontend binaries.

- `a2vm-core`: CPU, bus, machine integration, video, audio, disk
- `a2vm-tui`: terminal frontend (Braille text-mode display + keyboard + optional audio)
- `a2vm-gui`: native window frontend (pixels + winit + optional audio)

There is no Swift/FFI runtime in the current codebase.

## Workspace Layout

```
a2vm/
|- Cargo.toml
|- a2vm-core/
|  |- Cargo.toml
|  |- src/
|  |  |- lib.rs
|  |  |- audio.rs
|  |  |- bus.rs
|  |  |- disk.rs
|  |  |- error.rs
|  |  |- machine.rs
|  |  |- memory.rs
|  |  |- timing.rs
|  |  |- video.rs
|  |  `- cpu/
|  |     |- mod.rs
|  |     |- opcodes.rs
|  |     |- addressing.rs
|  |     |- disasm.rs
|  |     |- status.rs
|  |     `- tests.rs
|  `- tests/
|     |- klaus_dormann.rs
|     `- data/6502_functional_test.bin
|- a2vm-tui/
|  `- src/
|     |- main.rs
|     `- cli.rs
|- a2vm-gui/
|  `- src/
|     |- main.rs
|     `- cli.rs
`- docs/
   `- architecture.md
```

## Runtime Architecture

```
TUI (crossterm/ratatui)  GUI (winit/pixels)
          |                         |
          +-----------+-------------+
                      |
                 a2vm-core
                      |
     +----------------+----------------+
     |                |                |
    CPU             Machine           Bus
     |                |                |
     +------> Video / Audio / Disk <---+
```

Both frontends own an `AppleII` machine instance and run the same emulation APIs.

## Core Modules

### bus.rs

`Bus` defines CPU-visible memory and I/O operations.

- `read(&mut self, addr)` for side-effect reads
- `write(&mut self, addr, val)`
- `peek(&self, addr)` for side-effect-free debug/disassembly reads
- `read_word_page_wrap` models NMOS JMP indirect page-wrap behavior

### cpu/

- `opcodes.rs`: 256-entry opcode table (legal + ILL placeholders)
- `addressing.rs`: 13 NMOS addressing modes
- `status.rs`: status register bit operations
- `mod.rs`: fetch/decode/execute loop, interrupts, ALU helpers
- `disasm.rs`: side-effect-free instruction formatting via `Bus::peek`
- `tests.rs`: instruction-level unit tests for critical behavior

### machine.rs

`AppleII` owns `Cpu`, RAM/ROM, display mode state, speaker, and Disk II controller.

Important behavior:

- `step()` executes one instruction and ticks disk timing hook
- `run_cycles()` supports fast-disk execution path with RWTS trap
- keyboard latch and strobe semantics at `$C000/$C010`
- speaker toggle at `$C030`
- display soft-switches at `$C050-$C057`
- disk controller I/O at `$C0E0-$C0EF` and slot ROM at `$C600-$C6FF`

### disk.rs

Disk II controller with `.dsk` loading and nibblized track data.

- supports read path and write path (RWTS write trap + nibble write mode)
- persists sector writes back to the disk image when writable
- exposes raw-sector read/write helpers used by machine-level traps

### video.rs

Apple II display renderer:

- TEXT/LORES/HIRES bitmap generation
- RGBA conversion for GUI
- color/mono variants and frame-phase artifacts for GUI modes

### audio.rs

Speaker edge-timeline synthesis.

- records toggle cycles
- renders PCM by cycle budget
- supports reusable output buffer API to reduce allocations

### timing.rs

Shared timing constants used by core and frontends.

- `CPU_HZ` provides a single source of truth for Apple II target cycle timing

### error.rs

Typed core error enum for ROM/disk operations.

- maps I/O and validation failures to explicit variants
- frontends can print user-facing errors via `Display`

## Frontends

### a2vm-tui

- clap-based CLI argument parsing (`--rom`, `--disk`, `--fast-disk`) in `src/cli.rs`
- terminal rendering through Braille conversion
- keyboard mapping to Apple II ASCII
- optional audio playback with rodio

### a2vm-gui

- clap-based CLI argument parsing (`--rom`, `--disk`, `--fast-disk`, `--color-mode`) in `src/cli.rs`
- native event loop with winit
- framebuffer presentation with pixels
- optional audio playback with rodio

## Testing Strategy

- `a2vm-core/tests/klaus_dormann.rs`: full functional CPU test
- `a2vm-core/src/cpu/tests.rs`: focused instruction-level unit tests
- `a2vm-core/src/disk.rs` and `a2vm-core/src/machine.rs`: module-level behavior tests

ROM/disk integration tests are resilient to missing external assets by returning early when files are absent.

## Known Boundaries

- TUI and GUI intentionally keep separate frontend-side timing/CLI definitions to allow independent evolution
- unofficial 6502 opcode coverage is partial; unsupported cases currently fall back to placeholder behavior
- `Cpu` register fields are private and exposed via inline accessors for frontend status display
