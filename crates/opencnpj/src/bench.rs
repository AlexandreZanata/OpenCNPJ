//! Reproducible end-to-end query benchmark.
//!
//! Workloads are sampled deterministically (seeded) from the snapshot itself, so every
//! run uses real, varied queries instead of repeating one hot key. Each query includes the
//! same work the API does (search + record fetch + JSON shaping). `--cold` evicts the
//! snapshot from the OS page cache (posix_fadvise DONTNEED, no root needed) and reopens it
//! before each category.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use anyhow::Result;
use opencnpj_core::schema::col;
use opencnpj_store::Snapshot;
use opencnpj_store::numeric::{NumField, Range};
use opencnpj_store::postings::Field;
use opencnpj_store::search::{self, Limits, Query, RangeFilter, TextField};
use serde::Serialize;
use serde_json::json;

use crate::output::{self, Privacy};

#[derive(clap::Args)]
pub struct BenchArgs {
    #[arg(long)]
    pub snapshot: PathBuf,
    /// Queries per category.
    #[arg(long, default_value_t = 1000)]
    pub queries: usize,
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
    /// Thread counts for the throughput runs.
    #[arg(long, value_delimiter = ',', default_value = "1,4,16")]
    pub threads: Vec<usize>,
    /// Evict the snapshot from the page cache before each category (cold-cache numbers).
    #[arg(long)]
    pub cold: bool,
    /// Output directory for bench.json, bench.csv and bench.md.
    #[arg(long, default_value = "bench-results")]
    pub out: PathBuf,
    #[arg(long)]
    pub label: Option<String>,
    /// Only run categories whose name contains this string.
    #[arg(long)]
    pub only: Option<String>,
    /// Override the prefix/fuzzy expansion budget (MiB of encoded postings).
    #[arg(long)]
    pub expansion_mib: Option<u64>,
    /// Write the sampled workload as API URLs (one file per category) and exit.
    #[arg(long)]
    pub write_urls: Option<PathBuf>,
    /// Base URL used by --write-urls.
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    pub base_url: String,
    /// Decoded-postings cache budget in MiB.
    #[arg(long)]
    pub cache_mib: Option<usize>,
}

#[derive(Clone)]
enum Work {
    Lookup(String),
    Search(Query),
    Stats(Query, Field),
    /// `/api/v1/cpf/{cpf}`: owner and masked-partner matches.
    Cpf(String),
    /// Yearly histogram of a date column over the query's candidates.
    Year(Query, NumField),
}

#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    pub category: String,
    pub threads: usize,
    pub cache: String,
    pub queries: usize,
    pub qps: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub mean_ms: f64,
    pub max_ms: f64,
    pub empty_results: usize,
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

const POPULAR: &[&str] = &[
    "COMERCIO",
    "SERVICOS",
    "TRANSPORTES",
    "CONSTRUCOES",
    "RESTAURANTE",
    "IGREJA EVANGELICA",
    "SUPERMERCADO",
    "PADARIA",
    "AUTO PECAS",
    "FARMACIA",
    "CONDOMINIO EDIFICIO",
    "ASSOCIACAO",
    "ACADEMIA",
    "BAR",
    "SALAO BELEZA",
    "OFICINA MECANICA",
    "MERCADO",
    "LOJA",
    "CLINICA",
    "ESCOLA",
];

fn q(text: Option<String>, filters: Vec<(Field, Vec<String>)>, fuzzy: bool) -> Query {
    Query { text, text_field: TextField::Any, filters, fuzzy, offset: 0, limit: 20, ..Default::default() }
}

