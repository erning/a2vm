use std::io;
use std::path::Path;
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
use rodio::{OutputStream, Sink};

use a2vm_core::machine::AppleII;
use a2vm_core::video::{self, BITMAP_HEIGHT, BITMAP_SIZE, BITMAP_STRIDE, BITMAP_WIDTH};

/// Apple II CPU target clock (NTSC), ~1.023 MHz.
const CPU_HZ: u64 = 1_023_000;

/// Turbo multiplier when enabled from the TUI.
const TURBO_MULTIPLIER: u64 = 4;

/// Normal draw cadence (~60 FPS).
const NORMAL_RENDER_INTERVAL_US: u64 = 16_667;

/// Turbo draw cadence (reduced redraw pressure).
const TURBO_RENDER_INTERVAL_MS: u64 = 50;

/// Flash half-period used by Apple II text blinking.
const FLASH_HALF_PERIOD_MS: u128 = 267;

/// Perf sample window for measured emulation speed.
const PERF_SAMPLE_INTERVAL_MS: u64 = 250;

/// PCM output sample rate.
#[cfg(feature = "audio")]
const AUDIO_SAMPLE_RATE: u32 = 44_100;

/// Convert a 280×192 monochrome bitmap to a grid of Braille characters.
///
/// Each Braille character (U+2800..U+28FF) encodes a 2×4 dot matrix:
///   dot positions → bit mapping:
///     (0,0)→0  (1,0)→3
///     (0,1)→1  (1,1)→4
///     (0,2)→2  (1,2)→5
///     (0,3)→6  (1,3)→7
///
/// Result: 140 columns × 48 rows.
fn bitmap_to_braille(bitmap: &[u8; BITMAP_SIZE]) -> Vec<String> {
    let cols = BITMAP_WIDTH / 2; // 140
    let rows = BITMAP_HEIGHT / 4; // 48
    let mut lines = Vec::with_capacity(rows);

    for brow in 0..rows {
        let mut line = String::with_capacity(cols * 3); // UTF-8 braille = 3 bytes each
        for bcol in 0..cols {
            let px = bcol * 2;
            let py = brow * 4;
            let mut bits: u8 = 0;

            // Sample the 2×4 pixel block
            for dy in 0..4u8 {
                for dx in 0..2u8 {
                    let x = px + dx as usize;
                    let y = py + dy as usize;
                    let byte_idx = y * BITMAP_STRIDE + x / 8;
                    let bit_idx = 7 - (x % 8); // MSB-first
                    let pixel = (bitmap[byte_idx] >> bit_idx) & 1;

                    if pixel != 0 {
                        // Map (dx, dy) to braille bit position
                        let braille_bit = match (dx, dy) {
                            (0, 0) => 0,
                            (0, 1) => 1,
                            (0, 2) => 2,
                            (0, 3) => 6,
                            (1, 0) => 3,
                            (1, 1) => 4,
                            (1, 2) => 5,
                            (1, 3) => 7,
                            _ => unreachable!(),
                        };
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

/// Map a crossterm KeyEvent to an Apple II ASCII value.
/// Returns None if the key should not be sent to the Apple II.
fn map_key(key: KeyEvent) -> Option<u8> {
    // Don't send keys with Alt modifier
    if key.modifiers.contains(KeyModifiers::ALT) {
        return None;
    }

    match key.code {
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Ctrl+A..Z → $01..$1A
            let ctrl = (c.to_ascii_uppercase() as u8).wrapping_sub(b'@');
            if (1..=26).contains(&ctrl) {
                Some(ctrl)
            } else {
                None
            }
        }
        KeyCode::Char(c) => {
            let mut ascii = c as u8;
            // Apple II only has uppercase; convert lowercase
            if ascii.is_ascii_lowercase() {
                ascii -= 0x20;
            }
            Some(ascii)
        }
        KeyCode::Enter => Some(0x0D),
        KeyCode::Backspace => Some(0x08), // left arrow (delete)
        KeyCode::Delete => Some(0x7F),
        KeyCode::Left => Some(0x08),
        KeyCode::Right => Some(0x15),
        KeyCode::Up => Some(0x0B),
        KeyCode::Down => Some(0x0A),
        KeyCode::Esc => Some(0x1B),
        KeyCode::Tab => Some(0x09),
        _ => None,
    }
}

fn main() -> io::Result<()> {
    // Parse command line options
    let args: Vec<String> = std::env::args().collect();
    let mut rom_path_str: Option<String> = None;
    let mut disk_path_str: Option<String> = None;
    let mut fast_disk_str: Option<String> = None;
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
            other => {
                eprintln!("Error: unknown option: {other}");
                error = true;
            }
        }
        i += 1;
    }

    // --disk and --fast-disk are mutually exclusive
    if disk_path_str.is_some() && fast_disk_str.is_some() {
        eprintln!("Error: --disk and --fast-disk are mutually exclusive");
        error = true;
    }

    if show_help || error {
        eprintln!(
            "Usage: {} --rom <file> [--disk <file> | --fast-disk <file>]",
            args[0]
        );
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --rom <file>        Apple II/II+ ROM (12K or 20K) [env: A2VM_ROM]");
        eprintln!("  --disk <file>       .dsk disk image (143360 bytes)");
        eprintln!("  --fast-disk <file>  .dsk disk image with DOS 3.3 RWTS trap ($B7B5)");
        eprintln!("                      for instant sector reads; only for DOS 3.3 disks");
        eprintln!("  -h, --help          Show this help");
        std::process::exit(if error { 2 } else { 0 });
    }

    // Resolve ROM path: --rom > $A2VM_ROM
    let rom_path_str = rom_path_str
        .or_else(|| std::env::var("A2VM_ROM").ok())
        .unwrap_or_else(|| {
            eprintln!("Error: ROM not specified; use --rom <file> or set A2VM_ROM");
            std::process::exit(2);
        });
    let rom_path = Path::new(&rom_path_str);

    // Resolve disk: --disk or --fast-disk
    let fast_disk = fast_disk_str.is_some();
    let disk_file = disk_path_str.or(fast_disk_str);

    // Create Apple II and load ROM
    let mut apple = AppleII::new();
    apple.load_rom(rom_path)?;

    // Expose Disk II controller only when a disk image is provided.
    apple.set_disk_controller_enabled(disk_file.is_some());

    if let Some(ref disk) = disk_file {
        apple.load_disk(Path::new(disk))?;
    }

    if fast_disk {
        apple.set_fast_disk(true);
    }

    apple.reset();

    // Set up audio playback (best-effort).
    #[cfg(feature = "audio")]
    let mut audio: Option<(OutputStream, Sink)> = match OutputStream::try_default() {
        Ok((stream, handle)) => Sink::try_new(&handle).ok().map(|sink| (stream, sink)),
        Err(_) => None,
    };
    #[cfg(not(feature = "audio"))]
    let _audio: () = ();

    // Set up terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut bitmap = [0u8; BITMAP_SIZE];
    let mut last_bitmap = [0u8; BITMAP_SIZE];
    let mut braille_lines = vec!["".to_string(); BITMAP_HEIGHT / 4];
    let mut braille_initialized = false;
    let frame_duration = Duration::from_micros(NORMAL_RENDER_INTERVAL_US);
    let turbo_render_interval = Duration::from_millis(TURBO_RENDER_INTERVAL_MS);
    let perf_sample_interval = Duration::from_millis(PERF_SAMPLE_INTERVAL_MS);
    let boot_time = Instant::now();
    let mut last_render_time = Instant::now() - frame_duration;
    let mut last_emu_tick = Instant::now();
    let mut cycle_accum: u128 = 0;
    let mut turbo = false;
    let mut emu_mhz: f64 = 0.0;
    let mut perf_last_time = Instant::now();
    let mut perf_last_cycles = apple.cpu.cycles;

    // Main loop
    loop {
        let frame_start = Instant::now();

        // Poll keyboard events (non-blocking)
        while event::poll(Duration::ZERO)? {
            if let Event::Key(key) = event::read()? {
                // Ctrl+Q or Ctrl+C → quit
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('c') => {
                            // Restore terminal (best-effort: don't propagate cleanup errors)
                            terminal::disable_raw_mode().ok();
                            execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)
                                .ok();
                            return Ok(());
                        }
                        KeyCode::Char('r') => {
                            // Ctrl+R → reset
                            apple.reset();
                            continue;
                        }
                        KeyCode::Char('t') => {
                            // Ctrl+T → turbo toggle
                            turbo = !turbo;
                            continue;
                        }
                        _ => {}
                    }
                }

                if let Some(ascii) = map_key(key) {
                    apple.key_press(ascii);
                }
            }
        }

        // Run CPU based on real elapsed wall-clock time.
        let now = Instant::now();
        let mut dt = now.saturating_duration_since(last_emu_tick);
        last_emu_tick = now;
        if dt > Duration::from_millis(100) {
            dt = Duration::from_millis(100);
        }

        cycle_accum += dt.as_nanos() * CPU_HZ as u128;
        let mut cycles_to_run = (cycle_accum / 1_000_000_000) as u64;
        cycle_accum %= 1_000_000_000;

        if turbo {
            cycles_to_run = cycles_to_run.saturating_mul(TURBO_MULTIPLIER);
        }

        if cycles_to_run != 0 {
            apple.run_cycles(cycles_to_run);

            #[cfg(feature = "audio")]
            if let Some((_, sink)) = &mut audio {
                let pcm = apple.take_audio_samples(AUDIO_SAMPLE_RATE);
                if !pcm.is_empty() {
                    sink.append(SamplesBuffer::new(1, AUDIO_SAMPLE_RATE, pcm));
                }
            }
        }

        // Update measured emulation speed.
        let perf_now = Instant::now();
        let perf_elapsed = perf_now.saturating_duration_since(perf_last_time);
        if perf_elapsed >= perf_sample_interval {
            let delta_cycles = apple.cpu.cycles.saturating_sub(perf_last_cycles);
            let secs = perf_elapsed.as_secs_f64();
            if secs > 0.0 {
                emu_mhz = delta_cycles as f64 / secs / 1_000_000.0;
            }
            perf_last_cycles = apple.cpu.cycles;
            perf_last_time = perf_now;
        }

        // Render and redraw only when due.
        let render_interval = if turbo {
            turbo_render_interval
        } else {
            frame_duration
        };
        if Instant::now().saturating_duration_since(last_render_time) >= render_interval {
            let flash_on = ((boot_time.elapsed().as_millis() / FLASH_HALF_PERIOD_MS) & 1) == 0;
            video::render(apple.ram(), &apple.display, flash_on, &mut bitmap);

            if !braille_initialized || bitmap != last_bitmap {
                braille_lines = bitmap_to_braille(&bitmap);
                last_bitmap.copy_from_slice(&bitmap);
                braille_initialized = true;
            }

            // Draw TUI each render tick so the status bar updates immediately.
            // Fixed display size: 140×48 content + 2 for border = 142×50, plus 1 status bar
            const DISPLAY_W: u16 = 142; // 140 braille cols + 2 border
            const DISPLAY_H: u16 = 50; // 48 braille rows + 2 border

            terminal.draw(|frame| {
                let area = frame.area();

                // Center the fixed-size display within the terminal
                let x = area.x + area.width.saturating_sub(DISPLAY_W) / 2;
                let y = area.y;
                let display_rect =
                    Rect::new(x, y, DISPLAY_W.min(area.width), DISPLAY_H.min(area.height));

                // Braille display in a bordered block
                let display_lines: Vec<Line> = braille_lines
                    .iter()
                    .map(|s| Line::from(Span::styled(s.as_str(), Style::default().fg(Color::Green))))
                    .collect();

                let display =
                    Paragraph::new(display_lines).block(Block::default().borders(Borders::ALL).title(" A2VM "));
                frame.render_widget(display, display_rect);

                // Status bar below the display
                let status_y = display_rect.y + display_rect.height;
                if status_y < area.y + area.height {
                    let cpu = &apple.cpu;
                    let mode = if apple.display.text {
                        "TEXT"
                    } else if apple.display.hires {
                        "HGR"
                    } else {
                        "GR"
                    };
                    let disk_status = if apple.disk.motor_on {
                        format!("D:T{}", apple.disk.half_track / 2)
                    } else {
                        "D:--".to_string()
                    };
                    let fast_label = if apple.is_fast_disk() { " FAST" } else { "" };
                    let target_mhz = if turbo {
                        (CPU_HZ * TURBO_MULTIPLIER) as f64 / 1_000_000.0
                    } else {
                        CPU_HZ as f64 / 1_000_000.0
                    };
                    let status = format!(
                        " PC:{:04X} A:{:02X} X:{:02X} Y:{:02X} SP:{:02X} P:{:02X} {} {}{} {} EMU:{:.2}/{:.2}MHz | Ctrl+Q:Quit Ctrl+R:Reset Ctrl+T:Turbo",
                        cpu.pc,
                        cpu.a,
                        cpu.x,
                        cpu.y,
                        cpu.sp,
                        cpu.p.0,
                        mode,
                        disk_status,
                        fast_label,
                        if turbo { "TURBOx4" } else { "TURBOoff" },
                        emu_mhz,
                        target_mhz
                    );
                    let status_rect = Rect::new(display_rect.x, status_y, display_rect.width, 1);
                    let status_bar = Paragraph::new(Line::from(Span::styled(
                        status,
                        Style::default().fg(Color::Cyan),
                    )));
                    frame.render_widget(status_bar, status_rect);
                }
            })?;

            last_render_time = Instant::now();
        }

        // Frame rate limiting
        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }
}
