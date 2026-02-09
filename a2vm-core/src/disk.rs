use std::io;
use std::path::Path;

/// Nibblized track size in bytes.
const NIBBLE_TRACK_SIZE: usize = 6656;

/// Raw .dsk image size: 35 tracks × 16 sectors × 256 bytes.
const DSK_SIZE: usize = 143_360;

/// DOS 3.3 physical-to-logical sector interleave.
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
        let mut buf = Vec::with_capacity(NIBBLE_TRACK_SIZE);
        for (phys_sector, &logical_sector) in DOS33_SECTOR_ORDER.iter().enumerate() {
            let offset = (track * 16 + logical_sector) * 256;
            let sector_data = &raw[offset..offset + 256];

            nibblize_sector(&mut buf, track as u8, phys_sector as u8, sector_data);
        }
        // Fill remainder with sync bytes
        while buf.len() < NIBBLE_TRACK_SIZE {
            buf.push(0xFF);
        }
        out_track[..NIBBLE_TRACK_SIZE].copy_from_slice(&buf[..NIBBLE_TRACK_SIZE]);
    }
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

/// 6-and-2 encoding: 256 raw bytes → 342 nibblized bytes + 1 checksum byte.
fn encode_6and2(buf: &mut Vec<u8>, data: &[u8]) {
    // Build 342-byte 6-and-2 stream: 86 aux bytes + 256 main bytes.
    // The staging array needs two extra slots because the classic algorithm
    // iterates idx6 down from 0x101 and writes at ptr6 + idx6.
    let mut nibbles = [0u8; 0x158];
    let ptr2 = 0usize;
    let ptr6 = 0x56usize;

    let mut idx2: i32 = 0x55;
    for idx6 in (0..=0x101usize).rev() {
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
            idx2 = 0x55;
        }
    }

    // XOR chain + translate through WRITE_TABLE.
    let mut last = 0u8;
    for &val in &nibbles[..0x156] {
        buf.push(WRITE_TABLE[(last ^ val) as usize]);
        last = val;
    }
    buf.push(WRITE_TABLE[last as usize]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_6and2_stream(encoded: &[u8]) -> [u8; 256] {
        assert_eq!(encoded.len(), 343);

        let mut raw6 = [0u8; 256];
        let mut raw2 = [0u8; 86];
        let mut last = 0u8;

        for idx in 0..86 {
            let code = encoded[idx];
            let val = WRITE_TABLE
                .iter()
                .position(|&b| b == code)
                .expect("invalid 6-and-2 code") as u8;
            let dec = val ^ last;
            raw2[85 - idx] = dec;
            last = dec;
        }

        for idx in 0..256 {
            let code = encoded[86 + idx];
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
        let mut j = 85usize;
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
                j = 85;
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
}
