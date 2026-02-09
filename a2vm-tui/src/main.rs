use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, cursor};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;

use a2vm_core::machine::AppleII;
use a2vm_core::video::{self, BITMAP_HEIGHT, BITMAP_SIZE, BITMAP_STRIDE, BITMAP_WIDTH};

/// Cycles per frame at ~1.023 MHz / 60 fps ≈ 17050
const CYCLES_PER_FRAME: u64 = 17050;

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
    let cols = BITMAP_WIDTH / 2;  // 140
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

            let ch = char::from_u32(0x2800 + bits as u32).unwrap();
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
            if ascii >= b'a' && ascii <= b'z' {
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
    // Parse command line: ROM path + optional disk image
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <rom-file> [disk.dsk]", args[0]);
        eprintln!("  rom-file: path to Apple II ROM (12K, 16K, or 20K)");
        eprintln!("  disk.dsk: optional DOS 3.3 disk image (143360 bytes)");
        std::process::exit(1);
    }
    let rom_path = Path::new(&args[1]);

    // Create Apple II and load ROM
    let mut apple = AppleII::new();
    apple.load_rom(rom_path)?;

    // Load disk image if provided
    if args.len() >= 3 {
        let disk_path = Path::new(&args[2]);
        apple.load_disk(disk_path)?;
    }

    apple.reset();

    // Set up terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut bitmap = [0u8; BITMAP_SIZE];
    let frame_duration = Duration::from_micros(16_667); // ~60 fps
    let mut frame_count: u32 = 0;

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
                            // Restore terminal
                            terminal::disable_raw_mode()?;
                            execute!(
                                terminal.backend_mut(),
                                LeaveAlternateScreen,
                                cursor::Show
                            )?;
                            return Ok(());
                        }
                        KeyCode::Char('r') => {
                            // Ctrl+R → reset
                            apple.reset();
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

        // Run CPU for one frame
        apple.run_cycles(CYCLES_PER_FRAME);

        // Render display to bitmap (flash toggles every 16 frames ≈ 1.9 Hz)
        let flash_on = (frame_count / 16) % 2 == 0;
        video::render(apple.ram(), &apple.display, flash_on, &mut bitmap);
        frame_count = frame_count.wrapping_add(1);

        // Convert bitmap to braille
        let braille_lines = bitmap_to_braille(&bitmap);

        // Draw TUI
        // Fixed display size: 140×48 content + 2 for border = 142×50, plus 1 status bar
        const DISPLAY_W: u16 = 142; // 140 braille cols + 2 border
        const DISPLAY_H: u16 = 50;  // 48 braille rows + 2 border

        terminal.draw(|frame| {
            let area = frame.area();

            // Center the fixed-size display within the terminal
            let x = area.x + area.width.saturating_sub(DISPLAY_W) / 2;
            let y = area.y;
            let display_rect = Rect::new(x, y, DISPLAY_W.min(area.width), DISPLAY_H.min(area.height));

            // Braille display in a bordered block
            let display_lines: Vec<Line> = braille_lines
                .iter()
                .map(|s| Line::from(Span::styled(s.as_str(), Style::default().fg(Color::Green))))
                .collect();

            let display = Paragraph::new(display_lines)
                .block(Block::default().borders(Borders::ALL).title(" A2VM "));
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
                let status = format!(
                    " PC:{:04X} A:{:02X} X:{:02X} Y:{:02X} SP:{:02X} P:{:02X} {} {} | Ctrl+Q:Quit Ctrl+R:Reset",
                    cpu.pc, cpu.a, cpu.x, cpu.y, cpu.sp, cpu.p.0, mode, disk_status
                );
                let status_rect = Rect::new(display_rect.x, status_y, display_rect.width, 1);
                let status_bar = Paragraph::new(Line::from(Span::styled(
                    status,
                    Style::default().fg(Color::Cyan),
                )));
                frame.render_widget(status_bar, status_rect);
            }
        })?;

        // Frame rate limiting
        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }
}
