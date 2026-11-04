//! Development CLI for OpenCNPJ Intelligence artifacts and experiments.

mod catalog;

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
    Catalog {
        #[command(subcommand)]
        command: catalog::Command,
    },
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

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Some(Command::Catalog { command }) => catalog::run(command),
        Some(_) => Cli::command().error(ErrorKind::InvalidSubcommand, "subcommand not implemented").exit(),
        None => Ok(Cli::command().print_help()?),
    }
}
