//! Public HTTP API (axum). CPU-bound work runs on the blocking pool behind a semaphore.
//! Snapshots are swapped atomically: in-flight requests keep the `Arc<Snapshot>` they
//! started with. The admin suite (`admin.rs`) is served on its own listener.
//!
//! Data routes pass through `gate::gate` (accounts, plans, rate limits, quotas, usage) and
//! idempotent ones through `gate::cache` (shared response cache with request coalescing).
//! `--auth off` restores the open, unmetered behaviour.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use axum::body::Body;
use axum::extract::{MatchedPath, Path as UrlPath, Query as UrlQuery, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use opencnpj_insights::official::{
    self,
    query::{CategoryFilter, Request as OfficialRequest},
    registry::Registry,
    store::StoreSet,
};
use opencnpj_store::Snapshot;
use opencnpj_store::numeric::{self, NumField};
use opencnpj_store::postings::Field;
use opencnpj_store::search::{self, Limits, Query, TextField};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::Semaphore;

use crate::gate::{AuthMode, Caller, Gate, ResponseCache};
use crate::metrics::Metrics;
use crate::output::{self, LookupError, Privacy};
use crate::params::{self, Caps};

#[derive(clap::Args, Clone)]
pub struct ServeArgs {
    /// Snapshot file to serve.
    #[arg(long, conflicts_with = "data_dir")]
    pub snapshot: Option<PathBuf>,
    /// Data directory (`current` pointer, `snapshots/`, `reports/`, `bench/`, `jobs/`):
    /// enables hot reload, snapshot switching and admin jobs.
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub bind: SocketAddr,
    /// Admin suite listener (dashboard, metrics, operations). Empty string disables it.
    /// It has no authentication: keep it on a private address.
    #[arg(long, default_value = "127.0.0.1:8081")]
    pub admin_bind: String,
    /// Concurrent CPU-bound requests.
    #[arg(long, default_value_t = crate::default_threads() * 4)]
    pub max_concurrency: usize,
    /// Per-request deadline in milliseconds (0 = none).
    #[arg(long, default_value_t = 60_000)]
    pub timeout_ms: u64,
    /// Largest page size.
    #[arg(long, default_value_t = 10_000)]
    pub max_limit: usize,
    /// Deepest offset (default: no limit).
    #[arg(long, default_value_t = usize::MAX)]
    pub max_offset: usize,
    /// Disable `/api/v1/export`.
    #[arg(long)]
    pub disable_export: bool,
    /// Rows per export (0 = no limit).
    #[arg(long, default_value_t = 0)]
    pub export_max_rows: u64,
    /// Directory containing official `.ocind` indicator stores.
    #[arg(long)]
    pub official_dir: Option<PathBuf>,
    /// Decoded-postings cache budget in MiB.
    #[arg(long, default_value_t = 512)]
    pub postings_cache_mib: usize,
    /// Omit partners (sócios) from responses.
    #[arg(long)]
    pub hide_socios: bool,
    /// Omit phone numbers and e-mail from responses.
    #[arg(long)]
    pub hide_contacts: bool,
    /// Seconds between checks of `<data-dir>/current`.
    #[arg(long, default_value_t = 30)]
    pub reload_secs: u64,
    /// Accounts: `required` (a key or session on every data route), `optional` (anonymous
    /// callers get the `anonymous` plan per IP), `trial` (browser preview) or `off`.
    #[arg(long, value_enum, default_value_t = AuthMode::Required)]
    pub auth: AuthMode,
    /// File of denied VPN/proxy CIDRs for the trial and free tier (one per line).
    #[arg(long)]
    pub trial_deny_networks: Option<PathBuf>,
    /// HTTPS page or mailto address for users to request a plan upgrade.
    #[arg(long, env = "OPENCNPJ_UPGRADE_URL", value_parser = parse_upgrade_url)]
    pub upgrade_url: Option<String>,
    /// Accounts database (default `<data-dir>/accounts.sqlite`, or next to `--snapshot`).
    #[arg(long)]
    pub accounts_db: Option<PathBuf>,
    /// Base URL used in e-mailed links (default `http://<bind>`).
    #[arg(long)]
    pub public_url: Option<String>,
    /// SMTP server, e.g. `smtps://user:password@smtp.example.com:465`. Without it, e-mails
    /// are written to `<data-dir>/outbox/`.
    #[arg(long, env = "OPENCNPJ_SMTP_URL", hide_env_values = true)]
    pub smtp_url: Option<String>,
    #[arg(long, env = "OPENCNPJ_MAIL_FROM", default_value = "OpenCNPJ <no-reply@localhost>")]
    pub mail_from: String,
    /// Session lifetime after a login, in hours.
    #[arg(long, default_value_t = 168)]
    pub session_hours: i64,
    /// Shared response cache budget in MiB (0 disables it).
    #[arg(long, default_value_t = 256)]
    pub response_cache_mib: u64,
    /// Maximum age of a cached response (entries also expire when the snapshot changes).
    #[arg(long, default_value_t = 3600)]
    pub response_cache_ttl_secs: u64,
    /// Do not compress responses (gzip, br, zstd are negotiated by default).
    #[arg(long)]
    pub no_compression: bool,
    /// Take the client address from `X-Forwarded-For` (only behind a trusted reverse proxy).
    #[arg(long)]
    pub trust_proxy: bool,
    /// Bind with SO_REUSEPORT so a new process can take over the port before the old one
    /// drains (zero-downtime restarts; Linux/BSD).
    #[arg(long)]
    pub reuse_port: bool,
    /// Never check for new RFB releases.
    #[arg(long)]
    pub no_auto_update: bool,
    /// Release check interval with `--auth off` (with accounts it is a setting; 0 = never).
    #[arg(long, default_value_t = 0)]
    pub auto_update_hours: i64,
    /// On SIGTERM, keep answering for this long with `Connection: close` (clients move to
    /// the successor cleanly), then drain.
    #[arg(long, default_value_t = 5)]
    pub drain_secs: u64,
    /// Built web front end served at `/` (for example `web/dist`).
    #[arg(long)]
    pub web: Option<PathBuf>,
    /// Aggregates of the served snapshot (`.ocagg`): enables the assistant under
    /// `/api/v1/assistant`. The file must have been built from the snapshot served at start.
    #[arg(long)]
    pub assistant_agg: Option<PathBuf>,
    /// Directory of aggregates of earlier competences, for historical questions.
    #[arg(long)]
    pub assistant_history: Option<PathBuf>,
    /// Municipality catalogue of the assistant.
    #[arg(long, default_value = "crates/opencnpj-insights/catalog/municipalities.tsv")]
    pub assistant_catalog: PathBuf,
    /// Sector catalogue of the assistant.
    #[arg(long, default_value = "crates/opencnpj-insights/catalog/sectors.toml")]
    pub assistant_sectors: PathBuf,
    /// Data date of the snapshot, `YYYY-MM-DD`: relative periods are counted back from it.
    #[arg(long, default_value = "2026-10-11")]
    pub assistant_anchor: String,
    /// Rows per file generated by the assistant (plans can lower it).
    #[arg(long, default_value_t = 100_000)]
    pub assistant_export_rows: u32,
}

fn parse_upgrade_url(value: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(value).map_err(|e| e.to_string())?;
    match url.scheme() {
        "https" if url.host().is_some() => Ok(url.into()),
        "mailto" if !url.path().is_empty() => Ok(url.into()),
        _ => Err("expected an https or mailto URL".into()),
    }
}

impl ServeArgs {
    /// Parses `serve` flags as written on the command line (tests and embedding).
    pub fn from_flags<S: AsRef<str>>(flags: &[S]) -> Result<Self> {
        #[derive(clap::Parser)]
        struct Wrapper {
            #[command(flatten)]
            args: ServeArgs,
        }
        let argv = std::iter::once("serve").chain(flags.iter().map(AsRef::as_ref));
        Ok(<Wrapper as clap::Parser>::try_parse_from(argv)?.args)
    }
}

pub struct AppState {
    snap: RwLock<Arc<Snapshot>>,
    sha256: RwLock<Option<String>>,
    pub args: ServeArgs,
    /// Searches, stats, CPF lookups and exports.
    pub sem: Arc<Semaphore>,
    /// CNPJ lookups (sub-millisecond): their own lane, so they never queue behind searches.
    pub fast_sem: Arc<Semaphore>,
    pub metrics: Metrics,
    pub privacy: Privacy,
    /// Where `.ocnpj` files live (data dir `snapshots/`, or the served file's folder).
    pub snapshots_dir: PathBuf,
    pub jobs: crate::admin::Jobs,
    /// `info --index` is a full term-dictionary scan: computed once per snapshot.
    pub index_breakdown: Mutex<Option<(PathBuf, Value)>>,
    /// Official indicator stores, when configured.
    pub official_dir: Option<PathBuf>,
    /// Conversational assistant over the snapshot served at start (`--assistant-agg`).
    pub assistant: Option<Arc<crate::assistant::Assistant>>,
    pub gate: Arc<Gate>,
    pub(crate) trial: Option<Arc<crate::trial::Trial>>,
    pub mailer: Option<Arc<crate::mail::Mailer>>,
    pub updater: crate::autoupdate::Updater,
    /// Identity of the served snapshot (path, size, mtime): part of the response cache key,
    /// stable across processes so cached responses can be handed over.
    generation: AtomicU64,
    /// Set on SIGTERM: responses carry `Connection: close`.
    pub draining: std::sync::atomic::AtomicBool,
}

impl AppState {
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.snap.read().expect("lock").clone()
    }

    pub fn sha256(&self) -> Option<String> {
        self.sha256.read().expect("lock").clone()
    }

    /// Opens `path` and makes it the served snapshot.
    pub fn switch_to(&self, path: &Path) -> Result<()> {
        let s = Snapshot::open(path).with_context(|| format!("opening {}", path.display()))?;
        s.set_postings_cache_bytes(self.args.postings_cache_mib << 20);
        *self.snap.write().expect("lock") = Arc::new(s);
        *self.sha256.write().expect("lock") = read_sidecar(path);
        self.generation.store(snapshot_identity(path), Ordering::Relaxed);
        if let Some(c) = &self.gate.cache {
            c.clear();
        }
        self.metrics.reloads.fetch_add(1, Ordering::Relaxed);
        eprintln!("serving {}", path.display());
        Ok(())
    }
}

