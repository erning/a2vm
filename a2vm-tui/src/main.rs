use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;
#[cfg(feature = "audio")]
use rodio::buffer::SamplesBuffer;
#[cfg(feature = "audio")]
use rodio::source::Source;
#[cfg(feature = "audio")]
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};
#[cfg(feature = "audio")]
use std::io::Cursor;

use a2vm_core::keyboard::{map_apple_key, AppleKey};
use a2vm_core::machine::AppleII;
use a2vm_core::timing::CPU_HZ;
use a2vm_core::video::{self, BITMAP_HEIGHT, BITMAP_SIZE, BITMAP_STRIDE, BITMAP_WIDTH};
#[cfg(feature = "audio")]
use a2vm_oxide::noise::{DiskMechTracker, MechanicalEvent, MOVE_ARM_WAV};

mod cli;

const TURBO_MULTIPLIER: u64 = 4;
const NORMAL_RENDER_INTERVAL_US: u64 = 16_667;
const TURBO_RENDER_INTERVAL_MS: u64 = 50;
const FLASH_HALF_PERIOD_MS: u128 = 267;
const PERF_SAMPLE_INTERVAL_MS: u64 = 250;

#[cfg(feature = "audio")]
const AUDIO_SAMPLE_RATE: u32 = 44_100;

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

            let ch = char::from_u32(0x2800 + bits as u32).expect("valid braille codepoint");
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
    apple: AppleII,
    turbo: bool,
    emu_mhz: f64,

    boot_time: Instant,
    last_emu_tick: Instant,
    last_render_time: Instant,
    cycle_accum: u128,
    perf_last_time: Instant,
    perf_last_cycles: u64,

    #[cfg(feature = "audio")]
    _audio_stream: Option<OutputStream>,
    #[cfg(feature = "audio")]
    audio_sink: Option<Sink>,
    #[cfg(feature = "audio")]
    mech_sink: Option<Sink>,
    #[cfg(feature = "audio")]
    mech_tracker: DiskMechTracker,
    #[cfg(feature = "audio")]
    audio_buffer: Vec<f32>,
    noise: bool,

    bitmap: [u8; BITMAP_SIZE],
    last_bitmap: [u8; BITMAP_SIZE],
    braille_lines: Vec<String>,
    braille_initialized: bool,
}

impl TuiApp {
    fn new(cli: &cli::CliArgs) -> io::Result<Self> {
        let mut apple = AppleII::new();
        apple.load_rom(&cli.rom).map_err(io::Error::other)?;

        apple.set_disk_controller_enabled(!cli.disk.is_empty());

        for (drive, disk) in cli.disk.iter().enumerate() {
            apple
                .load_disk_into_drive(disk, drive)
                .map_err(io::Error::other)?;
        }

        if cli.fast_disk {
            apple.set_fast_disk(true);
        }

        apple.reset();

        #[cfg(feature = "audio")]
        let (_audio_stream, audio_sink, mech_sink) =
            match OutputStreamBuilder::open_default_stream() {
                Ok(stream) => {
                    let speaker = Sink::connect_new(stream.mixer());
                    let mech = Sink::connect_new(stream.mixer());
                    (Some(stream), Some(speaker), Some(mech))
                }
                Err(_) => (None, None, None),
            };

        let now = Instant::now();

        Ok(Self {
            apple,
            turbo: false,
            emu_mhz: 0.0,
            boot_time: now,
            last_emu_tick: now,
            last_render_time: now - Duration::from_micros(NORMAL_RENDER_INTERVAL_US),
            cycle_accum: 0,
            perf_last_time: now,
            perf_last_cycles: 0,
            #[cfg(feature = "audio")]
            _audio_stream,
            #[cfg(feature = "audio")]
            audio_sink,
            #[cfg(feature = "audio")]
            mech_sink,
            #[cfg(feature = "audio")]
            mech_tracker: DiskMechTracker::new(),
            #[cfg(feature = "audio")]
            audio_buffer: Vec::with_capacity(4096),
            noise: cli.noise,
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
                            self.apple.reset();
                            continue;
                        }
                        KeyCode::Char('t') => {
                            self.turbo = !self.turbo;
                            continue;
                        }
                        _ => {}
                    }
                }

