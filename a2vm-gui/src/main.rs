use std::sync::Arc;
use std::time::{Duration, Instant};

use pixels::{Pixels, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

#[cfg(feature = "audio")]
use rodio::buffer::SamplesBuffer;
#[cfg(feature = "audio")]
use rodio::{OutputStream, OutputStreamBuilder, Sink};

use a2vm_core::machine::AppleII;
use a2vm_core::timing::CPU_HZ;
use a2vm_core::video::{self, DisplayColorMode, RGBA_HEIGHT, RGBA_WIDTH};

mod cli;
use crate::cli::CliArgs;

/// Logical framebuffer: 280 wide × 192 tall (display only, no status bar).
const FB_WIDTH: u32 = RGBA_WIDTH as u32;
const FB_HEIGHT: u32 = RGBA_HEIGHT as u32; // 192

/// Default window scale.
const SCALE: u32 = 3;

/// Turbo multiplier.
const TURBO_MULTIPLIER: u64 = 4;

/// Frame interval (~60 FPS).
const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

/// Flash half-period for text blinking.
const FLASH_HALF_PERIOD_MS: u128 = 267;

/// Perf measurement interval.
const PERF_SAMPLE_INTERVAL: Duration = Duration::from_millis(250);

/// PCM output sample rate.
#[cfg(feature = "audio")]
const AUDIO_SAMPLE_RATE: u32 = 44_100;

// ── App ─────────────────────────────────────────────────────────────

struct App {
    apple: AppleII,
    pixels: Option<Pixels<'static>>,
    window: Option<Arc<Window>>,

    // Timing
    boot_time: Instant,
    last_tick: Instant,
    cycle_accum: u128,
    turbo: bool,
    emu_mhz: f64,
    perf_last_time: Instant,
    perf_last_cycles: u64,
    flash_on: bool,
    last_flash_on: bool,

    // Audio
    #[cfg(feature = "audio")]
    _audio_stream: Option<OutputStream>,
    #[cfg(feature = "audio")]
    audio_sink: Option<Sink>,
    #[cfg(feature = "audio")]
    audio_buffer: Vec<f32>,

    #[allow(dead_code)]
    fast_disk: bool,
    modifiers: ModifiersState,
    status_printed: bool,
    color_mode: DisplayColorMode,
    frame_phase: u64,
}

impl App {
    fn new(cli: &CliArgs) -> Self {
        let disk_file = cli.disk.as_ref().or(cli.fast_disk.as_ref());
        let mut apple = AppleII::new();
        apple.load_rom(&cli.rom).unwrap_or_else(|e| {
            eprintln!("Error loading ROM: {e}");
            std::process::exit(1);
        });

        apple.set_disk_controller_enabled(disk_file.is_some());

        if let Some(disk) = disk_file {
            apple.load_disk(disk).unwrap_or_else(|e| {
                eprintln!("Error loading disk: {e}");
                std::process::exit(1);
            });
        }

        if cli.fast_disk.is_some() {
            apple.set_fast_disk(true);
        }

        apple.reset();

        // Audio setup (best-effort)
        #[cfg(feature = "audio")]
        let (audio_stream, audio_sink) = match OutputStreamBuilder::open_default_stream() {
            Ok(stream) => {
                let sink = Sink::connect_new(stream.mixer());
                (Some(stream), Some(sink))
            }
            Err(_) => (None, None),
        };

        let now = Instant::now();

        App {
            apple,
            pixels: None,
            window: None,
            boot_time: now,
            last_tick: now,
            cycle_accum: 0,
            turbo: false,
            emu_mhz: 0.0,
            perf_last_time: now,
            perf_last_cycles: 0,
            flash_on: false,
            last_flash_on: false,
            #[cfg(feature = "audio")]
            _audio_stream: audio_stream,
            #[cfg(feature = "audio")]
            audio_sink,
            #[cfg(feature = "audio")]
            audio_buffer: Vec::with_capacity(4096),
            fast_disk: cli.fast_disk.is_some(),
            modifiers: ModifiersState::empty(),
            status_printed: false,
            color_mode: cli.color_mode.into(),
            frame_phase: 0,
        }
    }

    fn run_emulation(&mut self) {
        let now = Instant::now();
        let mut dt = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;

        // Cap dt to avoid spiral of death
        if dt > Duration::from_millis(100) {
            dt = Duration::from_millis(100);
        }

        self.cycle_accum += dt.as_nanos() * CPU_HZ as u128;
        let real_cycles = (self.cycle_accum / 1_000_000_000) as u64;
        self.cycle_accum %= 1_000_000_000;

        let mut cycles_to_run = real_cycles;
        if self.turbo {
            cycles_to_run = cycles_to_run.saturating_mul(TURBO_MULTIPLIER);
        }

        if cycles_to_run != 0 {
            self.apple.run_cycles(cycles_to_run);

            #[cfg(feature = "audio")]
            if let Some(ref sink) = self.audio_sink {
                self.audio_buffer.clear();
                self.apple.take_audio_samples_into(
                    AUDIO_SAMPLE_RATE,
                    real_cycles,
                    &mut self.audio_buffer,
                );
                if !self.audio_buffer.is_empty() {
                    sink.append(SamplesBuffer::new(
                        1,
                        AUDIO_SAMPLE_RATE,
                        std::mem::take(&mut self.audio_buffer),
                    ));
                }
            }
        }

        let perf_now = Instant::now();
        let perf_elapsed = perf_now.saturating_duration_since(self.perf_last_time);
        if perf_elapsed >= PERF_SAMPLE_INTERVAL {
            let delta_cycles = self
                .apple
                .cpu
                .cycles()
                .saturating_sub(self.perf_last_cycles);
            let secs = perf_elapsed.as_secs_f64();
            if secs > 0.0 {
                self.emu_mhz = delta_cycles as f64 / secs / 1_000_000.0;
            }
            self.perf_last_cycles = self.apple.cpu.cycles();
            self.perf_last_time = perf_now;

            let cpu = &self.apple.cpu;
            let mode = if self.apple.bus.display.text {
                "TEXT"
            } else if self.apple.bus.display.hires {
                "HGR"
            } else {
                "GR"
            };
            let disk_status = if self.apple.bus.disk.motor_on {
                format!("D:T{}", self.apple.bus.disk.half_track / 2)
            } else {
                "D:--".to_string()
            };
            let turbo_label = if self.turbo { " TURBO" } else { "" };
            let fast_label = if self.apple.is_fast_disk() {
                " FAST"
            } else {
                ""
            };
            eprint!(
                "\rPC:{:04X} A:{:02X} X:{:02X} Y:{:02X} SP:{:02X} {} {}{}{} {:.2}MHz",
                cpu.pc(),
                cpu.a(),
                cpu.x(),
                cpu.y(),
                cpu.sp(),
                mode,
                disk_status,
                turbo_label,
                fast_label,
                self.emu_mhz
            );
            self.status_printed = true;
        }

        // Flash toggle
        self.flash_on = ((self.boot_time.elapsed().as_millis() / FLASH_HALF_PERIOD_MS) & 1) == 0;
    }

    fn render_frame(&mut self) {
        // Skip rendering if video RAM and display mode haven't changed
        // (scanline mode always re-renders due to animation)
        let dirty = self.apple.bus.video_dirty
            || self.flash_on != self.last_flash_on
            || self.color_mode == DisplayColorMode::MonochromeScanlines;
        if !dirty {
            // Still need to present the existing frame
            if let Some(ref pixels) = self.pixels {
                if let Err(e) = pixels.render() {
                    eprintln!("render: {e}");
                }
            }
            return;
        }
        self.apple.bus.video_dirty = false;
        self.last_flash_on = self.flash_on;

        let pixels = match self.pixels.as_mut() {
            Some(p) => p,
            None => return,
        };

        let frame = pixels.frame_mut();

        video::render_rgba(
            self.apple.ram(),
            &self.apple.bus.display,
            self.flash_on,
            self.color_mode,
            self.frame_phase,
            frame,
        );
        self.frame_phase = self.frame_phase.wrapping_add(1);

        if let Err(e) = pixels.render() {
            eprintln!("render: {e}");
        }
    }

    fn handle_key(&mut self, event: &KeyEvent, event_loop: &ActiveEventLoop) {
        if event.state != ElementState::Pressed {
            return;
        }

        let ctrl = self.modifiers.control_key();
        let alt = self.modifiers.alt_key();

        // Ignore Alt-modified keys
        if alt {
            return;
        }

        // Hotkeys
        if ctrl {
            if let Key::Character(c) = &event.logical_key {
                match c.as_str() {
                    "q" | "c" => {
                        event_loop.exit();
                        return;
                    }
                    "r" => {
                        self.apple.reset();
                        return;
                    }
                    "t" => {
                        self.turbo = !self.turbo;
                        return;
                    }
                    _ => {}
                }
            }
        }

        // Map to Apple II ASCII
        if let Some(ascii) = map_winit_key(event, ctrl) {
            self.apple.key_press(ascii);
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if self.status_printed {
            eprintln!();
        }
    }
}

/// Map a winit key event to Apple II ASCII.
fn map_winit_key(event: &KeyEvent, ctrl: bool) -> Option<u8> {
    if ctrl {
        // Ctrl+A..Z → 0x01..0x1A
        if let Key::Character(c) = &event.logical_key {
            let ch = c.chars().next()?;
            let ctrl_code = (ch.to_ascii_uppercase() as u8).wrapping_sub(b'@');
            if (1..=26).contains(&ctrl_code) {
                return Some(ctrl_code);
            }
        }
        return None;
    }

    match &event.logical_key {
        Key::Character(c) => {
            let ch = c.chars().next()?;
            if ch.is_ascii() {
                let mut ascii = ch as u8;
                if ascii.is_ascii_lowercase() {
                    ascii -= 0x20;
                }
                Some(ascii)
            } else {
                None
            }
        }
        Key::Named(named) => match named {
            NamedKey::Enter => Some(0x0D),
            NamedKey::Backspace => Some(0x08),
            NamedKey::Delete => Some(0x7F),
            NamedKey::ArrowLeft => Some(0x08),
            NamedKey::ArrowRight => Some(0x15),
            NamedKey::ArrowUp => Some(0x0B),
            NamedKey::ArrowDown => Some(0x0A),
            NamedKey::Escape => Some(0x1B),
            NamedKey::Tab => Some(0x09),
            _ => None,
        },
        _ => None,
    }
}

// ── ApplicationHandler ──────────────────────────────────────────────

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let size = LogicalSize::new(FB_WIDTH * SCALE, FB_HEIGHT * SCALE);
        let attrs = Window::default_attributes()
            .with_title("A2VM")
            .with_inner_size(size)
            .with_min_inner_size(LogicalSize::new(FB_WIDTH, FB_HEIGHT));

        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));

        let physical = window.inner_size();
        let surface = SurfaceTexture::new(physical.width, physical.height, window.clone());
        let pixels = Pixels::new(FB_WIDTH, FB_HEIGHT, surface).expect("create pixels");

        self.window = Some(window);
        self.pixels = Some(pixels);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(ref mut pixels) = self.pixels {
                    pixels
                        .resize_surface(size.width.max(1), size.height.max(1))
                        .ok();
                }
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }
            WindowEvent::KeyboardInput {
                event: ref key_event,
                ..
            } => {
                self.handle_key(key_event, event_loop);
            }
            WindowEvent::RedrawRequested => {
                self.render_frame();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.run_emulation();

        if let Some(ref window) = self.window {
            window.request_redraw();
        }

        _event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME_INTERVAL));
    }
}

// ── main ────────────────────────────────────────────────────────────

fn main() {
    let cli = cli::parse();
    let mut app = App::new(&cli);

    let event_loop = EventLoop::new().expect("create event loop");
    event_loop.run_app(&mut app).expect("run event loop");
}