impl AppState {
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Re-evaluates the update schedule now (settings changed or a check was requested).
    pub fn wake_updater(&self) {
        self.updater.notify.notify_one();
    }
}

pub type Shared = Arc<AppState>;

/// Hash of a snapshot file's path, size and modification time.
pub fn snapshot_identity(path: &Path) -> u64 {
    let meta = std::fs::metadata(path).ok();
    let mtime = meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    h.update(std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()).to_string_lossy().as_bytes());
    h.update(&meta.map_or(0, |m| m.len()).to_le_bytes());
    h.update(&mtime.to_le_bytes());
    h.digest()
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into() }
    }

    pub fn bad(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(json!({ "error": self.code, "message": self.message, "code": self.status.as_u16() }));
        (self.status, body).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        if let Some(d) = e.downcast_ref::<crate::accounts::Denied>() {
            return ApiError::new(StatusCode::from_u16(d.status).unwrap_or(StatusCode::BAD_REQUEST), d.code, d.message.clone());
        }
        if let Some(LookupError::Invalid(m)) = e.downcast_ref::<LookupError>() {
            return ApiError::new(StatusCode::BAD_REQUEST, "invalid_document", m.clone());
        }
        if let Some(opencnpj_store::Error::Invalid(m)) = e.downcast_ref::<opencnpj_store::Error>() {
            return ApiError::bad(m.clone());
        }
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", e.to_string())
    }
}

pub type ApiResult = Result<Response, ApiError>;

/// Runs `f` on the blocking pool with a permit and the configured deadline.
pub async fn blocking<T: Send + 'static>(st: &Shared, f: impl FnOnce(Arc<Snapshot>) -> Result<T> + Send + 'static) -> Result<T, ApiError> {
    run_in_lane(st, &st.sem, f).await
}

/// Like `blocking`, on the lookup lane.
pub async fn blocking_fast<T: Send + 'static>(
    st: &Shared,
    f: impl FnOnce(Arc<Snapshot>) -> Result<T> + Send + 'static,
) -> Result<T, ApiError> {
    run_in_lane(st, &st.fast_sem, f).await
}

