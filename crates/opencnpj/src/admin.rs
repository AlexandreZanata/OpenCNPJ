//! Admin suite: dashboard UI, live metrics, snapshot operations, background jobs, query
//! explain and self-test benchmarks. Served on its own listener (`--admin-bind`) together
//! with the public API (so the UI can call it same-origin). No authentication: bind it to
//! a private address.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use axum::extract::{Path as UrlPath, Query as UrlQuery, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router, middleware};
use opencnpj_store::numeric::NumField;
use opencnpj_store::search::{self, Limits, TextField};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{self, ApiError, ApiResult, Shared};

const UI: &str = include_str!("admin.html");

/// `(RssAnon, RssFile)` in bytes from `/proc/self/status` (0 elsewhere).
pub fn rss() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        s.lines().find(|l| l.starts_with(name)).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()).map_or(0, |kb| kb * 1024)
    };
    (field("RssAnon:"), field("RssFile:"))
}

fn cpu_seconds_and_threads() -> (f64, u64) {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // Fields after the parenthesized command name.
    let rest = s.rsplit_once(')').map_or("", |x| x.1);
    let f: Vec<&str> = rest.split_whitespace().collect();
    let ticks = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    ((ticks(11) + ticks(12)) / 100.0, ticks(17) as u64)
}

fn unix(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

// ---------------------------------------------------------------------------------------
// Jobs: whitelisted subcommands of this binary, run as child processes with a log file.
// ---------------------------------------------------------------------------------------

struct Job {
    id: u64,
    kind: String,
    args: Vec<String>,
    started: SystemTime,
    finished: Option<SystemTime>,
    status: String,
    exit_code: Option<i32>,
    log: PathBuf,
    child: Option<Child>,
}

pub struct Jobs {
    dir: PathBuf,
    list: Mutex<Vec<Job>>,
}

#[derive(Deserialize, Default)]
pub struct JobRequest {
    kind: String,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    month: Option<String>,
    #[serde(default)]
    sources: bool,
    #[serde(default)]
    queries: Option<usize>,
}

impl Jobs {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, list: Mutex::new(Vec::new()) }
    }

    fn refresh(&self) {
        let mut list = self.list.lock().expect("lock");
        for j in list.iter_mut() {
            if let Some(c) = j.child.as_mut()
                && let Ok(Some(st)) = c.try_wait()
            {
                j.exit_code = st.code();
                j.status = if st.success() {
                    "ok"
                } else if j.status == "killing" {
                    "killed"
                } else {
                    "failed"
                }
                .into();
                j.finished = Some(SystemTime::now());
                j.child = None;
            }
        }
    }

    fn start(&self, kind: &str, args: Vec<String>) -> Result<u64> {
        std::fs::create_dir_all(&self.dir)?;
        let mut list = self.list.lock().expect("lock");
        if list.iter().any(|j| j.child.is_some() && j.kind == kind) {
            bail!("a {kind} job is already running");
        }
        let id = list.iter().map(|j| j.id).max().unwrap_or(0) + 1;
        let log = self.dir.join(format!("{}-{id:04}-{kind}.log", unix(SystemTime::now())));
        let out = std::fs::File::create(&log)?;
        let exe = std::env::current_exe()?;
        let child =
            Command::new(exe).args(&args).stdin(Stdio::null()).stdout(out.try_clone()?).stderr(out).spawn().context("spawning job")?;
        list.push(Job {
            id,
            kind: kind.into(),
            args,
            started: SystemTime::now(),
            finished: None,
            status: "running".into(),
            exit_code: None,
            log,
            child: Some(child),
        });
        Ok(id)
    }

    fn json(&self) -> Value {
        self.refresh();
        let list = self.list.lock().expect("lock");
        Value::Array(
            list.iter()
                .rev()
                .map(|j| {
                    json!({
                        "id": j.id, "kind": j.kind, "args": j.args, "status": j.status, "exit_code": j.exit_code,
                        "started": unix(j.started), "finished": j.finished.map(unix),
                        "elapsed_s": j.finished.unwrap_or_else(SystemTime::now).duration_since(j.started).map_or(0.0, |d| d.as_secs_f64()),
                        "log": j.log.file_name().map(|n| n.to_string_lossy().into_owned()),
                    })
                })
                .collect(),
        )
    }

    fn log_tail(&self, id: u64, lines: usize) -> Option<String> {
        let path = self.list.lock().expect("lock").iter().find(|j| j.id == id)?.log.clone();
        let text = std::fs::read_to_string(path).ok()?;
        let all: Vec<&str> = text.lines().collect();
        Some(all[all.len().saturating_sub(lines)..].join("\n"))
    }

    fn kill(&self, id: u64) -> bool {
        let mut list = self.list.lock().expect("lock");
        let Some(j) = list.iter_mut().find(|j| j.id == id) else { return false };
        if let Some(c) = j.child.as_mut() {
            let _ = c.kill();
            j.status = "killing".into();
            return true;
        }
        false
    }
}

