use super::layout::CHAR_ROM;

/// Status bar colors.
const STATUS_FG: [u8; 4] = [0x00, 0xFF, 0xFF, 0xFF]; // cyan
const STATUS_BG: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

/// Render a status bar line into an RGBA buffer using CHAR_ROM glyphs.
///
/// `rgba` - target buffer (must be at least `stride * (y_offset + 8) * 4` bytes).
/// `stride` - pixel width of the buffer (e.g. 280).
/// `y_offset` - pixel row where the status bar starts.
pub fn render_status_bar(text: &str, rgba: &mut [u8], stride: usize, y_offset: usize) {
    for (col, ch) in text.chars().take(40).enumerate() {
        let ascii = ch as u8;
        // CHAR_ROM layout: @ABC..Z[\]^_ !"#..0-9:;<=>?
        // ASCII 0x20-0x3F (space..?) -> ROM indices 32..63
        // ASCII 0x40-0x5F (@.._ )  -> ROM indices 0..31
        let char_index = if (0x20..0x40).contains(&ascii) {
            (ascii - 0x20 + 32) as usize
        } else if (0x40..0x60).contains(&ascii) {
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
