//! Apple II video rendering → 280×192 monochrome bitmap.
//!
//! The bitmap is 280 pixels wide × 192 pixels tall, stored as 6720 bytes
//! (each row = 35 bytes = 280 bits, MSB first within each byte).
//!
//! Unified pipeline: all display modes (TEXT, GR, HGR) render to the same
//! 280×192 bitmap, which is then encoded to Braille characters by the TUI.

/// Display mode state, controlled by soft switches $C050-$C057.
#[derive(Clone, Debug)]
pub struct DisplayMode {
    pub text: bool,  // $C051/$C050: TEXT on/off
    pub mixed: bool, // $C053/$C052: mixed mode (4 text rows at bottom)
    pub page2: bool, // $C055/$C054: display page 2
    pub hires: bool, // $C057/$C056: hi-res mode
}

/// Color mode for GUI display rendering.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum DisplayColorMode {
    /// Full color (Lo-Res 16-color, Hi-Res NTSC artifact colors).
    #[default]
    Color,
    /// Monochrome (green phosphor).
    Monochrome,
    /// Monochrome with simulated CRT scanlines.
    MonochromeScanlines,
}

impl Default for DisplayMode {
    fn default() -> Self {
        Self {
            text: true,
            mixed: false,
            page2: false,
            hires: false,
        }
    }
}

/// Apple II character generator rows in hardware order (64 chars × 8 rows).
/// Order is: @ABCDEFGHIJKLMNOPQRSTUVWXYZ[\]^_ !"#$%&'()*+,-./0123456789:;<=>?
/// Data sourced from Apple II video character ROM dumps.
const CHAR_ROM: [u8; 512] = [
    0x00, 0x1C, 0x22, 0x2A, 0x2E, 0x2C, 0x20, 0x1E, 0x00, 0x08, 0x14, 0x22, 0x22, 0x3E, 0x22, 0x22,
    0x00, 0x3C, 0x22, 0x22, 0x3C, 0x22, 0x22, 0x3C, 0x00, 0x1C, 0x22, 0x20, 0x20, 0x20, 0x22, 0x1C,
    0x00, 0x3C, 0x22, 0x22, 0x22, 0x22, 0x22, 0x3C, 0x00, 0x3E, 0x20, 0x20, 0x3C, 0x20, 0x20, 0x3E,
    0x00, 0x3E, 0x20, 0x20, 0x3C, 0x20, 0x20, 0x20, 0x00, 0x1E, 0x20, 0x20, 0x20, 0x26, 0x22, 0x1E,
    0x00, 0x22, 0x22, 0x22, 0x3E, 0x22, 0x22, 0x22, 0x00, 0x1C, 0x08, 0x08, 0x08, 0x08, 0x08, 0x1C,
    0x00, 0x02, 0x02, 0x02, 0x02, 0x02, 0x22, 0x1C, 0x00, 0x22, 0x24, 0x28, 0x30, 0x28, 0x24, 0x22,
    0x00, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x3E, 0x00, 0x22, 0x36, 0x2A, 0x2A, 0x22, 0x22, 0x22,
    0x00, 0x22, 0x22, 0x32, 0x2A, 0x26, 0x22, 0x22, 0x00, 0x1C, 0x22, 0x22, 0x22, 0x22, 0x22, 0x1C,
    0x00, 0x3C, 0x22, 0x22, 0x3C, 0x20, 0x20, 0x20, 0x00, 0x1C, 0x22, 0x22, 0x22, 0x2A, 0x24, 0x1A,
    0x00, 0x3C, 0x22, 0x22, 0x3C, 0x28, 0x24, 0x22, 0x00, 0x1C, 0x22, 0x20, 0x1C, 0x02, 0x22, 0x1C,
    0x00, 0x3E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x00, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x1C,
    0x00, 0x22, 0x22, 0x22, 0x22, 0x22, 0x14, 0x08, 0x00, 0x22, 0x22, 0x22, 0x2A, 0x2A, 0x36, 0x22,
    0x00, 0x22, 0x22, 0x14, 0x08, 0x14, 0x22, 0x22, 0x00, 0x22, 0x22, 0x14, 0x08, 0x08, 0x08, 0x08,
    0x00, 0x3E, 0x02, 0x04, 0x08, 0x10, 0x20, 0x3E, 0x00, 0x3E, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3E,
    0x00, 0x00, 0x20, 0x10, 0x08, 0x04, 0x02, 0x00, 0x00, 0x3E, 0x06, 0x06, 0x06, 0x06, 0x06, 0x3E,
    0x00, 0x00, 0x00, 0x08, 0x14, 0x22, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3E,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x08, 0x08, 0x08, 0x08, 0x00, 0x08,
    0x00, 0x14, 0x14, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x14, 0x14, 0x3E, 0x14, 0x3E, 0x14, 0x14,
    0x00, 0x08, 0x1E, 0x28, 0x1C, 0x0A, 0x3C, 0x08, 0x00, 0x30, 0x32, 0x04, 0x08, 0x10, 0x26, 0x06,
    0x00, 0x10, 0x28, 0x28, 0x10, 0x2A, 0x24, 0x1A, 0x00, 0x08, 0x08, 0x08, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x08, 0x10, 0x20, 0x20, 0x20, 0x10, 0x08, 0x00, 0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08,
    0x00, 0x08, 0x2A, 0x1C, 0x08, 0x1C, 0x2A, 0x08, 0x00, 0x00, 0x08, 0x08, 0x3E, 0x08, 0x08, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x08, 0x10, 0x00, 0x00, 0x00, 0x00, 0x3E, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x02, 0x04, 0x08, 0x10, 0x20, 0x00,
    0x00, 0x1C, 0x22, 0x26, 0x2A, 0x32, 0x22, 0x1C, 0x00, 0x08, 0x18, 0x08, 0x08, 0x08, 0x08, 0x1C,
    0x00, 0x1C, 0x22, 0x02, 0x0C, 0x10, 0x20, 0x3E, 0x00, 0x3E, 0x02, 0x04, 0x0C, 0x02, 0x22, 0x1C,
    0x00, 0x04, 0x0C, 0x14, 0x24, 0x3E, 0x04, 0x04, 0x00, 0x3E, 0x20, 0x3C, 0x02, 0x02, 0x22, 0x1C,
    0x00, 0x0E, 0x10, 0x20, 0x3C, 0x22, 0x22, 0x1C, 0x00, 0x3E, 0x02, 0x04, 0x08, 0x10, 0x10, 0x10,
    0x00, 0x1C, 0x22, 0x22, 0x1C, 0x22, 0x22, 0x1C, 0x00, 0x1C, 0x22, 0x22, 0x1E, 0x02, 0x04, 0x38,
    0x00, 0x00, 0x00, 0x08, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x08, 0x08, 0x10,
    0x00, 0x04, 0x08, 0x10, 0x20, 0x10, 0x08, 0x04, 0x00, 0x00, 0x00, 0x3E, 0x00, 0x3E, 0x00, 0x00,
    0x00, 0x10, 0x08, 0x04, 0x02, 0x04, 0x08, 0x10, 0x00, 0x1C, 0x22, 0x04, 0x08, 0x08, 0x00, 0x08,
];

