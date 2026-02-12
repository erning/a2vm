use std::path::Path;
use std::path::PathBuf;

use crate::error::{Error, Result};

/// Nibblized track size in bytes.
const NIBBLE_TRACK_SIZE: usize = 6656;

/// Raw .dsk image size: 35 tracks × 16 sectors × 256 bytes.
const DSK_SIZE: usize = 143_360;

/// DOS 3.3 physical-to-logical sector interleave.
///
/// This ordering optimizes sequential reads by accounting for the time
/// needed to process each sector between reads.
/// Reference: Beneath Apple DOS, Chapter 3
const DOS33_SECTOR_ORDER: [usize; 16] = [0, 7, 14, 6, 13, 5, 12, 4, 11, 3, 10, 2, 9, 1, 8, 15];

/// Stepper phase transition deltas in half-track units.
const PHASE_DELTA: [[i8; 4]; 4] = [[0, 1, 2, -1], [-1, 0, 1, 2], [-2, -1, 0, 1], [1, -2, -1, 0]];

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

/// Reverse lookup table for 6-and-2 decoding: maps disk nibble → 6-bit value.
/// Built from WRITE_TABLE at compile time. Entries not in WRITE_TABLE are 0xFF.
const REVERSE_TABLE: [u8; 256] = build_reverse_table();

const fn build_reverse_table() -> [u8; 256] {
    let mut table = [0xFFu8; 256];
    let mut i = 0;
    while i < 64 {
        table[WRITE_TABLE[i] as usize] = i as u8;
        i += 1;
    }
    table
}

const AUX_BYTES: usize = 86;
const MAIN_BYTES: usize = 256;
const TOTAL_NIBBLES: usize = AUX_BYTES + MAIN_BYTES;
const STAGING_SIZE: usize = TOTAL_NIBBLES + 2;
const IDX6_MAX: usize = 0x101;
const IDX2_START: i32 = (AUX_BYTES - 1) as i32;

/// A single floppy drive.
struct Drive {
    nibble_data: Box<[[u8; NIBBLE_TRACK_SIZE]; 35]>,
    raw_data: Option<Box<[u8; DSK_SIZE]>>,
    image_path: Option<PathBuf>,
    byte_position: usize,
    has_disk: bool,
    write_protected: bool,
    dirty: bool,
}

