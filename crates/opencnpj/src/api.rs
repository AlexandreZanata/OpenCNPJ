//! HTTP API (axum). CPU-bound work runs on the blocking pool behind a semaphore; every
//! request has a deadline. Snapshots are swapped atomically: in-flight requests keep the
//! `Arc<Snapshot>` they started with.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use axum::body::Body;
use axum::extract::{Path as UrlPath, Query as UrlQuery, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use opencnpj_store::Snapshot;
use opencnpj_store::postings::Field;
use opencnpj_store::search::{self, Limits, Query, TextField};
use serde_json::{Value, json};
use tokio::sync::Semaphore;

use crate::output::{self, LookupError, Privacy};

#[derive(clap::Args, Clone)]
pub struct ServeArgs {
    /// Snapshot file to serve.
    #[arg(long, conflicts_with = "data_dir")]
    pub snapshot: Option<PathBuf>,
    /// Directory with a `current` file naming the active snapshot (hot reload).
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub bind: SocketAddr,
    /// Concurrent CPU-bound requests.
    #[arg(long, default_value_t = crate::default_threads() * 2)]
    pub max_concurrency: usize,
    #[arg(long, default_value_t = 5000)]
    pub timeout_ms: u64,
    #[arg(long, default_value_t = 100)]
    pub max_limit: usize,
    /// Deepest offset allowed for ranked results (anti-enumeration).
    #[arg(long, default_value_t = 1000)]
    pub max_offset: usize,
    /// Enable `/api/v1/export` (bulk download).
    #[arg(long)]
    pub enable_export: bool,
    #[arg(long, default_value_t = 100_000)]
    pub export_max_rows: u64,
    /// Omit partners (sócios) from responses.
    #[arg(long)]
    pub hide_socios: bool,
    /// Omit phone numbers and e-mail from responses.
    #[arg(long)]
    pub hide_contacts: bool,
    /// Seconds between checks of `<data-dir>/current`.
    #[arg(long, default_value_t = 30)]
    pub reload_secs: u64,
}

struct Metrics {
    started: Instant,
    requests: AtomicU64,
    errors: AtomicU64,
    timeouts: AtomicU64,
    rejected: AtomicU64,
    by_route: RwLock<HashMap<&'static str, (u64, f64)>>,
    reloads: AtomicU64,
}

pub struct AppState {
    snap: RwLock<Arc<Snapshot>>,
    sha256: RwLock<Option<String>>,
    args: ServeArgs,
    sem: Arc<Semaphore>,
    metrics: Metrics,
    privacy: Privacy,
}

impl AppState {
    fn snapshot(&self) -> Arc<Snapshot> {
        self.snap.read().expect("lock").clone()
    }
}

type Shared = Arc<AppState>;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into() }
    }

    fn bad(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
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
        if let Some(LookupError::Invalid(m)) = e.downcast_ref::<LookupError>() {
            return ApiError::new(StatusCode::BAD_REQUEST, "invalid_cnpj", m.clone());
        }
        if let Some(opencnpj_store::Error::Invalid(m)) = e.downcast_ref::<opencnpj_store::Error>() {
            return ApiError::bad(m.clone());
        }
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", e.to_string())
    }
}

type ApiResult = Result<Response, ApiError>;

/// Runs `f` on the blocking pool with a permit and a deadline.
async fn blocking<T: Send + 'static>(
    st: &Shared,
    route: &'static str,
    f: impl FnOnce(Arc<Snapshot>) -> Result<T> + Send + 'static,
) -> Result<T, ApiError> {
    let t0 = Instant::now();
    st.metrics.requests.fetch_add(1, Ordering::Relaxed);
    let timeout = Duration::from_millis(st.args.timeout_ms);
    let permit = match tokio::time::timeout(timeout / 2, st.sem.clone().acquire_owned()).await {
        Ok(Ok(p)) => p,
        _ => {
            st.metrics.rejected.fetch_add(1, Ordering::Relaxed);
            return Err(ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "overloaded", "server is busy, retry later"));
        }
    };
    let snap = st.snapshot();
    let task = tokio::task::spawn_blocking(move || {
        let r = f(snap);
        drop(permit);
        r
    });
    let out = match tokio::time::timeout(timeout, task).await {
        Ok(Ok(Ok(v))) => Ok(v),
        Ok(Ok(Err(e))) => Err(ApiError::from(e)),
        Ok(Err(e)) => Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", e.to_string())),
        Err(_) => {
            st.metrics.timeouts.fetch_add(1, Ordering::Relaxed);
            Err(ApiError::new(StatusCode::GATEWAY_TIMEOUT, "timeout", "query exceeded the time limit"))
        }
    };
    if out.as_ref().is_err_and(|e| e.status.is_server_error()) {
        st.metrics.errors.fetch_add(1, Ordering::Relaxed);
    }
    let mut m = st.metrics.by_route.write().expect("lock");
    let e = m.entry(route).or_insert((0, 0.0));
    e.0 += 1;
    e.1 += t0.elapsed().as_secs_f64();
    out
}