/// Text page base addresses for each of 24 screen rows (page 1).
/// Page 2 addresses are $0400 higher.
const TEXT_LINE_ADDR: [u16; 24] = [
    0x0400, 0x0480, 0x0500, 0x0580, 0x0600, 0x0680, 0x0700, 0x0780, 0x0428, 0x04A8, 0x0528, 0x05A8,
    0x0628, 0x06A8, 0x0728, 0x07A8, 0x0450, 0x04D0, 0x0550, 0x05D0, 0x0650, 0x06D0, 0x0750, 0x07D0,
];

/// Bitmap dimensions: 280 pixels wide × 192 pixels tall.
pub const BITMAP_WIDTH: usize = 280;
pub const BITMAP_HEIGHT: usize = 192;
pub const BITMAP_STRIDE: usize = BITMAP_WIDTH / 8; // 35 bytes per row
pub const BITMAP_SIZE: usize = BITMAP_STRIDE * BITMAP_HEIGHT; // 6720

// ── Unified entry point ─────────────────────────────────────────────

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

// ── Text renderer ───────────────────────────────────────────────────

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

/// Legacy convenience wrapper (used by tests).
pub fn render_text_page(ram: &[u8], bitmap: &mut [u8; BITMAP_SIZE], flash_on: bool) {
    bitmap.fill(0);
    render_text_rows(ram, bitmap, flash_on, 0, 0, 24);
}

// ── Lo-Res (GR) renderer ───────────────────────────────────────────

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

// ── Hi-Res (HGR) renderer ──────────────────────────────────────────