fn job_args(st: &Shared, r: &JobRequest) -> Result<Vec<String>> {
    let snap = st.snapshot();
    let path = snap.path().display().to_string();
    let data = st.args.data_dir.clone();
    let comp = snap.manifest.competence.clone();
    let profile = r.profile.clone().unwrap_or_else(|| "balanced".into());
    if !["fast", "balanced", "compact"].contains(&profile.as_str()) {
        bail!("profile must be fast, balanced or compact");
    }
    let need_data = || data.clone().context("this job needs the server started with --data-dir");
    Ok(match r.kind.as_str() {
        "verify" => {
            let mut a = vec!["verify".into(), path];
            if r.sources {
                a.extend(["--sources".into(), need_data()?.join("raw").join(&comp).display().to_string()]);
            }
            a
        }
        "bench" => {
            let out =
                data.clone().unwrap_or_else(|| st.snapshots_dir.clone()).join("bench").join(format!("admin-{}", unix(SystemTime::now())));
            vec![
                "bench".into(),
                "--snapshot".into(),
                path,
                "--queries".into(),
                r.queries.unwrap_or(300).to_string(),
                "--threads".into(),
                "1,4,16".into(),
                "--out".into(),
                out.display().to_string(),
            ]
        }
        "download" => {
            let mut a = vec!["download".into(), "--dest".into(), need_data()?.join("raw").display().to_string()];
            if let Some(m) = &r.month {
                a.extend(["--month".into(), m.clone()]);
            }
            a
        }
        "update" => {
            let mut a = vec!["update".into(), "--data-dir".into(), need_data()?.display().to_string(), "--profile".into(), profile];
            if r.sources {
                a.push("--verify-sources".into());
            }
            a
        }
        "build" => {
            let d = need_data()?;
            let month = r.month.clone().unwrap_or(comp);
            vec![
                "build".into(),
                "--input".into(),
                d.join("raw").join(&month).display().to_string(),
                "--output".into(),
                d.join("snapshots").join(format!("{month}-{profile}.ocnpj")).display().to_string(),
                "--tmp".into(),
                d.join("work").join(&month).display().to_string(),
                "--profile".into(),
                profile.clone(),
                "--report".into(),
                d.join("reports").join(format!("build-{month}-{profile}.json")).display().to_string(),
            ]
        }
        k => bail!("unknown job kind {k}; use verify, bench, download, update or build"),
    })
}

async fn jobs_list(State(st): State<Shared>) -> Json<Value> {
    Json(st.jobs.json())
}

async fn jobs_start(State(st): State<Shared>, Json(r): Json<JobRequest>) -> ApiResult {
    let args = job_args(&st, &r).map_err(|e| ApiError::bad(e.to_string()))?;
    let id = st.jobs.start(&r.kind, args).map_err(|e| ApiError::new(StatusCode::CONFLICT, "job_error", e.to_string()))?;
    Ok(Json(json!({ "id": id })).into_response())
}

#[derive(Deserialize)]
struct TailQuery {
    lines: Option<usize>,
}

async fn jobs_log(State(st): State<Shared>, UrlPath(id): UrlPath<u64>, UrlQuery(q): UrlQuery<TailQuery>) -> ApiResult {
    match st.jobs.log_tail(id, q.lines.unwrap_or(200).min(10_000)) {
        Some(t) => Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], t).into_response()),
        None => Err(ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such job")),
    }
}

