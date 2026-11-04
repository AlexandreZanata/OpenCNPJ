//! `opencnpj` — single binary: download, build, verify, query, serve and benchmark.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use opencnpj::{QueryArgs, api, bench, default_threads, download, output, sorted, to_query};
use opencnpj_ingest::build::{self, BuildConfig, Profile};
use opencnpj_store::Snapshot;
use opencnpj_store::compress::BlockCodec;
use opencnpj_store::search::{self, Limits};

#[derive(Parser)]
#[command(name = "opencnpj", version, about = "Compact, offline search engine for Brazilian CNPJ open data")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Download the latest (or a given) monthly RFB release into DEST/YYYY-MM.
    Download(download::DownloadArgs),
    /// Download the latest release, rebuild if sources changed, verify and publish atomically.
    Update(download::UpdateArgs),
    /// Point `current` back to the previous snapshot.
    Rollback {
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
    },
    /// Build a `.ocnpj` snapshot from a directory of official ZIPs.
    Build(BuildArgs),
    /// Check every checksum, decode every block, and optionally compare against the sources.
    Verify {
        snapshot: PathBuf,
        /// Re-read the source ZIPs and compare row multisets (semantic reconstruction test).
        #[arg(long)]
        sources: Option<PathBuf>,
        #[arg(long, default_value_t = default_threads())]
        workers: usize,
    },
    /// Print the snapshot manifest and section sizes.
    Info {
        snapshot: PathBuf,
        /// Also report terms and postings bytes per index field.
        #[arg(long)]
        index: bool,
    },
    /// Look up a CNPJ (14 chars) or a CNPJ root (8 chars).
    Get { snapshot: PathBuf, id: String },
    /// Search by name and/or filters.
    Search {
        snapshot: PathBuf,
        #[command(flatten)]
        q: QueryArgs,
    },
    /// Stream matching establishments as NDJSON or CSV.
    Export {
        snapshot: PathBuf,
        #[command(flatten)]
        q: QueryArgs,
        #[arg(long, default_value = "ndjson")]
        format: String,
        #[arg(long, default_value_t = u64::MAX)]
        max: u64,
    },
    /// Rewrite one table as RFB-dialect CSV (semantic reconstruction, not byte-identical).
    Reconstruct {
        snapshot: PathBuf,
        #[arg(long)]
        table: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Run the HTTP API.
    Serve(api::ServeArgs),
    /// Reproducible query benchmark against a snapshot.
    Bench(bench::BenchArgs),
}

#[derive(Args)]
struct BuildArgs {
    /// Directory with the official ZIPs (e.g. data/raw/2026-10).
    #[arg(long)]
    input: PathBuf,
    /// Output snapshot file.
    #[arg(long)]
    output: PathBuf,
    /// Competence `YYYY-MM` (defaults to the input directory name).
    #[arg(long)]
    competence: Option<String>,
    /// compact | balanced | fast
    #[arg(long, default_value = "balanced")]
    profile: String,
    /// Override block codec: none, lz4, zstdN.
    #[arg(long)]
    codec: Option<String>,
    /// Override uncompressed block target in KiB.
    #[arg(long)]
    block_kib: Option<usize>,
    /// Override zstd dictionary size in KiB (0 disables).
    #[arg(long)]
    dict_kib: Option<usize>,
    /// Temporary directory (defaults to OUTPUT.tmp).
    #[arg(long)]
    tmp: Option<PathBuf>,
    #[arg(long, default_value_t = default_threads())]
    workers: usize,
    #[arg(long, default_value_t = 4)]
    readers: usize,
    /// Keep phase A output for `--from-partition` rebuilds.
    #[arg(long)]
    keep_partition: bool,
    /// Reuse the phase A output in `--tmp` instead of reading the ZIPs.
    #[arg(long)]
    from_partition: bool,
    /// Only build buckets with id % N == 0 (deterministic sample).
    #[arg(long)]
    sample_every: Option<u32>,
    /// Experiment: store every column as plain text (no typed codecs, no dictionaries).
    #[arg(long)]
    plain_columns: bool,
    /// Write the build report as JSON.
    #[arg(long)]
    report: Option<PathBuf>,
}

fn log(msg: &str) {
    eprintln!("[{}] {msg}", chrono_like_now());
}

fn chrono_like_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    format!("{:02}:{:02}:{:02}Z", secs / 3600 % 24, secs / 60 % 60, secs % 60)
}

