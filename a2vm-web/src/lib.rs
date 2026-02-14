use a2vm_core::machine::AppleII;
use a2vm_core::timing::CPU_HZ;
use a2vm_core::video::{DisplayColorMode, RGBA_SIZE};
use wasm_bindgen::prelude::*;

const TURBO_SPEEDS: [u64; 5] = [1, 2, 4, 8, 16];
const MAX_DT_MS: f64 = 100.0;

#[wasm_bindgen]
pub struct AppleIIWeb {
    apple: AppleII,
    turbo_index: usize,
    cycle_accum: f64,
    frame_buffer: Vec<u8>,
    audio_buffer: Vec<f32>,
}

#[wasm_bindgen]
impl AppleIIWeb {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();

        Self {
            apple: AppleII::new(),
            turbo_index: 0,
            cycle_accum: 0.0,
            frame_buffer: vec![0u8; RGBA_SIZE],
            audio_buffer: Vec::with_capacity(4096),
        }
    }

    pub fn load_rom(&mut self, data: &[u8]) -> Result<(), JsValue> {
        self.apple
            .load_rom_data(data)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))
    }

    pub fn load_disk(
        &mut self,
        data: &[u8],
        drive: usize,
        write_protected: bool,
    ) -> Result<(), JsValue> {
        self.apple
            .load_disk_bytes_into_drive(data, drive, write_protected)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))
    }

    pub fn reset(&mut self) {
        self.apple.reset();
        self.cycle_accum = 0.0;
    }

    pub fn tick(&mut self, delta_ms: f64) -> u64 {
        let dt = delta_ms.min(MAX_DT_MS);

        self.cycle_accum += dt * (CPU_HZ as f64) / 1000.0;
        let real_cycles = self.cycle_accum as u64;
        self.cycle_accum -= real_cycles as f64;

        let mut cycles_to_run = real_cycles;
        if self.turbo_index > 0 {
            cycles_to_run = cycles_to_run.saturating_mul(TURBO_SPEEDS[self.turbo_index]);
        }

        if cycles_to_run > 0 {
            self.apple.run_cycles(cycles_to_run);
        }

        real_cycles
    }

    pub fn render(&mut self, flash_on: bool, color_mode: u8) {
        let mode = match color_mode {
            0 => DisplayColorMode::Color,
            1 => DisplayColorMode::Monochrome,
            _ => DisplayColorMode::MonochromeScanlines,
        };

        a2vm_core::video::render_rgba(
            self.apple.ram(),
            &self.apple.bus.display,
            flash_on,
            mode,
            0,
            &mut self.frame_buffer,
        );
    }

    pub fn frame_buffer_ptr(&self) -> *const u8 {
        self.frame_buffer.as_ptr()
    }

    pub fn generate_audio(&mut self, sample_rate: u32, real_cycles: u64) -> Vec<f32> {
        self.audio_buffer.clear();
        self.apple
            .take_audio_samples_into(sample_rate, real_cycles, &mut self.audio_buffer);
        self.audio_buffer.clone()
    }

    pub fn key_press(&mut self, ascii: u8) {
        self.apple.key_press(ascii);
    }

    pub fn clear_key_strobe(&mut self) {
        self.apple.clear_kbd_strobe();
    }

    pub fn kbd_latch(&self) -> u8 {
        self.apple.kbd_latch()
    }

    pub fn set_turbo(&mut self, enabled: bool) {
        self.turbo_index = if enabled { 1 } else { 0 };
    }

    pub fn toggle_turbo(&mut self) -> bool {
        self.turbo_index = (self.turbo_index + 1) % TURBO_SPEEDS.len();
        self.turbo_index > 0
    }

    pub fn is_turbo(&self) -> bool {
        self.turbo_index > 0
    }

    pub fn turbo_speed(&self) -> u64 {
        TURBO_SPEEDS[self.turbo_index]
    }

    pub fn set_fast_disk(&mut self, enabled: bool) {
        self.apple.set_fast_disk(enabled);
    }

    pub fn is_fast_disk(&self) -> bool {
        self.apple.is_fast_disk()
    }

    pub fn pc(&self) -> u16 {
        self.apple.cpu.pc()
    }

    pub fn a(&self) -> u8 {
        self.apple.cpu.a()
    }

    pub fn x(&self) -> u8 {
        self.apple.cpu.x()
    }

    pub fn y(&self) -> u8 {
        self.apple.cpu.y()
    }

    pub fn sp(&self) -> u8 {
        self.apple.cpu.sp()
    }

    pub fn display_mode(&self) -> u8 {
        if self.apple.bus.display.text {
            0
        } else if self.apple.bus.display.hires {
            2
        } else {
            1
        }
    }

    pub fn is_motor_on(&self) -> bool {
        self.apple.bus.disk.motor_on
    }

    pub fn disk_track(&self) -> u8 {
        (self.apple.bus.disk.half_track / 2) as u8
    }

    pub fn cycles(&self) -> u64 {
        self.apple.cpu.cycles()
    }

    pub fn export_disk(&self, drive: usize) -> Option<Box<[u8]>> {
        self.apple
            .bus
            .disk
            .export_disk_bytes(drive)
            .map(|data| data.to_vec().into_boxed_slice())
    }

    pub fn has_disk(&self, drive: usize) -> bool {
        self.apple.has_disk(drive)
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        self.apple.read(addr)
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        self.apple.write(addr, val);
    }
}

impl Default for AppleIIWeb {
    fn default() -> Self {
        Self::new()
    }
}

mod console_error_panic_hook {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = console)]
        fn error(msg: String);

        #[wasm_bindgen(js_namespace = console, js_name = error)]
        fn error_with_stack(msg: String, stack: String);
    }

    pub fn set_once() {
        std::panic::set_hook(Box::new(|info| {
            let msg = info.to_string();
            if let Some(location) = info.location() {
                let stack = format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                );
                error_with_stack(msg, stack);
            } else {
                error(msg);
            }
        }));
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}
