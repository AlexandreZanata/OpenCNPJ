//! Discovery, import and query commands for official source-backed indicators.

use std::{collections::BTreeMap, path::PathBuf};

use anyhow::Result;
use clap::Subcommand;
use opencnpj_insights::official::{
    model::DatasetManifest,
    registry::Registry,
    store::{StoreSet, sha256_file},
};
use serde::Serialize;

#[derive(Subcommand)]
pub(crate) enum SourcesCommand {
    /// List the compiled source and dataset registry.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show registry states and locally published indicator assets.
    Status {
        /// Directory containing .ocind stores.
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Verify hashes, SQLite integrity and manifest row accounting.
    Verify {
        #[arg(long)]
        dir: PathBuf,
    },
}

pub(crate) fn run_sources(command: SourcesCommand) -> Result<()> {
    match command {
        SourcesCommand::List { json } => list(json),
        SourcesCommand::Status { dir, json } => status(dir.as_deref(), json),
        SourcesCommand::Verify { dir } => verify(&dir),
    }
}

#[derive(Serialize)]
struct SourceRow {
    source_id: String,
    dataset_id: String,
    title: String,
    publisher: String,
    state: String,
    licence: String,
    licence_status: String,
    licence_url: String,
    attribution: String,
    cadence: String,
    coverage: String,
    imported: bool,
    rows: Option<u64>,
    period_from: Option<String>,
    period_to: Option<String>,
    raw_sha256: Option<String>,
}

fn rows(registry: &Registry, manifests: &[&DatasetManifest]) -> Vec<SourceRow> {
    let published = manifests.iter().map(|manifest| (manifest.dataset_id.as_str(), *manifest)).collect::<BTreeMap<_, _>>();
    registry
        .datasets()
        .map(|dataset| {
            let source = registry.source(&dataset.source_id).expect("validated registry source");
            let manifest = published.get(dataset.id.as_str());
            SourceRow {
                source_id: dataset.source_id.clone(),
                dataset_id: dataset.id.clone(),
                title: dataset.title.clone(),
                publisher: source.publisher.clone(),
                state: if manifest.is_some() { "queryable".to_string() } else { dataset.state.clone() },
                licence: source.licence.clone(),
                licence_status: source.licence_status.clone(),
                licence_url: source.licence_url.clone(),
                attribution: source.attribution.clone(),
                cadence: dataset.cadence.clone(),
                coverage: dataset.coverage.clone(),
                imported: manifest.is_some(),
                rows: manifest.map(|value| value.accepted_rows),
                period_from: manifest.and_then(|value| value.period_from.clone()),
                period_to: manifest.and_then(|value| value.period_to.clone()),
                raw_sha256: manifest.map(|value| value.raw_sha256.clone()),
            }
        })
        .collect()
}

fn list(json: bool) -> Result<()> {
    let registry = Registry::builtin()?;
    let rows = rows(&registry, &[]);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    println!("source\tdataset\tstate\tlicence\tcadence\tcoverage");
    for row in rows {
        println!(
            "{}\t{}\t{}\t{} ({})\t{}\t{}",
            row.source_id, row.dataset_id, row.state, row.licence_status, row.licence_url, row.cadence, row.coverage
        );
    }
    println!("\nSources: {}", registry.sources().count());
    Ok(())
}

fn status(dir: Option<&std::path::Path>, json: bool) -> Result<()> {
    let registry = Registry::builtin()?;
    let stores = match dir {
        Some(dir) if dir.exists() => Some(StoreSet::open_dir(dir)?),
        Some(dir) => {
            eprintln!("{} does not exist; showing registry states only", dir.display());
            None
        }
        None => None,
    };
    let manifests =
        stores.as_ref().map(|stores| stores.stores().iter().map(|store| store.manifest()).collect::<Vec<_>>()).unwrap_or_default();
    let rows = rows(&registry, &manifests);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!("source\tdataset\tstate\timported\trows\tperiod\tlicence");
        for row in &rows {
            let period = match (&row.period_from, &row.period_to) {
                (Some(from), Some(to)) => format!("{from}..{to}"),
                _ => "-".into(),
            };
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                row.source_id,
                row.dataset_id,
                row.state,
                if row.imported { "yes" } else { "no" },
                row.rows.map(|value| value.to_string()).unwrap_or_else(|| "-".into()),
                period,
                row.licence_status
            );
        }
    }
    Ok(())
}

fn verify(dir: &std::path::Path) -> Result<()> {
    let stores = StoreSet::open_dir(dir)?;
    stores.verify()?;
    for store in stores.stores() {
        let manifest = store.manifest();
        println!(
            "{}\t{}\trows={}\tperiod={:?}..{:?}\tsha256={}",
            store.path().display(),
            manifest.dataset_id,
            manifest.accepted_rows,
            manifest.period_from,
            manifest.period_to,
            sha256_file(store.path())?
        );
    }
    Ok(())
}
