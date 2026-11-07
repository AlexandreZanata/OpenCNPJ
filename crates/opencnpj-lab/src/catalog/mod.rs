mod cnae;
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
    /// Build the CNAE hierarchy, legacy RFB codes, sector aliases and reports (WP-1.3).
    Cnae {
        /// Directory with ibge-subclasses.json, concla-2.3.zip and concla-2.2.zip.
        #[arg(long)]
        raw: PathBuf,
        /// Verified `catalog rfb` output directory (rfb_cnaes.tsv, used_codes.tsv).
        #[arg(long)]
        rfb: PathBuf,
        #[arg(long, default_value = "crates/opencnpj-insights/catalog/sectors.toml")]
        sectors: PathBuf,
        /// New output directory (must not exist).
        #[arg(long)]
        out: PathBuf,
    },
}

pub(crate) fn run(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Rfb { snapshot, out } => rfb::extract(&snapshot, &out),
        Command::Municipalities { snapshot, raw, out } => municipalities::build(&snapshot, &raw, &out),
        Command::Cnae { raw, rfb, sectors, out } => cnae::build(&raw, &rfb, &sectors, &out),
    }
}