async fn run_in_lane<T: Send + 'static>(
    st: &Shared,
    sem: &Arc<Semaphore>,
    f: impl FnOnce(Arc<Snapshot>) -> Result<T> + Send + 'static,
) -> Result<T, ApiError> {
    let timeout = match st.args.timeout_ms {
        0 => Duration::from_secs(86_400 * 365),
        ms => Duration::from_millis(ms),
    };
    let permit = match tokio::time::timeout(timeout, sem.clone().acquire_owned()).await {
        Ok(Ok(p)) => p,
        _ => {
            st.metrics.overload.fetch_add(1, Ordering::Relaxed);
            return Err(ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "overloaded", "server is busy, retry later"));
        }
    };
    let snap = st.snapshot();
    let task = tokio::task::spawn_blocking(move || {
        let r = f(snap);
        drop(permit);
        r
    });
    match tokio::time::timeout(timeout, task).await {
        Ok(Ok(Ok(v))) => Ok(v),
        Ok(Ok(Err(e))) => Err(ApiError::from(e)),
        Ok(Err(e)) => Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", e.to_string())),
        Err(_) => {
            st.metrics.timeouts.fetch_add(1, Ordering::Relaxed);
            Err(ApiError::new(StatusCode::GATEWAY_TIMEOUT, "timeout", "query exceeded the time limit"))
        }
    }
}

/// Records latency and status per matched route.
pub async fn track(State(st): State<Shared>, req: Request, next: Next) -> Response {
    let route = req.extensions().get::<MatchedPath>().map_or_else(|| "unmatched".to_string(), |p| p.as_str().to_string());
    let t0 = Instant::now();
    let mut resp = next.run(req).await;
    st.metrics.record(&route, t0.elapsed().as_micros() as u64, resp.status().as_u16());
    if st.draining.load(Ordering::Relaxed) {
        resp.headers_mut().insert(header::CONNECTION, HeaderValue::from_static("close"));
    }
    resp
}

fn json_response(v: Value) -> Response {
    let mut r = Json(v).into_response();
    r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=3600"));
    r
}

pub fn caps(args: &ServeArgs) -> Caps {
    Caps { max_limit: args.max_limit, max_offset: args.max_offset }
}

pub fn parse_query(pairs: &[(String, String)], args: &ServeArgs, default_field: TextField, require: bool) -> Result<Query, ApiError> {
    let mut q = params::parse(pairs, caps(args)).map_err(ApiError::bad)?;
    let explicit = pairs.iter().any(|(k, _)| matches!(k.as_str(), "field" | "campo" | "razao_social" | "nome_fantasia"));
    if default_field != TextField::Any && !explicit {
        q.text_field = default_field;
    }
    if require && !params::has_criteria(&q) {
        return Err(ApiError::bad("provide q (name, CNPJ or CPF) and/or at least one filter; see /api/v1/campos"));
    }
    Ok(q)
}

fn page_limit(st: &Shared, caller: Option<axum::Extension<Caller>>) -> usize {
    caller.map_or(st.args.max_limit, |caller| st.args.max_limit.min(caller.principal.plan.max_page_size as usize))
}

async fn get_cnpj(
    State(st): State<Shared>,
    UrlPath(id): UrlPath<String>,
    UrlQuery(p): UrlQuery<Vec<(String, String)>>,
    caller: Option<axum::Extension<Caller>>,
) -> ApiResult {
    if id.len() > 32 {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "invalid_document", "invalid cnpj"));
    }
    let num = |k: &str, d: usize| {
        p.iter().find(|(a, _)| a == k).map_or(Ok(d), |(_, v)| v.parse::<usize>().map_err(|_| ApiError::bad(format!("bad {k}"))))
    };
    let offset = num("estab_offset", 0)?;
    let limit = num("estab_limit", output::DEFAULT_ESTAB_PAGE.min(page_limit(&st, caller)))?;
    if limit == 0 || limit > st.args.max_limit {
        return Err(ApiError::bad(format!("estab_limit must be between 1 and {}", st.args.max_limit)));
    }
    let privacy = st.privacy;
    let v = blocking_fast(&st, move |s| output::lookup_page(&s, &id, privacy, offset, limit)).await?;
    match v {
        Some(v) => Ok(json_response(v)),
        None => Err(ApiError::new(StatusCode::NOT_FOUND, "cnpj_not_found", "cnpj not found")),
    }
}

async fn get_cpf(
    State(st): State<Shared>,
    UrlPath(cpf): UrlPath<String>,
    UrlQuery(p): UrlQuery<Vec<(String, String)>>,
    caller: Option<axum::Extension<Caller>>,
) -> ApiResult {
    let limit =
        p.iter().find(|(k, _)| k == "limit").map_or(Ok(100), |(_, v)| v.parse::<usize>().map_err(|_| ApiError::bad("bad limit")))?;
    let limit = limit.clamp(1, page_limit(&st, caller));
    let privacy = st.privacy;
    let v = blocking(&st, move |s| output::cpf_lookup(&s, &cpf, limit, privacy)).await?;
    Ok(json_response(v))
}

async fn search_handler(
    st: Shared,
    pairs: Vec<(String, String)>,
    default_field: TextField,
    caller: Option<axum::Extension<Caller>>,
) -> ApiResult {
    let mut q = parse_query(&pairs, &st.args, default_field, true)?;
    q.limit = q.limit.min(page_limit(&st, caller));
    let v = blocking(&st, move |s| {
        let t0 = Instant::now();
        let r = search::search(&s, &q, &Limits::DEFAULT)?;
        let t_search = t0.elapsed();
        let docs = s.docs(&crate::sorted(&r.docs))?;
        let mut v = output::search_json(&s, &r, docs, false);
        v["took_ms"] = json!({ "search": t_search.as_secs_f64() * 1e3, "total": t0.elapsed().as_secs_f64() * 1e3 });
        Ok(v)
    })
    .await?;
    Ok(json_response(v))
}

async fn search_route(
    State(st): State<Shared>,
    UrlQuery(p): UrlQuery<Vec<(String, String)>>,
    caller: Option<axum::Extension<Caller>>,
) -> ApiResult {
    search_handler(st, p, TextField::Any, caller).await
}

async fn legacy_empresas_search(
    State(st): State<Shared>,
    UrlQuery(p): UrlQuery<Vec<(String, String)>>,
    caller: Option<axum::Extension<Caller>>,
) -> ApiResult {
    search_handler(st, p, TextField::Razao, caller).await
}

