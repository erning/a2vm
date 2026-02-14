use crate::error::{Error, Result};

use super::codec::{decode_nibblized_track, nibblize_disk, nibblize_track};
use super::{DiskII, DSK_SIZE, SECTORS_PER_TRACK, TRACK_COUNT};

#[cfg(feature = "std")]
use std::path::Path;

impl DiskII {
    /// Load a .dsk image from bytes into a drive (0 or 1).
    pub fn load_disk_bytes(
        &mut self,
        data: &[u8],
        drive: usize,
        write_protected: bool,
    ) -> Result<()> {
        if drive >= 2 {
            return Err(Error::InvalidDiskLocation {
                drive,
                track: 0,
                sector: 0,
            });
        }
        if data.len() != DSK_SIZE {
            return Err(Error::InvalidDiskSize {
                expected: DSK_SIZE,
                actual: data.len(),
            });
        }
        let drv = &mut self.drives[drive];
        nibblize_disk(data, &mut drv.nibble_data);
        let mut raw = Box::new([0u8; DSK_SIZE]);
        raw.copy_from_slice(data);
        drv.raw_data = Some(raw);
        drv.has_disk = true;
        drv.write_protected = write_protected;
        drv.byte_position = 0;
        drv.dirty = false;
        drv.dirty_tracks.fill(false);
        self.last_error = None;
        Ok(())
    }

    #[cfg(feature = "std")]
    /// Load a .dsk image from file into a drive (0 or 1).
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
        let write_protected = std::fs::metadata(path)
            .map(|meta| meta.permissions().readonly())
            .unwrap_or(true);

        self.load_disk_bytes(&data, drive, write_protected)?;
        self.drives[drive].image_path = Some(path.to_path_buf());
        Ok(())
    }

    pub fn write_sector_raw(
        &mut self,
        drive: usize,
        track: u8,
        sector: u8,
        data: &[u8; 256],
    ) -> Result<()> {
        if drive >= 2 || track as usize >= TRACK_COUNT || sector as usize >= SECTORS_PER_TRACK {
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
        let offset = (track as usize * SECTORS_PER_TRACK + sector as usize) * 256;
        raw[offset..offset + 256].copy_from_slice(data);
        nibblize_track(
            &raw[..],
            &mut drv.nibble_data[track as usize],
            track as usize,
        );

        #[cfg(feature = "std")]
        if let Some(path) = drv.image_path.as_deref() {
            std::fs::write(path, &raw[..])?;
        }

        drv.dirty = false;
        drv.dirty_tracks[track as usize] = false;
        self.last_error = None;
        Ok(())
    }

    /// Read a raw 256-byte sector from the loaded .dsk image.
    /// Returns `None` if no raw data is available or track/sector is out of range.
    pub fn read_sector_raw(&self, drive: usize, track: u8, sector: u8) -> Option<[u8; 256]> {
        if drive >= 2 || track as usize >= TRACK_COUNT || sector as usize >= SECTORS_PER_TRACK {
            return None;
        }
        let raw = self.drives[drive].raw_data.as_ref()?;
        let offset = (track as usize * SECTORS_PER_TRACK + sector as usize) * 256;
        let mut buf = [0u8; 256];
        buf.copy_from_slice(&raw[offset..offset + 256]);
        Some(buf)
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
        let dirty_tracks: Vec<usize> = drv
            .dirty_tracks
            .iter()
            .enumerate()
            .filter_map(|(track, dirty)| dirty.then_some(track))
            .collect();
        if dirty_tracks.is_empty() {
            drv.dirty = false;
            return Ok(());
        }

        for &track in &dirty_tracks {
            decode_nibblized_track(&drv.nibble_data[track], raw, track)?;
        }

        #[cfg(feature = "std")]
        if let Some(path) = drv.image_path.as_deref() {
            std::fs::write(path, &raw[..])?;
        }

        for track in dirty_tracks {
            drv.dirty_tracks[track] = false;
        }
        drv.dirty = drv.dirty_tracks.iter().any(|&dirty| dirty);
        self.last_error = None;

        Ok(())
    }

    /// Check if a drive has unsynchronized (dirty) nibblized data.
    pub fn is_dirty(&self, drive: usize) -> bool {
        if drive >= 2 {
            return false;
        }
        self.drives[drive].dirty
    }

    /// Flush a specific drive's nibble data to the raw image and persist to disk.
    pub fn flush_drive(&mut self, drive: usize) -> Result<()> {
        self.sync_nibble_to_raw(drive)
    }

    /// Flush all drives' nibble data to raw images and persist to disk.
    pub fn flush_all_drives(&mut self) -> Result<()> {
        for drive in 0..2 {
            self.sync_nibble_to_raw(drive)?;
        }
        Ok(())
    }

    /// Export raw disk data for a drive as bytes.
    pub fn export_disk_bytes(&self, drive: usize) -> Option<&[u8]> {
        if drive >= 2 {
            return None;
        }
        let drv = &self.drives[drive];
        if !drv.has_disk {
            return None;
        }
        drv.raw_data.as_ref().map(|raw| raw.as_slice())
    }
}
