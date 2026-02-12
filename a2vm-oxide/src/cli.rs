use std::path::PathBuf;

use clap::Args;

/// Embedded Apple II+ ROM (20K).
pub const DEFAULT_ROM: &[u8] = include_bytes!("../../roms/apple2p.rom");

/// Shared arguments for all A2VM frontends.
#[derive(Args)]
pub struct SharedArgs {
    #[arg(
        long,
        value_name = "FILE",
        help = "Apple II/II+ ROM file (12K or 20K). If not specified, uses embedded Apple II+ ROM."
    )]
    pub rom: Option<PathBuf>,

    #[arg(long, value_name = "FILE", help = ".dsk disk image (143360 bytes)")]
    pub disk: Vec<PathBuf>,

    #[arg(
        long = "fast-disk",
        default_value_t = false,
        help = "Enable DOS 3.3 RWTS fast path for all mounted drives"
    )]
    pub fast_disk: bool,

    #[arg(
        long = "noise",
        default_value_t = false,
        help = "Enable realistic mechanical noise simulation"
    )]
    pub noise: bool,
}

impl SharedArgs {
    /// Returns ROM data: from file if specified, otherwise embedded default.
    pub fn rom_data(&self) -> Result<Vec<u8>, std::io::Error> {
        match &self.rom {
            Some(path) => std::fs::read(path),
            None => Ok(DEFAULT_ROM.to_vec()),
        }
    }
}