fn run_build(a: BuildArgs) -> Result<()> {
    let mut profile = Profile::preset(&a.profile).with_context(|| format!("unknown profile {}", a.profile))?;
    if let Some(c) = &a.codec {
        profile.codec = BlockCodec::parse(c).with_context(|| format!("bad codec {c}"))?;
        profile.name = "custom".into();
    }
    if let Some(k) = a.block_kib {
        profile.block_target = k << 10;
        profile.name = "custom".into();
    }
    if let Some(k) = a.dict_kib {
        profile.zstd_dict_bytes = k << 10;
        profile.name = "custom".into();
    }
    let competence = match a.competence.clone().or_else(|| build::competence_from_dir(&a.input)) {
        Some(c) => c,
        None => bail!("--competence YYYY-MM is required when the input directory is not named YYYY-MM"),
    };
    let tmp = a.tmp.clone().unwrap_or_else(|| a.output.with_extension("ocnpj.work"));
    let cfg = BuildConfig {
        input: a.input.clone(),
        output: a.output.clone(),
        tmp: tmp.clone(),
        competence,
        profile,
        workers: a.workers,
        readers: a.readers,
        keep_partition: a.keep_partition || a.from_partition,
        sample_every: a.sample_every,
        plain_columns: a.plain_columns,
    };
    let t0 = Instant::now();
    let mut report = if a.from_partition {
        let state = build::load_partition(&tmp)?;
        build::build_from_partition(&state, &cfg, &log)?
    } else {
        build::build(&cfg, &log)?
    };
    report.timings_s.insert("total_wall".into(), t0.elapsed().as_secs_f64());
    log(&format!(
        "snapshot {} ({:.3} GiB, sha256 {}) in {:.1}s: {} companies, {} documents, {} blocks, {} terms",
        report.output.display(),
        report.file_bytes as f64 / f64::from(1 << 30),
        report.sha256,
        t0.elapsed().as_secs_f64(),
        report.counts.companies,
        report.counts.documents,
        report.counts.blocks,
        report.counts.terms
    ));
    let json = serde_json::to_string_pretty(&report)?;
    match &a.report {
        Some(p) => std::fs::write(p, json)?,
        None => println!("{json}"),
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Download(a) => download::run(a),
        Cmd::Build(a) => run_build(a),
        Cmd::Update(a) => download::update(a, &log),
        Cmd::Rollback { data_dir } => {
            let p = download::rollback(&data_dir)?;
            log(&format!("current → {p}"));
            Ok(())
        }
        Cmd::Verify { snapshot, sources, workers } => {
            let t0 = Instant::now();
            let snap = Snapshot::open(&snapshot)?;
            let mut last = 0;
            let (companies, docs) = snap.verify(|i, n| {
                let pct = i * 100 / n.max(1);
                if pct >= last + 10 {
                    last = pct;
                    log(&format!("verify: {pct}% of blocks"));
                }
            })?;
            log(&format!("structure ok: {companies} companies, {docs} documents, {:.1}s", t0.elapsed().as_secs_f64()));
            let sha = opencnpj_ingest::source::sha256_file(&snapshot)?;
            let sidecar = snapshot.with_extension("ocnpj.sha256");
            match std::fs::read_to_string(&sidecar) {
                Ok(s) if s.split_whitespace().next() == Some(sha.as_str()) => log(&format!("sha256 ok: {sha}")),
                Ok(_) => bail!("sha256 mismatch against {}", sidecar.display()),
                Err(_) => log(&format!("sha256 {sha} (no sidecar to compare)")),
            }
            if let Some(dir) = sources {
                output::verify_against_sources(&snap, &dir, workers, &log)?;
            }
            log(&format!("verify passed in {:.1}s", t0.elapsed().as_secs_f64()));
            Ok(())
        }
        Cmd::Info { snapshot, index } => {
            let snap = Snapshot::open(&snapshot)?;
            let mut v = output::info(&snap);
            if index {
                v["index"] = output::index_breakdown(&snap);
            }
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(())
        }
        Cmd::Get { snapshot, id } => {
            let snap = Snapshot::open(&snapshot)?;
            let t0 = Instant::now();
            let v = output::lookup(&snap, &id)?;
            let el = t0.elapsed();
            match v {
                Some(v) => println!("{}", serde_json::to_string_pretty(&v)?),
                None => bail!("not found"),
            }
            log(&format!("lookup took {:.3} ms", el.as_secs_f64() * 1e3));
            Ok(())
        }
        Cmd::Search { snapshot, q } => {
            let snap = Snapshot::open(&snapshot)?;
            let query = to_query(&q)?;
            let t0 = Instant::now();
            let r = search::search(&snap, &query, &Limits::DEFAULT)?;
            let t_search = t0.elapsed();
            let docs = snap.docs(&sorted(&r.docs))?;
            let el = t0.elapsed();
            log(&format!("search {:.3} ms, fetch {:.3} ms", t_search.as_secs_f64() * 1e3, (el - t_search).as_secs_f64() * 1e3));
            let v = output::search_json(&snap, &r, docs, false);
            println!("{}", serde_json::to_string_pretty(&v)?);
            log(&format!("search took {:.3} ms", el.as_secs_f64() * 1e3));
            Ok(())
        }
        Cmd::Export { snapshot, q, format, max } => {
            let snap = Snapshot::open(&snapshot)?;
            let mut query = to_query(&q)?;
            query.limit = usize::MAX;
            let mut out = std::io::BufWriter::new(std::io::stdout().lock());
            let n = output::export(&snap, &query, &format, max, &mut out)?;
            log(&format!("exported {n} rows"));
            Ok(())
        }
        Cmd::Reconstruct { snapshot, table, out } => {
            let snap = Snapshot::open(&snapshot)?;
            let n = output::reconstruct(&snap, &table, &out)?;
            log(&format!("wrote {n} rows to {}", out.display()));
            Ok(())
        }
        Cmd::Serve(a) => api::run(a),
        Cmd::Bench(a) => bench::run(a),
    }
}
