//! Apple II video rendering.
//!
//! Unified pipeline: all display modes (TEXT, GR, HGR) render to a shared
//! 280×192 pixel grid, either as monochrome bitmap bits (TUI path) or
//! RGBA pixels (GUI path).

mod layout;
mod mode;
mod mono;
mod overlay;
mod rgba;

pub use mode::{DisplayColorMode, DisplayMode};
pub use mono::{render, render_text_page};
pub use overlay::render_status_bar;
pub use rgba::render_rgba;

/// Bitmap dimensions: 280 pixels wide × 192 pixels tall.
pub const BITMAP_WIDTH: usize = layout::DISPLAY_WIDTH;
pub const BITMAP_HEIGHT: usize = layout::DISPLAY_HEIGHT;
pub const BITMAP_STRIDE: usize = BITMAP_WIDTH / 8; // 35 bytes per row
pub const BITMAP_SIZE: usize = BITMAP_STRIDE * BITMAP_HEIGHT; // 6720

/// RGBA frame dimensions (same as monochrome bitmap).
pub const RGBA_WIDTH: usize = layout::DISPLAY_WIDTH;
pub const RGBA_HEIGHT: usize = layout::DISPLAY_HEIGHT;
pub const RGBA_SIZE: usize = RGBA_WIDTH * RGBA_HEIGHT * 4;

#[cfg(test)]
mod tests;