async fn jobs_kill(State(st): State<Shared>, UrlPath(id): UrlPath<u64>) -> Json<Value> {
    Json(json!({ "killed": st.jobs.kill(id) }))
}

// ---------------------------------------------------------------------------------------
// Overview, metrics, snapshots, cache, index, reports.
// ---------------------------------------------------------------------------------------

async fn overview(State(st): State<Shared>) -> Json<Value> {
    let s = st.snapshot();
    let (anon, file) = rss();
    let (cpu, threads) = cpu_seconds_and_threads();
    let mut info = crate::output::info(&s);
    if let Some(o) = info.as_object_mut() {
        o.remove("sources");
    }
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_s": st.metrics.started.elapsed().as_secs_f64(),
        "requests_total": st.metrics.total_requests(),
        "timeouts_total": st.metrics.timeouts.load(std::sync::atomic::Ordering::Relaxed),
        "overload_total": st.metrics.overload.load(std::sync::atomic::Ordering::Relaxed),
        "reloads_total": st.metrics.reloads.load(std::sync::atomic::Ordering::Relaxed),
        "process": { "rss_anon_bytes": anon, "rss_file_bytes": file, "cpu_seconds": cpu, "threads": threads, "peak_rss_bytes": opencnpj_ingest::build::peak_rss() },
        "cache": s.cache_stats(),
        "snapshot": info,
        "sha256": st.sha256(),
        "numeric_columns": NumField::ALL.iter().filter(|f| s.numeric(**f).is_some()).map(|f| f.name()).collect::<Vec<_>>(),
        "config": {
            "bind": st.args.bind.to_string(), "admin_bind": st.args.admin_bind, "data_dir": st.args.data_dir,
            "max_concurrency": st.args.max_concurrency, "timeout_ms": st.args.timeout_ms, "max_limit": st.args.max_limit,
            "max_offset": if st.args.max_offset == usize::MAX { json!("unlimited") } else { json!(st.args.max_offset) },
            "export": !st.args.disable_export, "export_max_rows": st.args.export_max_rows,
            "hide_socios": st.args.hide_socios, "hide_contacts": st.args.hide_contacts,
            "postings_cache_mib": st.args.postings_cache_mib, "authentication": "none",
        },
    }))
}

#[derive(Deserialize)]
struct SeriesQuery {
    seconds: Option<usize>,
}

async fn metrics(State(st): State<Shared>, UrlQuery(q): UrlQuery<SeriesQuery>) -> Json<Value> {
    Json(json!({ "routes": st.metrics.routes_json(), "series": st.metrics.series_json(q.seconds.unwrap_or(300)) }))
}

async fn snapshots(State(st): State<Shared>) -> Json<Value> {
    let active = st.snapshot().path().to_path_buf();
    let files: Vec<Value> = crate::output::distributable(&st.snapshots_dir)
        .into_iter()
        .filter(|(n, _)| n.ends_with(".ocnpj"))
        .map(|(name, bytes)| {
            let p = st.snapshots_dir.join(&name);
            let modified = std::fs::metadata(&p).and_then(|m| m.modified()).ok().map(unix);
            json!({ "name": name, "bytes": bytes, "sha256": api::read_sidecar(&p), "modified": modified, "active": p == active })
        })
        .collect();
    Json(json!({ "dir": st.snapshots_dir, "files": files }))
}

#[derive(Deserialize)]
struct Activate {
    name: String,
}

async fn activate(State(st): State<Shared>, Json(a): Json<Activate>) -> ApiResult {
    if a.name.contains('/') || a.name.contains("..") || !a.name.ends_with(".ocnpj") {
        return Err(ApiError::bad("name must be a .ocnpj file in the snapshots directory"));
    }
    let path = st.snapshots_dir.join(&a.name);
    let st2 = st.clone();
    let p2 = path.clone();
    tokio::task::spawn_blocking(move || st2.switch_to(&p2))
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", e.to_string()))?
        .map_err(|e| ApiError::bad(format!("{e:#}")))?;
    if let Some(d) = &st.args.data_dir {
        let rel = path.strip_prefix(d).map(|p| p.display().to_string()).unwrap_or_else(|_| format!("snapshots/{}", a.name));
        crate::download::set_pointer(d, "current", &rel).map_err(ApiError::from)?;
    }
    *st.index_breakdown.lock().expect("lock") = None;
    Ok(Json(json!({ "active": a.name })).into_response())
}