fn workloads(snap: &Snapshot, n: usize, seed: u64) -> Result<Vec<(&'static str, Vec<Work>)>> {
    let mut rng = Rng(seed);
    let mut ids: Vec<u32> = (0..n * 2).map(|_| (rng.next() % u64::from(snap.doc_count())) as u32).collect();
    ids.sort_unstable();
    ids.dedup();
    let docs = snap.docs(&ids)?;
    let mut order: Vec<usize> = (0..docs.len()).collect();
    for i in (1..order.len()).rev() {
        order.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }
    let mut cats: BTreeMap<&'static str, Vec<Work>> = BTreeMap::new();
    for &i in &order {
        let d = &docs[i];
        let c = &d.company;
        let razao = c.empresas.first().map(|e| e[col::emp::RAZAO_SOCIAL].clone()).unwrap_or_default();
        let toks = opencnpj_core::text::tokens(&razao);
        cats.entry("lookup_cnpj_root").or_default().push(Work::Lookup(c.basico.clone()));
        if !toks.is_empty() {
            cats.entry("name_full").or_default().push(Work::Search(q(Some(format!("{razao} ")), vec![], false)));
        }
        if toks.len() >= 2 && toks[1].len() >= 3 {
            cats.entry("name_prefix_typeahead").or_default().push(Work::Search(q(
                Some(format!("{} {}", toks[0], &toks[1][..3])),
                vec![],
                false,
            )));
            let mut typo: Vec<char> = toks[0].chars().collect();
            if typo.len() >= 5 {
                // One substitution: edit distance 1.
                typo[2] = if typo[2] == 'X' { 'Y' } else { 'X' };
                let t: String = typo.into_iter().collect();
                cats.entry("name_fuzzy").or_default().push(Work::Search(q(Some(format!("{t} {} ", toks[1])), vec![], true)));
            }
        }
        let Some(e) = d.estab.map(|k| &c.estabelecimentos[k]) else { continue };
        cats.entry("lookup_cnpj").or_default().push(Work::Lookup(format!("{}{}{}", c.basico, e[col::est::ORDEM], e[col::est::DV])));
        let (uf, mun, cnae) = (e[col::est::UF].clone(), e[col::est::MUNICIPIO].clone(), e[col::est::CNAE_PRINCIPAL].clone());
        cats.entry("filter_selective").or_default().push(Work::Search(q(
            None,
            vec![(Field::Municipio, vec![mun.clone()]), (Field::Cnae, vec![cnae.clone()]), (Field::Situacao, vec!["02".into()])],
            false,
        )));
        cats.entry("filter_broad").or_default().push(Work::Search(q(
            None,
            vec![(Field::Uf, vec![uf.clone()]), (Field::Situacao, vec!["02".into()])],
            false,
        )));
        if let Some(t) = toks.first() {
            cats.entry("name_plus_uf").or_default().push(Work::Search(q(
                Some(format!("{t} ")),
                vec![(Field::Uf, vec![uf.clone()])],
                false,
            )));
        }
        cats.entry("stats_uf_by_cnae").or_default().push(Work::Stats(q(None, vec![(Field::Cnae, vec![cnae.clone()])], false), Field::Uf));
        // Features of format 1.2: identity, prefixes, CNAE hierarchy, ranges, sorting.
        if snap.numeric(NumField::InicioAtividade).is_some() {
            cats.entry("filter_cnpj_prefix")
                .or_default()
                .push(Work::Search(Query { cnpj_prefixes: vec![c.basico[..6].to_string()], ..q(None, vec![], false) }));
            if cnae.len() >= 2 {
                cats.entry("filter_cnae_division_uf_active").or_default().push(Work::Search(q(
                    None,
                    vec![
                        (Field::CnaeQualquer, vec![cnae[..2].to_string()]),
                        (Field::Uf, vec![uf.clone()]),
                        (Field::Situacao, vec!["02".into()]),
                    ],
                    false,
                )));
            }
            if let Some(v) = opencnpj_store::numeric::date_value(&e[col::est::DATA_INICIO]) {
                let y = opencnpj_store::numeric::date_year(v);
                let lo = opencnpj_store::numeric::date_value(&format!("{y}0101")).unwrap_or(1);
                let hi = opencnpj_store::numeric::date_value(&format!("{y}1231")).unwrap_or(u64::MAX);
                let range = RangeFilter { field: NumField::InicioAtividade, range: Some(Range { lo, hi }) };
                cats.entry("filter_opened_in_year_uf")
                    .or_default()
                    .push(Work::Search(Query { ranges: vec![range], ..q(None, vec![(Field::Uf, vec![uf.clone()])], false) }));
            }
            cats.entry("filter_mei_active_municipio").or_default().push(Work::Search(Query {
                ranges: vec![RangeFilter { field: NumField::ExclusaoMei, range: None }],
                ..q(None, vec![(Field::Mei, vec!["S".into()]), (Field::Municipio, vec![mun.clone()])], false)
            }));
            cats.entry("sort_newest_in_municipio").or_default().push(Work::Search(Query {
                sort: Some((NumField::InicioAtividade, true)),
                ..q(None, vec![(Field::Municipio, vec![mun.clone()])], false)
            }));
            cats.entry("stats_year_opened_by_uf")
                .or_default()
                .push(Work::Year(q(None, vec![(Field::Uf, vec![uf.clone()])], false), NumField::InicioAtividade));
            for t in &toks {
                if t.len() == 11 && t.bytes().all(|b| b.is_ascii_digit()) {
                    cats.entry("lookup_cpf").or_default().push(Work::Cpf(t.clone()));
                }
            }
        }
        for (cat, f) in [
            ("stats_municipio_by_uf", Field::Municipio),
            ("stats_cnae_by_uf", Field::Cnae),
            ("stats_cnae_secundaria_by_uf", Field::CnaeSecundaria),
        ] {
            cats.entry(cat).or_default().push(Work::Stats(q(None, vec![(Field::Uf, vec![uf.clone()])], false), f));
        }
    }
    for i in 0..n {
        cats.entry("name_popular").or_default().push(Work::Search(q(Some(POPULAR[i % POPULAR.len()].to_string()), vec![], false)));
    }
    Ok(cats
        .into_iter()
        .map(|(k, mut v)| {
            // Group-bys over a whole UF were seconds per query with postings only.
            v.truncate(if k.ends_with("_by_uf") { (n / 50).max(10) } else { n });
            (k, v)
        })
        .collect())
}

