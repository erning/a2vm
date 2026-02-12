use clap::{error::ErrorKind, CommandFactory, Parser};

use a2vm_oxide::SharedArgs;

#[derive(Parser)]
#[command(
    about = "Terminal frontend for the A2VM Apple II emulator",
    after_help = "Notes:\n  - If --rom is not specified, uses embedded Apple II+ ROM.\n  - --disk can be passed up to two times (drive 1, then drive 2).\n  - --fast-disk enables fast-disk mode for all mounted drives."
)]
pub(crate) struct CliArgs {
    #[command(flatten)]
    pub(crate) shared: SharedArgs,
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