                if let Some(ascii) = map_key(key) {
                    self.apple.key_press(ascii);
                }
            }
        }
        Ok(false)
    }

    fn run_emulation(&mut self) {
        let now = Instant::now();
        let mut dt = now.saturating_duration_since(self.last_emu_tick);
        self.last_emu_tick = now;

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

        if cycles_to_run == 0 {
            return;
        }

        self.apple.run_cycles(cycles_to_run);

        #[cfg(feature = "audio")]
        if let Some(ref sink) = self.audio_sink {
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

        #[cfg(feature = "audio")]
        if self.noise {
            if let Some(ref sink) = self.mech_sink {
                let event = self
                    .mech_tracker
                    .check(self.apple.bus.disk.motor_on, self.apple.bus.disk.half_track);
                if let Some(evt) = event {
                    match evt {
                        MechanicalEvent::MotorStart => {
                            let cursor = Cursor::new(MOVE_ARM_WAV);
                            if let Ok(source) = Decoder::new(cursor) {
                                sink.append(source.repeat_infinite());
                            }
                        }
                        MechanicalEvent::TrackSeek => {
                            sink.stop();
                            let cursor = Cursor::new(MOVE_ARM_WAV);
                            if let Ok(source) = Decoder::new(cursor) {
                                sink.append(source.repeat_infinite());
                            }
                        }
                        MechanicalEvent::MotorStop => {
                            sink.stop();
                        }
                    }
                }
            }
        }
    }

    fn update_perf(&mut self) {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.perf_last_time);

        if elapsed >= Duration::from_millis(PERF_SAMPLE_INTERVAL_MS) {
            let delta_cycles = self
                .apple
                .cpu
                .cycles()
                .saturating_sub(self.perf_last_cycles);
            let secs = elapsed.as_secs_f64();
            if secs > 0.0 {
                self.emu_mhz = delta_cycles as f64 / secs / 1_000_000.0;
            }
            self.perf_last_cycles = self.apple.cpu.cycles();
            self.perf_last_time = now;
        }
    }

    fn render(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        let render_interval = if self.turbo {
            Duration::from_millis(TURBO_RENDER_INTERVAL_MS)
        } else {
            Duration::from_micros(NORMAL_RENDER_INTERVAL_US)
        };

        if Instant::now().saturating_duration_since(self.last_render_time) < render_interval {
            return Ok(());
        }

        let flash_on = ((self.boot_time.elapsed().as_millis() / FLASH_HALF_PERIOD_MS) & 1) == 0;
        video::render(
            self.apple.ram(),
            &self.apple.bus.display,
            flash_on,
            &mut self.bitmap,
        );

        if !self.braille_initialized || self.bitmap != self.last_bitmap {
            self.braille_lines = bitmap_to_braille(&self.bitmap);
            self.last_bitmap.copy_from_slice(&self.bitmap);
            self.braille_initialized = true;
        }

        let emu_mhz = self.emu_mhz;
        let apple = &self.apple;
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
                let status = format!(
                    " PC:{:04X} A:{:02X} X:{:02X} Y:{:02X} SP:{:02X} P:{:02X} {} {}{} EMU:{:.2}MHz | Ctrl+Q:Quit Ctrl+R:Reset Ctrl+T:Turbo",
                    cpu.pc(),
                    cpu.a(),
                    cpu.x(),
                    cpu.y(),
                    cpu.sp(),
                    cpu.p().0,
                    mode,
                    disk_status,
                    fast_label,
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

    terminal::enable_raw_mode()?;
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

        app.run_emulation();
        app.update_perf();
        app.render(&mut terminal)?;

        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }

    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)?;

    Ok(())
}