/// Executes one unit of work like the API would. Returns false when the result is empty.
fn execute(snap: &Snapshot, w: &Work, limits: &Limits) -> Result<bool> {
    Ok(match w {
        Work::Lookup(id) => output::lookup_with(snap, id, Privacy::default())?.is_some(),
        Work::Search(qr) => {
            let r = search::search(snap, qr, limits)?;
            let docs = snap.docs(&crate::sorted(&r.docs))?;
            let v = output::search_json(snap, &r, docs, false);
            std::hint::black_box(serde_json::to_vec(&v)?);
            r.total > 0
        }
        Work::Stats(qr, f) => !search::group_counts(snap, qr, *f, limits)?.is_empty(),
        Work::Cpf(cpf) => {
            let v = output::cpf_lookup(snap, cpf, 20, Privacy::default())?;
            std::hint::black_box(serde_json::to_vec(&v)?);
            v["titular"]["total"].as_u64().unwrap_or(0) > 0
        }
        Work::Year(qr, f) => {
            let set = search::candidates(snap, qr, limits)?.all;
            let col = snap.numeric(*f).ok_or_else(|| anyhow::anyhow!("no numeric column"))?;
            !col.histogram(snap.bytes(), set.as_ref(), opencnpj_store::numeric::date_year).is_empty()
        }
    })
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() as f64 - 1.0) * p).round() as usize]
}

fn measure(snap: &Snapshot, cat: &str, work: &[Work], threads: usize, cache: &str, limits: &Limits) -> Result<Stats> {
    let next = AtomicUsize::new(0);
    let t0 = Instant::now();
    let results: Vec<Result<(Vec<f64>, usize)>> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| -> Result<(Vec<f64>, usize)> {
                    let mut lat = Vec::new();
                    let mut empty = 0;
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(w) = work.get(i) else { break };
                        let t = Instant::now();
                        if !execute(snap, w, limits)? {
                            empty += 1;
                        }
                        lat.push(t.elapsed().as_secs_f64() * 1e3);
                    }
                    Ok((lat, empty))
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("bench thread")).collect()
    });
    let wall = t0.elapsed().as_secs_f64();
    let mut lat = Vec::new();
    let mut empty = 0;
    for r in results {
        let (l, e) = r?;
        lat.extend(l);
        empty += e;
    }
    lat.sort_by(f64::total_cmp);
    Ok(Stats {
        category: cat.to_string(),
        threads,
        cache: cache.to_string(),
        queries: lat.len(),
        qps: lat.len() as f64 / wall,
        p50_ms: pct(&lat, 0.50),
        p95_ms: pct(&lat, 0.95),
        p99_ms: pct(&lat, 0.99),
        mean_ms: lat.iter().sum::<f64>() / lat.len().max(1) as f64,
        max_ms: lat.last().copied().unwrap_or(0.0),
        empty_results: empty,
    })
}

#[cfg(unix)]
fn evict(path: &Path) -> Result<()> {
    use std::os::fd::AsRawFd;
    let f = std::fs::File::open(path)?;
    // SAFETY: plain syscall on a valid, owned file descriptor.
    let rc = unsafe { libc::posix_fadvise(f.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED) };
    anyhow::ensure!(rc == 0, "posix_fadvise failed: {rc}");
    Ok(())
}

