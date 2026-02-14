use std::path::Path;

use crate::audio::Speaker;
use crate::bus::Bus;
use crate::cpu::status::C;
use crate::cpu::Cpu;
use crate::disk::DiskII;
use crate::error::{Error, Result};
use crate::video::DisplayMode;

mod bus_state;
mod runtime;
mod rwts;

const RWTS_ENTRY_PC: u16 = 0xB7B5;
const RWTS_IOB_DRIVE_OFFSET: u16 = 0x02;
const RWTS_IOB_TRACK_OFFSET: u16 = 0x04;
const RWTS_IOB_SECTOR_OFFSET: u16 = 0x05;
const RWTS_IOB_BUFFER_LO_OFFSET: u16 = 0x08;
const RWTS_IOB_BUFFER_HI_OFFSET: u16 = 0x09;
const RWTS_IOB_COMMAND_OFFSET: u16 = 0x0C;
const RWTS_IOB_ERROR_OFFSET: u16 = 0x0D;
const RWTS_CMD_SEEK: u8 = 0x01;
const RWTS_CMD_READ: u8 = 0x02;
const RWTS_CMD_WRITE: u8 = 0x03;
const RWTS_ERROR_OK: u8 = 0x00;
const RWTS_ERROR_IO: u8 = 0x27;

/// Bus state: RAM, ROM, keyboard, display, disk, speaker.
///
/// Separated from CPU to allow simultaneous mutable borrows,
/// eliminating the `mem::take` pattern.
///
/// Memory map:
///   $0000-$BFFF  48K RAM
///   $C000        Keyboard latch (read: last key | bit7=strobe)
///   $C010        Keyboard strobe clear (read/write: clears bit 7 of latch)
///   $C011-$C04F  I/O stubs (read: $00)
///   $C050-$C057  Display mode soft switches
///   $C058-$C0FF  I/O stubs (read: $00)
///   $C100-$CFFF  Slot ROM stubs (read: $00)
///   $D000-$FFFF  12K ROM
pub struct BusState {
    pub display: DisplayMode,
    pub disk: DiskII,
    pub(crate) speaker: Speaker,
    bus_cycle: u64,
    pub(crate) disk_controller_enabled: bool,
    fast_disk: bool,
    ram: [u8; 0xC000], // 48K RAM
    rom: [u8; 0x3000], // 12K ROM ($D000-$FFFF)
    rom_loaded: bool,
    kbd_latch: u8,         // $C000: keyboard latch (bit 7 = strobe)
    pub video_dirty: bool, // Set on writes to video RAM ($0400-$5FFF)
    display_mode_gen: u8,  // Incremented on display mode switch changes
}

/// Apple II emulator: CPU + bus (RAM/ROM/IO).
pub struct AppleII {
    pub cpu: Cpu,
    pub bus: BusState,
}

impl AppleII {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            bus: BusState::new(),
        }
    }

    /// Load a ROM file into $D000-$FFFF.
    ///
    /// Supported sizes:
    ///   - 12K (12288): $D000-$FFFF directly (Apple II, Apple II+)
    ///   - 20K (20480): $B000-$FFFF image, uses $D000-$FFFF at offset $2000 (Apple II+)
    pub fn load_rom(&mut self, path: &Path) -> Result<()> {
        let data = std::fs::read(path)?;
        self.load_rom_data(&data)
    }

    /// Load ROM data directly from a byte slice.
    pub fn load_rom_data(&mut self, data: &[u8]) -> Result<()> {
        match data.len() {
            0x3000 => {
                // 12K ROM -> $D000-$FFFF (Apple II / Apple II+)
                self.bus.rom.copy_from_slice(data);
                self.bus.disk.clear_slot_rom();
            }
            0x5000 => {
                // 20K ROM -> $B000-$FFFF image, use $D000-$FFFF at offset $2000
                self.bus.rom.copy_from_slice(&data[0x2000..]);
                // Extract Disk II slot 6 ROM at $C600-$C6FF (offset $1600)
                self.bus.disk.load_slot_rom(&data[0x1600..0x1700]);
            }
            _ => {
                return Err(Error::UnsupportedRomSize { actual: data.len() });
            }
        }
        self.bus.rom_loaded = true;
        Ok(())
    }

    /// Reset the CPU: reads the reset vector from $FFFC-$FFFD.
    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
        self.bus.speaker.reset(self.cpu.cycles());
    }
}

impl AppleII {
    /// Simulate a key press: sets keyboard latch with strobe bit.
    /// `ascii` should be the 7-bit ASCII value (e.g., 0x41 for 'A').
    /// The latch stores `ascii | 0x80` (bit 7 = strobe).
    pub fn key_press(&mut self, ascii: u8) {
        self.bus.kbd_latch = ascii | 0x80;
    }

    pub fn load_disk_into_drive(&mut self, path: &Path, drive: usize) -> Result<()> {
        self.bus.disk_controller_enabled = true;
        self.bus.disk.load_disk(path, drive)
    }

    /// Load a .dsk disk image into drive 1.
    pub fn load_disk(&mut self, path: &Path) -> Result<()> {
        self.load_disk_into_drive(path, 0)
    }

    /// Enable or disable Disk II slot-6 mapping.
    pub fn set_disk_controller_enabled(&mut self, enabled: bool) {
        self.bus.disk_controller_enabled = enabled;
    }

    /// Enable or disable fast-disk mode (RWTS trap).
    pub fn set_fast_disk(&mut self, enabled: bool) {
        self.bus.fast_disk = enabled;
    }

    /// Returns whether fast-disk mode is active.
    pub fn is_fast_disk(&self) -> bool {
        self.bus.fast_disk
    }

    /// Return and clear the last deferred disk error.
    pub fn take_disk_error(&mut self) -> Option<Error> {
        self.bus.disk.take_last_error()
    }

    /// Read-only access to RAM (for video rendering).
    pub fn ram(&self) -> &[u8] {
        &self.bus.ram
    }

    /// Convenience: bus read (with side effects).
    pub fn read(&mut self, addr: u16) -> u8 {
        self.bus.read(addr)
    }

    /// Convenience: bus write.
    pub fn write(&mut self, addr: u16, val: u8) {
        self.bus.write(addr, val);
    }

    /// Convenience: bus peek (no side effects).
    pub fn peek(&self, addr: u16) -> u8 {
        self.bus.peek(addr)
    }
}

impl Default for AppleII {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
