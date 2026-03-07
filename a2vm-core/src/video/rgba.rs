use super::layout::{hgr_line_addr, CHAR_ROM, TEXT_LINE_ADDR};
use super::mode::{DisplayColorMode, DisplayMode};
use super::{RGBA_HEIGHT, RGBA_SIZE, RGBA_WIDTH};

/// Apple II standard Lo-Res 16-color palette (RGBA).
const LORES_PALETTE: [[u8; 4]; 16] = [
    [0x00, 0x00, 0x00, 0xFF], //  0: Black
    [0xDD, 0x00, 0x33, 0xFF], //  1: Magenta (Deep Red)
    [0x00, 0x00, 0x99, 0xFF], //  2: Dark Blue
    [0xDD, 0x22, 0xDD, 0xFF], //  3: Purple (Violet)
    [0x00, 0x77, 0x22, 0xFF], //  4: Dark Green
    [0x55, 0x55, 0x55, 0xFF], //  5: Grey 1
    [0x22, 0x22, 0xFF, 0xFF], //  6: Medium Blue
    [0x66, 0xAA, 0xFF, 0xFF], //  7: Light Blue
    [0x88, 0x55, 0x00, 0xFF], //  8: Brown
    [0xFF, 0x66, 0x00, 0xFF], //  9: Orange
    [0xAA, 0xAA, 0xAA, 0xFF], // 10: Grey 2
    [0xFF, 0x99, 0x88, 0xFF], // 11: Pink
    [0x11, 0xDD, 0x00, 0xFF], // 12: Green (Light Green)
    [0xFF, 0xFF, 0x00, 0xFF], // 13: Yellow
    [0x44, 0xFF, 0x99, 0xFF], // 14: Aquamarine
    [0xFF, 0xFF, 0xFF, 0xFF], // 15: White
];

/// Hi-Res NTSC artifact colors.
const HIRES_PURPLE: [u8; 4] = [0xDD, 0x22, 0xDD, 0xFF];
const HIRES_GREEN: [u8; 4] = [0x11, 0xDD, 0x00, 0xFF];
const HIRES_BLUE: [u8; 4] = [0x22, 0x22, 0xFF, 0xFF];
const HIRES_ORANGE: [u8; 4] = [0xFF, 0x66, 0x00, 0xFF];
const HIRES_WHITE: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];
const HIRES_BLACK: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

/// Text mode phosphor colors.
const TEXT_FG: [u8; 4] = [0x33, 0xFF, 0x33, 0xFF];
const TEXT_BG: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

/// Monochrome colors (green phosphor).
const MONO_FG: [u8; 4] = [0x33, 0xFF, 0x33, 0xFF];
const MONO_BG: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

/// Render the current Apple II display into a 280×192 RGBA buffer.
pub fn render_rgba(
    ram: &[u8],
    mode: &DisplayMode,
    flash_on: bool,
    color_mode: DisplayColorMode,
    frame_phase: u64,
    rgba: &mut [u8],
) {
    debug_assert!(rgba.len() >= RGBA_SIZE);

    let page_offset: usize = if mode.page2 { 0x0400 } else { 0 };

    if mode.text {
        let bg = if color_mode == DisplayColorMode::Color {
            TEXT_BG
        } else {
            MONO_BG
        };
        fill_rgba(rgba, bg);
        render_text_rows_rgba(ram, rgba, flash_on, color_mode, page_offset, 0, 24);
    } else if mode.hires {
        let bg = if color_mode == DisplayColorMode::Color {
            HIRES_BLACK
        } else {
            MONO_BG
        };
        fill_rgba(rgba, bg);
        let hires_base: usize = if mode.page2 { 0x4000 } else { 0x2000 };
        let scanlines = if mode.mixed { 160 } else { 192 };
        render_hires_scanlines_rgba(ram, rgba, hires_base, scanlines, color_mode);
        if mode.mixed {
            let bg = if color_mode == DisplayColorMode::Color {
                TEXT_BG
            } else {
                MONO_BG
            };
            fill_rgba_region(rgba, bg, 0, 160, RGBA_WIDTH, 32);
            render_text_rows_rgba(ram, rgba, flash_on, color_mode, page_offset, 20, 24);
        }
    } else {
        let bg = if color_mode == DisplayColorMode::Color {
            LORES_PALETTE[0]
        } else {
            MONO_BG
        };
        fill_rgba(rgba, bg);
        let text_rows = if mode.mixed { 20 } else { 24 };
        render_lores_rows_rgba(ram, rgba, page_offset, text_rows, color_mode);
        if mode.mixed {
            let bg = if color_mode == DisplayColorMode::Color {
                TEXT_BG
            } else {
                MONO_BG
            };
            fill_rgba_region(rgba, bg, 0, 160, RGBA_WIDTH, 32);
            render_text_rows_rgba(ram, rgba, flash_on, color_mode, page_offset, 20, 24);
        }
    }

    if color_mode == DisplayColorMode::MonochromeScanlines {
        apply_scanlines(rgba, frame_phase);
    }
}

