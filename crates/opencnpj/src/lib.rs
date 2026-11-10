//! Library behind the `opencnpj` binary: HTTP API, admin suite, accounts, quotas,
//! download/update and benchmark harness. Split from `main.rs` so integration tests can
//! drive the router in-process.

pub mod accounts;
pub mod accounts_cli;
pub mod admin;
pub mod api;
pub mod assistant;
pub mod auth_api;
pub mod autoupdate;
pub mod bench;
pub mod download;
pub mod gate;
pub mod loadtest;
pub mod mail;
pub mod manage;
pub mod metrics;
pub mod output;
pub mod params;

use anyhow::Result;
use clap::Args;
use opencnpj_store::search::Query;

#[derive(Args, Clone, Default)]
pub struct QueryArgs {
    /// Name query (razão social / nome fantasia).
    #[arg(long)]
    pub q: Option<String>,
    /// any | razao | fantasia
    #[arg(long, default_value = "any")]
    pub field: String,
    #[arg(long)]
    pub fuzzy: bool,
    /// Any API parameter, repeatable: --filter uf=SP --filter cnae=47 --filter data_inicio_atividade_de=2024-01-01
    #[arg(long = "filter", value_parser = parse_filter)]
    pub filters: Vec<(String, String)>,
    #[arg(long, default_value_t = 10)]
    pub limit: usize,
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
}

fn parse_filter(s: &str) -> Result<(String, String), String> {
    let (k, v) = s.split_once('=').ok_or("expected key=value")?;
    Ok((k.to_string(), v.to_string()))
}

pub fn default_threads() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get())
}

/// CLI flags → the same parameter grammar as the HTTP API (see `params`).
pub fn to_query(a: &QueryArgs) -> Result<Query> {
    let mut pairs: Vec<(String, String)> = a.filters.clone();
    if let Some(q) = &a.q {
        pairs.push(("q".into(), q.clone()));
    }
    pairs.push(("field".into(), a.field.clone()));
    pairs.push(("fuzzy".into(), if a.fuzzy { "true" } else { "auto" }.into()));
    pairs.push(("limit".into(), a.limit.to_string()));
    pairs.push(("offset".into(), a.offset.to_string()));
    params::parse(&pairs, params::Caps { max_limit: usize::MAX, max_offset: usize::MAX }).map_err(|e| anyhow::anyhow!(e))
}

/// Ascending copy (blocks are decompressed once when ids are sorted).
pub fn sorted(ids: &[u32]) -> Vec<u32> {
    let mut v = ids.to_vec();
    v.sort_unstable();
    v
}