/// `group_by`: any filter field, or `ano_<date column>` for a yearly histogram.
async fn stats(State(st): State<Shared>, UrlQuery(p): UrlQuery<Vec<(String, String)>>) -> ApiResult {
    let group = p.iter().find(|(k, _)| k == "group_by" || k == "agrupar").map_or("uf".to_string(), |(_, v)| v.clone());
    let rest: Vec<(String, String)> = p.into_iter().filter(|(k, _)| k != "group_by" && k != "agrupar").collect();
    let q = parse_query(&rest, &st.args, TextField::Any, false)?;
    let has = params::has_criteria(&q);
    if let Some(col) = group.strip_prefix("ano_").and_then(NumField::from_name).filter(|f| f.is_date()) {
        let v = blocking(&st, move |s| {
            let set = if has { search::candidates(&s, &q, &Limits::DEFAULT)?.all } else { None };
            let c = s.numeric(col).context("snapshot has no numeric columns (rebuild with format 1.2)")?;
            let h = c.histogram(s.bytes(), set.as_ref(), numeric::date_year);
            let rows: Vec<Value> = h.into_iter().map(|(y, n)| json!({ "value": y.to_string(), "count": n })).collect();
            Ok(json!({ "group_by": group, "groups": rows, "snapshot": output::snapshot_ref(&s) }))
        })
        .await?;
        return Ok(json_response(v));
    }
    let field = Field::from_name(&group)
        .filter(|f| Field::FILTERS.contains(f) && !matches!(f, Field::CnaeQualquer | Field::CpfQualquer))
        .ok_or_else(|| ApiError::bad("bad group_by; use a filter field or ano_<date column>"))?;
    let v = blocking(&st, move |s| {
        let counts = if has {
            search::group_counts(&s, &q, field, &Limits::DEFAULT)?
        } else {
            search::group_counts(&s, &Query::default(), field, &Limits::DEFAULT)?
        };
        let rows: Vec<Value> = counts
            .into_iter()
            .map(|(value, n)| {
                let label = match field {
                    Field::Municipio => s.label("municipios", &value),
                    Field::Cnae | Field::CnaeSecundaria => s.label("cnaes", &value),
                    Field::Natureza => s.label("naturezas", &value),
                    Field::Motivo => s.label("motivos", &value),
                    Field::Situacao => opencnpj_core::model::fixed_label("situacao_cadastral", &value),
                    Field::Porte => opencnpj_core::model::fixed_label("porte_empresa", &value),
                    Field::MatrizFilial => opencnpj_core::model::fixed_label("identificador_matriz_filial", &value),
                    _ => None,
                };
                json!({ "value": value, "label": label, "count": n })
            })
            .collect();
        Ok(json!({ "group_by": field.name(), "groups": rows, "snapshot": output::snapshot_ref(&s) }))
    })
    .await?;
    Ok(json_response(v))
}

async fn export(
    State(st): State<Shared>,
    caller: Option<axum::Extension<Caller>>,
    UrlQuery(p): UrlQuery<Vec<(String, String)>>,
) -> ApiResult {
    if st.args.disable_export {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "export_disabled", "export is disabled on this server"));
    }
    let format = p.iter().find(|(k, _)| k == "format").map_or("ndjson".to_string(), |(_, v)| v.clone());
    if format != "ndjson" && format != "csv" {
        return Err(ApiError::bad("format must be ndjson or csv"));
    }
    // The caller's plan can lower the server cap.
    let server_cap = st.args.export_max_rows;
    let plan_cap = caller.and_then(|c| c.principal.plan.max_export_rows).map(|x| x.max(0) as u64);
    let cap = match (server_cap, plan_cap) {
        (0, Some(p)) => p,
        (s, Some(p)) => s.min(p),
        (s, None) => s,
    };
    let max = match (p.iter().find(|(k, _)| k == "max"), cap) {
        (Some((_, v)), cap) => v.parse::<u64>().map_err(|_| ApiError::bad("bad max"))?.min(if cap == 0 { u64::MAX } else { cap }),
        (None, 0) => u64::MAX,
        (None, cap) => cap,
    };
    let rest: Vec<(String, String)> = p.into_iter().filter(|(k, _)| k != "limit").collect();
    let mut q = parse_query(&rest, &st.args, TextField::Any, true)?;
    q.limit = usize::MAX;
    let permit =
        st.sem.clone().acquire_owned().await.map_err(|_| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "overloaded", "closed"))?;
    let snap = st.snapshot();
    let privacy = st.privacy;
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<Vec<u8>, std::io::Error>>(8);
    let fmt = format.clone();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut w = ChannelWriter { tx, buf: Vec::with_capacity(1 << 16) };
        if let Err(e) = output::export(&snap, &q, &fmt, max, privacy, &mut w) {
            let _ = w.tx.blocking_send(Err(std::io::Error::other(e.to_string())));
        }
    });
    // Query errors (bad filter values) surface before any row: answer them with a 400
    // instead of a 200 whose stream breaks.
    let first = rx.recv().await;
    if let Some(Err(e)) = &first {
        return Err(ApiError::bad(e.to_string().trim_start_matches("invalid: ").to_string()));
    }
    let head = futures_util::stream::iter(first);
    let rest = futures_util::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|item| (item, rx)) });
    let ctype = if format == "csv" { "text/csv; charset=utf-8" } else { "application/x-ndjson" };
    Ok(([(header::CONTENT_TYPE, ctype)], Body::from_stream(futures_util::StreamExt::chain(head, rest))).into_response())
}

struct ChannelWriter {
    tx: tokio::sync::mpsc::Sender<Result<Vec<u8>, std::io::Error>>,
    buf: Vec<u8>,
}

