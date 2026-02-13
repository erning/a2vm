use std::sync::Arc;
use std::time::{Duration, Instant};

use pixels::{Pixels, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use a2vm_core::keyboard::{map_apple_key, AppleKey};
use a2vm_core::video::{self, DisplayColorMode, RGBA_HEIGHT, RGBA_WIDTH};
use a2vm_oxide::runner::EmulatorRunner;

mod cli;
use crate::cli::CliArgs;

/// Logical framebuffer: 280 wide × 192 tall (display only, no status bar).
const FB_WIDTH: u32 = RGBA_WIDTH as u32;
const FB_HEIGHT: u32 = RGBA_HEIGHT as u32; // 192

/// Default window scale.
const SCALE: u32 = 3;

/// Frame interval (~60 FPS).
const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

/// Flash half-period for text blinking.
const FLASH_HALF_PERIOD_MS: u128 = 267;

// ── App ─────────────────────────────────────────────────────────────

struct App {
    runner: EmulatorRunner,
    pixels: Option<Pixels<'static>>,
    window: Option<Arc<Window>>,

    // Rendering state
    boot_time: Instant,
    flash_on: bool,
    last_flash_on: bool,
    color_mode: DisplayColorMode,
    frame_phase: u64,

    // Status output
    status_printed: bool,
    modifiers: ModifiersState,
}

impl App {
    fn new(cli: &CliArgs) -> Result<Self, Box<dyn std::error::Error>> {
        let rom_data = cli.shared.rom_data()?;

        let disk_paths: Vec<&std::path::Path> =
            cli.shared.disk.iter().map(|p| p.as_path()).collect();

        #[cfg(feature = "audio")]
        let runner = match EmulatorRunner::new(
            rom_data,
            &disk_paths,
            cli.shared.fast_disk,
            cli.shared.noise,
        ) {
            Ok(r) => r,
            Err(e) => return Err(format!("Failed to create emulator: {e}").into()),
        };

        #[cfg(not(feature = "audio"))]
        let runner = match EmulatorRunner::new(rom_data, &disk_paths, cli.shared.fast_disk) {
            Ok(r) => r,
            Err(e) => return Err(format!("Failed to create emulator: {e}").into()),
        };

        Ok(Self {
            runner,
            pixels: None,
            window: None,
            boot_time: Instant::now(),
            flash_on: false,
            last_flash_on: false,
            color_mode: cli.color_mode.into(),
            frame_phase: 0,
            status_printed: false,
            modifiers: ModifiersState::empty(),
        })
    }

    fn render_frame(&mut self) {
        // Skip rendering if video RAM and display mode haven't changed
        // (scanline mode always re-renders due to animation)
        let dirty = self.runner.apple().bus.video_dirty
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
        self.runner.apple_mut().bus.video_dirty = false;
        self.last_flash_on = self.flash_on;

        let pixels = match self.pixels.as_mut() {
            Some(p) => p,
            None => return,
        };

        let frame = pixels.frame_mut();

        video::render_rgba(
            self.runner.apple().ram(),
            &self.runner.apple().bus.display,
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

    fn update_status(&mut self) {
        // Update flash toggle
        self.flash_on = ((self.boot_time.elapsed().as_millis() / FLASH_HALF_PERIOD_MS) & 1) == 0;

        // Print status line to stderr (similar to TUI status bar)
        let cpu = &self.runner.apple().cpu;
        let mode = if self.runner.apple().bus.display.text {
            "TEXT"
        } else if self.runner.apple().bus.display.hires {
            "HGR"
        } else {
            "GR"
        };
        let disk_status = if self.runner.apple().bus.disk.motor_on {
            format!("D:T{}", self.runner.apple().bus.disk.half_track / 2)
        } else {
            "D:--".to_string()
        };
        let turbo_label = if self.runner.is_turbo() { " TURBO" } else { "" };
        let fast_label = if self.runner.apple().is_fast_disk() {
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
            self.runner.emu_mhz()
        );
        self.status_printed = true;
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
                    "q" => {
                        event_loop.exit();
                        return;
                    }
                    "r" => {
                        self.runner.reset();
                        return;
                    }
                    "t" => {
                        self.runner.toggle_turbo();
                        return;
                    }
                    _ => {}
                }
            }
        }

        // Map to Apple II ASCII
        if let Some(ascii) = map_winit_key(event, ctrl) {
            self.runner.apple_mut().key_press(ascii);
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
        if let Key::Character(c) = &event.logical_key {
            let ch = c.chars().next()?;
            return map_apple_key(AppleKey::Control(ch));
        }
        return None;
    }

    match &event.logical_key {
        Key::Character(c) => {
            let ch = c.chars().next()?;
            map_apple_key(AppleKey::Printable(ch))
        }
        Key::Named(named) => match named {
            NamedKey::Enter => map_apple_key(AppleKey::Enter),
            NamedKey::Backspace => map_apple_key(AppleKey::Backspace),
            NamedKey::Delete => map_apple_key(AppleKey::Delete),
            NamedKey::Space => map_apple_key(AppleKey::Printable(' ')),
            NamedKey::ArrowLeft => map_apple_key(AppleKey::Left),
            NamedKey::ArrowRight => map_apple_key(AppleKey::Right),
            NamedKey::ArrowUp => map_apple_key(AppleKey::Up),
            NamedKey::ArrowDown => map_apple_key(AppleKey::Down),
            NamedKey::Escape => map_apple_key(AppleKey::Escape),
            NamedKey::Tab => map_apple_key(AppleKey::Tab),
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
        // Run emulation tick
        self.runner.tick();

        // Update status line
        self.update_status();

        // Request redraw
        if let Some(ref window) = self.window {
            window.request_redraw();
        }

        _event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME_INTERVAL));
    }
}

// ── main ────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::parse();
    let mut app = App::new(&cli)?;

    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}
