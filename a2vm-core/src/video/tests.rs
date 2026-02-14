use super::layout::{hgr_line_addr, TEXT_LINE_ADDR};
use super::*;

fn pixel_on(bitmap: &[u8; BITMAP_SIZE], x: usize, y: usize) -> bool {
    let byte_idx = y * BITMAP_STRIDE + x / 8;
    let bit_idx = 7 - (x % 8);
    (bitmap[byte_idx] & (1 << bit_idx)) != 0
}

fn rgba_pixel(rgba: &[u8], x: usize, y: usize) -> [u8; 4] {
    let idx = (y * RGBA_WIDTH + x) * 4;
    [rgba[idx], rgba[idx + 1], rgba[idx + 2], rgba[idx + 3]]
}

#[test]
fn test_bitmap_dimensions() {
    assert_eq!(BITMAP_STRIDE, 35);
    assert_eq!(BITMAP_SIZE, 6720);
}

#[test]
fn test_empty_screen() {
    let mut ram = vec![0u8; 0x0800];
    for cell in ram.iter_mut().take(0x0800).skip(0x0400) {
        *cell = 0xA0;
    }
    let mut bitmap = [0u8; BITMAP_SIZE];
    render_text_page(&ram, &mut bitmap, false);
    assert!(bitmap.iter().all(|&b| b == 0));
}

#[test]
fn test_inverse_space_fills() {
    let mut ram = vec![0u8; 0x0800];
    for cell in ram.iter_mut().take(0x0800).skip(0x0400) {
        *cell = 0x00;
    }
    let mut bitmap = [0u8; BITMAP_SIZE];
    render_text_page(&ram, &mut bitmap, false);
    assert!(bitmap.iter().any(|&b| b != 0));
}

#[test]
fn test_text_line_addresses() {
    for &addr in &TEXT_LINE_ADDR {
        assert!((0x0400..0x0800).contains(&addr));
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
    for cell in ram.iter_mut().take(0x0800).skip(0x0400) {
        *cell = 0xA0;
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

#[test]
fn test_render_rgba_text_blank_is_background() {
    let mut ram = vec![0u8; 0x0800];
    for cell in ram.iter_mut().take(0x0800).skip(0x0400) {
        *cell = 0xA0;
    }

    let mode = DisplayMode::default();
    let mut rgba = vec![0xFF; RGBA_SIZE];
    render_rgba(&ram, &mode, false, DisplayColorMode::Color, 0, &mut rgba);

    assert_eq!(rgba_pixel(&rgba, 0, 0), [0x00, 0x00, 0x00, 0xFF]);
    assert_eq!(
        rgba_pixel(&rgba, RGBA_WIDTH - 1, RGBA_HEIGHT - 1),
        [0x00, 0x00, 0x00, 0xFF]
    );
}

#[test]
fn test_render_rgba_lores_color_cell_uses_palette() {
    let mut ram = vec![0u8; 0x0800];
    // Top nibble color = 1, bottom nibble color = 2.
    ram[0x0400] = 0x21;

    let mode = DisplayMode {
        text: false,
        mixed: false,
        page2: false,
        hires: false,
    };
    let mut rgba = vec![0x00; RGBA_SIZE];
    render_rgba(&ram, &mode, false, DisplayColorMode::Color, 0, &mut rgba);

    assert_eq!(rgba_pixel(&rgba, 0, 0), [0xDD, 0x00, 0x33, 0xFF]);
    assert_eq!(rgba_pixel(&rgba, 0, 4), [0x00, 0x00, 0x99, 0xFF]);
}

#[test]
fn test_scanline_mode_modulates_rows() {
    let mut ram = vec![0u8; 0x0800];
    // Ensure we have lit pixels so scanline modulation can be observed.
    ram[0x0400] = 0x11;
    let mode = DisplayMode {
        text: false,
        mixed: false,
        page2: false,
        hires: false,
    };

    let mut rgba_plain = vec![0u8; RGBA_SIZE];
    render_rgba(
        &ram,
        &mode,
        false,
        DisplayColorMode::Monochrome,
        0,
        &mut rgba_plain,
    );

    let mut rgba_scan = vec![0u8; RGBA_SIZE];
    render_rgba(
        &ram,
        &mode,
        false,
        DisplayColorMode::MonochromeScanlines,
        1,
        &mut rgba_scan,
    );

    assert_ne!(rgba_pixel(&rgba_plain, 0, 0), rgba_pixel(&rgba_scan, 0, 0));
}