async fn reload(State(st): State<Shared>) -> ApiResult {
    let path = match &st.args.data_dir {
        Some(d) => api::resolve_current(d).map_err(ApiError::from)?,
        None => st.snapshot().path().to_path_buf(),
    };
    let st2 = st.clone();
    let p2 = path.clone();
    tokio::task::spawn_blocking(move || st2.switch_to(&p2))
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", e.to_string()))?
        .map_err(|e| ApiError::bad(format!("{e:#}")))?;
    Ok(Json(json!({ "reloaded": path })).into_response())
}

#[derive(Deserialize)]
struct CacheBudget {
    budget_mib: usize,
}

async fn cache_get(State(st): State<Shared>) -> Json<Value> {
    Json(json!(st.snapshot().cache_stats()))
}

async fn cache_set(State(st): State<Shared>, Json(b): Json<CacheBudget>) -> Json<Value> {
    let s = st.snapshot();
    s.set_postings_cache_bytes(b.budget_mib << 20);
    Json(json!(s.cache_stats()))
}

async fn cache_clear(State(st): State<Shared>) -> Json<Value> {
    let s = st.snapshot();
    s.clear_cache();
    Json(json!(s.cache_stats()))
}

/// Terms and postings bytes per index field (one full dictionary scan, cached).
async fn index(State(st): State<Shared>) -> ApiResult {
    let path = st.snapshot().path().to_path_buf();
    if let Some((p, v)) = st.index_breakdown.lock().expect("lock").as_ref()
        && *p == path
    {
        return Ok(Json(v.clone()).into_response());
    }
    let t0 = Instant::now();
    let v = api::blocking(&st, |s| Ok(crate::output::index_breakdown(&s))).await?;
    let v = json!({ "fields": v, "computed_ms": t0.elapsed().as_secs_f64() * 1e3 });
    *st.index_breakdown.lock().expect("lock") = Some((path, v.clone()));
    Ok(Json(v).into_response())
}

/// JSON reports under the data dir (`reports/*.json`, `bench/*/bench.json`).
fn report_files(st: &Shared) -> Vec<PathBuf> {
    let base = st.args.data_dir.clone().unwrap_or_else(|| st.snapshots_dir.clone());
    let mut out = Vec::new();
    for dir in [base.join("reports"), st.snapshots_dir.clone()] {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "json") {
                out.push(p);
            }
        }
    }
    for e in std::fs::read_dir(base.join("bench")).into_iter().flatten().flatten() {
        let p = e.path().join("bench.json");
        if p.is_file() {
            out.push(p);
        }
    }
    out.sort();
    out.dedup();
    out
}

async fn reports(State(st): State<Shared>) -> Json<Value> {
    let files: Vec<Value> = report_files(&st)
        .into_iter()
        .map(|p| {
            let kind = if p.ends_with("bench.json") { "bench" } else { "build" };
            let modified = std::fs::metadata(&p).and_then(|m| m.modified()).ok().map(unix);
            json!({ "id": report_id(&p), "kind": kind, "path": p, "modified": modified })
        })
        .collect();
    Json(Value::Array(files))
}

