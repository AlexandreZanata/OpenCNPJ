//! Development CLI for OpenCNPJ Intelligence artifacts and experiments.

mod aggregate;
mod catalog;
mod eval;
mod query;
mod synth;

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
    Agg {
        #[command(subcommand)]
        command: aggregate::Command,
    },
    /// Compare aggregate results with engine counts.
    DiffAgg(aggregate::DiffArgs),
    /// Execute a query written in the intermediate language.
    Run,
    /// Execute the golden IR set with aggregates and the engine and compare the results.
    DiffExec(query::DiffArgs),
    /// Benchmark aggregate and engine execution on the golden set.
    BenchExec(query::DiffArgs),
    /// Count the golden set through a running engine API.
    RemoteCount(query::RemoteArgs),
    /// Generate synthetic evaluation data.
    Synth(synth::Args),
    /// Compile language packs.
    Lang,
    /// Train experimental classifiers.
    Train,
    /// Evaluate predictions on a frozen labelled set.
    Eval(eval::EvalArgs),
    /// Benchmark question interpretation.
    BenchNlu(eval::BenchNluArgs),
    /// Benchmark aggregate query execution.
    BenchAgg(query::DiffArgs),
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Some(Command::Catalog { command }) => catalog::run(command),
        Some(Command::Agg { command }) => aggregate::run(command),
        Some(Command::DiffAgg(args)) => aggregate::diff(args),
        Some(Command::DiffExec(args)) => query::diff(args),
        Some(Command::BenchExec(args)) => query::bench(args),
        Some(Command::RemoteCount(args)) => query::remote_count(args),
        Some(Command::Synth(args)) => synth::run(args),
        Some(Command::Eval(args)) => eval::run_eval(args),
        Some(Command::BenchNlu(args)) => eval::run_bench_nlu(args),
        Some(Command::BenchAgg(args)) => query::bench(args),
        Some(_) => Cli::command().error(ErrorKind::InvalidSubcommand, "subcommand not implemented").exit(),
        None => Ok(Cli::command().print_help()?),
    }
}
