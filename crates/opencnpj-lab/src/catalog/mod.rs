mod rfb;

use std::path::PathBuf;

use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Extract RFB reference tables and observed codes from a snapshot.
    Rfb {
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Rfb { snapshot, out } => rfb::extract(&snapshot, &out),
    }
}
