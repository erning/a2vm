use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute};

/// RAII guard for terminal state restoration.
/// Ensures raw mode and alternate screen are properly cleaned up on panic or error.
struct TerminalGuard;

impl TerminalGuard {
    fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
    }
}

use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;

use a2vm_core::keyboard::{map_apple_key, AppleKey};
use a2vm_core::video::{self, BITMAP_HEIGHT, BITMAP_SIZE, BITMAP_STRIDE, BITMAP_WIDTH};
use a2vm_oxide::runner::EmulatorRunner;

mod cli;

const NORMAL_RENDER_INTERVAL_US: u64 = 16_667;
const TURBO_RENDER_INTERVAL_MS: u64 = 50;
const FLASH_HALF_PERIOD_MS: u128 = 267;

const DISPLAY_W: u16 = 142;
const DISPLAY_H: u16 = 50;

const BRAILLE_BIT: [[u8; 4]; 2] = [[0, 1, 2, 6], [3, 4, 5, 7]];

fn bitmap_to_braille(bitmap: &[u8; BITMAP_SIZE]) -> Vec<String> {
    let cols = BITMAP_WIDTH / 2;
    let rows = BITMAP_HEIGHT / 4;
    let mut lines = Vec::with_capacity(rows);

    for brow in 0..rows {
        let mut line = String::with_capacity(cols * 3);
        for bcol in 0..cols {
            let px = bcol * 2;
            let py = brow * 4;
            let mut bits: u8 = 0;

            for (dx, dy_bits) in BRAILLE_BIT.iter().enumerate() {
                for (dy, &braille_bit) in dy_bits.iter().enumerate() {
                    let x = px + dx;
                    let y = py + dy;
                    let byte_idx = y * BITMAP_STRIDE + x / 8;
                    let bit_idx = 7 - (x % 8);
                    let pixel = (bitmap[byte_idx] >> bit_idx) & 1;

                    if pixel != 0 {
                        bits |= 1 << braille_bit;
                    }
                }
            }

            // 0x2800-0x28FF is the Braille Patterns Unicode block
            // bits is 0-255, so this is always a valid char
            let ch = unsafe { char::from_u32_unchecked(0x2800 + bits as u32) };
            line.push(ch);
        }
        lines.push(line);
    }

    lines
}

fn map_key(key: KeyEvent) -> Option<u8> {
    if key.modifiers.contains(KeyModifiers::ALT) {
        return None;
    }

    match key.code {
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            map_apple_key(AppleKey::Control(c))
        }
        KeyCode::Char(c) => map_apple_key(AppleKey::Printable(c)),
        KeyCode::Enter => map_apple_key(AppleKey::Enter),
        KeyCode::Backspace => map_apple_key(AppleKey::Backspace),
        KeyCode::Delete => map_apple_key(AppleKey::Delete),
        KeyCode::Left => map_apple_key(AppleKey::Left),
        KeyCode::Right => map_apple_key(AppleKey::Right),
        KeyCode::Up => map_apple_key(AppleKey::Up),
        KeyCode::Down => map_apple_key(AppleKey::Down),
        KeyCode::Esc => map_apple_key(AppleKey::Escape),
        KeyCode::Tab => map_apple_key(AppleKey::Tab),
        _ => None,
    }
}

struct TuiApp {
    runner: EmulatorRunner,

    boot_time: Instant,
    last_render_time: Instant,

    bitmap: [u8; BITMAP_SIZE],
    last_bitmap: [u8; BITMAP_SIZE],
    braille_lines: Vec<String>,
    braille_initialized: bool,
}

impl TuiApp {
    fn new(cli: &cli::CliArgs) -> io::Result<Self> {
        let rom_data = cli.shared.rom_data().map_err(io::Error::other)?;

        let disk_paths: Vec<&std::path::Path> =
            cli.shared.disk.iter().map(|p| p.as_path()).collect();

        #[cfg(feature = "audio")]
        let runner = EmulatorRunner::new(
            rom_data,
            &disk_paths,
            cli.shared.fast_disk,
            cli.shared.noise,
        )
        .map_err(io::Error::other)?;

        #[cfg(not(feature = "audio"))]
        let runner = EmulatorRunner::new(rom_data, &disk_paths, cli.shared.fast_disk)
            .map_err(io::Error::other)?;

        let now = Instant::now();

        Ok(Self {
            runner,
            boot_time: now,
            last_render_time: now - Duration::from_micros(NORMAL_RENDER_INTERVAL_US),
            bitmap: [0u8; BITMAP_SIZE],
            last_bitmap: [0u8; BITMAP_SIZE],
            braille_lines: vec!["".to_string(); BITMAP_HEIGHT / 4],
            braille_initialized: false,
        })
    }