impl std::io::Write for ChannelWriter {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(b);
        if self.buf.len() >= 1 << 16 {
            self.flush()?;
        }
        Ok(b.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.buf.is_empty() {
            let chunk = std::mem::replace(&mut self.buf, Vec::with_capacity(1 << 16));
            self.tx.blocking_send(Ok(chunk)).map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "client disconnected"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct OfficialQuery {
    pub variable: String,
    pub operation: Option<String>,
    pub denominator: Option<String>,
    pub geography: Option<String>,
    pub period: Option<String>,
    pub classification: Option<String>,
    pub category: Option<String>,
    pub level: Option<String>,
    pub uf: Option<String>,
    pub limit: Option<usize>,
    pub ascending: Option<bool>,
    pub scale: Option<f64>,
}

async fn official_sources(State(st): State<Shared>) -> ApiResult {
    let registry = Registry::builtin()?;
    let stores = st.official_dir.as_ref().and_then(|dir| StoreSet::open_dir(dir).ok());
    let manifests =
        stores.as_ref().map(|stores| stores.stores().iter().map(|store| store.manifest().clone()).collect::<Vec<_>>()).unwrap_or_default();
    let published = manifests.iter().map(|manifest| (manifest.dataset_id.as_str(), manifest)).collect::<std::collections::BTreeMap<_, _>>();
    let datasets = registry
        .datasets()
        .map(|dataset| {
            let source = registry.source(&dataset.source_id).expect("validated source");
            let manifest = published.get(dataset.id.as_str());
            json!({
                "source_id": dataset.source_id,
                "dataset_id": dataset.id,
                "title": dataset.title,
                "publisher": source.publisher,
                "state": if manifest.is_some() { "queryable" } else { dataset.state.as_str() },
                "licence": source.licence,
                "licence_status": source.licence_status,
                "licence_url": source.licence_url,
                "attribution": source.attribution,
                "cadence": dataset.cadence,
                "coverage": dataset.coverage,
                "imported": manifest.is_some(),
                "rows": manifest.map(|manifest| manifest.accepted_rows),
                "period_from": manifest.and_then(|manifest| manifest.period_from.clone()),
                "period_to": manifest.and_then(|manifest| manifest.period_to.clone()),
                "raw_sha256": manifest.map(|manifest| manifest.raw_sha256.clone()),
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "official_dir": st.official_dir.as_ref().map(|dir| dir.display().to_string()),
        "imported_stores": manifests.len(),
        "datasets": datasets,
    }))
    .into_response())
}

async fn official_query(State(st): State<Shared>, UrlQuery(params): UrlQuery<OfficialQuery>) -> ApiResult {
    let dir = st
        .official_dir
        .as_ref()
        .ok_or_else(|| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "official_unavailable", "serve without --official-dir"))?;
    ensure_official_limit(params.limit)?;
    let category = match (&params.classification, &params.category) {
        (Some(classification), Some(category)) => Some(CategoryFilter::new(classification, category)),
        (None, None) => None,
        _ => return Err(ApiError::bad("classification and category must be supplied together")),
    };
    let uf_code =
        params.uf.as_deref().map(|uf| if uf.len() == 2 { official::query::uf_code(uf).unwrap_or(uf).to_string() } else { uf.to_string() });
    let geographies = split_values(params.geography.as_deref());
    let periods = split_values(params.period.as_deref());
    let request = match params.operation.as_deref().unwrap_or("lookup") {
        "lookup" => OfficialRequest::Lookup { variable: params.variable, geographies, periods, category },
        "compare" => OfficialRequest::Compare { variable: params.variable, geographies, period: periods.first().cloned(), category },
        "rank" => OfficialRequest::Rank {
            variable: params.variable,
            period: periods.first().cloned(),
            geography_level: params.level.unwrap_or_else(|| "N6".into()),
            uf_code,
            category,
            limit: params.limit.unwrap_or(10),
            descending: !params.ascending.unwrap_or(false),
        },
        "trend" => OfficialRequest::Trend {
            variable: params.variable,
            geography: geographies.first().cloned().ok_or_else(|| ApiError::bad("trend needs geography"))?,
            periods,
            category,
        },
        "ratio" => OfficialRequest::Ratio {
            numerator: params.variable,
            denominator: params.denominator.ok_or_else(|| ApiError::bad("ratio needs denominator"))?,
            geographies,
            period: periods.first().cloned(),
            category,
            scale: params.scale.unwrap_or(1.0),
        },
        other => return Err(ApiError::bad(format!("unknown official operation {other}"))),
    };
    let registry = Registry::builtin()?;
    let stores = StoreSet::open_dir(dir)
        .map_err(|error| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "official_unavailable", error.to_string()))?;
    let answer = official::query::execute(&stores, &registry, &request).map_err(|error| ApiError::bad(error.to_string()))?;
    Ok(Json(
        serde_json::to_value(answer)
            .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", error.to_string()))?,
    )
    .into_response())
}

fn split_values(value: Option<&str>) -> Vec<String> {
    value.into_iter().flat_map(|value| value.split(',')).map(str::trim).filter(|value| !value.is_empty()).map(str::to_string).collect()
}

fn ensure_official_limit(limit: Option<usize>) -> Result<(), ApiError> {
    if let Some(limit) = limit
        && (limit == 0 || limit > 100)
    {
        return Err(ApiError::bad("official limit must be between 1 and 100"));
    }
    Ok(())
}

async fn snapshot_info(State(st): State<Shared>) -> ApiResult {
    let s = st.snapshot();
    let mut v = output::info(&s);
    let m = v.as_object_mut().expect("object");
    m.remove("path");
    m.insert("sha256".into(), json!(st.sha256()));
    m.insert(
        "trust".into(),
        json!("sha256 identifies this snapshot file; source ZIP hashes are computed locally and do not authenticate Receita Federal as the origin"),
    );
    Ok(Json(v).into_response())
}

/// Snapshots and checksums offered for download, with the active one marked.
async fn distribution(State(st): State<Shared>) -> ApiResult {
    let s = st.snapshot();
    let active = s.path().file_name().map(|n| n.to_string_lossy().into_owned());
    let files: Vec<Value> = output::distributable(&st.snapshots_dir)
        .into_iter()
        .map(|(name, bytes)| {
            let sha = name.ends_with(".ocnpj").then(|| read_sidecar(&st.snapshots_dir.join(&name))).flatten();
            json!({ "name": name, "bytes": bytes, "sha256": sha, "url": format!("/downloads/{name}"), "active": Some(&name) == active.as_ref() })
        })
        .collect();
    Ok(Json(json!({
        "files": files,
        "competence": s.manifest.competence,
        "sources": s.manifest.sources,
        "how_to_verify": "download the .ocnpj and its .sha256, run `sha256sum -c`, then `opencnpj verify <file>` (add `--sources DIR` with the official ZIPs for a row-by-row check)",
    }))
    .into_response())
}

/// Streams a distributable file; supports a single `Range: bytes=a-b`.
async fn download(State(st): State<Shared>, UrlPath(name): UrlPath<String>, headers: HeaderMap) -> ApiResult {
    let files = output::distributable(&st.snapshots_dir);
    let Some((_, len)) = files.into_iter().find(|(n, _)| *n == name) else {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such file"));
    };
    let mut start = 0u64;
    let mut end = len.saturating_sub(1);
    let mut partial = false;
    if let Some(r) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("bytes=")) {
        let (a, b) = r.split_once('-').ok_or_else(|| ApiError::bad("bad range"))?;
        match (a.parse::<u64>().ok(), b.parse::<u64>().ok()) {
            (Some(a), Some(b)) => (start, end) = (a, b.min(end)),
            (Some(a), None) => start = a,
            (None, Some(n)) => start = len.saturating_sub(n),
            _ => return Err(ApiError::bad("bad range")),
        }
        if start > end || start >= len {
            return Err(ApiError::new(StatusCode::RANGE_NOT_SATISFIABLE, "bad_range", format!("file has {len} bytes")));
        }
        partial = true;
    }
    let mut f = tokio::fs::File::open(st.snapshots_dir.join(&name)).await.map_err(|e| ApiError::from(anyhow::Error::from(e)))?;
    f.seek(std::io::SeekFrom::Start(start)).await.map_err(|e| ApiError::from(anyhow::Error::from(e)))?;
    let remaining = if len == 0 { 0 } else { end - start + 1 };
    let stream = futures_util::stream::unfold((f, remaining), |(mut f, left)| async move {
        if left == 0 {
            return None;
        }
        let mut buf = vec![0u8; (1 << 20).min(left as usize)];
        match f.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok::<_, std::io::Error>(buf), (f, left - n as u64)))
            }
            Err(e) => Some((Err(e), (f, 0))),
        }
    });
    let mut resp = Response::new(Body::from_stream(stream));
    let h = resp.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    h.insert(header::CONTENT_LENGTH, HeaderValue::from(remaining));
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    if partial {
        *resp.status_mut() = StatusCode::PARTIAL_CONTENT;
        if let Ok(v) = HeaderValue::from_str(&format!("bytes {start}-{end}/{len}")) {
            resp.headers_mut().insert(header::CONTENT_RANGE, v);
        }
    }
    Ok(resp)
}

