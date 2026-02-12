use std::path::PathBuf;

use clap::{error::ErrorKind, CommandFactory, Parser, ValueEnum};

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
    after_help = "Notes:\n  - --rom is required unless A2VM_ROM is set.\n  - --disk can be passed up to two times (drive 1, then drive 2).\n  - --fast-disk enables fast-disk mode for all mounted drives."
)]
pub(crate) struct CliArgs {
    #[arg(
        long,
        env = "A2VM_ROM",
        value_name = "FILE",
        help = "Apple II/II+ ROM file (12K or 20K)"
    )]
    pub(crate) rom: PathBuf,
    #[arg(long, value_name = "FILE", help = ".dsk disk image (143360 bytes)")]
    pub(crate) disk: Vec<PathBuf>,
    #[arg(
        long = "fast-disk",
        default_value_t = false,
        help = "Enable DOS 3.3 RWTS fast path for all mounted drives"
    )]
    pub(crate) fast_disk: bool,
    #[arg(
        long = "color-mode",
        value_enum,
        value_name = "MODE",
        default_value_t = CliColorMode::Color,
        help = "Display mode: color, mono, or mono-scanlines"
    )]
    pub(crate) color_mode: CliColorMode,
    #[arg(
        long = "noise",
        default_value_t = false,
        help = "Enable realistic mechanical noise simulation"
    )]
    pub(crate) noise: bool,
}

pub(crate) fn parse() -> CliArgs {
    let args = CliArgs::parse();
    if args.disk.len() > 2 {
        CliArgs::command()
            .error(
                ErrorKind::TooManyValues,
                "at most two --disk values are supported",
            )
            .exit();
    }
    args
}
