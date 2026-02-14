use super::codec::{
    decode_nibblized_track, encode_4and4, encode_6and2, nibblize_disk, nibblize_sector, AUX_BYTES,
    MAIN_BYTES, WRITE_TABLE,
};
use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDsk {
    path: PathBuf,
}

impl TempDsk {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDsk {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn write_temp_dsk(bytes: &[u8]) -> TempDsk {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    path.push(format!("a2vm-disk-test-{nanos}.dsk"));
    fs::write(&path, bytes).unwrap();
    TempDsk { path }
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
    let mut out = Box::new([[0u8; NIBBLE_TRACK_SIZE]; TRACK_COUNT]);
    nibblize_disk(&raw, &mut out);

    // Every track should be exactly NIBBLE_TRACK_SIZE bytes
    for track in 0..TRACK_COUNT {
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
    let temp = write_temp_dsk(&raw);
    let path = temp.path();

    let mut disk = DiskII::new();
    disk.load_disk(path, 0).unwrap();

    let mut sector = [0u8; 256];
    for (i, b) in sector.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(3).wrapping_add(7);
    }

    disk.write_sector_raw(0, 0, 0, &sector).unwrap();

    let read_back = disk.read_sector_raw(0, 0, 0).unwrap();
    assert_eq!(read_back, sector);