fn json_response(v: Value) -> Response {
    let mut r = Json(v).into_response();
    r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=3600"));
    r
}

async fn get_cnpj(State(st): State<Shared>, UrlPath(id): UrlPath<String>, UrlQuery(p): UrlQuery<HashMap<String, String>>) -> ApiResult {
    if id.len() > 32 {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "invalid_cnpj", "invalid cnpj"));
    }
    let num = |k: &str, d: usize| p.get(k).map_or(Ok(d), |v| v.parse::<usize>().map_err(|_| ApiError::bad(format!("bad {k}"))));
    let offset = num("estab_offset", 0)?;
    let limit = num("estab_limit", output::DEFAULT_ESTAB_PAGE)?;
    if limit == 0 || limit > 1000 {
        return Err(ApiError::bad("estab_limit must be between 1 and 1000"));
    }
    let privacy = st.privacy;
    let v = blocking(&st, "cnpj", move |s| output::lookup_page(&s, &id, privacy, offset, limit)).await?;
    match v {
        Some(v) => Ok(json_response(v)),
        None => Err(ApiError::new(StatusCode::NOT_FOUND, "cnpj_not_found", "cnpj not found")),
    }
}

/// Builds a query from URL parameters (filters accept comma-separated values).
fn query_from(params: &HashMap<String, String>, args: &ServeArgs, ranked_default_field: TextField) -> Result<Query, ApiError> {
    let mut q = Query { limit: 20, ..Default::default() };
    for (k, v) in params {
        let k = k.as_str();
        match k {
            "q" | "razao_social" | "nome_fantasia" | "nome" => {
                if v.len() > 200 {
                    return Err(ApiError::bad("query too long"));
                }
                q.text = Some(v.clone()).filter(|s| !s.trim().is_empty());
                q.text_field = match k {
                    "razao_social" => TextField::Razao,
                    "nome_fantasia" => TextField::Fantasia,
                    _ => ranked_default_field,
                };
            }
            "field" => {
                q.text_field = match v.as_str() {
                    "any" => TextField::Any,
                    "razao" | "razao_social" => TextField::Razao,
                    "fantasia" | "nome_fantasia" => TextField::Fantasia,
                    _ => return Err(ApiError::bad("field must be any, razao_social or nome_fantasia")),
                }
            }
            "fuzzy" => q.fuzzy = matches!(v.as_str(), "1" | "true"),
            "limit" => q.limit = v.parse().map_err(|_| ApiError::bad("bad limit"))?,
            "offset" => q.offset = v.parse().map_err(|_| ApiError::bad("bad offset"))?,
            "cursor" => q.after = Some(v.parse().map_err(|_| ApiError::bad("bad cursor"))?),
            "format" | "group_by" => {}
            _ => {
                let alias = match k {
                    "cnae_principal" | "cnae_fiscal_principal" => "cnae",
                    "cnae_fiscal_secundaria" => "cnae_secundaria",
                    "situacao_cadastral" => "situacao",
                    "porte_empresa" => "porte",
                    "natureza_juridica" => "natureza",
                    "opcao_simples" => "simples",
                    "opcao_mei" => "mei",
                    "matriz" | "identificador_matriz_filial" => "matriz_filial",
                    other => other,
                };
                let f = Field::from_name(alias)
                    .filter(|f| Field::FILTERS.contains(f))
                    .ok_or_else(|| ApiError::bad(format!("unknown parameter {k}")))?;
                let values: Vec<String> = v.split(',').filter(|s| !s.is_empty()).take(50).map(str::to_string).collect();
                q.filters.push((f, values));
            }
        }
    }
    if q.limit == 0 || q.limit > args.max_limit {
        return Err(ApiError::bad(format!("limit must be between 1 and {}", args.max_limit)));
    }
    if q.offset > args.max_offset {
        return Err(ApiError::bad(format!("offset above {}; use cursor pagination with filters", args.max_offset)));
    }
    if q.text.is_none() && q.filters.is_empty() {
        return Err(ApiError::bad("provide q (name) and/or at least one filter"));
    }
    Ok(q)
}

async fn search_handler(st: Shared, params: HashMap<String, String>, default_field: TextField) -> ApiResult {
    let q = query_from(&params, &st.args, default_field)?;
    let v = blocking(&st, "search", move |s| {
        let r = search::search(&s, &q, &Limits::DEFAULT)?;
        let docs = s.docs(&crate::sorted(&r.docs))?;
        Ok(output::search_json(&s, &r, docs, false))
    })
    .await?;
    Ok(json_response(v))
}