impl Drive {
    fn new() -> Self {
        Self {
            nibble_data: Box::new([[0u8; NIBBLE_TRACK_SIZE]; 35]),
            raw_data: None,
            image_path: None,
            byte_position: 0,
            has_disk: false,
            write_protected: true,
            dirty: false,
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
    read_latch: u8,
    data_ready: bool,
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
            read_latch: 0,
            data_ready: false,
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
    pub fn load_disk(&mut self, path: &Path, drive: usize) -> Result<()> {
        if drive >= 2 {
            return Err(Error::InvalidDiskLocation {
                drive,
                track: 0,
                sector: 0,
            });
        }
        let data = std::fs::read(path)?;
        if data.len() != DSK_SIZE {
            return Err(Error::InvalidDiskSize {
                expected: DSK_SIZE,
                actual: data.len(),
            });
        }
        let drv = &mut self.drives[drive];
        nibblize_disk(&data, &mut drv.nibble_data);
        let mut raw = Box::new([0u8; DSK_SIZE]);
        raw.copy_from_slice(&data);
        let write_protected = std::fs::metadata(path)
            .map(|meta| meta.permissions().readonly())
            .unwrap_or(true);
        drv.raw_data = Some(raw);
        drv.image_path = Some(path.to_path_buf());
        drv.has_disk = true;
        drv.write_protected = write_protected;
        drv.byte_position = 0;
        drv.dirty = false;
        Ok(())
    }

    pub fn write_sector_raw(
        &mut self,
        drive: usize,
        track: u8,
        sector: u8,
        data: &[u8; 256],
    ) -> Result<()> {
        if drive >= 2 || track >= 35 || sector >= 16 {
            return Err(Error::InvalidDiskLocation {
                drive,
                track,
                sector,
            });
        }

        let drv = &mut self.drives[drive];
        if !drv.has_disk {
            return Err(Error::DiskNotLoaded);
        }
        if drv.write_protected {
            return Err(Error::DiskWriteProtected);
        }

        let raw = drv.raw_data.as_mut().ok_or(Error::DiskNotLoaded)?;
        let offset = (track as usize * 16 + sector as usize) * 256;
        raw[offset..offset + 256].copy_from_slice(data);
        nibblize_track(
            &raw[..],
            &mut drv.nibble_data[track as usize],
            track as usize,
        );

        drv.dirty = true;
        if let Some(path) = drv.image_path.as_deref() {
            std::fs::write(path, &raw[..])?;
            drv.dirty = false;
        }

        Ok(())
    }

    /// Handle I/O read at $C0E0-$C0EF.
    pub fn io_read(&mut self, addr: u16) -> u8 {
        let switch = (addr & 0x0F) as u8;
        self.handle_switch(switch);

        // Return register selected by Q6/Q7 after switch side effects.
        if self.q7 {
            // Write mode not implemented yet.
            0
        } else if self.q6 {
            // Status register: write-protect sense in bit 7.
            let drv = &self.drives[self.selected_drive];
            if drv.write_protected {
                0x80
            } else {
                0x00
            }
        } else {
            // Data register: bit 7 indicates fresh data in latch.
            if !self.data_ready && self.motor_on && self.drives[self.selected_drive].has_disk {
                self.read_latch = self.read_nibble();
                self.data_ready = true;
            }
            let out = if self.data_ready {
                self.read_latch
            } else {
                self.read_latch & 0x7F
            };
            self.data_ready = false;
            out
        }
    }

    /// Handle I/O write at $C0E0-$C0EF.
    pub fn io_write(&mut self, addr: u16, val: u8) {
        let switch = (addr & 0x0F) as u8;
        self.handle_switch(switch);
        if self.q6 && self.q7 {
            // Q6 on + Q7 on = write mode: latch data
            self.write_latch = val;
            self.write_nibble(val);
        }
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
            0x08 => {
                // Motor off: sync nibblized data back to raw before stopping
                if self.motor_on {
                    let _ = self.sync_nibble_to_raw(self.selected_drive);
                }
                self.motor_on = false;
                self.data_ready = false;
            }
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
        if !on || !self.motor_on {
            return;
        }

        let current_phase = (self.half_track as usize) % 4;
        let delta = PHASE_DELTA[current_phase][phase] as i16;
        let next = (self.half_track as i16 + delta).clamp(0, 69);
        self.half_track = next as u8;
    }

    /// Read a raw 256-byte sector from the loaded .dsk image.
    /// Returns `None` if no raw data is available or track/sector is out of range.
    pub fn read_sector_raw(&self, drive: usize, track: u8, sector: u8) -> Option<[u8; 256]> {
        if drive >= 2 || track >= 35 || sector >= 16 {
            return None;
        }
        let raw = self.drives[drive].raw_data.as_ref()?;
        let offset = (track as usize * 16 + sector as usize) * 256;
        let mut buf = [0u8; 256];
        buf.copy_from_slice(&raw[offset..offset + 256]);
        Some(buf)
    }

    /// Tick hook for future cycle-accurate disk timing.
    pub fn tick(&mut self, _cycles: u32) {}

    /// Read one nibble from the current track position and advance rotation.
    fn read_nibble(&mut self) -> u8 {
        let drv = &mut self.drives[self.selected_drive];
        if !drv.has_disk {
            return 0;
        }
        let track = (self.half_track / 2) as usize;
        let val = drv.nibble_data[track][drv.byte_position];
        drv.byte_position += 1;
        if drv.byte_position >= NIBBLE_TRACK_SIZE {
            drv.byte_position = 0;
        }
        val
    }

    fn write_nibble(&mut self, val: u8) {
        let drv = &mut self.drives[self.selected_drive];
        if !drv.has_disk || drv.write_protected || !self.motor_on {
            return;
        }
        let track = (self.half_track / 2) as usize;
        drv.nibble_data[track][drv.byte_position] = val;
        drv.byte_position += 1;
        if drv.byte_position >= NIBBLE_TRACK_SIZE {
            drv.byte_position = 0;
        }
        drv.dirty = true;
    }

    /// Sync nibblized track data back to raw sectors.
    /// Call this when disk motor turns off or periodically during writes.
    pub fn sync_nibble_to_raw(&mut self, drive: usize) -> Result<()> {
        if drive >= 2 {
            return Err(Error::InvalidDiskLocation {
                drive,
                track: 0,
                sector: 0,
            });
        }

        let drv = &mut self.drives[drive];
        if !drv.has_disk || !drv.dirty {
            return Ok(());
        }
        if drv.write_protected {
            return Err(Error::DiskWriteProtected);
        }

        let raw = drv.raw_data.as_mut().ok_or(Error::DiskNotLoaded)?;

        // Decode each track's sectors back to raw
        for track in 0..35 {
            decode_nibblized_track(&drv.nibble_data[track], raw, track)?;
        }

        drv.dirty = false;

        // Persist to file if path is set
        if let Some(path) = drv.image_path.as_deref() {
            std::fs::write(path, &raw[..])?;
        }

        Ok(())
    }

    /// Check if a drive has unsynchronized (dirty) nibblized data.
    pub fn is_dirty(&self, drive: usize) -> bool {
        if drive >= 2 {
            return false;
        }
        self.drives[drive].dirty
    }

    /// Flush a specific drive's nibblized data back to the raw image file.
    pub fn flush_drive(&mut self, drive: usize) -> crate::error::Result<()> {
        self.sync_nibble_to_raw(drive)
    }

    /// Flush all drives that have pending writes.
    pub fn flush_all_drives(&mut self) -> crate::error::Result<()> {
        for drive in 0..2 {
            self.sync_nibble_to_raw(drive)?;
        }
        Ok(())
    }

    /// Clear the slot ROM (for 12K ROM loading when switching from 20K).
    pub fn clear_slot_rom(&mut self) {
        self.slot_rom.fill(0);
        self.slot_rom_loaded = false;
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
    for (track, out_track) in out.iter_mut().enumerate() {
        nibblize_track(raw, out_track, track);
    }
}

fn nibblize_track(raw: &[u8], out_track: &mut [u8; NIBBLE_TRACK_SIZE], track: usize) {
    let mut buf = Vec::with_capacity(NIBBLE_TRACK_SIZE);
    for (phys_sector, &logical_sector) in DOS33_SECTOR_ORDER.iter().enumerate() {
        let offset = (track * 16 + logical_sector) * 256;
        let sector_data = &raw[offset..offset + 256];

        nibblize_sector(&mut buf, track as u8, phys_sector as u8, sector_data);
    }
    while buf.len() < NIBBLE_TRACK_SIZE {
        buf.push(0xFF);
    }
    out_track[..NIBBLE_TRACK_SIZE].copy_from_slice(&buf[..NIBBLE_TRACK_SIZE]);
}

/// Nibblize a single sector: address field + data field.
fn nibblize_sector(buf: &mut Vec<u8>, track: u8, sector: u8, data: &[u8]) {
    let volume = 0xFE; // standard volume number

    // Gap 1 / Gap 3 (values follow DOS 3.3 layout)
    let gap = if sector == 0 {
        0x80
    } else if track == 0 {
        0x28
    } else {
        0x26
    };
    for _ in 0..gap {
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

    // Gap 2: 5 sync bytes
    for _ in 0..5 {
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

    // Gap 3: 1 sync byte
    buf.push(0xFF);
}

/// Decode a nibblized track back to raw sectors.
/// Uses DOS 3.3 sector ordering to map physical sectors to logical sectors.
fn decode_nibblized_track(
    nibble_track: &[u8; NIBBLE_TRACK_SIZE],
    raw: &mut [u8; DSK_SIZE],
    track: usize,
) -> Result<()> {
    let mut pos = 0usize;
    let mut phys_sector = 0;

    while pos < NIBBLE_TRACK_SIZE {
        // Look for data field prologue: D5 AA AD
        if pos + 2 < NIBBLE_TRACK_SIZE
            && nibble_track[pos] == 0xD5
            && nibble_track[pos + 1] == 0xAA
            && nibble_track[pos + 2] == 0xAD
        {
            pos += 3;

            let mut encoded = [0u8; 343];
            let mut encoded_len = 0;

            while pos < NIBBLE_TRACK_SIZE && encoded_len < 343 {
                let val = nibble_track[pos];
                if val == 0xDE {
                    break;
                }
                encoded[encoded_len] = val;
                encoded_len += 1;
                pos += 1;
            }

            if encoded_len == 343 {
                if let Some(data) = decode_6and2_sector(&encoded) {
                    // Map physical sector to logical sector using DOS 3.3 order
                    if phys_sector < 16 && track < 35 {
                        let logical_sector = DOS33_SECTOR_ORDER[phys_sector];
                        let offset = (track * 16 + logical_sector) * 256;
                        raw[offset..offset + 256].copy_from_slice(&data);
                    }
                    phys_sector += 1;
                }
            }
        }
        pos += 1;
    }

    Ok(())
}

fn decode_6and2_sector(encoded: &[u8; 343]) -> Option<[u8; 256]> {
    let mut raw6 = [0u8; MAIN_BYTES];
    let mut raw2 = [0u8; AUX_BYTES];
    let mut last = 0u8;

    for idx in 0..AUX_BYTES {
        let code = encoded[idx];
        let val = REVERSE_TABLE[code as usize];
        if val == 0xFF {
            return None;
        }
        let dec = val ^ last;
        raw2[AUX_BYTES - 1 - idx] = dec;
        last = dec;
    }

    for idx in 0..MAIN_BYTES {
        let code = encoded[AUX_BYTES + idx];
        let val = REVERSE_TABLE[code as usize];
        if val == 0xFF {
            return None;
        }
        let dec = val ^ last;
        raw6[idx] = dec;
        last = dec;
    }

    let checksum_code = encoded[342];
    let checksum_val = REVERSE_TABLE[checksum_code as usize];
    if checksum_val == 0xFF {
        return None;
    }
    let checksum = checksum_val ^ last;
    if checksum != 0 {
        return None;
    }

    let mut data = raw6;
    let mut j = AUX_BYTES - 1;
    for byte in &mut data {
        *byte <<= 1;
        if (raw2[j] & 0x01) != 0 {
            *byte |= 0x01;
        }
        raw2[j] >>= 1;

        *byte <<= 1;
        if (raw2[j] & 0x01) != 0 {
            *byte |= 0x01;
        }
        raw2[j] >>= 1;

        if j == 0 {
            j = AUX_BYTES - 1;
        } else {
            j -= 1;
        }
    }

    Some(data)
}

/// 6-and-2 encoding: 256 raw bytes → 342 nibblized bytes + 1 checksum byte.
fn encode_6and2(buf: &mut Vec<u8>, data: &[u8]) {
    let mut nibbles = [0u8; STAGING_SIZE];
    let ptr2 = 0usize;
    let ptr6 = AUX_BYTES;

    let mut idx2: i32 = IDX2_START;
    for idx6 in (0..=IDX6_MAX).rev() {
        let mut val6 = data[idx6 & 0xFF];
        let mut val2 = nibbles[ptr2 + idx2 as usize];

        val2 = (val2 << 1) | (val6 & 1);
        val6 >>= 1;
        val2 = (val2 << 1) | (val6 & 1);
        val6 >>= 1;

        nibbles[ptr6 + idx6] = val6;
        nibbles[ptr2 + idx2 as usize] = val2;

        idx2 -= 1;
        if idx2 < 0 {
            idx2 = IDX2_START;
        }
    }

    let mut last = 0u8;
    for &val in &nibbles[..TOTAL_NIBBLES] {
        buf.push(WRITE_TABLE[(last ^ val) as usize]);
        last = val;
    }
    buf.push(WRITE_TABLE[last as usize]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_temp_dsk(bytes: &[u8]) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("a2vm-disk-test-{nanos}.dsk"));
        fs::write(&path, bytes).unwrap();
        path
    }

    fn decode_6and2_stream(encoded: &[u8]) -> [u8; 256] {
        assert_eq!(encoded.len(), 343);

        let mut raw6 = [0u8; MAIN_BYTES];
        let mut raw2 = [0u8; AUX_BYTES];
        let mut last = 0u8;

        for idx in 0..AUX_BYTES {
            let code = encoded[idx];
            let val = WRITE_TABLE
                .iter()
                .position(|&b| b == code)
                .expect("invalid 6-and-2 code") as u8;
            let dec = val ^ last;
            raw2[AUX_BYTES - 1 - idx] = dec;
            last = dec;
        }

        for idx in 0..MAIN_BYTES {
            let code = encoded[AUX_BYTES + idx];
            let val = WRITE_TABLE
                .iter()
                .position(|&b| b == code)
                .expect("invalid 6-and-2 code") as u8;
            let dec = val ^ last;
            raw6[idx] = dec;
            last = dec;
        }

        let checksum_code = encoded[342];
        let checksum_val = WRITE_TABLE
            .iter()
            .position(|&b| b == checksum_code)
            .expect("invalid checksum code") as u8;
        let checksum = checksum_val ^ last;
        assert_eq!(checksum, 0, "6-and-2 checksum mismatch");

        let mut data = raw6;
        let mut j = AUX_BYTES - 1;
        for byte in &mut data {
            *byte <<= 1;
            if (raw2[j] & 0x01) != 0 {
                *byte |= 0x01;
            }
            raw2[j] >>= 1;

            *byte <<= 1;
            if (raw2[j] & 0x01) != 0 {
                *byte |= 0x01;
            }
            raw2[j] >>= 1;

            if j == 0 {
                j = AUX_BYTES - 1;
            } else {
                j -= 1;
            }
        }

        data
    }

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
    fn test_encode_6and2_roundtrip() {
        let mut input = [0u8; 256];
        for (i, b) in input.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(37).wrapping_add(11);
        }

        let mut stream = Vec::new();
        encode_6and2(&mut stream, &input);
        assert_eq!(stream.len(), 343);

        let decoded = decode_6and2_stream(&stream);
        assert_eq!(decoded, input);
    }

    #[test]
    fn test_nibblize_sector_markers() {
        let data = [0u8; 256];
        let mut buf = Vec::new();
        nibblize_sector(&mut buf, 0, 0, &data);

        // Address field prologue for track 0 sector 0 is after 0x80 sync bytes.
        assert_eq!(buf[0x80], 0xD5);
        assert_eq!(buf[0x81], 0xAA);
        assert_eq!(buf[0x82], 0x96);

        // Address epilogue should be DE AA EB
        // Prologue (3) + encoded fields (8) => epilogue starts at 0x8B.
        assert_eq!(buf[0x8B], 0xDE);
        assert_eq!(buf[0x8C], 0xAA);
        assert_eq!(buf[0x8D], 0xEB);

        // Data field prologue after 5 sync bytes.
        assert_eq!(buf[0x93], 0xD5);
        assert_eq!(buf[0x94], 0xAA);
        assert_eq!(buf[0x95], 0xAD);
    }

    #[test]
    fn test_write_table_valid_nibbles() {
        // All entries should have high bit set and be valid disk nibbles (>= 0x96)
        for &val in &WRITE_TABLE {
            assert!(val >= 0x96, "WRITE_TABLE entry {:#04X} < 0x96", val);
            assert!(
                val & 0x80 != 0,
                "WRITE_TABLE entry {:#04X} missing high bit",
                val
            );
        }
    }

    #[test]
    fn test_stepper_motor_forward() {
        let mut disk = DiskII::new();
        disk.motor_on = true;
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
        disk.motor_on = true;
        disk.half_track = 10;

        // Current phase = 10 % 4 = 2, previous = 1
        disk.set_phase(1, true);
        assert_eq!(disk.half_track, 9);
    }

    #[test]
    fn test_stepper_motor_clamp() {
        let mut disk = DiskII::new();
        disk.motor_on = true;
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
        assert_eq!(disk.read_nibble(), 0x00);
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

    #[test]
    fn test_write_sector_raw_updates_raw_and_file() {
        let raw = vec![0u8; DSK_SIZE];
        let path = write_temp_dsk(&raw);

        let mut disk = DiskII::new();
        disk.load_disk(&path, 0).unwrap();

        let mut sector = [0u8; 256];
        for (i, b) in sector.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(3).wrapping_add(7);
        }

        disk.write_sector_raw(0, 0, 0, &sector).unwrap();

        let read_back = disk.read_sector_raw(0, 0, 0).unwrap();
        assert_eq!(read_back, sector);

        let persisted = fs::read(&path).unwrap();
        assert_eq!(&persisted[..256], &sector);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_q6_q7_write_mode_writes_nibble_stream() {
        let raw = vec![0u8; DSK_SIZE];
        let path = write_temp_dsk(&raw);

        let mut disk = DiskII::new();
        disk.load_disk(&path, 0).unwrap();

        disk.io_write(0xC0E9, 0x00);
        disk.io_write(0xC0ED, 0x00);
        disk.io_write(0xC0EF, 0x00);

        let track = (disk.half_track / 2) as usize;
        let write_pos = disk.drives[0].byte_position;
        disk.io_write(0xC0EF, 0xA5);
        assert_eq!(disk.drives[0].nibble_data[track][write_pos], 0xA5);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_nibble_to_raw_sync_roundtrip() {
        let mut raw = vec![0u8; DSK_SIZE];
        for (i, byte) in raw.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(17).wrapping_add(43);
        }
        let path = write_temp_dsk(&raw);

        let mut disk = DiskII::new();
        disk.load_disk(&path, 0).unwrap();

        let original_sector = disk.read_sector_raw(0, 0, 0).unwrap();

        disk.drives[0].dirty = true;
        disk.sync_nibble_to_raw(0).unwrap();

        let after_sync = disk.read_sector_raw(0, 0, 0).unwrap();
        assert_eq!(original_sector, after_sync);

        let persisted = fs::read(&path).unwrap();
        assert_eq!(&persisted[..256], &original_sector[..]);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_decode_nibblized_track() {
        let mut raw = [0u8; DSK_SIZE];
        for (i, byte) in raw.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_add(1);
        }

        let mut nibble_tracks = Box::new([[0u8; NIBBLE_TRACK_SIZE]; 35]);
        nibblize_disk(&raw, &mut nibble_tracks);

        let mut decoded = [0u8; DSK_SIZE];
        for track in 0..35 {
            decode_nibblized_track(&nibble_tracks[track], &mut decoded, track).unwrap();
        }

        assert_eq!(raw, decoded);
    }

    #[test]
    fn test_sync_preserves_write_protection() {
        let raw = vec![0u8; DSK_SIZE];
        let path = write_temp_dsk(&raw);

        let mut disk = DiskII::new();
        disk.load_disk(&path, 0).unwrap();
        disk.drives[0].write_protected = true;
        disk.drives[0].dirty = true;

        let result = disk.sync_nibble_to_raw(0);
        assert!(matches!(result, Err(Error::DiskWriteProtected)));

        fs::remove_file(path).unwrap();
    }
}