    let persisted = fs::read(path).unwrap();
    assert_eq!(&persisted[..256], &sector);
}

#[test]
fn test_q6_q7_write_mode_writes_nibble_stream() {
    let raw = vec![0u8; DSK_SIZE];
    let temp = write_temp_dsk(&raw);
    let path = temp.path();

    let mut disk = DiskII::new();
    disk.load_disk(path, 0).unwrap();

    disk.io_write(0xC0E9, 0x00);
    disk.io_write(0xC0ED, 0x00);
    disk.io_write(0xC0EF, 0x00);

    let track = (disk.half_track / 2) as usize;
    let write_pos = disk.drives[0].byte_position;
    disk.io_write(0xC0EF, 0xA5);
    assert_eq!(disk.drives[0].nibble_data[track][write_pos], 0xA5);
}

#[test]
fn test_nibble_to_raw_sync_roundtrip() {
    let mut raw = vec![0u8; DSK_SIZE];
    for (i, byte) in raw.iter_mut().enumerate() {
        *byte = (i as u8).wrapping_mul(17).wrapping_add(43);
    }
    let temp = write_temp_dsk(&raw);
    let path = temp.path();

    let mut disk = DiskII::new();
    disk.load_disk(path, 0).unwrap();

    let original_sector = disk.read_sector_raw(0, 0, 0).unwrap();

    disk.drives[0].dirty = true;
    disk.drives[0].dirty_tracks[0] = true;
    disk.sync_nibble_to_raw(0).unwrap();

    let after_sync = disk.read_sector_raw(0, 0, 0).unwrap();
    assert_eq!(original_sector, after_sync);

    let persisted = fs::read(path).unwrap();
    assert_eq!(&persisted[..256], &original_sector[..]);
}

#[test]
fn test_decode_nibblized_track() {
    let mut raw = [0u8; DSK_SIZE];
    for (i, byte) in raw.iter_mut().enumerate() {
        *byte = (i as u8).wrapping_add(1);
    }

    let mut nibble_tracks = Box::new([[0u8; NIBBLE_TRACK_SIZE]; TRACK_COUNT]);
    nibblize_disk(&raw, &mut nibble_tracks);

    let mut decoded = [0u8; DSK_SIZE];
    for track in 0..TRACK_COUNT {
        decode_nibblized_track(&nibble_tracks[track], &mut decoded, track).unwrap();
    }

    assert_eq!(raw, decoded);
}

#[test]
fn test_decode_nibblized_track_rejects_corrupt_data() {
    let raw = [0u8; DSK_SIZE];
    let mut nibble_tracks = Box::new([[0u8; NIBBLE_TRACK_SIZE]; TRACK_COUNT]);
    nibblize_disk(&raw, &mut nibble_tracks);

    // Break first track's first data prologue.
    nibble_tracks[0][0x93] = 0x00;

    let mut decoded = [0u8; DSK_SIZE];
    let result = decode_nibblized_track(&nibble_tracks[0], &mut decoded, 0);
    assert!(matches!(result, Err(Error::DiskDecodeFailed { track: 0 })));
}

#[test]
fn test_sync_preserves_write_protection() {
    let raw = vec![0u8; DSK_SIZE];
    let temp = write_temp_dsk(&raw);
    let path = temp.path();

    let mut disk = DiskII::new();
    disk.load_disk(path, 0).unwrap();
    disk.drives[0].write_protected = true;
    disk.drives[0].dirty = true;
    disk.drives[0].dirty_tracks[0] = true;

    let result = disk.sync_nibble_to_raw(0);
    assert!(matches!(result, Err(Error::DiskWriteProtected)));
}

#[test]
fn test_motor_off_syncs_nibble_to_raw() {
    let raw = vec![0u8; DSK_SIZE];
    let temp = write_temp_dsk(&raw);
    let path = temp.path();

    let mut disk = DiskII::new();
    disk.load_disk(path, 0).unwrap();

    disk.drives[0].write_protected = false;

    disk.io_write(0xC0E9, 0x00);
    disk.io_write(0xC0ED, 0x00);
    disk.io_write(0xC0EF, 0x00);

    for i in 0..10 {
        disk.io_write(0xC0EF, (0xA0 + i) as u8);
    }

    assert!(disk.drives[0].dirty);

    disk.io_write(0xC0E8, 0x00);

    assert!(!disk.drives[0].dirty);
}

#[test]
fn test_flush_all_drives() {
    let raw1 = vec![0u8; DSK_SIZE];
    let raw2 = vec![0xFFu8; DSK_SIZE];
    let temp1 = write_temp_dsk(&raw1);
    let temp2 = write_temp_dsk(&raw2);
    let path1 = temp1.path();
    let path2 = temp2.path();

    let mut disk = DiskII::new();
    disk.load_disk(path1, 0).unwrap();
    disk.load_disk(path2, 1).unwrap();

    disk.drives[0].dirty = true;
    disk.drives[1].dirty = true;
    disk.drives[0].dirty_tracks[0] = true;
    disk.drives[1].dirty_tracks[0] = true;

    disk.flush_all_drives().unwrap();

    assert!(!disk.drives[0].dirty);
    assert!(!disk.drives[1].dirty);
}

#[test]
fn test_sync_failure_keeps_dirty_state_for_retry() {
    let raw = vec![0u8; DSK_SIZE];
    let temp = write_temp_dsk(&raw);
    let path = temp.path();

    let mut disk = DiskII::new();
    disk.load_disk(path, 0).unwrap();

    disk.drives[0].dirty = true;
    disk.drives[0].dirty_tracks[0] = true;
    disk.drives[0].image_path = Some(PathBuf::from("/definitely/missing/a2vm-sync-fail.dsk"));

    let result = disk.sync_nibble_to_raw(0);
    assert!(result.is_err());
    assert!(disk.drives[0].dirty);
    assert!(disk.drives[0].dirty_tracks[0]);
}

#[test]
fn test_take_last_error_on_motor_off_sync_failure() {
    let raw = vec![0u8; DSK_SIZE];
    let temp = write_temp_dsk(&raw);
    let path = temp.path();

    let mut disk = DiskII::new();
    disk.load_disk(path, 0).unwrap();
    disk.drives[0].dirty = true;
    disk.drives[0].dirty_tracks[0] = true;
    disk.drives[0].image_path = Some(PathBuf::from("/definitely/missing/a2vm-switch-fail.dsk"));

    disk.motor_on = true;
    disk.io_write(0xC0E8, 0x00);

    let err = disk.take_last_error();
    assert!(matches!(err, Some(Error::Io(_))));
    assert!(disk.take_last_error().is_none());
}