async fn dominio(State(st): State<Shared>, UrlPath(tabela): UrlPath<String>) -> ApiResult {
    let s = st.snapshot();
    match s.refs.get(&tabela) {
        Some(rows) => Ok(json_response(json!(rows.iter().map(|r| json!({"codigo": r[0], "descricao": r[1]})).collect::<Vec<_>>()))),
        None => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("unknown table; available: {:?}", s.refs.keys().collect::<Vec<_>>()),
        )),
    }
}

/// Municipalities of one state with their RFB codes, by name: reference data for filter
/// forms, served without an account like the other reference tables.
async fn municipios_uf(State(st): State<Shared>, UrlPath(uf): UrlPath<String>) -> ApiResult {
    let uf = uf.to_ascii_uppercase();
    if uf.len() != 2 || !uf.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(ApiError::bad("uf must be a two-letter state code"));
    }
    let q = parse_query(&[("uf".to_string(), uf)], &st.args, TextField::Any, false)?;
    let v = blocking(&st, move |s| {
        let mut rows: Vec<(String, String)> = search::group_counts(&s, &q, Field::Municipio, &Limits::DEFAULT)?
            .into_iter()
            .map(|(code, _)| (s.label("municipios", &code).unwrap_or_default().to_string(), code))
            .collect();
        rows.sort();
        Ok(json!(rows.into_iter().map(|(name, code)| json!({ "codigo": code, "descricao": name })).collect::<Vec<_>>()))
    })
    .await?;
    Ok(json_response(v))
}

async fn dominios(State(st): State<Shared>) -> ApiResult {
    let s = st.snapshot();
    Ok(Json(json!(
        s.refs.iter().map(|(k, v)| json!({ "tabela": k, "linhas": v.len(), "url": format!("/api/v1/dominios/{k}") })).collect::<Vec<_>>()
    ))
    .into_response())
}

/// Self-description of every search parameter.
async fn campos(State(st): State<Shared>) -> ApiResult {
    let s = st.snapshot();
    let numeric: Vec<Value> = NumField::ALL
        .iter()
        .map(|f| {
            json!({
                "campo": f.name(),
                "tipo": if f.is_date() { "data (YYYY-MM-DD, YYYYMMDD ou ano)" } else { "valor em reais" },
                "parametros": [format!("{}_de", f.name()), format!("{}_ate", f.name()), format!("{}_vazio", f.name())],
                "disponivel": s.numeric(*f).is_some(),
            })
        })
        .collect();
    Ok(Json(json!({
        "texto": {
            "q": "name tokens (razão social and nome fantasia), a CNPJ or CNPJ prefix (8+ digits), or an 11-digit CPF inside an MEI name",
            "field": ["any", "razao_social", "nome_fantasia"],
            "fuzzy": ["auto (default: retry with typo tolerance when nothing matches)", "true", "false"],
        },
        "filtros": Field::FILTERS.iter().map(|f| f.name()).collect::<Vec<_>>(),
        "filtros_notas": {
            "cnae / cnae_secundaria / cnae_qualquer": "7 digits, or a prefix (2 = divisão, 3 = grupo, 5 = classe), or a section letter A-U",
            "cep": "8 digits or a prefix", "natureza": "4 digits or a prefix", "ddd": "area code",
            "contato": "email or telefone (aliases tem_email=true, tem_telefone=true)",
            "cpf": "owner CPF in MEI/EI names (exact) or masked partner CPF (6 middle digits, possible match)",
            "cpf_titular": "owner CPF only (exact)",
            "socio_cnpj / socio_cpf": "partner document: 14-digit CNPJ (exact) or CPF (masked match)",
            "cnpj": "CNPJ prefix of 1 to 14 characters",
            "mei_ativo / simples_ativo": "opted in and never excluded",
            "values": "comma-separated values are OR-ed; repeated parameters are AND-ed; codes accept a trailing *",
        },
        "intervalos": numeric,
        "ordenar": NumField::ALL.iter().flat_map(|f| [format!("{}:asc", f.name()), format!("{}:desc", f.name())]).collect::<Vec<_>>(),
        "paginacao": { "limit": st.args.max_limit, "offset": "no limit", "cursor": "next_cursor from filter-only queries" },
        "agrupar": "GET /api/v1/stats?group_by=<filter field | ano_<date column>>&<filters>",
    }))
    .into_response())
}

async fn root() -> Json<Value> {
    Json(json!({
        "service": "opencnpj",
        "version": env!("CARGO_PKG_VERSION"),
        "endpoints": [
            "/api/v1/cnpj/{cnpj}", "/api/v1/empresas/{cnpj_basico}", "/api/v1/cpf/{cpf}", "/api/v1/search", "/api/v1/stats",
            "/api/v1/export", "/api/v1/campos", "/api/v1/dominios", "/api/v1/snapshot", "/api/v1/distribution", "/downloads/{file}",
            "/api/v1/platform",
            "/healthz", "/readyz", "/metrics"
        ],
    }))
}

async fn healthz() -> &'static str {
    "ok"
}

async fn readyz(State(st): State<Shared>) -> Response {
    let s = st.snapshot();
    Json(json!({ "status": "ready", "competence": s.manifest.competence, "documents": s.doc_count() })).into_response()
}

