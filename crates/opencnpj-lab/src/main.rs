//! Development CLI for OpenCNPJ Intelligence artifacts and experiments.

mod aggregate;
mod assistant;
mod backfill;
mod catalog;
mod contracts;
mod eval;
mod official;
mod oracle;
mod query;
mod questions;
mod serve;
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
    /// Inspect and verify official source assets.
    Sources {
        #[command(subcommand)]
        command: official::SourcesCommand,
    },
    /// Import, query and ask about official source-backed indicators.
    Official {
        #[command(subcommand)]
        command: official::OfficialCommand,
    },
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
    /// Interpret a Portuguese question and optionally execute it on real data.
    Ask(questions::AskArgs),
    /// Answer one chat turn (rendered blocks, extras, file offer) and optionally write the file.
    Answer(serve::AnswerArgs),
    /// Serve the chat page and its API with every asset loaded once.
    Serve(serve::ServeArgs),
    /// Check answer contracts on evaluation sets (and, with data, against the records).
    Contracts(contracts::ContractArgs),
    /// Interpret every question and audit predictions and backend results.
    EvalQuestions(questions::EvalArgs),
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
        Some(Command::Sources { command }) => official::run_sources(command),
        Some(Command::Official { command }) => official::run_official(command),
        Some(Command::Ask(args)) => questions::ask(args),
        Some(Command::Answer(args)) => serve::answer(args),
        Some(Command::Serve(args)) => serve::serve(args),
        Some(Command::EvalQuestions(args)) => questions::evaluate(args),
        Some(Command::Contracts(args)) => contracts::run(args),
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
