//! Development CLI for OpenCNPJ Intelligence artifacts and experiments.

use clap::{CommandFactory, Parser, Subcommand, error::ErrorKind};

#[derive(Parser)]
#[command(name = "opencnpj-intel", about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Build reference catalogues.
    Catalog,
    /// Build aggregate artifacts.
    Agg,
    /// Compare aggregate results with engine counts.
    DiffAgg,
    /// Execute a query written in the intermediate language.
    Run,
    /// Generate synthetic evaluation data.
    Synth,
    /// Compile language packs.
    Lang,
    /// Train experimental classifiers.
    Train,
    /// Evaluate interpreters on labelled sets.
    Eval,
    /// Benchmark question interpretation.
    BenchNlu,
    /// Benchmark aggregate query execution.
    BenchAgg,
}

fn main() -> std::io::Result<()> {
    if Cli::parse().command.is_some() {
        Cli::command().error(ErrorKind::InvalidSubcommand, "subcommand not implemented").exit();
    }
    Cli::command().print_help()
}
