mod municipalities;
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
    /// Build the municipality and geography catalogue from verified local inputs.
    Municipalities {
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long)]
        raw: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Rfb { snapshot, out } => rfb::extract(&snapshot, &out),
        Command::Municipalities { snapshot, raw, out } => municipalities::build(&snapshot, &raw, &out),
    }
}