/// Compute the RAM address for HGR scanline `y` (0-191).
///
/// HGR memory is interleaved:
///   addr = base + (y%8)*$400 + ((y/8)%8)*$80 + (y/64)*40
#[inline]
fn hgr_line_addr(base: usize, y: usize) -> usize {
    base + ((y & 7) << 10) + (((y >> 3) & 7) << 7) + (y >> 6) * 40
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

// ── Bitmap helpers ──────────────────────────────────────────────────

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

// ── RGBA constants ──────────────────────────────────────────────────

/// RGBA frame dimensions (same as monochrome bitmap).
pub const RGBA_WIDTH: usize = 280;
pub const RGBA_HEIGHT: usize = 192;
pub const RGBA_SIZE: usize = RGBA_WIDTH * RGBA_HEIGHT * 4;

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

/// Status bar colors.
const STATUS_FG: [u8; 4] = [0x00, 0xFF, 0xFF, 0xFF]; // cyan
const STATUS_BG: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

// ── RGBA rendering ──────────────────────────────────────────────────

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
fn render_hires_scanlines_rgba(
    ram: &[u8],
    rgba: &mut [u8],
    base: usize,
    num_lines: usize,
    color_mode: DisplayColorMode,
) {
    for y in 0..num_lines {
        let addr = hgr_line_addr(base, y);
        for col in 0..40usize {
            let byte = ram[addr + col];
            let high_bit = byte & 0x80 != 0;
            let pixel_x = col * 7;

            for bit in 0..7usize {
                let on = byte & (1 << bit) != 0;
                let x = pixel_x + bit;
                let screen_col = x;

                let color: [u8; 4] = if color_mode == DisplayColorMode::Color {
                    if !on {
                        HIRES_BLACK
                    } else {
                        let prev_on = if x > 0 {
                            let prev_col = (x - 1) / 7;
                            let prev_bit = (x - 1) % 7;
                            ram[hgr_line_addr(base, y) + prev_col] & (1 << prev_bit) != 0
                        } else {
                            false
                        };
                        let next_on = if x < 279 {
                            let next_col = (x + 1) / 7;
                            let next_bit = (x + 1) % 7;
                            ram[hgr_line_addr(base, y) + next_col] & (1 << next_bit) != 0
                        } else {
                            false
                        };

                        if prev_on || next_on {
                            HIRES_WHITE
                        } else if high_bit {
                            if screen_col % 2 == 0 {
                                HIRES_BLUE
                            } else {
                                HIRES_ORANGE
                            }
                        } else if screen_col % 2 == 0 {
                            HIRES_PURPLE
                        } else {
                            HIRES_GREEN
                        }
                    }
                } else if on {
                    MONO_FG
                } else {
                    MONO_BG
                };

                let idx = (y * RGBA_WIDTH + x) * 4;
                rgba[idx..idx + 4].copy_from_slice(&color);
            }
        }
    }
}

fn apply_scanlines(rgba: &mut [u8], frame_phase: u64) {
    let global_flicker = 0.985 + 0.015 * ((frame_phase as f32) * 0.11).sin();
    let line_offset = ((frame_phase >> 4) & 1) as usize;

    for y in 0..RGBA_HEIGHT {
        let scanline = ((y + line_offset) & 1) == 1;
        let row_wobble = 0.01 * ((y as f32) * 0.35 + (frame_phase as f32) * 0.07).sin();

        for x in 0..RGBA_WIDTH {
            let idx = (y * RGBA_WIDTH + x) * 4;
            if idx + 3 >= rgba.len() {
                continue;
            }

            let lum = (rgba[idx].max(rgba[idx + 1]).max(rgba[idx + 2]) as f32) * (1.0 / 255.0);
            let mut keep = if scanline {
                0.2 + 0.75 * lum + row_wobble
            } else {
                0.96 + 0.03 * lum
            };
            keep = (keep * global_flicker).clamp(0.35, 1.0);

            let gain = (keep * 256.0) as u16;
            rgba[idx] = ((rgba[idx] as u16 * gain) >> 8) as u8;
            rgba[idx + 1] = ((rgba[idx + 1] as u16 * gain) >> 8) as u8;
            rgba[idx + 2] = ((rgba[idx + 2] as u16 * gain) >> 8) as u8;
        }
    }
}

/// Render a status bar line into an RGBA buffer using CHAR_ROM glyphs.
///
/// `rgba` — target buffer (must be at least `stride * (y_offset + 8) * 4` bytes).
/// `stride` — pixel width of the buffer (e.g. 280).
/// `y_offset` — pixel row where the status bar starts.
pub fn render_status_bar(text: &str, rgba: &mut [u8], stride: usize, y_offset: usize) {
    for (col, ch) in text.chars().take(40).enumerate() {
        let ascii = ch as u8;
        // CHAR_ROM layout: @ABC..Z[\]^_ !"#..0-9:;<=>?
        // ASCII 0x20-0x3F (space..?) → ROM indices 32..63
        // ASCII 0x40-0x5F (@.._ )  → ROM indices 0..31
        let char_index = if ascii >= 0x20 && ascii < 0x40 {
            (ascii - 0x20 + 32) as usize
        } else if ascii >= 0x40 && ascii < 0x60 {
            (ascii - 0x40) as usize
        } else {
            0
        };

        for line in 0..8usize {
            let pixels = CHAR_ROM[char_index * 8 + line];
            let py = y_offset + line;
            let px = col * 7;
            for bit in 0..7usize {
                let on = pixels & (1 << bit) != 0;
                let color = if on { &STATUS_FG } else { &STATUS_BG };
                let x = px + (6 - bit);
                let idx = (py * stride + x) * 4;
                if idx + 4 <= rgba.len() {
                    rgba[idx..idx + 4].copy_from_slice(color);
                }
            }
        }
    }
}

// ── RGBA helpers ────────────────────────────────────────────────────

/// Fill entire RGBA buffer with a single color.
fn fill_rgba(rgba: &mut [u8], color: [u8; 4]) {
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.copy_from_slice(&color);
    }
}

