//! Public HTTP API (axum). CPU-bound work runs on the blocking pool behind a semaphore.
//! Snapshots are swapped atomically: in-flight requests keep the `Arc<Snapshot>` they
//! started with. The admin suite (`admin.rs`) is served on its own listener.
//!
//! This build has no authentication and no request limits by default (page size up to
//! `--max-limit`, no offset cap, export on, no rate limiting). Put a gateway in front
//! before exposing it publicly.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
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
use opencnpj_store::Snapshot;
use opencnpj_store::numeric::{self, NumField};
use opencnpj_store::postings::Field;
use opencnpj_store::search::{self, Limits, Query, TextField};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::Semaphore;

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
}

pub struct AppState {
    snap: RwLock<Arc<Snapshot>>,
    sha256: RwLock<Option<String>>,
    pub args: ServeArgs,
    pub sem: Arc<Semaphore>,
    pub metrics: Metrics,
    pub privacy: Privacy,
    /// Where `.ocnpj` files live (data dir `snapshots/`, or the served file's folder).
    pub snapshots_dir: PathBuf,
    pub jobs: crate::admin::Jobs,
    /// `info --index` is a full term-dictionary scan: computed once per snapshot.
    pub index_breakdown: Mutex<Option<(PathBuf, Value)>>,
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
        self.metrics.reloads.fetch_add(1, Ordering::Relaxed);
        eprintln!("serving {}", path.display());
        Ok(())
    }
}

pub type Shared = Arc<AppState>;

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
    let timeout = match st.args.timeout_ms {
        0 => Duration::from_secs(86_400 * 365),
        ms => Duration::from_millis(ms),
    };
    let permit = match tokio::time::timeout(timeout, st.sem.clone().acquire_owned()).await {
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
    let resp = next.run(req).await;
    st.metrics.record(&route, t0.elapsed().as_micros() as u64, resp.status().as_u16());
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
    if default_field != TextField::Any && !pairs.iter().any(|(k, _)| k == "field") {
        q.text_field = default_field;
    }
    if require && !params::has_criteria(&q) {
        return Err(ApiError::bad("provide q (name, CNPJ or CPF) and/or at least one filter; see /api/v1/campos"));
    }
    Ok(q)
}

async fn get_cnpj(State(st): State<Shared>, UrlPath(id): UrlPath<String>, UrlQuery(p): UrlQuery<Vec<(String, String)>>) -> ApiResult {
    if id.len() > 32 {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "invalid_document", "invalid cnpj"));
    }
    let num = |k: &str, d: usize| {
        p.iter().find(|(a, _)| a == k).map_or(Ok(d), |(_, v)| v.parse::<usize>().map_err(|_| ApiError::bad(format!("bad {k}"))))
    };
    let offset = num("estab_offset", 0)?;
    let limit = num("estab_limit", output::DEFAULT_ESTAB_PAGE)?;
    if limit == 0 || limit > st.args.max_limit {
        return Err(ApiError::bad(format!("estab_limit must be between 1 and {}", st.args.max_limit)));
    }
    let privacy = st.privacy;
    let v = blocking(&st, move |s| output::lookup_page(&s, &id, privacy, offset, limit)).await?;
    match v {
        Some(v) => Ok(json_response(v)),
        None => Err(ApiError::new(StatusCode::NOT_FOUND, "cnpj_not_found", "cnpj not found")),
    }
}

async fn get_cpf(State(st): State<Shared>, UrlPath(cpf): UrlPath<String>, UrlQuery(p): UrlQuery<Vec<(String, String)>>) -> ApiResult {
    let limit =
        p.iter().find(|(k, _)| k == "limit").map_or(Ok(100), |(_, v)| v.parse::<usize>().map_err(|_| ApiError::bad("bad limit")))?;
    let limit = limit.clamp(1, st.args.max_limit);
    let privacy = st.privacy;
    let v = blocking(&st, move |s| output::cpf_lookup(&s, &cpf, limit, privacy)).await?;
    Ok(json_response(v))
}

