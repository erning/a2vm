use std::path::PathBuf;

use super::{DSK_SIZE, NIBBLE_TRACK_SIZE, TRACK_COUNT};

/// A single floppy drive.
pub(super) struct Drive {
    pub(super) nibble_data: Box<[[u8; NIBBLE_TRACK_SIZE]; TRACK_COUNT]>,
    pub(super) raw_data: Option<Box<[u8; DSK_SIZE]>>,
    pub(super) image_path: Option<PathBuf>,
    pub(super) byte_position: usize,
    pub(super) has_disk: bool,
    pub(super) write_protected: bool,
    pub(super) dirty: bool,
    pub(super) dirty_tracks: [bool; TRACK_COUNT],
}

impl Drive {
    pub(super) fn new() -> Self {
        Self {
            nibble_data: Box::new([[0u8; NIBBLE_TRACK_SIZE]; TRACK_COUNT]),
            raw_data: None,
            image_path: None,
            byte_position: 0,
            has_disk: false,
            write_protected: true,
            dirty: false,
            dirty_tracks: [false; TRACK_COUNT],
        }
    }
}
