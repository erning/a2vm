use std::io;
use std::path::Path;

/// Nibblized track size in bytes.
const NIBBLE_TRACK_SIZE: usize = 6656;

/// Raw .dsk image size: 35 tracks × 16 sectors × 256 bytes.
const DSK_SIZE: usize = 143_360;

/// DOS 3.3 physical-to-logical sector interleave.
const DOS33_SECTOR_ORDER: [usize; 16] = [0, 7, 14, 6, 13, 5, 12, 4, 11, 3, 10, 2, 9, 1, 8, 15];

/// 6-and-2 write translation table (64 entries, all values >= 0x96).
#[rustfmt::skip]
const WRITE_TABLE: [u8; 64] = [
    0x96, 0x97, 0x9A, 0x9B, 0x9D, 0x9E, 0x9F, 0xA6,
    0xA7, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF, 0xB2, 0xB3,
    0xB4, 0xB5, 0xB6, 0xB7, 0xB9, 0xBA, 0xBB, 0xBC,
    0xBD, 0xBE, 0xBF, 0xCB, 0xCD, 0xCE, 0xCF, 0xD3,
    0xD6, 0xD7, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE,
    0xDF, 0xE5, 0xE6, 0xE7, 0xE9, 0xEA, 0xEB, 0xEC,
    0xED, 0xEE, 0xEF, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6,
    0xF7, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF,
];

/// A single floppy drive.
struct Drive {
    nibble_data: Box<[[u8; NIBBLE_TRACK_SIZE]; 35]>,
    byte_position: usize,
    has_disk: bool,
    write_protected: bool,
}

impl Drive {
    fn new() -> Self {
        Self {
            nibble_data: Box::new([[0u8; NIBBLE_TRACK_SIZE]; 35]),
            byte_position: 0,
            has_disk: false,
            write_protected: true,
        }
    }
}

/// Disk II controller for slot 6.
pub struct DiskII {
    drives: [Drive; 2],
    selected_drive: usize,
    pub half_track: u8,
    phases: [bool; 4],
    pub motor_on: bool,
    q6: bool,
    q7: bool,
    write_latch: u8,
    slot_rom: [u8; 256],
    slot_rom_loaded: bool,
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
            write_latch: 0,
            slot_rom: [0; 256],
            slot_rom_loaded: false,
        }
    }

    /// Load slot ROM bytes (256 bytes for $C600-$C6FF).
    pub fn load_slot_rom(&mut self, data: &[u8]) {
        let len = data.len().min(256);
        self.slot_rom[..len].copy_from_slice(&data[..len]);
        self.slot_rom_loaded = true;
    }

    /// Read from slot ROM area ($C600-$C6FF).
    pub fn read_slot_rom(&self, addr: u16) -> u8 {
        if self.slot_rom_loaded {
            self.slot_rom[(addr & 0xFF) as usize]
        } else {
            0
        }
    }

    /// Load a .dsk image into a drive (0 or 1).
    pub fn load_disk(&mut self, path: &Path, drive: usize) -> io::Result<()> {
        let data = std::fs::read(path)?;
        if data.len() != DSK_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("DSK image must be {} bytes, got {}", DSK_SIZE, data.len()),
            ));
        }
        let drv = &mut self.drives[drive];
        nibblize_disk(&data, &mut drv.nibble_data);
        drv.has_disk = true;
        drv.write_protected = true;
        drv.byte_position = 0;
        Ok(())
    }

    /// Handle I/O read at $C0E0-$C0EF.
    pub fn io_read(&mut self, addr: u16) -> u8 {
        let switch = (addr & 0x0F) as u8;
        self.handle_switch(switch);

        // Return value depends on Q6/Q7 state
        if switch == 0x0C {
            // Q6 off — if Q7 off, read nibble; if Q7 on, write mode (return 0)
            if !self.q7 {
                return self.read_nibble();
            }
        } else if switch == 0x0D {
            // Q6 on — sense write protect
            if !self.q7 {
                let drv = &self.drives[self.selected_drive];
                return if drv.write_protected { 0x80 } else { 0x00 };
            }
        } else if switch == 0x0E {
            // Q7 off — if Q6 off, read mode
            if !self.q6 {
                return self.read_nibble();
            }
        }

        0
    }

    /// Handle I/O write at $C0E0-$C0EF.
    pub fn io_write(&mut self, addr: u16, val: u8) {
        let switch = (addr & 0x0F) as u8;
        if switch == 0x0D {
            // Q6 on + Q7 on = write mode: latch data
            self.write_latch = val;
        }
        self.handle_switch(switch);
    }

    /// Process a soft-switch toggle.
    fn handle_switch(&mut self, switch: u8) {
        match switch {
            0x00 => self.set_phase(0, false),
            0x01 => self.set_phase(0, true),
            0x02 => self.set_phase(1, false),
            0x03 => self.set_phase(1, true),
            0x04 => self.set_phase(2, false),
            0x05 => self.set_phase(2, true),
            0x06 => self.set_phase(3, false),
            0x07 => self.set_phase(3, true),
            0x08 => self.motor_on = false,
            0x09 => self.motor_on = true,
            0x0A => self.selected_drive = 0,
            0x0B => self.selected_drive = 1,
            0x0C => self.q6 = false,
            0x0D => self.q6 = true,
            0x0E => self.q7 = false,
            0x0F => self.q7 = true,
            _ => {}
        }
    }

    /// Stepper motor phase control.
    fn set_phase(&mut self, phase: usize, on: bool) {
        self.phases[phase] = on;
        if !on {
            return;
        }

        let current_phase = (self.half_track as usize) % 4;
        let next = (current_phase + 1) % 4;
        let prev = (current_phase + 3) % 4;

        if phase == next {
            if self.half_track < 69 {
                self.half_track += 1;
            }
        } else if phase == prev {
            if self.half_track > 0 {
                self.half_track -= 1;
            }
        }
    }

    /// Read one nibble from the current track position.
    fn read_nibble(&mut self) -> u8 {
        let drv = &mut self.drives[self.selected_drive];
        if !drv.has_disk {
            return 0xFF;
        }
        let track = (self.half_track / 2) as usize;
        let val = drv.nibble_data[track][drv.byte_position];
        drv.byte_position += 1;
        if drv.byte_position >= NIBBLE_TRACK_SIZE {
            drv.byte_position = 0;
        }
        val
    }
}