    fn handle_input(&mut self) -> io::Result<bool> {
        while event::poll(Duration::ZERO)? {
            if let Event::Key(key) = event::read()? {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match key.code {
                        KeyCode::Char('q') => return Ok(true),
                        KeyCode::Char('r') => {
                            self.runner.reset();
                            continue;
                        }
                        KeyCode::Char('t') => {
                            self.runner.toggle_turbo();
                            continue;
                        }
                        _ => {}
                    }
                }

                if let Some(ascii) = map_key(key) {
                    self.runner.apple_mut().key_press(ascii);
                }
            }
        }
        Ok(false)
    }

    fn render(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        let render_interval = if self.runner.is_turbo() {
            Duration::from_millis(TURBO_RENDER_INTERVAL_MS)
        } else {
            Duration::from_micros(NORMAL_RENDER_INTERVAL_US)
        };

        if Instant::now().saturating_duration_since(self.last_render_time) < render_interval {
            return Ok(());
        }

        let flash_on = ((self.boot_time.elapsed().as_millis() / FLASH_HALF_PERIOD_MS) & 1) == 0;
        video::render(
            self.runner.apple().ram(),
            &self.runner.apple().bus.display,
            flash_on,
            &mut self.bitmap,
        );

        if !self.braille_initialized || self.bitmap != self.last_bitmap {
            self.braille_lines = bitmap_to_braille(&self.bitmap);
            self.last_bitmap.copy_from_slice(&self.bitmap);
            self.braille_initialized = true;
        }

        let emu_mhz = self.runner.emu_mhz();
        let apple = self.runner.apple();
        let braille_lines = &self.braille_lines;

        terminal.draw(|frame| {
            let area = frame.area();

            let x = area.x + area.width.saturating_sub(DISPLAY_W) / 2;
            let y = area.y;
            let display_rect =
                Rect::new(x, y, DISPLAY_W.min(area.width), DISPLAY_H.min(area.height));

            let display_lines: Vec<Line> = braille_lines
                .iter()
                .map(|s| Line::from(Span::styled(s.as_str(), Style::default().fg(Color::Green))))
                .collect();

            let display = Paragraph::new(display_lines)
                .block(Block::default().borders(Borders::ALL).title(" A2VM "));
            frame.render_widget(display, display_rect);

            let status_y = display_rect.y + display_rect.height;
            if status_y < area.y + area.height {
                let cpu = &apple.cpu;
                let mode = if apple.bus.display.text {
                    "TEXT"
                } else if apple.bus.display.hires {
                    "HGR"
                } else {
                    "GR"
                };
                let disk_status = if apple.bus.disk.motor_on {
                    format!("D:T{}", apple.bus.disk.half_track / 2)
                } else {
                    "D:--".to_string()
                };
                let fast_label = if apple.is_fast_disk() { " FAST" } else { "" };
                let turbo_label = if self.runner.is_turbo() { " TURBO" } else { "" };
                let status = format!(
                    " PC:{:04X} A:{:02X} X:{:02X} Y:{:02X} SP:{:02X} P:{:02X} {} {}{}{} EMU:{:.2}MHz | Ctrl+Q:Quit Ctrl+R:Reset Ctrl+T:Turbo",
                    cpu.pc(),
                    cpu.a(),
                    cpu.x(),
                    cpu.y(),
                    cpu.sp(),
                    cpu.p().0,
                    mode,
                    disk_status,
                    fast_label,
                    turbo_label,
                    emu_mhz
                );
                let status_rect = Rect::new(display_rect.x, status_y, display_rect.width, 1);
                let status_bar = Paragraph::new(Line::from(Span::styled(
                    status,
                    Style::default().fg(Color::Cyan),
                )));
                frame.render_widget(status_bar, status_rect);
            }
        })?;

        self.last_render_time = Instant::now();
        Ok(())
    }
}

fn main() -> io::Result<()> {
    let cli = cli::parse();

    let _guard = TerminalGuard::new()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = TuiApp::new(&cli)?;

    let frame_duration = Duration::from_micros(NORMAL_RENDER_INTERVAL_US);

    loop {
        let frame_start = Instant::now();

        if app.handle_input()? {
            break;
        }

        app.runner.tick();
        app.render(&mut terminal)?;

        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }

    Ok(())
}