fn report_id(p: &Path) -> String {
    let parent = p.parent().and_then(|d| d.file_name()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    format!("{parent}__{name}")
}

async fn report(State(st): State<Shared>, UrlPath(id): UrlPath<String>) -> ApiResult {
    let p = report_files(&st)
        .into_iter()
        .find(|p| report_id(p) == id)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such report"))?;
    let text = std::fs::read_to_string(&p).map_err(|e| ApiError::from(anyhow::Error::from(e)))?;
    Ok(([(header::CONTENT_TYPE, "application/json")], text).into_response())
}

// ---------------------------------------------------------------------------------------
// Measurements: explain a query, quick self-test benchmark.
// ---------------------------------------------------------------------------------------

/// Runs a search and reports the time of each stage and the size of each filter.
async fn explain(State(st): State<Shared>, UrlQuery(p): UrlQuery<Vec<(String, String)>>) -> ApiResult {
    let q = api::parse_query(&p, &st.args, TextField::Any, true)?;
    let v = api::blocking(&st, move |s| {
        let mut filters = Vec::new();
        for (f, values) in &q.filters {
            let t = Instant::now();
            let one = search::Query { filters: vec![(*f, values.clone())], limit: 1, ..Default::default() };
            let n = search::candidates(&s, &one, &Limits::DEFAULT)?.all.map_or(0, |b| b.len());
            filters.push(json!({ "field": f.name(), "values": values, "documents": n, "ms": t.elapsed().as_secs_f64() * 1e3 }));
        }
        let mut ranges = Vec::new();
        for r in &q.ranges {
            let t = Instant::now();
            let n = s.numeric(r.field).map(|c| c.select(s.bytes(), r.range, None).len());
            ranges.push(json!({ "field": r.field.name(), "documents": n, "ms": t.elapsed().as_secs_f64() * 1e3 }));
        }
        let t0 = Instant::now();
        let c = search::candidates(&s, &q, &Limits::DEFAULT)?;
        let t_cand = t0.elapsed();
        let t1 = Instant::now();
        let r = search::search(&s, &q, &Limits::DEFAULT)?;
        let t_search = t1.elapsed();
        let t2 = Instant::now();
        let docs = s.docs(&crate::sorted(&r.docs))?;
        let t_fetch = t2.elapsed();
        let t3 = Instant::now();
        let out = crate::output::search_json(&s, &r, docs, false);
        let t_json = t3.elapsed();
        Ok(json!({
            "plan": {
                "text": q.text, "text_field": format!("{:?}", q.text_field), "fuzzy": q.fuzzy, "fuzzy_auto": q.fuzzy_auto,
                "cnpj_prefixes": q.cnpj_prefixes, "sort": q.sort.map(|(f, d)| format!("{}:{}", f.name(), if d { "desc" } else { "asc" })),
                "filters_alone": filters, "ranges_alone": ranges,
                "exact_candidates": c.all.as_ref().map(|b| b.len()), "matched_by_razao": c.by_razao.len(), "matched_by_fantasia": c.by_fantasia.len(),
                "expansion_truncated": c.truncated,
            },
            "timings_ms": {
                "candidates": t_cand.as_secs_f64() * 1e3, "search_total": t_search.as_secs_f64() * 1e3,
                "fetch_records": t_fetch.as_secs_f64() * 1e3, "json": t_json.as_secs_f64() * 1e3,
            },
            "cache": s.cache_stats(),
            "result": out,
        }))
    })
    .await?;
    Ok(Json(v).into_response())
}

#[derive(Deserialize)]
struct SelfTest {
    queries: Option<usize>,
}

async fn selftest(State(st): State<Shared>, Json(t): Json<SelfTest>) -> ApiResult {
    let n = t.queries.unwrap_or(200).clamp(10, 5000);
    let v = api::blocking(&st, move |s| crate::bench::quick(&s, n)).await?;
    Ok(Json(v).into_response())
}

async fn ui() -> Html<&'static str> {
    Html(UI)
}

pub fn router(st: Shared) -> Router {
    let admin = Router::new()
        .route("/admin", get(ui))
        .route("/admin/", get(ui))
        .route("/admin/api/overview", get(overview))
        .route("/admin/api/metrics", get(metrics))
        .route("/admin/api/snapshots", get(snapshots))
        .route("/admin/api/snapshots/activate", post(activate))
        .route("/admin/api/reload", post(reload))
        .route("/admin/api/cache", get(cache_get).post(cache_set).delete(cache_clear))
        .route("/admin/api/index", get(index))
        .route("/admin/api/reports", get(reports))
        .route("/admin/api/reports/{id}", get(report))
        .route("/admin/api/jobs", get(jobs_list).post(jobs_start))
        .route("/admin/api/jobs/{id}/log", get(jobs_log))
        .route("/admin/api/jobs/{id}", axum::routing::delete(jobs_kill))
        .route("/admin/api/explain", get(explain))
        .route("/admin/api/selftest", post(selftest))
        .layer(middleware::from_fn_with_state(st.clone(), api::track))
        .with_state(st.clone());
    admin.merge(api::router(st))
}
