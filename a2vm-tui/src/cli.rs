use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(
    about = "Terminal frontend for the A2VM Apple II emulator",
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
}

pub(crate) fn parse() -> CliArgs {
    CliArgs::parse()
}