pub async fn prometheus(State(st): State<Shared>) -> Response {
    let s = st.snapshot();
    let mut out = st.metrics.prometheus();
    let c = s.cache_stats();
    out.push_str(&format!("opencnpj_snapshot_documents {}\n", s.doc_count()));
    out.push_str(&format!("opencnpj_snapshot_bytes {}\n", s.file_len()));
    out.push_str(&format!("opencnpj_postings_cache_bytes {}\n", c.bytes));
    out.push_str(&format!("opencnpj_postings_cache_hits_total {}\n", c.hits));
    out.push_str(&format!("opencnpj_postings_cache_misses_total {}\n", c.misses));
    if let Some(rc) = &st.gate.cache {
        let v = rc.stats();
        for k in ["entries", "bytes", "hits", "misses", "coalesced", "not_modified"] {
            out.push_str(&format!("opencnpj_response_cache_{k} {}\n", v[k]));
        }
    }
    let g = st.gate.stats();
    for k in ["rate_limited", "quota_exceeded", "auth"] {
        out.push_str(&format!("opencnpj_rejected_total{{reason=\"{k}\"}} {}\n", g["rejected"][k]));
    }
    let (anon, file) = crate::admin::rss();
    out.push_str(&format!("opencnpj_process_rss_anon_bytes {anon}\nopencnpj_process_rss_file_bytes {file}\n"));
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], out).into_response()
}

pub fn resolve_current(dir: &Path) -> Result<PathBuf> {
    let name = std::fs::read_to_string(dir.join("current")).with_context(|| format!("reading {}/current", dir.display()))?;
    let p = dir.join(name.trim());
    anyhow::ensure!(p.starts_with(dir) && !name.contains(".."), "current must name a file inside the data dir");
    Ok(p)
}

pub fn read_sidecar(p: &Path) -> Option<String> {
    std::fs::read_to_string(p.with_extension("ocnpj.sha256")).ok()?.split_whitespace().next().map(str::to_string)
}

pub fn router(st: Shared) -> Router {
    // Idempotent JSON routes: shared response cache (inside the gate, so hits still count).
    let cached = Router::new()
        .route("/api/v1/cnpj/{cnpj}", get(get_cnpj))
        .route("/api/v1/estabelecimentos/{cnpj}", get(get_cnpj))
        .route("/api/v1/empresas/{basico}", get(get_cnpj))
        .route("/api/v1/cpf/{cpf}", get(get_cpf))
        .route("/api/v1/search", get(search_route))
        .route("/api/v1/busca", get(search_route))
        .route("/api/v1/empresas/search", get(legacy_empresas_search))
        .route("/api/v1/estabelecimentos/search", get(search_route))
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/official/query", get(official_query))
        .route_layer(middleware::from_fn_with_state(st.clone(), crate::gate::cache));
    let data = Router::new()
        .route("/api/v1/export", get(export))
        .route("/downloads/{name}", get(download))
        .merge(cached)
        .merge(crate::assistant::http::router())
        .route_layer(middleware::from_fn_with_state(st.clone(), crate::gate::gate));
    let mut public = Router::new()
        .route("/api/v1", get(root))
        .route("/api/v1/platform", get(crate::assistant::http::platform))
        .route("/api/v1/trial", get(crate::trial::status))
        .route("/api/v1/campos", get(campos))
        .route("/api/v1/snapshot", get(snapshot_info))
        .route("/api/v1/distribution", get(distribution))
        .route("/api/v1/official/sources", get(official_sources))
        .route("/api/v1/dominios", get(dominios))
        .route("/api/v1/dominios/{tabela}", get(dominio))
        .route("/api/v1/dominios/municipios/{uf}", get(municipios_uf))
        .route("/healthz", get(healthz))
        .route("/health", get(healthz))
        .route("/readyz", get(readyz))
        .merge(crate::auth_api::router())
        .merge(crate::conversations::router());
    if !st.gate.enabled() {
        // With accounts, Prometheus metrics move to the admin listener.
        public = public.route("/metrics", get(prometheus));
    }
    // The web front end, when configured, owns `/`; otherwise `/` describes the service.
    public = if st.args.web.is_some() { public.fallback(crate::assistant::http::web) } else { public.route("/", get(root)) };
    public.merge(data).layer(middleware::from_fn_with_state(st.clone(), track)).with_state(st)
}

pub fn state(args: ServeArgs) -> Result<Shared> {
    if args.auth == AuthMode::Trial && args.trust_proxy {
        let admin_is_local = args.admin_bind.is_empty() || args.admin_bind.parse::<SocketAddr>().is_ok_and(|addr| addr.ip().is_loopback());
        anyhow::ensure!(args.bind.ip().is_loopback() && admin_is_local, "trial proxy mode requires loopback listeners");
    }
    let path = match (&args.snapshot, &args.data_dir) {
        (Some(p), _) => p.clone(),
        (None, Some(d)) => resolve_current(d)?,
        (None, None) => anyhow::bail!("--snapshot or --data-dir is required"),
    };
    let t0 = Instant::now();
    let snap = Snapshot::open(&path).with_context(|| format!("opening {}", path.display()))?;
    snap.set_postings_cache_bytes(args.postings_cache_mib << 20);
    eprintln!("opened {} ({} documents) in {:.1} ms", path.display(), snap.doc_count(), t0.elapsed().as_secs_f64() * 1e3);
    let snapshots_dir = match &args.data_dir {
        Some(d) => d.join("snapshots"),
        None => path.parent().map_or_else(|| PathBuf::from("."), Path::to_path_buf),
    };
    let jobs_dir = args.data_dir.as_ref().map_or_else(|| snapshots_dir.join("jobs"), |d| d.join("jobs"));
    let base = args.data_dir.clone().unwrap_or_else(|| snapshots_dir.clone());
    let (accounts, mailer, trial) = if args.auth == AuthMode::Off {
        (None, None, None)
    } else {
        let db = args.accounts_db.clone().unwrap_or_else(|| base.join("accounts.sqlite"));
        let acc = crate::accounts::Accounts::open(&db, args.session_hours)?;
        let public_url = args.public_url.clone().unwrap_or_else(|| format!("http://{}", args.bind));
        let mailer = crate::mail::Mailer::new(args.smtp_url.as_deref(), &args.mail_from, base.join("outbox"), &public_url)?;
        eprintln!("accounts in {} (auth {}), mail via {}", db.display(), args.auth.name(), mailer.describe());
        let trial = if args.auth == AuthMode::Trial {
            let secure = reqwest::Url::parse(&public_url).is_ok_and(|url| url.scheme() == "https");
            Some(Arc::new(crate::trial::Trial::open(&db, args.trial_deny_networks.as_deref(), secure)?))
        } else {
            None
        };
        (Some(Arc::new(acc)), Some(Arc::new(mailer)), trial)
    };
    let cache = (args.response_cache_mib > 0).then(|| ResponseCache::new(args.response_cache_mib, args.response_cache_ttl_secs));
    let mut gate = Gate::new(args.auth, accounts, cache, args.trust_proxy);
    gate.set_handover_path(base.join(".verified-keys"));
    gate.take_handover();
    let gate = Arc::new(gate);
    Ok(Arc::new(AppState {
        sha256: RwLock::new(read_sidecar(&path)),
        snap: RwLock::new(Arc::new(snap)),
        sem: Arc::new(Semaphore::new(args.max_concurrency.max(1))),
        fast_sem: Arc::new(Semaphore::new(args.max_concurrency.max(1))),
        metrics: Metrics::default(),
        privacy: Privacy { hide_socios: args.hide_socios, hide_contacts: args.hide_contacts },
        snapshots_dir,
        jobs: crate::admin::Jobs::new(jobs_dir),
        index_breakdown: Mutex::new(None),
        official_dir: args.official_dir.clone(),
        assistant: crate::assistant::http::open(&args, &path)?,
        gate,
        trial,
        mailer,
        updater: crate::autoupdate::Updater::default(),
        generation: AtomicU64::new(snapshot_identity(&path)),
        draining: std::sync::atomic::AtomicBool::new(false),
        args,
    }))
}

