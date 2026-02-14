# A2VM-Web

Browser-based Apple II/II+ emulator using WebAssembly.

## Quick Start

### Prerequisites

- Rust toolchain with wasm32 target
- wasm-pack
- Node.js & npm

### Setup

```bash
# Install wasm32 target (if not already installed)
rustup target add wasm32-unknown-unknown

# Install wasm-pack (if not already installed)
cargo install wasm-pack

# Add cargo bin to PATH (if not already added)
export PATH="$HOME/.cargo/bin:$PATH"

# Build the WASM module
cd a2vm-web
wasm-pack build --target web --out-dir web/pkg
```

**Note**: If `wasm-pack` is not found after installation, either:
1. Add `export PATH="$HOME/.cargo/bin:$PATH"` to your shell config (~/.bashrc, ~/.zshrc)
2. Or use the full path: `~/.cargo/bin/wasm-pack`

# Install frontend dependencies
cd web
npm install

# Start development server
npm run dev
```

Open http://localhost:3000 in your browser.

### Usage

1. **Load ROM**: Click "Load ROM" and select an Apple II/II+ ROM file (12K or 20K)
2. **Load Disk**: Click "Load Disk 1" or "Load Disk 2" to load .dsk images
3. **Controls**:
   - Type normally for keyboard input
   - Ctrl+R: Reset
   - Ctrl+T: Toggle turbo mode
   - Turbo button: Cycle through speed multipliers (1x, 2x, 4x, 8x, 16x)
   - Fast Disk: Enable DOS 3.3 RWTS trap for faster disk access
   - Audio: Toggle speaker audio output (click to enable, requires user interaction)
   - Export Disk: Download modified disk image
4. **Screen Scaling**:
   - −/+ buttons: Decrease/increase scale (25% steps, 100%-500%)
   - Fit button: Auto-fit to window size
   - Screen automatically adapts to window resize

## Architecture

```
a2vm-web/
├── Cargo.toml          # Rust WASM crate config
├── src/
│   └── lib.rs          # WASM bindings (AppleIIWeb struct)
└── web/                # TypeScript frontend
    ├── index.html      # Main page
    ├── src/
    │   ├── main.ts     # Entry point
    │   └── emulator.ts # Emulator wrapper class
    └── pkg/            # Generated WASM output
```

## Building for Production

```bash
cd a2vm-web/web
npm run build
```

Output will be in `web/dist/`.

## Features

- Full 6502 CPU emulation
- TEXT/GR/HGR video modes with color/monochrome/scanlines
- Disk II controller with .dsk support
- Speaker audio synthesis via Web Audio API
- Scalable display (100%-500% with auto-fit)
- Turbo mode (up to 16x speed)
- Fast disk mode (RWTS trap)
- Export modified disk images

## Troubleshooting

### wasm-pack not found

After `cargo install wasm-pack`, the binary is in `~/.cargo/bin/`. Make sure this directory is in your PATH:

```bash
# Temporarily add to current session
export PATH="$HOME/.cargo/bin:$PATH"

# Or use full path
~/.cargo/bin/wasm-pack build --target web --out-dir web/pkg
```

### WebAssembly module fails to load

Make sure you built the WASM module first:

```bash
cd a2vm-web
wasm-pack build --target web --out-dir web/pkg
```
