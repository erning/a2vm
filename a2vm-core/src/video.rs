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
    pub text: bool,   // $C051/$C050: TEXT on/off
    pub mixed: bool,  // $C053/$C052: mixed mode (4 text rows at bottom)
    pub page2: bool,  // $C055/$C054: display page 2
    pub hires: bool,  // $C057/$C056: hi-res mode
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

/// Apple II character generator ROM: 64 characters × 8 rows.
/// Characters cover ASCII $20–$5F (space through underscore).
/// Each byte holds 7 pixel bits; bit 0 = leftmost pixel, bit 6 = rightmost.
const CHAR_ROM: [u8; 512] = [
    // $20 SPACE
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // $21 !
    0x08, 0x08, 0x08, 0x08, 0x08, 0x00, 0x08, 0x00,
    // $22 "
    0x14, 0x14, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00,
    // $23 #
    0x14, 0x14, 0x3E, 0x14, 0x3E, 0x14, 0x14, 0x00,
    // $24 $
    0x08, 0x3C, 0x0A, 0x1C, 0x28, 0x1E, 0x08, 0x00,
    // $25 %
    0x06, 0x26, 0x10, 0x08, 0x04, 0x32, 0x30, 0x00,
    // $26 &
    0x04, 0x0A, 0x0A, 0x04, 0x2A, 0x12, 0x2C, 0x00,
    // $27 '
    0x08, 0x08, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00,
    // $28 (
    0x10, 0x08, 0x04, 0x04, 0x04, 0x08, 0x10, 0x00,
    // $29 )
    0x04, 0x08, 0x10, 0x10, 0x10, 0x08, 0x04, 0x00,
    // $2A *
    0x08, 0x2A, 0x1C, 0x08, 0x1C, 0x2A, 0x08, 0x00,
    // $2B +
    0x00, 0x08, 0x08, 0x3E, 0x08, 0x08, 0x00, 0x00,
    // $2C ,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x08, 0x04,
    // $2D -
    0x00, 0x00, 0x00, 0x3E, 0x00, 0x00, 0x00, 0x00,
    // $2E .
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00,
    // $2F /
    0x00, 0x20, 0x10, 0x08, 0x04, 0x02, 0x00, 0x00,
    // $30 0
    0x1C, 0x22, 0x32, 0x2A, 0x26, 0x22, 0x1C, 0x00,
    // $31 1
    0x08, 0x0C, 0x08, 0x08, 0x08, 0x08, 0x1C, 0x00,
    // $32 2
    0x1C, 0x22, 0x20, 0x18, 0x04, 0x02, 0x3E, 0x00,
    // $33 3
    0x3E, 0x20, 0x10, 0x18, 0x20, 0x22, 0x1C, 0x00,
    // $34 4
    0x10, 0x18, 0x14, 0x12, 0x3E, 0x10, 0x10, 0x00,
    // $35 5
    0x3E, 0x02, 0x1E, 0x20, 0x20, 0x22, 0x1C, 0x00,
    // $36 6
    0x38, 0x04, 0x02, 0x1E, 0x22, 0x22, 0x1C, 0x00,
    // $37 7
    0x3E, 0x20, 0x10, 0x08, 0x04, 0x04, 0x04, 0x00,
    // $38 8
    0x1C, 0x22, 0x22, 0x1C, 0x22, 0x22, 0x1C, 0x00,
    // $39 9
    0x1C, 0x22, 0x22, 0x3C, 0x20, 0x10, 0x0E, 0x00,
    // $3A :
    0x00, 0x00, 0x08, 0x00, 0x08, 0x00, 0x00, 0x00,
    // $3B ;
    0x00, 0x00, 0x08, 0x00, 0x08, 0x08, 0x04, 0x00,
    // $3C <
    0x10, 0x08, 0x04, 0x02, 0x04, 0x08, 0x10, 0x00,
    // $3D =
    0x00, 0x00, 0x3E, 0x00, 0x3E, 0x00, 0x00, 0x00,
    // $3E >
    0x04, 0x08, 0x10, 0x20, 0x10, 0x08, 0x04, 0x00,
    // $3F ?
    0x1C, 0x22, 0x10, 0x08, 0x08, 0x00, 0x08, 0x00,
    // $40 @
    0x1C, 0x22, 0x2A, 0x3A, 0x1A, 0x02, 0x3C, 0x00,
    // $41 A
    0x08, 0x14, 0x22, 0x22, 0x3E, 0x22, 0x22, 0x00,
    // $42 B
    0x1E, 0x22, 0x22, 0x1E, 0x22, 0x22, 0x1E, 0x00,
    // $43 C
    0x1C, 0x22, 0x02, 0x02, 0x02, 0x22, 0x1C, 0x00,
    // $44 D
    0x0E, 0x12, 0x22, 0x22, 0x22, 0x12, 0x0E, 0x00,
    // $45 E
    0x3E, 0x02, 0x02, 0x1E, 0x02, 0x02, 0x3E, 0x00,
    // $46 F
    0x3E, 0x02, 0x02, 0x1E, 0x02, 0x02, 0x02, 0x00,
    // $47 G
    0x1C, 0x22, 0x02, 0x02, 0x32, 0x22, 0x3C, 0x00,
    // $48 H
    0x22, 0x22, 0x22, 0x3E, 0x22, 0x22, 0x22, 0x00,
    // $49 I
    0x1C, 0x08, 0x08, 0x08, 0x08, 0x08, 0x1C, 0x00,
    // $4A J
    0x20, 0x20, 0x20, 0x20, 0x20, 0x22, 0x1C, 0x00,
    // $4B K
    0x22, 0x12, 0x0A, 0x06, 0x0A, 0x12, 0x22, 0x00,
    // $4C L
    0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x3E, 0x00,
    // $4D M
    0x22, 0x36, 0x2A, 0x2A, 0x22, 0x22, 0x22, 0x00,
    // $4E N
    0x22, 0x22, 0x26, 0x2A, 0x32, 0x22, 0x22, 0x00,
    // $4F O
    0x1C, 0x22, 0x22, 0x22, 0x22, 0x22, 0x1C, 0x00,
    // $50 P
    0x1E, 0x22, 0x22, 0x1E, 0x02, 0x02, 0x02, 0x00,
    // $51 Q
    0x1C, 0x22, 0x22, 0x22, 0x2A, 0x12, 0x2C, 0x00,
    // $52 R
    0x1E, 0x22, 0x22, 0x1E, 0x0A, 0x12, 0x22, 0x00,
    // $53 S
    0x1C, 0x22, 0x02, 0x1C, 0x20, 0x22, 0x1C, 0x00,
    // $54 T
    0x3E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x00,
    // $55 U
    0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x1C, 0x00,
    // $56 V
    0x22, 0x22, 0x22, 0x22, 0x22, 0x14, 0x08, 0x00,
    // $57 W
    0x22, 0x22, 0x22, 0x2A, 0x2A, 0x36, 0x22, 0x00,
    // $58 X
    0x22, 0x22, 0x14, 0x08, 0x14, 0x22, 0x22, 0x00,
    // $59 Y
    0x22, 0x22, 0x14, 0x08, 0x08, 0x08, 0x08, 0x00,
    // $5A Z
    0x3E, 0x20, 0x10, 0x08, 0x04, 0x02, 0x3E, 0x00,
    // $5B [
    0x1C, 0x04, 0x04, 0x04, 0x04, 0x04, 0x1C, 0x00,
    // $5C backslash
    0x00, 0x02, 0x04, 0x08, 0x10, 0x20, 0x00, 0x00,
    // $5D ]
    0x1C, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1C, 0x00,
    // $5E ^
    0x08, 0x14, 0x22, 0x00, 0x00, 0x00, 0x00, 0x00,
    // $5F _
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3E, 0x00,
];

