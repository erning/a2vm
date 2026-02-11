use std::path::Path;
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
use a2vm_core::video::{self, DisplayColorMode, RGBA_HEIGHT, RGBA_WIDTH};

/// Logical framebuffer: 280 wide × 192 tall (display only, no status bar).
const FB_WIDTH: u32 = RGBA_WIDTH as u32;
const FB_HEIGHT: u32 = RGBA_HEIGHT as u32; // 192

/// Default window scale.
const SCALE: u32 = 3;

/// Apple II CPU target clock (NTSC), ~1.023 MHz.
const CPU_HZ: u64 = 1_023_000;

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

// ── CLI parsing ─────────────────────────────────────────────────────

struct CliArgs {
    rom_path: String,
    disk_file: Option<String>,
    fast_disk: bool,
    color_mode: DisplayColorMode,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut rom_path_str: Option<String> = None;
    let mut disk_path_str: Option<String> = None;
    let mut fast_disk_str: Option<String> = None;
    let mut color_mode_str: Option<String> = None;
    let mut show_help = false;
    let mut error = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => show_help = true,
            "--rom" => {
                i += 1;
                rom_path_str = args.get(i).cloned();
            }
            "--disk" => {
                i += 1;
                disk_path_str = args.get(i).cloned();
            }
            "--fast-disk" => {
                i += 1;
                fast_disk_str = args.get(i).cloned();
            }
            "--color-mode" => {
                i += 1;
                color_mode_str = args.get(i).cloned();
            }
            other => {
                eprintln!("Error: unknown option: {other}");
                error = true;
            }
        }
        i += 1;
    }

    if disk_path_str.is_some() && fast_disk_str.is_some() {
        eprintln!("Error: --disk and --fast-disk are mutually exclusive");
        error = true;
    }

    let color_mode = match color_mode_str.as_deref() {
        Some("color") | None => DisplayColorMode::Color,
        Some("mono") => DisplayColorMode::Monochrome,
        Some("mono-scanlines") => DisplayColorMode::MonochromeScanlines,
        Some(other) => {
            eprintln!(
                "Error: invalid color mode '{other}'; use 'color', 'mono', or 'mono-scanlines'"
            );
            error = true;
            DisplayColorMode::Color
        }
    };

    if show_help || error {
        eprintln!(
            "Usage: {} --rom <file> [--disk <file> | --fast-disk <file>] [--color-mode <mode>]",
            args[0]
        );
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --rom <file>        Apple II/II+ ROM (12K or 20K) [env: A2VM_ROM]");
        eprintln!("  --disk <file>       .dsk disk image (143360 bytes)");
        eprintln!("  --fast-disk <file>  .dsk disk image with DOS 3.3 RWTS trap ($B7B5)");
        eprintln!("                      for instant sector reads; only for DOS 3.3 disks");
        eprintln!(
            "  --color-mode <mode> Display mode: 'color' (default), 'mono', 'mono-scanlines'"
        );
        eprintln!("  -h, --help          Show this help");
        std::process::exit(if error { 2 } else { 0 });
    }

    let rom_path = rom_path_str
        .or_else(|| std::env::var("A2VM_ROM").ok())
        .unwrap_or_else(|| {
            eprintln!("Error: ROM not specified; use --rom <file> or set A2VM_ROM");
            std::process::exit(2);
        });

    let fast_disk = fast_disk_str.is_some();
    let disk_file = disk_path_str.or(fast_disk_str);

    CliArgs {
        rom_path,
        disk_file,
        fast_disk,
        color_mode,
    }
}

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

    // Audio
    #[cfg(feature = "audio")]
    _audio_stream: Option<OutputStream>,
    #[cfg(feature = "audio")]
    audio_sink: Option<Sink>,

    #[allow(dead_code)]
    fast_disk: bool,
    modifiers: ModifiersState,
    status_printed: bool,
    color_mode: DisplayColorMode,
    frame_phase: u64,
}

impl App {
    fn new(cli: &CliArgs) -> Self {
        let mut apple = AppleII::new();
        apple
            .load_rom(Path::new(&cli.rom_path))
            .unwrap_or_else(|e| {
                eprintln!("Error loading ROM: {e}");
                std::process::exit(1);
            });

        apple.set_disk_controller_enabled(cli.disk_file.is_some());

        if let Some(ref disk) = cli.disk_file {
            apple.load_disk(Path::new(disk)).unwrap_or_else(|e| {
                eprintln!("Error loading disk: {e}");
                std::process::exit(1);
            });
        }

        if cli.fast_disk {
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
            #[cfg(feature = "audio")]
            _audio_stream: audio_stream,
            #[cfg(feature = "audio")]
            audio_sink,
            fast_disk: cli.fast_disk,
            modifiers: ModifiersState::empty(),
            status_printed: false,
            color_mode: cli.color_mode,
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
                let pcm = self
                    .apple
                    .take_audio_samples(AUDIO_SAMPLE_RATE, real_cycles);
                if !pcm.is_empty() {
                    sink.append(SamplesBuffer::new(1, AUDIO_SAMPLE_RATE, pcm));
                }
            }
        }

        let perf_now = Instant::now();
        let perf_elapsed = perf_now.saturating_duration_since(self.perf_last_time);
        if perf_elapsed >= PERF_SAMPLE_INTERVAL {
            let delta_cycles = self.apple.cpu.cycles.saturating_sub(self.perf_last_cycles);
            let secs = perf_elapsed.as_secs_f64();
            if secs > 0.0 {
                self.emu_mhz = delta_cycles as f64 / secs / 1_000_000.0;
            }
            self.perf_last_cycles = self.apple.cpu.cycles;
            self.perf_last_time = perf_now;

            let cpu = &self.apple.cpu;
            let mode = if self.apple.display.text {
                "TEXT"
            } else if self.apple.display.hires {
                "HGR"
            } else {
                "GR"
            };
            let disk_status = if self.apple.disk.motor_on {
                format!("D:T{}", self.apple.disk.half_track / 2)
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
                cpu.pc,
                cpu.a,
                cpu.x,
                cpu.y,
                cpu.sp,
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
        let pixels = match self.pixels.as_mut() {
            Some(p) => p,
            None => return,
        };

        let frame = pixels.frame_mut();

        video::render_rgba(
            self.apple.ram(),
            &self.apple.display,
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
    let cli = parse_args();
    let mut app = App::new(&cli);

    let event_loop = EventLoop::new().expect("create event loop");
    event_loop.run_app(&mut app).expect("run event loop");
}
