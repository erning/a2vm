use super::layout::{hgr_line_addr, CHAR_ROM, TEXT_LINE_ADDR};
use super::{BITMAP_SIZE, BITMAP_STRIDE};
use crate::video::mode::DisplayMode;

/// Render the current Apple II display into a 280×192 monochrome bitmap.
///
/// Dispatches to text, lo-res, or hi-res renderer based on `mode`,
/// with optional 4-row text window at the bottom when mixed mode is on.
pub fn render(ram: &[u8], mode: &DisplayMode, flash_on: bool, bitmap: &mut [u8; BITMAP_SIZE]) {
    bitmap.fill(0);

    let page_offset: usize = if mode.page2 { 0x0400 } else { 0 };

    if mode.text {
        // Full-screen text
        render_text_rows(ram, bitmap, flash_on, page_offset, 0, 24);
    } else if mode.hires {
        // Hi-Res graphics
        let hires_base: usize = if mode.page2 { 0x4000 } else { 0x2000 };
        let scanlines = if mode.mixed { 160 } else { 192 };
        render_hires_scanlines(ram, bitmap, hires_base, scanlines);
        if mode.mixed {
            render_text_rows(ram, bitmap, flash_on, page_offset, 20, 24);
        }
    } else {
        // Lo-Res graphics
        let text_rows = if mode.mixed { 20 } else { 24 };
        render_lores_rows(ram, bitmap, page_offset, text_rows);
        if mode.mixed {
            render_text_rows(ram, bitmap, flash_on, page_offset, 20, 24);
        }
    }
}

/// Legacy convenience wrapper (used by tests).
pub fn render_text_page(ram: &[u8], bitmap: &mut [u8; BITMAP_SIZE], flash_on: bool) {
    bitmap.fill(0);
    render_text_rows(ram, bitmap, flash_on, 0, 0, 24);
}

/// Render text rows `start_row..end_row` into the bitmap.
fn render_text_rows(
    ram: &[u8],
    bitmap: &mut [u8; BITMAP_SIZE],
    flash_on: bool,
    page_offset: usize,
    start_row: usize,
    end_row: usize,
) {
    for (row, &line_addr) in TEXT_LINE_ADDR
        .iter()
        .enumerate()
        .take(end_row)
        .skip(start_row)
    {
        let base = line_addr as usize + page_offset;
        for col in 0..40usize {
            let screen_code = ram[base + col];

            // Apple II text mode uses 64 glyphs selected by low 6 bits.
            // CHAR_ROM is already in hardware order, so no remap is needed.
            let char_index = (screen_code & 0x3F) as usize;

            // $00-$3F: always inverse; $40-$7F: inverse when flash_on
            let is_inverse = screen_code < 0x40 || (screen_code < 0x80 && flash_on);

            for line in 0..8usize {
                let mut pixels = CHAR_ROM[char_index * 8 + line];
                if is_inverse {
                    pixels ^= 0x7F;
                }

                let pixel_y = row * 8 + line;
                let pixel_x = col * 7;
                // Character ROM uses bit 6 as leftmost and bit 0 as rightmost.
                for bit in 0..7usize {
                    if pixels & (1 << bit) != 0 {
                        set_pixel(bitmap, pixel_x + (6 - bit), pixel_y);
                    }
                }
            }
        }
    }
}

/// Render lo-res graphics for text rows 0..`num_text_rows`.
///
/// Each text-row byte encodes two vertically stacked 7×4 color blocks:
///   lower nibble = top block, upper nibble = bottom block.
/// Monochrome: color 0 (black) = off, any other color = on.
fn render_lores_rows(
    ram: &[u8],
    bitmap: &mut [u8; BITMAP_SIZE],
    page_offset: usize,
    num_text_rows: usize,
) {
    for (text_row, &line_addr) in TEXT_LINE_ADDR.iter().enumerate().take(num_text_rows) {
        let base = line_addr as usize + page_offset;
        for col in 0..40usize {
            let byte = ram[base + col];
            let top_color = byte & 0x0F;
            let bot_color = byte >> 4;

            let px = col * 7;
            let py = text_row * 8;

            if top_color != 0 {
                fill_rect(bitmap, px, py, 7, 4);
            }
            if bot_color != 0 {
                fill_rect(bitmap, px, py + 4, 7, 4);
            }
        }
    }
}

/// Render `num_lines` scanlines of hi-res graphics.
fn render_hires_scanlines(
    ram: &[u8],
    bitmap: &mut [u8; BITMAP_SIZE],
    base: usize,
    num_lines: usize,
) {
    for y in 0..num_lines {
        let addr = hgr_line_addr(base, y);
        for col in 0..40usize {
            let byte = ram[addr + col];
            // Bits 0-6 are pixels (bit 0 = leftmost), bit 7 = palette (ignored for mono)
            let pixel_x = col * 7;
            for bit in 0..7usize {
                if byte & (1 << bit) != 0 {
                    set_pixel(bitmap, pixel_x + bit, y);
                }
            }
        }
    }
}

/// Set a single pixel in the bitmap. Bitmap is MSB-first.
#[inline]
fn set_pixel(bitmap: &mut [u8; BITMAP_SIZE], x: usize, y: usize) {
    let byte_idx = y * BITMAP_STRIDE + x / 8;
    let bit_idx = 7 - (x % 8);
    bitmap[byte_idx] |= 1 << bit_idx;
}

/// Fill a solid rectangle in the bitmap.
fn fill_rect(bitmap: &mut [u8; BITMAP_SIZE], x: usize, y: usize, w: usize, h: usize) {
    for dy in 0..h {
        for dx in 0..w {
            set_pixel(bitmap, x + dx, y + dy);
        }
    }
}