/// Text page base addresses for each of 24 screen rows (page 1).
/// Page 2 addresses are $0400 higher.
const TEXT_LINE_ADDR: [u16; 24] = [
    0x0400, 0x0480, 0x0500, 0x0580, 0x0600, 0x0680, 0x0700, 0x0780,
    0x0428, 0x04A8, 0x0528, 0x05A8, 0x0628, 0x06A8, 0x0728, 0x07A8,
    0x0450, 0x04D0, 0x0550, 0x05D0, 0x0650, 0x06D0, 0x0750, 0x07D0,
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

            // Apple II char ROM order: 0-31 = @A-Z[\]^_ ($40-$5F),
            // 32-63 = space..? ($20-$3F).
            // Our ROM is in ASCII order ($20-$5F), so XOR with 0x20 to remap.
            let char_index = ((screen_code & 0x3F) ^ 0x20) as usize;

            // $00-$3F: always inverse; $40-$7F: inverse when flash_on
            let is_inverse = screen_code < 0x40 || (screen_code < 0x80 && flash_on);

            for line in 0..8usize {
                let mut pixels = CHAR_ROM[char_index * 8 + line];
                if is_inverse {
                    pixels ^= 0x7F;
                }

                let pixel_y = row * 8 + line;
                let pixel_x = col * 7;
                // bit 0 = leftmost pixel
                for bit in 0..7usize {
                    if pixels & (1 << bit) != 0 {
                        set_pixel(bitmap, pixel_x + bit, pixel_y);
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
}
