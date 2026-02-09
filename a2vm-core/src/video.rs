/// Apple II video rendering → 280×192 monochrome bitmap.
///
/// The bitmap is 280 pixels wide × 192 pixels tall, stored as 6720 bytes
/// (each row = 35 bytes = 280 bits, MSB first within each byte).
///
/// Unified pipeline: all display modes (TEXT, GR, HGR) render to the same
/// 280×192 bitmap, which is then encoded to Braille characters by the TUI.

/// Display mode state, controlled by soft switches $C050-$C057.
#[derive(Clone, Debug)]
pub struct DisplayMode {
    pub text: bool,  // $C051/$C050: TEXT on/off
    pub mixed: bool, // $C053/$C052: mixed mode (4 text rows at bottom)
    pub page2: bool, // $C055/$C054: display page 2
    pub hires: bool, // $C057/$C056: hi-res mode
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
    for row in start_row..end_row {
        let base = TEXT_LINE_ADDR[row] as usize + page_offset;
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
    for text_row in 0..num_text_rows {
        let base = TEXT_LINE_ADDR[text_row] as usize + page_offset;
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
