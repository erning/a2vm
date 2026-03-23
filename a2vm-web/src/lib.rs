use wasm_bindgen::prelude::*;

use a2vm_core::machine::AppleII;

/// Expose wasm linear memory to JS for zero-copy RGBA buffer access.
#[wasm_bindgen]
pub fn wasm_memory() -> JsValue {
    wasm_bindgen::memory()
}
use a2vm_core::video::{self, DisplayColorMode, RGBA_HEIGHT, RGBA_SIZE, RGBA_WIDTH};

/// Embedded Apple II+ ROM (20K).
const DEFAULT_ROM: &[u8] = include_bytes!("../../a2vm-oxide/assets/apple2p.rom");

/// Flash half-period for text blinking (ms).
const FLASH_HALF_PERIOD_MS: f64 = 267.0;

#[wasm_bindgen]
pub struct Emulator {
    apple: AppleII,
    rgba_buf: Vec<u8>,
    boot_time_ms: f64,
    frame_phase: u64,
}

#[wasm_bindgen]
impl Emulator {
    /// Create a new emulator with embedded ROM.
    #[wasm_bindgen(constructor)]
    pub fn new(now_ms: f64) -> Emulator {
        let mut apple = AppleII::new();
        apple.load_rom_data(DEFAULT_ROM).expect("embedded ROM");
        apple.reset();

        Emulator {
            apple,
            rgba_buf: vec![0u8; RGBA_SIZE],
            boot_time_ms: now_ms,
            frame_phase: 0,
        }
    }

    /// Run the given number of CPU cycles.
    pub fn run_cycles(&mut self, cycles: u32) {
        self.apple.run_cycles(cycles as u64);
    }

    /// Send a key press (7-bit ASCII).
    pub fn key_press(&mut self, ascii: u8) {
        self.apple.key_press(ascii);
    }

    /// Render the display into the internal RGBA buffer.
    /// Returns a pointer to the buffer (280*192*4 bytes).
    pub fn render_rgba(&mut self, now_ms: f64) -> *const u8 {
        let elapsed = now_ms - self.boot_time_ms;
        let flash_on = ((elapsed / FLASH_HALF_PERIOD_MS) as u64 & 1) == 0;

        video::render_rgba(
            self.apple.ram(),
            &self.apple.bus.display,
            flash_on,
            DisplayColorMode::Color,
            self.frame_phase,
            &mut self.rgba_buf,
        );
        self.apple.bus.video_dirty = false;
        self.frame_phase = self.frame_phase.wrapping_add(1);

        self.rgba_buf.as_ptr()
    }

    /// Display width in pixels.
    pub fn display_width(&self) -> u32 {
        RGBA_WIDTH as u32
    }

    /// Display height in pixels.
    pub fn display_height(&self) -> u32 {
        RGBA_HEIGHT as u32
    }

    /// Reset the CPU.
    pub fn reset(&mut self) {
        self.apple.reset();
    }
}
