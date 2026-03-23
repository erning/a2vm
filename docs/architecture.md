# A2VM - Apple II Emulator Architecture

## Overview

A2VM is a Rust workspace with an emulation core, shared runtime, FFI bridge, and multiple frontends.

- `a2vm-core`: CPU, bus/machine integration, disk, video, audio, keyboard
- `a2vm-oxide`: shared CLI, embedded assets, mechanical noise tracker, shared frontend runtime (`EmulatorRunner`)
- `a2vm-ffi`: C-compatible static library wrapping `EmulatorRunner` for native frontends
- `a2vm-tui`: terminal frontend (`ratatui` + `crossterm`)
- `a2vm-gui`: cross-platform GUI (`pixels` + `winit`) — to be replaced by native frontends
- `a2vm-macos`: macOS native frontend (Swift + AppKit + Metal)
- `a2vm-web`: browser frontend (WebAssembly + WebGPU)

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
├── a2vm-ffi/
│   └── src/
│       └── lib.rs              # C FFI: opaque A2VMEmulator handle + free functions
├── a2vm-gui/
│   └── src/
│       ├── main.rs
│       └── cli.rs
├── a2vm-macos/
│   ├── Shaders.metal           # Metal shaders (passthrough, bloom, blur, CRT composite)
│   ├── MetalRenderer.swift     # Multi-pass Metal pipeline with CRT effects
│   ├── EmulatorController.swift # Swift wrapper around C FFI
│   ├── EmulatorView.swift      # MTKViewDelegate + keyboard input
│   ├── KeyMapper.swift         # NSEvent → Apple II ASCII
│   ├── main.swift              # NSApplication entry point
│   ├── a2vm-ffi-Bridging.h     # C header for FFI
│   └── Info.plist              # App bundle metadata
├── a2vm-web/
│   ├── src/
│   │   └── lib.rs              # wasm-bindgen: Emulator wrapper (no Instant, no fs)
│   └── www/
│       ├── index.html          # Auto-scaling canvas
│       ├── main.js             # WebGPU CRT pipeline + requestAnimationFrame loop
│       └── shaders.wgsl        # WGSL shaders (passthrough, bloom, blur, CRT composite)
├── Makefile                    # Build targets for macOS native app
└── docs/
    └── architecture.md
```

## Runtime Architecture

```text
TUI (ratatui/crossterm)    GUI (winit/pixels)    macOS (Swift+Metal)    Web (wasm+WebGPU)
          |                        |                      |                      |
          |                        |               a2vm-ffi (C API)        a2vm-core (direct)
          |                        |                      |                      |
          +-----------+------------+----------------------+------+               |
                      |                                          |               |
           a2vm-oxide::EmulatorRunner                     a2vm-oxide      (JS timing loop)
                      |                                          |               |
                      +------------------------------------------+---------------+
                                              |
                                           AppleII
                                              |
                                 +------------+------------+
                                 |            |            |
                                CPU          Bus      Video/Audio/Disk
```

All frontends use the same emulation core (`AppleII`). Desktop frontends use `EmulatorRunner` for timing/audio. The web frontend calls `a2vm-core` directly — JS manages timing via `requestAnimationFrame` and WebGPU handles CRT rendering (avoiding `std::time::Instant` which is unavailable in wasm). Native frontends (macOS, future Linux/Windows) link via `a2vm-ffi` C static library. The cross-platform `a2vm-gui` will be phased out.

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
- cross-platform fallback; to be replaced by native frontends

### `a2vm-ffi`

C-compatible static library (`crate-type = ["staticlib"]`) wrapping `EmulatorRunner`.

- opaque `A2VMEmulator` pointer pattern (create/destroy lifecycle)
- ~10 free functions: `a2vm_create`, `a2vm_destroy`, `a2vm_tick`, `a2vm_reset`, `a2vm_key_press`, `a2vm_render_rgba`, `a2vm_video_dirty`, `a2vm_display_width`, `a2vm_display_height`
- ROM embedded in library (via `a2vm-oxide::DEFAULT_ROM`)
- audio handled internally by `EmulatorRunner` (rodio) — not exposed through FFI
- hand-written C header: `a2vm-macos/a2vm-ffi-Bridging.h`

### `a2vm-macos`

Native macOS frontend: Swift + AppKit + Metal.

**Build:** `make macos-app` (cargo → swiftc → xcrun metal → .app bundle). No SPM or Xcode project.

**Metal rendering pipeline (multi-pass):**

```text
Source (280×192) → [Upscale 4x, nearest] → Intermediate (1120×768)
                                                  ↓
                                            [Bloom extract] → half-res → [Blur H] → [Blur V]
                                                  ↓                                      ↓
                                            [CRT Composite: distortion + scanlines + bloom + vignette]
                                                  ↓
                                              Drawable
```

**CRT effects** (all on by default, controlled by `CRTSettings`):
- Bloom: bright-pass extraction + 9-tap Gaussian blur → additive glow
- Scanlines: sharp dark bands via `smoothstep` aligned to 192 emulator rows
- Barrel distortion: subtle screen curvature in UV space
- Vignette: radial edge/corner darkening
- Phosphor background: warm dark gray replaces pure black

**Planned features:**
- Menu bar (File: Open Disk/ROM, Eject; Machine: Reset, Turbo, Fast Disk; View: Color mode, Fullscreen)
- NSOpenPanel for disk/ROM file loading
- Status bar (bottom of window: PC, MHz, disk status)
- App icon and proper .app bundle signing

### `a2vm-web`

Browser frontend: Rust emulator core compiled to WebAssembly + WebGPU rendering in JavaScript.

**Architecture:** Rust side is a thin wasm-bindgen wrapper around `a2vm-core` (no `EmulatorRunner`, no `Instant`, no filesystem). JS side handles timing, keyboard input, and WebGPU rendering.

**Build:** `wasm-pack build --target web a2vm-web` → `a2vm-web/pkg/` (67KB wasm).

**WebGPU CRT pipeline (same structure as macOS Metal):**

```text
Source (280×192) → [Upscale 4x, nearest] → 1120×768
    → [Bloom extract] → half-res → [Blur H] → [Blur V]
    → [CRT Composite: distortion + scanlines + bloom + vignette]
    → Canvas
```

WGSL shaders are direct translations of the Metal shaders. Per-pass uniform buffers (general, blurH, blurV) avoid the WebGPU constraint of single-buffer-per-submit.

**Timing:** JS `requestAnimationFrame` loop calculates delta time, converts to CPU cycles (`dt_ms × 1023`), calls `emulator.run_cycles()`. No `std::time::Instant` dependency.

**Requirements:** WebGPU (Chrome 113+, Edge 113+). Requires secure context (HTTPS or localhost).

**Planned features:**
- Audio via Web Audio API (AudioWorklet + PCM ring buffer)
- Disk loading via File API drag-and-drop
- WebGL2 fallback for browsers without WebGPU

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
- `make macos-app`: macOS native build (requires Xcode Command Line Tools)

Core tests cover CPU behavior, disk persistence, ROM loading edge cases, keyboard/speaker semantics, and boot-path smoke checks.