#[cfg(not(unix))]
fn evict(_: &Path) -> Result<()> {
    anyhow::bail!("--cold is only supported on Unix")
}

fn env_info() -> serde_json::Value {
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    let cpu =
        read("/proc/cpuinfo").lines().find(|l| l.starts_with("model name")).and_then(|l| l.split(':').nth(1)).map(|s| s.trim().to_string());
    let mem = read("/proc/meminfo").lines().next().map(|l| l.split_whitespace().nth(1).unwrap_or("").to_string());
    let cmd = |c: &str, a: &[&str]| {
        std::process::Command::new(c).args(a).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    json!({
        "cpu": cpu,
        "threads": crate::default_threads(),
        "mem_total_kib": mem,
        "kernel": read("/proc/sys/kernel/osrelease").trim(),
        "os": read("/etc/os-release").lines().find(|l| l.starts_with("PRETTY_NAME")).map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string()),
        "git_commit": cmd("git", &["rev-parse", "HEAD"]),
        "rustc": cmd("rustc", &["--version"]),
        "opencnpj": env!("CARGO_PKG_VERSION"),
        "zstd": zstd::zstd_safe::version_string(),
    })
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

fn work_url(base: &str, w: &Work) -> String {
    let filters = |q: &Query| {
        let mut s: String = q.filters.iter().map(|(f, v)| format!("&{}={}", f.name(), enc(&v.join(",")))).collect();
        for p in &q.cnpj_prefixes {
            s.push_str(&format!("&cnpj={}", enc(p)));
        }
        for r in &q.ranges {
            let fmt =
                |v: u64| if r.field.is_date() { opencnpj_store::numeric::date_string(v) } else { format!("{:.2}", (v - 1) as f64 / 100.0) };
            match r.range {
                None => s.push_str(&format!("&{}_vazio=true", r.field.name())),
                Some(x) => s.push_str(&format!("&{0}_de={1}&{0}_ate={2}", r.field.name(), fmt(x.lo), fmt(x.hi))),
            }
        }
        if let Some((f, desc)) = q.sort {
            s.push_str(&format!("&ordenar={}:{}", f.name(), if desc { "desc" } else { "asc" }));
        }
        s
    };
    match w {
        Work::Lookup(id) => format!("{base}/api/v1/cnpj/{id}"),
        Work::Search(q) => {
            let text = q.text.as_deref().map(|t| format!("&q={}", enc(t))).unwrap_or_default();
            let fuzzy = if q.fuzzy { "&fuzzy=true" } else { "" };
            format!("{base}/api/v1/search?limit={}{text}{fuzzy}{}", q.limit, filters(q))
        }
        Work::Stats(q, f) => format!("{base}/api/v1/stats?group_by={}{}", f.name(), filters(q)),
        Work::Cpf(cpf) => format!("{base}/api/v1/cpf/{cpf}?limit=20"),
        Work::Year(q, f) => format!("{base}/api/v1/stats?group_by=ano_{}{}", f.name(), filters(q)),
    }
}

/// Small in-process benchmark for the admin self-test: `n` sampled queries per category,
/// one warm-up pass, then one thread.
pub fn quick(snap: &Snapshot, n: usize) -> Result<serde_json::Value> {
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.as_secs());
    let cats = workloads(snap, n, seed)?;
    let limits = Limits::DEFAULT;
    let mut out = Vec::new();
    for (cat, work) in &cats {
        measure(snap, cat, work, 1, "warmup", &limits)?;
        out.push(measure(snap, cat, work, 1, "warm", &limits)?);
    }
    Ok(json!({ "seed": seed, "queries_per_category": n, "results": out }))
}

