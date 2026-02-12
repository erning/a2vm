use clap::{error::ErrorKind, CommandFactory, Parser, ValueEnum};

use a2vm_core::video::DisplayColorMode;
use a2vm_oxide::SharedArgs;

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
    after_help = "Notes:\n  - If --rom is not specified, uses embedded Apple II+ ROM.\n  - --disk can be passed up to two times (drive 1, then drive 2).\n  - --fast-disk enables fast-disk mode for all mounted drives."
)]
pub(crate) struct CliArgs {
    #[command(flatten)]
    pub(crate) shared: SharedArgs,

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
    let args = CliArgs::parse();
    if args.shared.disk.len() > 2 {
        CliArgs::command()
            .error(
                ErrorKind::TooManyValues,
                "at most two --disk values are supported",
            )
            .exit();
    }
    args
}