impl Default for DiskII {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Nibblization: convert raw .dsk sector data to nibble stream
// ---------------------------------------------------------------------------

/// 4-and-4 encode: byte B → two disk bytes.
fn encode_4and4(buf: &mut Vec<u8>, val: u8) {
    buf.push((val >> 1) | 0xAA);
    buf.push(val | 0xAA);
}

/// Nibblize an entire .dsk image (35 tracks × 16 sectors) into nibble tracks.
fn nibblize_disk(raw: &[u8], out: &mut [[u8; NIBBLE_TRACK_SIZE]; 35]) {
    for track in 0..35 {
        let mut buf = Vec::with_capacity(NIBBLE_TRACK_SIZE);
        for phys_sector in 0..16 {
            let logical_sector = DOS33_SECTOR_ORDER[phys_sector];
            let offset = (track * 16 + logical_sector) * 256;
            let sector_data = &raw[offset..offset + 256];

            nibblize_sector(&mut buf, track as u8, phys_sector as u8, sector_data);
        }
        // Fill remainder with sync bytes
        while buf.len() < NIBBLE_TRACK_SIZE {
            buf.push(0xFF);
        }
        out[track][..NIBBLE_TRACK_SIZE].copy_from_slice(&buf[..NIBBLE_TRACK_SIZE]);
    }
}

/// Nibblize a single sector: address field + data field.
fn nibblize_sector(buf: &mut Vec<u8>, track: u8, sector: u8, data: &[u8]) {
    let volume = 0xFE; // standard volume number

    // Gap 1: 20 sync bytes
    for _ in 0..20 {
        buf.push(0xFF);
    }

    // Address field prologue
    buf.push(0xD5);
    buf.push(0xAA);
    buf.push(0x96);

    // 4-and-4 encoded: volume, track, sector, checksum
    encode_4and4(buf, volume);
    encode_4and4(buf, track);
    encode_4and4(buf, sector);
    encode_4and4(buf, volume ^ track ^ sector);

    // Address field epilogue
    buf.push(0xDE);
    buf.push(0xAA);
    buf.push(0xEB);

    // Gap 2: 6 sync bytes
    for _ in 0..6 {
        buf.push(0xFF);
    }

    // Data field prologue
    buf.push(0xD5);
    buf.push(0xAA);
    buf.push(0xAD);

    // 6-and-2 encode the 256 data bytes
    encode_6and2(buf, data);

    // Data field epilogue
    buf.push(0xDE);
    buf.push(0xAA);
    buf.push(0xEB);
}

/// 6-and-2 encoding: 256 raw bytes → 342 nibblized bytes + 1 checksum byte.
fn encode_6and2(buf: &mut Vec<u8>, data: &[u8]) {
    // Build the 86 auxiliary bytes (2-bit parts, in reverse order)
    let mut aux = [0u8; 86];
    for i in 0..256 {
        let two_bits = data[i] & 0x03; // low 2 bits
        // Distribute into aux buffer (reverse mapping)
        let aux_idx = match i {
            0..=85 => 85 - i,
            86..=171 => 171 - i,
            172..=255 => 257 - i,
            _ => unreachable!(),
        };
        let shift = match i {
            0..=85 => 0,
            86..=171 => 2,
            172..=255 => 4,
            _ => unreachable!(),
        };
        aux[aux_idx] |= two_bits << shift;
    }

    // Build 342-byte buffer: 86 aux + 256 main (upper 6 bits)
    let mut nib = [0u8; 343]; // 342 data + 1 checksum
    for i in 0..86 {
        nib[i] = aux[i];
    }
    for i in 0..256 {
        nib[86 + i] = data[i] >> 2;
    }

    // XOR chain
    let mut prev = 0u8;
    for i in 0..342 {
        let val = nib[i];
        nib[i] = val ^ prev;
        prev = val;
    }
    nib[342] = prev; // checksum

    // Translate through WRITE_TABLE
    for byte in &nib {
        buf.push(WRITE_TABLE[*byte as usize & 0x3F]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_4and4_encoding() {
        let mut buf = Vec::new();
        encode_4and4(&mut buf, 0xFE);
        // 0xFE >> 1 = 0x7F, 0x7F | 0xAA = 0xFF
        // 0xFE | 0xAA = 0xFE
        assert_eq!(buf[0], 0xFF);
        assert_eq!(buf[1], 0xFE);
    }

    #[test]
    fn test_4and4_zero() {
        let mut buf = Vec::new();
        encode_4and4(&mut buf, 0x00);
        assert_eq!(buf[0], 0xAA);
        assert_eq!(buf[1], 0xAA);
    }

    #[test]
    fn test_nibblize_track_size() {
        // Create a fake 1-track disk (just zeros)
        let raw = vec![0u8; DSK_SIZE];
        let mut out = Box::new([[0u8; NIBBLE_TRACK_SIZE]; 35]);
        nibblize_disk(&raw, &mut out);

        // Every track should be exactly NIBBLE_TRACK_SIZE bytes
        for track in 0..35 {
            assert_eq!(out[track].len(), NIBBLE_TRACK_SIZE);
        }
    }

    #[test]
    fn test_nibblize_sector_markers() {
        let data = [0u8; 256];
        let mut buf = Vec::new();
        nibblize_sector(&mut buf, 0, 0, &data);

        // Find address field prologue (after 20 sync bytes)
        assert_eq!(buf[20], 0xD5);
        assert_eq!(buf[21], 0xAA);
        assert_eq!(buf[22], 0x96);

        // Address epilogue should be DE AA EB
        // After prologue: 3 bytes + 4×2=8 bytes 4-and-4 + 3 epilogue = 14
        assert_eq!(buf[31], 0xDE);
        assert_eq!(buf[32], 0xAA);
        assert_eq!(buf[33], 0xEB);

        // Data field prologue after 6 sync bytes
        assert_eq!(buf[40], 0xD5);
        assert_eq!(buf[41], 0xAA);
        assert_eq!(buf[42], 0xAD);
    }

    #[test]
    fn test_write_table_valid_nibbles() {
        // All entries should have high bit set and be valid disk nibbles (>= 0x96)
        for &val in &WRITE_TABLE {
            assert!(val >= 0x96, "WRITE_TABLE entry {:#04X} < 0x96", val);
            assert!(val & 0x80 != 0, "WRITE_TABLE entry {:#04X} missing high bit", val);
        }
    }

    #[test]
    fn test_stepper_motor_forward() {
        let mut disk = DiskII::new();
        assert_eq!(disk.half_track, 0);

        // Energize phase 1 (next from phase 0)
        disk.set_phase(1, true);
        assert_eq!(disk.half_track, 1);

        // Energize phase 2
        disk.set_phase(2, true);
        assert_eq!(disk.half_track, 2);
    }

    #[test]
    fn test_stepper_motor_backward() {
        let mut disk = DiskII::new();
        disk.half_track = 10;

        // Current phase = 10 % 4 = 2, previous = 1
        disk.set_phase(1, true);
        assert_eq!(disk.half_track, 9);
    }

    #[test]
    fn test_stepper_motor_clamp() {
        let mut disk = DiskII::new();
        disk.half_track = 0;

        // Try to go below 0
        disk.set_phase(3, true); // prev from phase 0
        assert_eq!(disk.half_track, 0);

        // Try to go above 69
        disk.half_track = 69;
        let current_phase = 69 % 4; // = 1
        let next = (current_phase + 1) % 4; // = 2
        disk.set_phase(next, true);
        assert_eq!(disk.half_track, 69);
    }

    #[test]
    fn test_read_nibble_no_disk() {
        let mut disk = DiskII::new();
        assert_eq!(disk.read_nibble(), 0xFF);
    }

    #[test]
    fn test_io_switches() {
        let mut disk = DiskII::new();

        // Motor on
        disk.io_read(0xC0E9);
        assert!(disk.motor_on);

        // Motor off
        disk.io_read(0xC0E8);
        assert!(!disk.motor_on);

        // Select drive 2
        disk.io_read(0xC0EB);
        assert_eq!(disk.selected_drive, 1);

        // Select drive 1
        disk.io_read(0xC0EA);
        assert_eq!(disk.selected_drive, 0);
    }
}