async fn search_handler(st: Shared, pairs: Vec<(String, String)>, default_field: TextField) -> ApiResult {
    let q = parse_query(&pairs, &st.args, default_field, true)?;
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

async fn search_route(State(st): State<Shared>, UrlQuery(p): UrlQuery<Vec<(String, String)>>) -> ApiResult {
    search_handler(st, p, TextField::Any).await
}

async fn legacy_empresas_search(State(st): State<Shared>, UrlQuery(p): UrlQuery<Vec<(String, String)>>) -> ApiResult {
    search_handler(st, p, TextField::Razao).await
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

async fn export(State(st): State<Shared>, UrlQuery(p): UrlQuery<Vec<(String, String)>>) -> ApiResult {
    if st.args.disable_export {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "export_disabled", "export is disabled on this server"));
    }
    let format = p.iter().find(|(k, _)| k == "format").map_or("ndjson".to_string(), |(_, v)| v.clone());
    if format != "ndjson" && format != "csv" {
        return Err(ApiError::bad("format must be ndjson or csv"));
    }
    let max = match (p.iter().find(|(k, _)| k == "max"), st.args.export_max_rows) {
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

async fn prometheus(State(st): State<Shared>) -> Response {
    let s = st.snapshot();
    let mut out = st.metrics.prometheus();
    let c = s.cache_stats();
    out.push_str(&format!("opencnpj_snapshot_documents {}\n", s.doc_count()));
    out.push_str(&format!("opencnpj_snapshot_bytes {}\n", s.file_len()));
    out.push_str(&format!("opencnpj_postings_cache_bytes {}\n", c.bytes));
    out.push_str(&format!("opencnpj_postings_cache_hits_total {}\n", c.hits));
    out.push_str(&format!("opencnpj_postings_cache_misses_total {}\n", c.misses));
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
    Router::new()
        .route("/", get(root))
        .route("/api/v1/cnpj/{cnpj}", get(get_cnpj))
        .route("/api/v1/estabelecimentos/{cnpj}", get(get_cnpj))
        .route("/api/v1/empresas/{basico}", get(get_cnpj))
        .route("/api/v1/cpf/{cpf}", get(get_cpf))
        .route("/api/v1/search", get(search_route))
        .route("/api/v1/busca", get(search_route))
        .route("/api/v1/empresas/search", get(legacy_empresas_search))
        .route("/api/v1/estabelecimentos/search", get(search_route))
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/export", get(export))
        .route("/api/v1/campos", get(campos))
        .route("/api/v1/snapshot", get(snapshot_info))
        .route("/api/v1/distribution", get(distribution))
        .route("/downloads/{name}", get(download))
        .route("/api/v1/dominios", get(dominios))
        .route("/api/v1/dominios/{tabela}", get(dominio))
        .route("/healthz", get(healthz))
        .route("/health", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(prometheus))
        .layer(middleware::from_fn_with_state(st.clone(), track))
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
    snap.set_postings_cache_bytes(args.postings_cache_mib << 20);
    eprintln!("opened {} ({} documents) in {:.1} ms", path.display(), snap.doc_count(), t0.elapsed().as_secs_f64() * 1e3);
    let snapshots_dir = match &args.data_dir {
        Some(d) => d.join("snapshots"),
        None => path.parent().map_or_else(|| PathBuf::from("."), Path::to_path_buf),
    };
    let jobs_dir = args.data_dir.as_ref().map_or_else(|| snapshots_dir.join("jobs"), |d| d.join("jobs"));
    Ok(Arc::new(AppState {
        sha256: RwLock::new(read_sidecar(&path)),
        snap: RwLock::new(Arc::new(snap)),
        sem: Arc::new(Semaphore::new(args.max_concurrency.max(1))),
        metrics: Metrics::default(),
        privacy: Privacy { hide_socios: args.hide_socios, hide_contacts: args.hide_contacts },
        snapshots_dir,
        jobs: crate::admin::Jobs::new(jobs_dir),
        index_breakdown: Mutex::new(None),
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

pub fn run(args: ServeArgs) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async move {
        let st = state(args.clone())?;
        if let Some(dir) = args.data_dir.clone() {
            tokio::spawn(reload_loop(st.clone(), dir));
        }
        if !args.admin_bind.is_empty() {
            let addr: SocketAddr = args.admin_bind.parse().context("bad --admin-bind")?;
            let admin = tokio::net::TcpListener::bind(addr).await?;
            eprintln!("admin suite on http://{addr}/admin (no authentication)");
            let app = crate::admin::router(st.clone());
            tokio::spawn(async move {
                if let Err(e) = axum::serve(admin, app).await {
                    eprintln!("admin server stopped: {e}");
                }
            });
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