/// Fill a rectangular region in the RGBA buffer.
fn fill_rgba_region(rgba: &mut [u8], color: [u8; 4], x: usize, y: usize, w: usize, h: usize) {
    for dy in 0..h {
        let row_start = ((y + dy) * RGBA_WIDTH + x) * 4;
        for dx in 0..w {
            let idx = row_start + dx * 4;
            if idx + 4 <= rgba.len() {
                rgba[idx..idx + 4].copy_from_slice(&color);
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel_on(bitmap: &[u8; BITMAP_SIZE], x: usize, y: usize) -> bool {
        let byte_idx = y * BITMAP_STRIDE + x / 8;
        let bit_idx = 7 - (x % 8);
        (bitmap[byte_idx] & (1 << bit_idx)) != 0
    }

    #[test]
    fn test_bitmap_dimensions() {
        assert_eq!(BITMAP_STRIDE, 35);
        assert_eq!(BITMAP_SIZE, 6720);
    }

    #[test]
    fn test_empty_screen() {
        let mut ram = vec![0u8; 0x0800];
        for addr in 0x0400..0x0800 {
            ram[addr] = 0xA0; // normal space
        }
        let mut bitmap = [0u8; BITMAP_SIZE];
        render_text_page(&ram, &mut bitmap, false);
        assert!(bitmap.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_inverse_space_fills() {
        let mut ram = vec![0u8; 0x0800];
        for addr in 0x0400..0x0800 {
            ram[addr] = 0x00; // inverse space → solid block
        }
        let mut bitmap = [0u8; BITMAP_SIZE];
        render_text_page(&ram, &mut bitmap, false);
        assert!(bitmap.iter().any(|&b| b != 0));
    }

    #[test]
    fn test_text_line_addresses() {
        for &addr in &TEXT_LINE_ADDR {
            assert!(addr >= 0x0400 && addr < 0x0800);
        }
    }

    #[test]
    fn test_hgr_line_addr() {
        assert_eq!(hgr_line_addr(0x2000, 0), 0x2000);
        assert_eq!(hgr_line_addr(0x2000, 1), 0x2400);
        assert_eq!(hgr_line_addr(0x2000, 8), 0x2080);
        assert_eq!(hgr_line_addr(0x2000, 64), 0x2028);
        // Last line
        let last = hgr_line_addr(0x2000, 191);
        assert!(last + 39 <= 0x3FFF); // fits in 8K page
    }

    #[test]
    fn test_lores_black_is_blank() {
        let ram = vec![0u8; 0x0800]; // all color 0
        let mut bitmap = [0u8; BITMAP_SIZE];
        let mode = DisplayMode {
            text: false,
            mixed: false,
            page2: false,
            hires: false,
        };
        render(&ram, &mode, false, &mut bitmap);
        assert!(bitmap.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_lores_nonzero_color_lights() {
        let mut ram = vec![0u8; 0x0800];
        // Set first cell to color 15 (both nibbles)
        ram[0x0400] = 0xFF;
        let mut bitmap = [0u8; BITMAP_SIZE];
        let mode = DisplayMode {
            text: false,
            mixed: false,
            page2: false,
            hires: false,
        };
        render(&ram, &mode, false, &mut bitmap);
        assert!(bitmap.iter().any(|&b| b != 0));
    }

    #[test]
    fn test_text_slash_orientation_not_mirrored() {
        let mut ram = vec![0u8; 0x0800];
        for addr in 0x0400..0x0800 {
            ram[addr] = 0xA0; // normal space
        }

        // '/' glyph index = 0x2F in Apple II 64-char hardware order.
        ram[0x0400] = 0x80 | 0x2F;

        let mut bitmap = [0u8; BITMAP_SIZE];
        render_text_page(&ram, &mut bitmap, false);

        // '/' should descend from right to left in the 7x8 cell.
        assert!(pixel_on(&bitmap, 5, 2));
        assert!(pixel_on(&bitmap, 1, 6));
        assert!(!pixel_on(&bitmap, 1, 2));
        assert!(!pixel_on(&bitmap, 5, 6));
    }
}