/// Polls `<data-dir>/current` and swaps the snapshot when it changes.
async fn reload_loop(st: Shared, dir: PathBuf) {
    let mut last: Option<(PathBuf, SystemTime)> = None;
    loop {
        tokio::time::sleep(Duration::from_secs(st.args.reload_secs.max(1))).await;
        let Ok(path) = resolve_current(&dir) else { continue };
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        let cur = st.snapshot().path().to_path_buf();
        let changed = path != cur || last.as_ref().is_some_and(|(p, t)| *p == path && Some(*t) != mtime);
        if let Some(t) = mtime {
            last = Some((path.clone(), t));
        }
        if changed {
            let st2 = st.clone();
            let p2 = path.clone();
            match tokio::task::spawn_blocking(move || st2.switch_to(&p2)).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => eprintln!("refusing to switch to {}: {e:#}", path.display()),
                Err(e) => eprintln!("reload task failed: {e}"),
            }
        }
    }
}

/// Binds a TCP listener, optionally with SO_REUSEPORT (zero-downtime restarts).
fn listen(addr: SocketAddr, reuse_port: bool) -> Result<tokio::net::TcpListener> {
    let sock = socket2::Socket::new(socket2::Domain::for_address(addr), socket2::Type::STREAM, Some(socket2::Protocol::TCP))?;
    sock.set_reuse_address(true)?;
    #[cfg(all(unix, not(any(target_os = "solaris", target_os = "illumos"))))]
    if reuse_port {
        sock.set_reuse_port(true)?;
    }
    #[cfg(not(all(unix, not(any(target_os = "solaris", target_os = "illumos")))))]
    if reuse_port {
        anyhow::bail!("--reuse-port is not supported on this platform");
    }
    sock.set_nonblocking(true)?;
    sock.bind(&addr.into()).with_context(|| format!("binding {addr}"))?;
    sock.listen(4096)?;
    Ok(tokio::net::TcpListener::from_std(sock.into())?)
}

/// Gzip/brotli/zstd at the fastest level; never for snapshot downloads.
fn compressed(app: Router, enabled: bool) -> Router {
    use tower_http::compression::predicate::{NotForContentType, Predicate};
    use tower_http::compression::{CompressionLayer, CompressionLevel, DefaultPredicate};
    if !enabled {
        return app;
    }
    let when = DefaultPredicate::new().and(NotForContentType::const_new("application/octet-stream"));
    app.layer(CompressionLayer::new().quality(CompressionLevel::Fastest).compress_when(when))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = term => {},
    }
    eprintln!("shutting down: draining in-flight requests");
}

pub fn run(args: ServeArgs) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async move {
        let st = state(args.clone())?;
        st.gate.start_background();
        if let Some(dir) = args.data_dir.clone() {
            tokio::spawn(reload_loop(st.clone(), dir));
            tokio::spawn(crate::autoupdate::run(st.clone()));
        }
        if let Some(acc) = st.gate.accounts.clone() {
            let admins = tokio::task::spawn_blocking(move || {
                acc.list_users(&crate::accounts::UserFilter { role: Some("admin".into()), ..Default::default() })
            })
            .await??;
            if admins["total"] == 0 {
                eprintln!("no admin account yet: create one with `opencnpj accounts create --role admin --username NAME --email ADDRESS`");
            }
        }
        if !args.admin_bind.is_empty() {
            let addr: SocketAddr = args.admin_bind.parse().context("bad --admin-bind")?;
            let admin = listen(addr, args.reuse_port)?;
            let auth = if st.gate.enabled() { "staff login required" } else { "no authentication" };
            eprintln!("admin suite on http://{addr}/admin ({auth})");
            let app = compressed(crate::admin::router(st.clone()), !args.no_compression);
            tokio::spawn(async move {
                if let Err(e) = axum::serve(admin, app.into_make_service_with_connect_info::<SocketAddr>()).await {
                    eprintln!("admin server stopped: {e}");
                }
            });
        }
        let listener = listen(args.bind, args.reuse_port)?;
        eprintln!("listening on http://{} (auth {})", args.bind, args.auth.name());
        let app = compressed(router(st.clone()), !args.no_compression);
        let st_down = st.clone();
        let shutdown = async move {
            shutdown_signal().await;
            // Hand verified keys and cached responses to the successor first, then tell
            // clients to reconnect (they land on the successor through SO_REUSEPORT), then
            // drain whatever is left.
            let st3 = st_down.clone();
            let _ = tokio::task::spawn_blocking(move || st3.gate.give_handover()).await;
            st_down.draining.store(true, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_secs(st_down.args.drain_secs)).await;
        };
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(shutdown).await?;
        let st2 = st.clone();
        tokio::task::spawn_blocking(move || st2.gate.flush()).await?;
        eprintln!("usage flushed; bye");
        Ok(())
    })
}
