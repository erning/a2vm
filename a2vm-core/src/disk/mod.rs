use crate::error::Error;

/// Nibblized track size in bytes.
const NIBBLE_TRACK_SIZE: usize = 6656;

/// Raw .dsk image size: 35 tracks × 16 sectors × 256 bytes.
const DSK_SIZE: usize = 143_360;
const TRACK_COUNT: usize = 35;
const SECTORS_PER_TRACK: usize = 16;

/// Stepper phase transition deltas in half-track units.
const PHASE_DELTA: [[i8; 4]; 4] = [[0, 1, 2, -1], [-1, 0, 1, 2], [-2, -1, 0, 1], [1, -2, -1, 0]];

/// Disk II controller for slot 6.
pub struct DiskII {
    drives: [Drive; 2],
    selected_drive: usize,
    pub half_track: u8,
    phases: [bool; 4],
    pub motor_on: bool,
    q6: bool,
    q7: bool,
    read_latch: u8,
    data_ready: bool,
    write_latch: u8,
    slot_rom: [u8; 256],
    slot_rom_loaded: bool,
    last_error: Option<Error>,
}

impl DiskII {
    pub fn new() -> Self {
        Self {
            drives: [Drive::new(), Drive::new()],
            selected_drive: 0,
            half_track: 0,
            phases: [false; 4],
            motor_on: false,
            q6: false,
            q7: false,
            read_latch: 0,
            data_ready: false,
            write_latch: 0,
            slot_rom: [0; 256],
            slot_rom_loaded: false,
            last_error: None,
        }
    }

    /// Load slot ROM bytes (256 bytes for $C600-$C6FF).
    pub fn load_slot_rom(&mut self, data: &[u8]) {
        let len = data.len().min(256);
        self.slot_rom[..len].copy_from_slice(&data[..len]);
        self.slot_rom_loaded = true;
    }

    /// Clear slot ROM data (for 12K ROM mode).
    pub fn clear_slot_rom(&mut self) {
        self.slot_rom.fill(0);
        self.slot_rom_loaded = false;
    }

    /// Read from slot ROM area ($C600-$C6FF).
    pub fn read_slot_rom(&self, addr: u16) -> u8 {
        if self.slot_rom_loaded {
            self.slot_rom[(addr & 0xFF) as usize]
        } else {
            0
        }
    }

    /// Tick hook for future cycle-accurate disk timing.
    pub fn tick(&mut self, _cycles: u32) {}

    /// Return and clear the last deferred disk error.
    pub fn take_last_error(&mut self) -> Option<Error> {
        self.last_error.take()
    }
}

impl std::fmt::Debug for DiskII {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiskII")
            .field("selected_drive", &self.selected_drive)
            .field("half_track", &self.half_track)
            .field("motor_on", &self.motor_on)
            .field("q6", &self.q6)
            .field("q7", &self.q7)
            .field("slot_rom_loaded", &self.slot_rom_loaded)
            .finish_non_exhaustive()
    }
}

impl Default for DiskII {
    fn default() -> Self {
        Self::new()
    }
}

mod codec;
mod drive;
mod image;
mod io;
use drive::Drive;

#[cfg(test)]
mod tests;
