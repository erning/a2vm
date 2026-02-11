use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use a2vm_core::video::DisplayColorMode;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum CliColorMode {
    Color,
    Mono,
    MonoScanlines,
}

impl From<CliColorMode> for DisplayColorMode {
    fn from(value: CliColorMode) -> Self {
        match value {
            CliColorMode::Color => DisplayColorMode::Color,
            CliColorMode::Mono => DisplayColorMode::Monochrome,
            CliColorMode::MonoScanlines => DisplayColorMode::MonochromeScanlines,
        }
    }
}

#[derive(Parser)]
#[command(
    about = "Graphical frontend for the A2VM Apple II emulator",
    after_help = "Notes:\n  - --rom is required unless A2VM_ROM is set.\n  - --disk and --fast-disk are mutually exclusive.\n  - --fast-disk is only for DOS 3.3 formatted disks."
)]
pub(crate) struct CliArgs {
    #[arg(
        long,
        env = "A2VM_ROM",
        value_name = "FILE",
        help = "Apple II/II+ ROM file (12K or 20K)"
    )]
    pub(crate) rom: PathBuf,
    #[arg(
        long,
        value_name = "FILE",
        conflicts_with = "fast_disk",
        help = ".dsk disk image (143360 bytes)"
    )]
    pub(crate) disk: Option<PathBuf>,
    #[arg(
        long = "fast-disk",
        value_name = "FILE",
        conflicts_with = "disk",
        help = ".dsk image with DOS 3.3 RWTS trap for instant sector reads"
    )]
    pub(crate) fast_disk: Option<PathBuf>,
    #[arg(
        long = "color-mode",
        value_enum,
        value_name = "MODE",
        default_value_t = CliColorMode::Color,
        help = "Display mode: color, mono, or mono-scanlines"
    )]
    pub(crate) color_mode: CliColorMode,
}

pub(crate) fn parse() -> CliArgs {
    CliArgs::parse()
}