/// Render text rows `start_row..end_row` into RGBA buffer.
fn render_text_rows_rgba(
    ram: &[u8],
    rgba: &mut [u8],
    flash_on: bool,
    color_mode: DisplayColorMode,
    page_offset: usize,
    start_row: usize,
    end_row: usize,
) {
    let fg = if color_mode == DisplayColorMode::Color {
        TEXT_FG
    } else {
        MONO_FG
    };
    let bg = if color_mode == DisplayColorMode::Color {
        TEXT_BG
    } else {
        MONO_BG
    };

    for (row, &line_addr) in TEXT_LINE_ADDR
        .iter()
        .enumerate()
        .take(end_row)
        .skip(start_row)
    {
        let base = line_addr as usize + page_offset;
        for col in 0..40usize {
            let screen_code = ram[base + col];
            let char_index = (screen_code & 0x3F) as usize;
            let is_inverse = screen_code < 0x40 || (screen_code < 0x80 && flash_on);

            for line in 0..8usize {
                let mut pixels = CHAR_ROM[char_index * 8 + line];
                if is_inverse {
                    pixels ^= 0x7F;
                }

                let pixel_y = row * 8 + line;
                let pixel_x = col * 7;
                for bit in 0..7usize {
                    let on = pixels & (1 << bit) != 0;
                    let color = if on { &fg } else { &bg };
                    let x = pixel_x + (6 - bit);
                    let idx = (pixel_y * RGBA_WIDTH + x) * 4;
                    rgba[idx..idx + 4].copy_from_slice(color);
                }
            }
        }
    }
}

/// Render lo-res graphics rows into RGBA buffer.
fn render_lores_rows_rgba(
    ram: &[u8],
    rgba: &mut [u8],
    page_offset: usize,
    num_text_rows: usize,
    color_mode: DisplayColorMode,
) {
    for (text_row, &line_addr) in TEXT_LINE_ADDR.iter().enumerate().take(num_text_rows) {
        let base = line_addr as usize + page_offset;
        for col in 0..40usize {
            let byte = ram[base + col];
            let top_color = (byte & 0x0F) as usize;
            let bot_color = (byte >> 4) as usize;

            let px = col * 7;
            let py = text_row * 8;

            if color_mode == DisplayColorMode::Color {
                fill_rgba_region(rgba, LORES_PALETTE[top_color], px, py, 7, 4);
                fill_rgba_region(rgba, LORES_PALETTE[bot_color], px, py + 4, 7, 4);
            } else {
                let top_on = top_color != 0;
                let bot_on = bot_color != 0;
                fill_rgba_region(rgba, if top_on { MONO_FG } else { MONO_BG }, px, py, 7, 4);
                fill_rgba_region(
                    rgba,
                    if bot_on { MONO_FG } else { MONO_BG },
                    px,
                    py + 4,
                    7,
                    4,
                );
            }
        }
    }
}

/// Render hi-res scanlines into RGBA buffer with NTSC artifact color.
///
/// Uses a sliding window to track neighbor pixels, avoiding division
/// by 7 to look up adjacent bytes.
fn render_hires_scanlines_rgba(
    ram: &[u8],
    rgba: &mut [u8],
    base: usize,
    num_lines: usize,
    color_mode: DisplayColorMode,
) {
    for y in 0..num_lines {
        let addr = hgr_line_addr(base, y);

        if color_mode != DisplayColorMode::Color {
            // Monochrome fast path: no neighbor lookups needed
            for col in 0..40usize {
                let byte = ram[addr + col];
                let pixel_x = col * 7;
                for bit in 0..7usize {
                    let color = if byte & (1 << bit) != 0 {
                        MONO_FG
                    } else {
                        MONO_BG
                    };
                    let idx = (y * RGBA_WIDTH + pixel_x + bit) * 4;
                    rgba[idx..idx + 4].copy_from_slice(&color);
                }
            }
            continue;
        }

        // Color path: use sliding window to avoid division by 7
        let mut prev_on = false;
        for col in 0..40usize {
            let byte = ram[addr + col];
            let high_bit = byte & 0x80 != 0;
            let pixel_x = col * 7;

            for bit in 0..7usize {
                let on = byte & (1 << bit) != 0;
                let x = pixel_x + bit;

                // Look ahead for next pixel
                let next_on = if bit < 6 {
                    byte & (1 << (bit + 1)) != 0
                } else if col < 39 {
                    ram[addr + col + 1] & 1 != 0
                } else {
                    false
                };

                let color = if !on {
                    HIRES_BLACK
                } else if prev_on || next_on {
                    HIRES_WHITE
                } else if high_bit {
                    if x % 2 == 0 {
                        HIRES_BLUE
                    } else {
                        HIRES_ORANGE
                    }
                } else if x % 2 == 0 {
                    HIRES_PURPLE
                } else {
                    HIRES_GREEN
                };

                let idx = (y * RGBA_WIDTH + x) * 4;
                rgba[idx..idx + 4].copy_from_slice(&color);
                prev_on = on;
            }
        }
    }
}