async fn search(State(st): State<Shared>, UrlQuery(p): UrlQuery<HashMap<String, String>>) -> ApiResult {
    search_handler(st, p, TextField::Any).await
}

async fn legacy_empresas_search(State(st): State<Shared>, UrlQuery(p): UrlQuery<HashMap<String, String>>) -> ApiResult {
    search_handler(st, p, TextField::Razao).await
}

async fn stats(State(st): State<Shared>, UrlQuery(p): UrlQuery<HashMap<String, String>>) -> ApiResult {
    let group = p.get("group_by").cloned().unwrap_or_else(|| "uf".into());
    let field = Field::from_name(&group).filter(|f| Field::FILTERS.contains(f)).ok_or_else(|| ApiError::bad("bad group_by"))?;
    let mut q = Query::default();
    if p.keys().any(|k| k != "group_by") {
        let mut p2 = p.clone();
        p2.remove("group_by");
        p2.insert("limit".into(), "1".into());
        q = query_from(&p2, &st.args, TextField::Any)?;
    }
    let v = blocking(&st, "stats", move |s| {
        let counts = search::group_counts(&s, &q, field, &Limits::DEFAULT)?;
        let rows: Vec<Value> = counts
            .into_iter()
            .map(|(value, n)| {
                let label = match field {
                    Field::Municipio => s.label("municipios", &value),
                    Field::Cnae | Field::CnaeSecundaria => s.label("cnaes", &value),
                    Field::Natureza => s.label("naturezas", &value),
                    Field::Situacao => opencnpj_core::model::fixed_label("situacao_cadastral", &value),
                    Field::Porte => opencnpj_core::model::fixed_label("porte_empresa", &value),
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

async fn export(State(st): State<Shared>, UrlQuery(p): UrlQuery<HashMap<String, String>>) -> ApiResult {
    if !st.args.enable_export {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "export_disabled", "export is disabled on this server"));
    }
    let format = p.get("format").cloned().unwrap_or_else(|| "ndjson".into());
    if format != "ndjson" && format != "csv" {
        return Err(ApiError::bad("format must be ndjson or csv"));
    }
    let mut p2 = p.clone();
    p2.insert("limit".into(), "1".into());
    let mut q = query_from(&p2, &st.args, TextField::Any)?;
    q.limit = usize::MAX;
    let max = st.args.export_max_rows;
    let permit = match tokio::time::timeout(Duration::from_millis(st.args.timeout_ms), st.sem.clone().acquire_owned()).await {
        Ok(Ok(p)) => p,
        _ => return Err(ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "overloaded", "server is busy, retry later")),
    };
    let snap = st.snapshot();
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Vec<u8>, std::io::Error>>(8);
    let fmt = format.clone();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut w = ChannelWriter { tx, buf: Vec::with_capacity(1 << 16) };
        if let Err(e) = output::export(&snap, &q, &fmt, max, &mut w) {
            let _ = w.tx.blocking_send(Err(std::io::Error::other(e.to_string())));
        }
    });
    let stream = futures_util::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|item| (item, rx)) });
    let ctype = if format == "csv" { "text/csv; charset=utf-8" } else { "application/x-ndjson" };
    Ok(([(header::CONTENT_TYPE, ctype)], Body::from_stream(stream)).into_response())
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

async fn snapshot_info(State(st): State<Shared>) -> ApiResult {
    let s = st.snapshot();
    let mut v = output::info(&s);
    let m = v.as_object_mut().expect("object");
    m.remove("path");
    m.insert("sha256".into(), json!(*st.sha256.read().expect("lock")));
    m.insert(
        "trust".into(),
        json!("sha256 identifies this snapshot file; source ZIP hashes are computed locally and do not authenticate Receita Federal as the origin"),
    );
    Ok(Json(v).into_response())
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

async fn healthz() -> &'static str {
    "ok"
}

async fn readyz(State(st): State<Shared>) -> Response {
    let s = st.snapshot();
    Json(json!({ "status": "ready", "competence": s.manifest.competence, "documents": s.doc_count() })).into_response()
}

