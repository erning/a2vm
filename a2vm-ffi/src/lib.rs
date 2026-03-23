//! C-compatible FFI wrapper for the A2VM Apple II emulator.
//!
//! Exposes an opaque `A2VMEmulator` handle and free functions that
//! Swift (or any C caller) can use to drive the emulation loop.

use std::borrow::Cow;
use std::slice;
use std::time::Instant;

use a2vm_core::video::{self, DisplayColorMode, RGBA_HEIGHT, RGBA_SIZE, RGBA_WIDTH};
use a2vm_oxide::cli::DEFAULT_ROM;
use a2vm_oxide::runner::EmulatorRunner;

/// Flash half-period for text blinking (same as a2vm-gui).
const FLASH_HALF_PERIOD_MS: u128 = 267;

/// Opaque emulator handle exposed through the C API.
pub struct A2VMEmulator {
    runner: EmulatorRunner,
    boot_time: Instant,
}

// ── Lifecycle ───────────────────────────────────────────────────────

/// Create a new emulator with the embedded default ROM.
#[no_mangle]
pub extern "C" fn a2vm_create() -> *mut A2VMEmulator {
    let rom_data = Cow::Borrowed(DEFAULT_ROM);
    let runner = match EmulatorRunner::new(rom_data, &[], false, false) {
        Ok(r) => r,
        Err(_) => return std::ptr::null_mut(),
    };
    Box::into_raw(Box::new(A2VMEmulator {
        runner,
        boot_time: Instant::now(),
    }))
}

/// Destroy an emulator instance.
///
/// # Safety
/// `emu` must be a valid pointer returned by `a2vm_create`.
#[no_mangle]
pub unsafe extern "C" fn a2vm_destroy(emu: *mut A2VMEmulator) {
    if !emu.is_null() {
        drop(Box::from_raw(emu));
    }
}

// ── Emulation ───────────────────────────────────────────────────────

/// Run one frame's worth of emulation cycles.
///
/// # Safety
/// `emu` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn a2vm_tick(emu: *mut A2VMEmulator) {
    let emu = &mut *emu;
    emu.runner.tick();
}

/// Reset the CPU (reads reset vector, clears state).
///
/// # Safety
/// `emu` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn a2vm_reset(emu: *mut A2VMEmulator) {
    let emu = &mut *emu;
    emu.runner.reset();
}

// ── Input ───────────────────────────────────────────────────────────

/// Send a key press to the emulator (7-bit ASCII).
///
/// # Safety
/// `emu` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn a2vm_key_press(emu: *mut A2VMEmulator, ascii: u8) {
    let emu = &mut *emu;
    emu.runner.apple_mut().key_press(ascii);
}

// ── Video ───────────────────────────────────────────────────────────

/// Render the current display into an RGBA buffer.
///
/// `buf` must point to at least 280*192*4 = 215040 bytes.
/// `color_mode`: 0 = Color, 1 = Monochrome, 2 = MonochromeScanlines.
///
/// # Safety
/// `emu` and `buf` must be valid pointers. `buf` must have room for RGBA_SIZE bytes.
#[no_mangle]
pub unsafe extern "C" fn a2vm_render_rgba(
    emu: *mut A2VMEmulator,
    buf: *mut u8,
    color_mode: u8,
    frame_phase: u64,
) {
    let emu = &mut *emu;
    let frame = slice::from_raw_parts_mut(buf, RGBA_SIZE);

    let mode = match color_mode {
        1 => DisplayColorMode::Monochrome,
        2 => DisplayColorMode::MonochromeScanlines,
        _ => DisplayColorMode::Color,
    };

    let flash_on =
        ((emu.boot_time.elapsed().as_millis() / FLASH_HALF_PERIOD_MS) & 1) == 0;

    video::render_rgba(
        emu.runner.apple().ram(),
        &emu.runner.apple().bus.display,
        flash_on,
        mode,
        frame_phase,
        frame,
    );

    emu.runner.apple_mut().bus.video_dirty = false;
}

/// Check if the video output has changed since the last render.
///
/// # Safety
/// `emu` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn a2vm_video_dirty(emu: *const A2VMEmulator) -> bool {
    let emu = &*emu;
    emu.runner.apple().bus.video_dirty
}

/// Returns the display width in pixels (280).
#[no_mangle]
pub extern "C" fn a2vm_display_width() -> u32 {
    RGBA_WIDTH as u32
}

/// Returns the display height in pixels (192).
#[no_mangle]
pub extern "C" fn a2vm_display_height() -> u32 {
    RGBA_HEIGHT as u32
}