// Scanline CRT simulation constants
const SCANLINE_FLICKER_BASE: f32 = 0.985;
const SCANLINE_FLICKER_RANGE: f32 = 0.015;
const SCANLINE_FLICKER_SPEED: f32 = 0.11;
const SCANLINE_WOBBLE_AMP: f32 = 0.01;
const SCANLINE_WOBBLE_SPATIAL: f32 = 0.35;
const SCANLINE_WOBBLE_TEMPORAL: f32 = 0.07;
const SCANLINE_DARK_BASE: f32 = 0.2;
const SCANLINE_DARK_LUM: f32 = 0.75;
const SCANLINE_BRIGHT_BASE: f32 = 0.96;
const SCANLINE_BRIGHT_LUM: f32 = 0.03;
const SCANLINE_MIN_KEEP: f32 = 0.35;

fn apply_scanlines(rgba: &mut [u8], frame_phase: u64) {
    let global_flicker = SCANLINE_FLICKER_BASE
        + SCANLINE_FLICKER_RANGE * ((frame_phase as f32) * SCANLINE_FLICKER_SPEED).sin();
    let line_offset = ((frame_phase >> 4) & 1) as usize;

    for y in 0..RGBA_HEIGHT {
        let scanline = ((y + line_offset) & 1) == 1;
        let row_wobble = SCANLINE_WOBBLE_AMP
            * ((y as f32) * SCANLINE_WOBBLE_SPATIAL
                + (frame_phase as f32) * SCANLINE_WOBBLE_TEMPORAL)
                .sin();

        for x in 0..RGBA_WIDTH {
            let idx = (y * RGBA_WIDTH + x) * 4;
            if idx + 3 >= rgba.len() {
                continue;
            }

            let lum = (rgba[idx].max(rgba[idx + 1]).max(rgba[idx + 2]) as f32) * (1.0 / 255.0);
            let mut keep = if scanline {
                SCANLINE_DARK_BASE + SCANLINE_DARK_LUM * lum + row_wobble
            } else {
                SCANLINE_BRIGHT_BASE + SCANLINE_BRIGHT_LUM * lum
            };
            keep = (keep * global_flicker).clamp(SCANLINE_MIN_KEEP, 1.0);

            let gain = (keep * 256.0) as u16;
            rgba[idx] = ((rgba[idx] as u16 * gain) >> 8) as u8;
            rgba[idx + 1] = ((rgba[idx + 1] as u16 * gain) >> 8) as u8;
            rgba[idx + 2] = ((rgba[idx + 2] as u16 * gain) >> 8) as u8;
        }
    }
}

/// Fill entire RGBA buffer with a single color.
fn fill_rgba(rgba: &mut [u8], color: [u8; 4]) {
    debug_assert_eq!(rgba.len() % 4, 0);
    let word = u32::from_ne_bytes(color);
    // Safety: RGBA buffer length is always a multiple of 4
    let (prefix, aligned, suffix) = unsafe { rgba.align_to_mut::<u32>() };
    for b in prefix.chunks_exact_mut(4) {
        b.copy_from_slice(&color);
    }
    aligned.fill(word);
    for b in suffix.chunks_exact_mut(4) {
        b.copy_from_slice(&color);
    }
}

/// Fill a rectangular region in the RGBA buffer.
fn fill_rgba_region(rgba: &mut [u8], color: [u8; 4], x: usize, y: usize, w: usize, h: usize) {
    if w == 0 || h == 0 {
        return;
    }
    debug_assert!(x < RGBA_WIDTH && y < RGBA_HEIGHT);
    debug_assert!(x + w <= RGBA_WIDTH);
    debug_assert!(y + h <= RGBA_HEIGHT);

    for dy in 0..h {
        let row_start = ((y + dy) * RGBA_WIDTH + x) * 4;
        let row_end = row_start + w * 4;
        fill_rgba(&mut rgba[row_start..row_end], color);
    }
}