async fn metrics(State(st): State<Shared>) -> Response {
    let m = &st.metrics;
    let s = st.snapshot();
    let mut out = String::new();
    out.push_str(&format!("opencnpj_uptime_seconds {}\n", m.started.elapsed().as_secs_f64()));
    out.push_str(&format!("opencnpj_requests_total {}\n", m.requests.load(Ordering::Relaxed)));
    out.push_str(&format!("opencnpj_errors_total {}\n", m.errors.load(Ordering::Relaxed)));
    out.push_str(&format!("opencnpj_timeouts_total {}\n", m.timeouts.load(Ordering::Relaxed)));
    out.push_str(&format!("opencnpj_overload_rejections_total {}\n", m.rejected.load(Ordering::Relaxed)));
    out.push_str(&format!("opencnpj_snapshot_reloads_total {}\n", m.reloads.load(Ordering::Relaxed)));
    out.push_str(&format!("opencnpj_snapshot_documents {}\n", s.doc_count()));
    out.push_str(&format!("opencnpj_snapshot_bytes {}\n", s.file_len()));
    out.push_str(&format!("opencnpj_process_peak_rss_bytes {}\n", opencnpj_ingest::build::peak_rss()));
    for (route, (n, secs)) in m.by_route.read().expect("lock").iter() {
        out.push_str(&format!("opencnpj_route_requests_total{{route=\"{route}\"}} {n}\n"));
        out.push_str(&format!("opencnpj_route_seconds_total{{route=\"{route}\"}} {secs}\n"));
    }
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], out).into_response()
}

fn resolve_current(dir: &Path) -> Result<PathBuf> {
    let name = std::fs::read_to_string(dir.join("current")).with_context(|| format!("reading {}/current", dir.display()))?;
    let p = dir.join(name.trim());
    anyhow::ensure!(p.starts_with(dir) && !name.contains(".."), "current must name a file inside the data dir");
    Ok(p)
}

fn read_sidecar(p: &Path) -> Option<String> {
    std::fs::read_to_string(p.with_extension("ocnpj.sha256")).ok()?.split_whitespace().next().map(str::to_string)
}

pub fn router(st: Shared) -> Router {
    Router::new()
        .route("/api/v1/cnpj/{cnpj}", get(get_cnpj))
        .route("/api/v1/estabelecimentos/{cnpj}", get(get_cnpj))
        .route("/api/v1/empresas/{basico}", get(get_cnpj))
        .route("/api/v1/search", get(search))
        .route("/api/v1/empresas/search", get(legacy_empresas_search))
        .route("/api/v1/estabelecimentos/search", get(search))
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/export", get(export))
        .route("/api/v1/snapshot", get(snapshot_info))
        .route("/api/v1/dominios/{tabela}", get(dominio))
        .route("/healthz", get(healthz))
        .route("/health", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics))
        .layer(tower::limit::ConcurrencyLimitLayer::new(4096))
        .with_state(st)
}

pub fn state(args: ServeArgs) -> Result<Shared> {
    let path = match (&args.snapshot, &args.data_dir) {
        (Some(p), _) => p.clone(),
        (None, Some(d)) => resolve_current(d)?,
        (None, None) => anyhow::bail!("--snapshot or --data-dir is required"),
    };
    let t0 = Instant::now();
    let snap = Snapshot::open(&path).with_context(|| format!("opening {}", path.display()))?;
    eprintln!("opened {} ({} documents) in {:.1} ms", path.display(), snap.doc_count(), t0.elapsed().as_secs_f64() * 1e3);
    let privacy = Privacy { hide_socios: args.hide_socios, hide_contacts: args.hide_contacts };
    Ok(Arc::new(AppState {
        sha256: RwLock::new(read_sidecar(&path)),
        snap: RwLock::new(Arc::new(snap)),
        sem: Arc::new(Semaphore::new(args.max_concurrency.max(1))),
        metrics: Metrics {
            started: Instant::now(),
            requests: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            timeouts: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
            by_route: RwLock::new(HashMap::new()),
            reloads: AtomicU64::new(0),
        },
        privacy,
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
        if !changed {
            continue;
        }
        let p2 = path.clone();
        match tokio::task::spawn_blocking(move || Snapshot::open(&p2)).await {
            Ok(Ok(s)) => {
                *st.snap.write().expect("lock") = Arc::new(s);
                *st.sha256.write().expect("lock") = read_sidecar(&path);
                st.metrics.reloads.fetch_add(1, Ordering::Relaxed);
                eprintln!("switched to snapshot {}", path.display());
            }
            Ok(Err(e)) => eprintln!("refusing to switch to {}: {e}", path.display()),
            Err(e) => eprintln!("reload task failed: {e}"),
        }
    }
}

pub fn run(args: ServeArgs) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async move {
        let st = state(args.clone())?;
        if let Some(dir) = args.data_dir.clone() {
            tokio::spawn(reload_loop(st.clone(), dir));
        }
        let listener = tokio::net::TcpListener::bind(args.bind).await?;
        eprintln!("listening on http://{}", args.bind);
        axum::serve(listener, router(st))
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
        Ok(())
    })
}
