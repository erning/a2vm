use crate::error::{Error, Result};

use super::{DSK_SIZE, NIBBLE_TRACK_SIZE, SECTORS_PER_TRACK, TRACK_COUNT};

/// DOS 3.3 physical-to-logical sector interleave.
const DOS33_SECTOR_ORDER: [usize; 16] = [0, 7, 14, 6, 13, 5, 12, 4, 11, 3, 10, 2, 9, 1, 8, 15];

/// 6-and-2 write translation table (64 entries, all values >= 0x96).
#[rustfmt::skip]
pub(super) const WRITE_TABLE: [u8; 64] = [
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

pub(super) const AUX_BYTES: usize = 86;
pub(super) const MAIN_BYTES: usize = 256;
const TOTAL_NIBBLES: usize = AUX_BYTES + MAIN_BYTES;
const STAGING_SIZE: usize = TOTAL_NIBBLES + 2;
const IDX6_MAX: usize = 0x101;
const IDX2_START: i32 = (AUX_BYTES - 1) as i32;

// ---------------------------------------------------------------------------
// Nibblization: convert raw .dsk sector data to nibble stream
// ---------------------------------------------------------------------------

/// 4-and-4 encode: byte B → two disk bytes.
pub(super) fn encode_4and4(buf: &mut Vec<u8>, val: u8) {
    buf.push((val >> 1) | 0xAA);
    buf.push(val | 0xAA);
}

/// Nibblize an entire .dsk image (35 tracks × 16 sectors) into nibble tracks.
pub(super) fn nibblize_disk(raw: &[u8], out: &mut [[u8; NIBBLE_TRACK_SIZE]; TRACK_COUNT]) {
    for (track, out_track) in out.iter_mut().enumerate().take(TRACK_COUNT) {
        nibblize_track(raw, out_track, track);
    }
}

pub(super) fn nibblize_track(raw: &[u8], out_track: &mut [u8; NIBBLE_TRACK_SIZE], track: usize) {
    let mut buf = Vec::with_capacity(NIBBLE_TRACK_SIZE);
    for (phys_sector, &logical_sector) in DOS33_SECTOR_ORDER.iter().enumerate() {
        let offset = (track * SECTORS_PER_TRACK + logical_sector) * 256;
        let sector_data = &raw[offset..offset + 256];

        nibblize_sector(&mut buf, track as u8, phys_sector as u8, sector_data);
    }
    while buf.len() < NIBBLE_TRACK_SIZE {
        buf.push(0xFF);
    }
    out_track[..NIBBLE_TRACK_SIZE].copy_from_slice(&buf[..NIBBLE_TRACK_SIZE]);
}

/// Nibblize a single sector: address field + data field.
pub(super) fn nibblize_sector(buf: &mut Vec<u8>, track: u8, sector: u8, data: &[u8]) {
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
pub(super) fn decode_nibblized_track(
    nibble_track: &[u8; NIBBLE_TRACK_SIZE],
    raw: &mut [u8; DSK_SIZE],
    track: usize,
) -> Result<()> {
    if track >= TRACK_COUNT {
        return Err(Error::DiskDecodeFailed { track });
    }

    let mut pos = 0usize;
    let mut phys_sector = 0usize;

    while pos < NIBBLE_TRACK_SIZE && phys_sector < SECTORS_PER_TRACK {
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

            if encoded_len != 343 {
                return Err(Error::DiskDecodeFailed { track });
            }

            let data = decode_6and2_sector(&encoded).ok_or(Error::DiskDecodeFailed { track })?;

            // Map physical sector to logical sector using DOS 3.3 order.
            let logical_sector = DOS33_SECTOR_ORDER[phys_sector];
            let offset = (track * SECTORS_PER_TRACK + logical_sector) * 256;
            raw[offset..offset + 256].copy_from_slice(&data);
            phys_sector += 1;
        }
        pos += 1;
    }

    if phys_sector != SECTORS_PER_TRACK {
        return Err(Error::DiskDecodeFailed { track });
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
pub(super) fn encode_6and2(buf: &mut Vec<u8>, data: &[u8]) {
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