pub fn run(a: BenchArgs) -> Result<()> {
    let mut limits = Limits::DEFAULT;
    if let Some(m) = a.expansion_mib {
        limits.max_expansion_bytes = m << 20;
    }
    let t_open = Instant::now();
    if a.cold {
        evict(&a.snapshot)?;
    }
    let mut snap = Snapshot::open(&a.snapshot)?;
    if let Some(m) = a.cache_mib {
        snap.set_postings_cache_bytes(m << 20);
    }
    let open_ms = t_open.elapsed().as_secs_f64() * 1e3;
    let t_w = Instant::now();
    let cats = workloads(&snap, a.queries, a.seed)?;
    eprintln!("workloads sampled in {:.1}s (open {:.1} ms)", t_w.elapsed().as_secs_f64(), open_ms);
    if let Some(dir) = &a.write_urls {
        std::fs::create_dir_all(dir)?;
        for (cat, work) in &cats {
            let urls: Vec<String> = work.iter().map(|w| work_url(&a.base_url, w)).collect();
            std::fs::write(dir.join(format!("{cat}.txt")), urls.join("\n") + "\n")?;
        }
        eprintln!("wrote URL lists to {}", dir.display());
        return Ok(());
    }
    let mut all = Vec::new();
    for (cat, work) in &cats {
        if a.only.as_deref().is_some_and(|o| !cat.contains(o)) {
            continue;
        }
        if a.cold {
            drop(snap);
            evict(&a.snapshot)?;
            snap = Snapshot::open(&a.snapshot)?;
            if let Some(m) = a.cache_mib {
                snap.set_postings_cache_bytes(m << 20);
            }
            let s = measure(&snap, cat, work, 1, "cold", &limits)?;
            eprintln!("{cat:<24} cold  p50 {:>8.3} ms  p95 {:>8.3} ms  p99 {:>8.3} ms", s.p50_ms, s.p95_ms, s.p99_ms);
            all.push(s);
        }
        // Warm-up pass, then measured passes.
        measure(&snap, cat, work, crate::default_threads(), "warmup", &limits)?;
        for &t in &a.threads {
            let s = measure(&snap, cat, work, t, "warm", &limits)?;
            eprintln!(
                "{cat:<24} warm t={t:<2} p50 {:>8.3} ms  p95 {:>8.3} ms  p99 {:>8.3} ms  {:>9.0} q/s  empty {}",
                s.p50_ms, s.p95_ms, s.p99_ms, s.qps, s.empty_results
            );
            all.push(s);
        }
    }
    std::fs::create_dir_all(&a.out)?;
    let meta = json!({
        "label": a.label,
        "snapshot": {
            "path": a.snapshot,
            "bytes": snap.file_len(),
            "profile": snap.manifest.profile,
            "codec": snap.manifest.block_codec,
            "block_target_bytes": snap.manifest.block_target_bytes,
            "documents": snap.doc_count(),
            "competence": snap.manifest.competence,
        },
        "open_ms": open_ms,
        "cold_start": a.cold,
        "seed": a.seed,
        "queries_per_category": a.queries,
        "max_expansion_bytes": limits.max_expansion_bytes,
        "postings_cache_mib": a.cache_mib,
        "peak_rss_bytes": opencnpj_ingest::build::peak_rss(),
        "environment": env_info(),
        "methodology": "Queries sampled from the snapshot with a seeded PRNG; each query = search + fetch of up to 20 records + JSON serialization (lookups: record + JSON). Warm runs follow one untimed warm-up pass. Cold runs evict the file with posix_fadvise(DONTNEED) and reopen the snapshot before the category.",
        "results": all,
    });
    std::fs::write(a.out.join("bench.json"), serde_json::to_vec_pretty(&meta)?)?;
    let mut csv = String::from("category,threads,cache,queries,qps,p50_ms,p95_ms,p99_ms,mean_ms,max_ms,empty_results\n");
    let mut md = String::from(
        "| category | threads | cache | queries | q/s | p50 ms | p95 ms | p99 ms | max ms | empty |\n|---|---:|---|---:|---:|---:|---:|---:|---:|---:|\n",
    );
    for s in &all {
        csv.push_str(&format!(
            "{},{},{},{},{:.1},{:.4},{:.4},{:.4},{:.4},{:.4},{}\n",
            s.category, s.threads, s.cache, s.queries, s.qps, s.p50_ms, s.p95_ms, s.p99_ms, s.mean_ms, s.max_ms, s.empty_results
        ));
        md.push_str(&format!(
            "| {} | {} | {} | {} | {:.0} | {:.3} | {:.3} | {:.3} | {:.3} | {} |\n",
            s.category, s.threads, s.cache, s.queries, s.qps, s.p50_ms, s.p95_ms, s.p99_ms, s.max_ms, s.empty_results
        ));
    }
    std::fs::write(a.out.join("bench.csv"), csv)?;
    std::fs::write(a.out.join("bench.md"), md)?;
    eprintln!("wrote {}/bench.{{json,csv,md}}", a.out.display());
    Ok(())
}
