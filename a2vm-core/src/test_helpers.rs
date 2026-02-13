//! Shared test helpers for a2vm-core tests.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// RAII guard for a temporary file that deletes itself on drop.
pub struct TempFile {
    path: PathBuf,
}

impl TempFile {
    /// Get the path to the temporary file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Create a temporary file with the given contents.
pub fn create_temp_file(bytes: &[u8], prefix: &str, suffix: &str) -> TempFile {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    path.push(format!("a2vm-test-{prefix}-{nanos}-{suffix}"));
    fs::write(&path, bytes).unwrap();
    TempFile { path }
}

/// Create a temporary ROM file.
pub fn create_temp_rom(bytes: &[u8]) -> TempFile {
    create_temp_file(bytes, "rom", "bin")
}

/// Create a temporary disk file.
pub fn create_temp_disk(bytes: &[u8]) -> TempFile {
    create_temp_file(bytes, "disk", "dsk")
}
