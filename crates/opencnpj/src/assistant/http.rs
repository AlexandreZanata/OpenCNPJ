//! HTTP face of the assistant: `/api/v1/assistant/ask`, `/api/v1/assistant/export`, the
//! public platform description and the built web front end.
//!
//! The two assistant routes are data routes: they pass through `gate::gate`, so every turn
//! and every file is authenticated, checked against the caller's plan and counted in the
//! caller's quota like any other search.

use std::path::{Component, Path};
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::body::Body;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, State};
use axum::http::{HeaderName, HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use opencnpj_nlu::context::Conversation;
use opencnpj_query::{export::Export, ir::Request as Query};
use serde::Deserialize;
use serde_json::json;

use super::{Assistant, Options, assets::DataArgs};
use crate::api::{ApiError, ApiResult, ServeArgs, Shared, blocking};
use crate::gate::{Caller, LoggedQuery};

/// Loads the assistant over the snapshot served at start, when `--assistant-agg` is given.
pub fn open(args: &ServeArgs, snapshot: &Path) -> Result<Option<Arc<Assistant>>> {
    let Some(agg) = &args.assistant_agg else { return Ok(None) };
    let started = std::time::Instant::now();
    let data = DataArgs {
        catalog: args.assistant_catalog.clone(),
        sectors: args.assistant_sectors.clone(),
        anchor: args.assistant_anchor.clone(),
        agg: Some(agg.clone()),
        history: args.assistant_history.clone(),
        snapshot: Some(snapshot.to_path_buf()),
    };
    let options = Options { hide_contacts: args.hide_contacts, export_max_rows: args.assistant_export_rows };
    let assistant = Assistant::open(&data, options).context("loading the assistant (--assistant-agg must match the served snapshot)")?;
    eprintln!("assistant ready in {} ms", started.elapsed().as_millis());
    Ok(Some(Arc::new(assistant)))
}

fn assistant(st: &Shared) -> Result<Arc<Assistant>, ApiError> {
    st.assistant.clone().ok_or_else(|| {
        ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "assistant_unavailable", "this server runs without the assistant (--assistant-agg)")
    })
}

/// Rows a file may hold for this caller: the server cap, lowered by the plan.
fn export_rows(st: &Shared, caller: Option<&Caller>) -> u32 {
    let plan = caller.and_then(|c| c.principal.plan.max_export_rows).map(|rows| u32::try_from(rows.max(0)).unwrap_or(u32::MAX));
    plan.map_or(st.args.assistant_export_rows, |rows| rows.min(st.args.assistant_export_rows))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AskBody {
    question: String,
    #[serde(default)]
    context: Option<Conversation>,
}

/// One conversation turn. The reply carries the conversation state to send with the next
/// question; the server keeps none.
async fn ask(State(st): State<Shared>, caller: Option<Extension<Caller>>, body: Result<Json<AskBody>, JsonRejection>) -> ApiResult {
    let assistant = assistant(&st)?;
    let Json(body) = body.map_err(|_| ApiError::bad("send {\"question\": \"…\", \"context\": …} as JSON"))?;
    let question = body.question.trim().to_owned();
    if question.is_empty() || question.chars().count() > 1000 {
        return Err(ApiError::bad("question must have between 1 and 1000 characters"));
    }
    let rows = export_rows(&st, caller.as_ref().map(|c| &c.0));
    let mut state = body.context.unwrap_or_default();
    let logged = question.clone();
    let reply = blocking(&st, move |_| Ok(assistant.ask_capped(&question, &mut state, rows))).await?;
    let mut response = Json(reply).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.extensions_mut().insert(LoggedQuery(logged));
    Ok(response)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportBody {
    request: Query,
    export: Export,
    #[serde(default)]
    question: Option<String>,
}

/// The file offered by a turn. The request is canonicalized and validated again, so a
/// client cannot widen it; the plan decides whether files are allowed and how many rows.
async fn export(State(st): State<Shared>, caller: Option<Extension<Caller>>, body: Result<Json<ExportBody>, JsonRejection>) -> ApiResult {
    let assistant = assistant(&st)?;
    let Json(body) = body.map_err(|_| ApiError::bad("send the request and export of an assistant reply as JSON"))?;
    let rows = export_rows(&st, caller.as_ref().map(|c| &c.0));
    let question = body.question.as_deref().map(|q| q.chars().take(1000).collect::<String>());
    let logged = question.clone().unwrap_or_default();
    let file = blocking(&st, move |_| assistant.export_capped(body.request, &body.export, question.as_deref(), rows))
        .await
        .map_err(|e| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "export_failed", e.message().to_owned()))?;
    let mut response = (
        [
            (header::CONTENT_TYPE, file.media_type.to_owned()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", file.filename)),
            (header::CACHE_CONTROL, "no-store".to_owned()),
            (HeaderName::from_static("x-opencnpj-rows"), file.rows.to_string()),
        ],
        file.bytes,
    )
        .into_response();
    response.extensions_mut().insert(LoggedQuery(logged));
    Ok(response)
}

pub fn router() -> Router<Shared> {
    Router::new().route("/api/v1/assistant/ask", post(ask)).route("/api/v1/assistant/export", post(export))
}

/// What a client needs before logging in: how accounts work here and what is served.
pub async fn platform(State(st): State<Shared>) -> Response {
    let snapshot = st.snapshot();
    let registration = st.gate.accounts.as_ref().map(|accounts| accounts.settings().registration);
    Json(json!({
        "service": "opencnpj",
        "version": env!("CARGO_PKG_VERSION"),
        "auth": st.gate.mode.name(),
        "registration": registration,
        "assistant": st.assistant.is_some(),
        "search": true,
        "export": !st.args.disable_export,
        "contacts": !st.args.hide_contacts,
        "competence": snapshot.manifest.competence,
        "documents": snapshot.doc_count(),
    }))
    .into_response()
}

fn media_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("map" | "json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Serves the built front end (`--web`). Only plain relative paths inside the directory are
/// read; anything else is a 404.
pub async fn web(State(st): State<Shared>, uri: Uri) -> Response {
    let not_found = || ApiError::new(StatusCode::NOT_FOUND, "not_found", "not found").into_response();
    let Some(root) = &st.args.web else { return not_found() };
    if uri.path().starts_with("/api/") || uri.path().starts_with("/admin/") {
        return not_found();
    }
    let relative = Path::new(uri.path().trim_start_matches('/'));
    if relative.components().any(|c| !matches!(c, Component::Normal(part) if !part.to_string_lossy().starts_with('.'))) {
        return not_found();
    }
    let path = if relative.as_os_str().is_empty() { root.join("index.html") } else { root.join(relative) };
    let Ok(bytes) = tokio::fs::read(&path).await else { return not_found() };
    let page = path.extension().is_some_and(|e| e == "html");
    let mut response = Response::new(Body::from(bytes));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(media_type(&path)));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    if page {
        headers.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
            ),
        );
    }
    response
}
