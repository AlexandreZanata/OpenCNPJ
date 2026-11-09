//! Local chat service: the web page, `/api/local/ask` and `/api/local/export`.
//!
//! Every asset is loaded once at start-up; a question costs only its interpretation and
//! query. Access is limited to loopback and, when given, one LAN subnet.

use crate::{
    assistant::{Assistant, Options},
    questions::DataArgs,
};
use anyhow::{Context, Result, ensure};
use axum::{
    Json, Router,
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::{StatusCode, Uri, header},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use opencnpj_nlu::context::Conversation;
use opencnpj_query::{export::Export, ir::Request as Query};
use serde::Deserialize;
use serde_json::json;
use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Instant,
};

#[derive(clap::Args)]
pub struct ServeArgs {
    #[command(flatten)]
    pub data: DataArgs,
    /// Address to listen on; repeat for loopback and a LAN address. A LAN address needs
    /// --allow-network.
    #[arg(long, default_value = "127.0.0.1:5174")]
    pub listen: Vec<SocketAddr>,
    /// IPv4 client subnet allowed besides loopback, e.g. 192.168.0.0/24.
    #[arg(long)]
    pub allow_network: Option<String>,
    /// Built front end served at `/` (web/dist).
    #[arg(long)]
    pub web: Option<PathBuf>,
    /// Never write phone or e-mail columns.
    #[arg(long)]
    pub hide_contacts: bool,
    /// Rows per exported file.
    #[arg(long, default_value_t = 100_000)]
    pub export_max_rows: u32,
}

#[derive(clap::Args)]
pub struct AnswerArgs {
    #[command(flatten)]
    pub data: DataArgs,
    #[arg(long)]
    pub text: String,
    /// Conversation state JSON, read and updated like `ask --state`.
    #[arg(long)]
    pub state: Option<PathBuf>,
    /// Write the file the question asks for into this directory.
    #[arg(long)]
    pub export_dir: Option<PathBuf>,
    #[arg(long)]
    pub hide_contacts: bool,
    #[arg(long, default_value_t = 100_000)]
    pub export_max_rows: u32,
}

/// One chat turn from the command line, printed as the service would return it.
pub fn answer(args: AnswerArgs) -> Result<()> {
    let assistant = Assistant::open(&args.data, Options { hide_contacts: args.hide_contacts, export_max_rows: args.export_max_rows })?;
    let mut state = match &args.state {
        Some(path) if path.exists() => serde_json::from_str(&fs::read_to_string(path)?)?,
        _ => Conversation::default(),
    };
    let mut reply = assistant.ask(&args.text, &mut state);
    if let (Some(dir), Some(offer)) = (&args.export_dir, reply.get("export").filter(|e| !e.is_null()).cloned()) {
        let request: Query = serde_json::from_value(offer["request"].clone())?;
        let export: Export = serde_json::from_value(offer["export"].clone())?;
        let started = Instant::now();
        let file = assistant.export(request, &export, Some(&args.text))?;
        fs::create_dir_all(dir)?;
        let path = dir.join(&file.filename);
        fs::write(&path, &file.bytes)?;
        reply["export"]["written"] =
            json!({"path": path, "bytes": file.bytes.len(), "rows": file.rows, "elapsed_us": started.elapsed().as_micros() as u64});
    }
    println!("{}", serde_json::to_string_pretty(&reply)?);
    if let Some(path) = &args.state {
        fs::write(path, serde_json::to_string_pretty(&state)?)?;
    }
    Ok(())
}

struct Service {
    assistant: Assistant,
    web: Option<PathBuf>,
    network: Option<(Ipv4Addr, u32)>,
}

fn parse_network(text: &str) -> Result<(Ipv4Addr, u32)> {
    let (address, prefix) = text.split_once('/').context("network must be written as ADDRESS/PREFIX")?;
    let address: Ipv4Addr = address.parse().context("network address")?;
    let prefix: u32 = prefix.parse().context("network prefix")?;
    ensure!(prefix <= 32, "network prefix must be at most 32");
    Ok((address, prefix))
}

fn admits(network: Option<(Ipv4Addr, u32)>, ip: IpAddr) -> bool {
    let ip = match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => return v6.is_loopback(),
        },
        v4 => v4,
    };
    let IpAddr::V4(ip) = ip else { return false };
    if ip.is_loopback() {
        return true;
    }
    network.is_some_and(|(base, prefix)| {
        let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
        u32::from(ip) & mask == u32::from(base) & mask
    })
}

pub fn serve(args: ServeArgs) -> Result<()> {
    let network = args.allow_network.as_deref().map(parse_network).transpose()?;
    for address in &args.listen {
        let ip = address.ip();
        ensure!(!ip.is_unspecified() && !ip.is_multicast(), "--listen must be a specific unicast address");
        ensure!(admits(network, ip), "a LAN address needs --allow-network with a subnet that contains it");
    }
    if let Some(web) = &args.web {
        ensure!(web.join("index.html").is_file(), "missing {}", web.join("index.html").display());
    }
    let started = Instant::now();
    let assistant = Assistant::open(&args.data, Options { hide_contacts: args.hide_contacts, export_max_rows: args.export_max_rows })?;
    eprintln!(
        "assets loaded in {} ms (snapshot: {})",
        started.elapsed().as_millis(),
        if assistant.has_snapshot() { "yes" } else { "no, lists and exports of companies are unavailable" }
    );
    let service = Arc::new(Service { assistant, web: args.web, network });
    let app = Router::new()
        .route("/api/local/ask", post(ask))
        .route("/api/local/export", post(export))
        .route("/api/local/health", get(health))
        .fallback(get(static_file))
        .layer(middleware::from_fn_with_state(service.clone(), guard))
        .with_state(service);
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    runtime.block_on(async {
        let mut servers = tokio::task::JoinSet::new();
        for address in args.listen {
            let listener = tokio::net::TcpListener::bind(address).await?;
            println!("http://{}/?local=1#/consulta", listener.local_addr()?);
            let service = app.clone().into_make_service_with_connect_info::<SocketAddr>();
            servers.spawn(async move { axum::serve(listener, service).await });
        }
        while let Some(result) = servers.join_next().await {
            result??;
        }
        Ok(())
    })
}

async fn guard(State(service): State<Arc<Service>>, ConnectInfo(peer): ConnectInfo<SocketAddr>, request: Request, next: Next) -> Response {
    if !admits(service.network, peer.ip()) {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }
    next.run(request).await
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({"error": message}))).into_response()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AskBody {
    question: String,
    #[serde(default)]
    context: Option<Conversation>,
}

async fn ask(State(service): State<Arc<Service>>, body: Result<Json<AskBody>, axum::extract::rejection::JsonRejection>) -> Response {
    let Ok(Json(body)) = body else { return error(StatusCode::BAD_REQUEST, "Envie a pergunta e o contexto em JSON.") };
    let question = body.question.trim().to_owned();
    if question.is_empty() || question.chars().count() > 1000 {
        return error(StatusCode::BAD_REQUEST, "Digite uma pergunta de até 1.000 caracteres.");
    }
    let mut state = body.context.unwrap_or_default();
    let reply = tokio::task::spawn_blocking(move || service.assistant.ask(&question, &mut state)).await;
    match reply {
        Ok(reply) => ([(header::CACHE_CONTROL, "no-store")], Json(reply)).into_response(),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "O motor local não conseguiu concluir a consulta."),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportBody {
    request: Query,
    export: Export,
    #[serde(default)]
    question: Option<String>,
}

async fn export(State(service): State<Arc<Service>>, body: Result<Json<ExportBody>, axum::extract::rejection::JsonRejection>) -> Response {
    let Ok(Json(body)) = body else { return error(StatusCode::BAD_REQUEST, "Pedido de arquivo inválido.") };
    let file = tokio::task::spawn_blocking(move || {
        let question = body.question.as_deref().map(|q| q.chars().take(1000).collect::<String>());
        service.assistant.export(body.request, &body.export, question.as_deref())
    })
    .await;
    match file {
        Ok(Ok(file)) => (
            [
                (header::CONTENT_TYPE, file.media_type.to_owned()),
                (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", file.filename)),
                (header::CACHE_CONTROL, "no-store".to_owned()),
                (header::HeaderName::from_static("x-opencnpj-rows"), file.rows.to_string()),
            ],
            file.bytes,
        )
            .into_response(),
        Ok(Err(e)) => error(StatusCode::UNPROCESSABLE_ENTITY, &format!("Não consegui gerar o arquivo: {e}")),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "Não consegui gerar o arquivo."),
    }
}

async fn health(State(service): State<Arc<Service>>) -> Response {
    Json(json!({"status": "ok", "snapshot": service.assistant.has_snapshot()})).into_response()
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
        _ => "application/octet-stream",
    }
}

async fn static_file(State(service): State<Arc<Service>>, uri: Uri) -> Response {
    let Some(web) = &service.web else { return error(StatusCode::NOT_FOUND, "front end not configured") };
    if uri.path() == "/" && !uri.query().is_some_and(|q| q.split('&').any(|p| p == "local=1")) {
        return Redirect::to("/?local=1").into_response();
    }
    let relative = Path::new(uri.path().trim_start_matches('/'));
    if relative.components().any(|c| !matches!(c, Component::Normal(part) if !part.to_string_lossy().starts_with('.'))) {
        return error(StatusCode::NOT_FOUND, "not found");
    }
    let path = if relative.as_os_str().is_empty() { web.join("index.html") } else { web.join(relative) };
    match tokio::fs::read(&path).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, media_type(&path)), (header::CACHE_CONTROL, "no-cache")], Body::from(bytes)).into_response(),
        Err(_) => error(StatusCode::NOT_FOUND, "not found"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_and_the_allowed_subnet_are_admitted() {
        let lan = Some(parse_network("172.19.2.0/24").unwrap());
        assert!(admits(lan, "127.0.0.1".parse().unwrap()));
        assert!(admits(lan, "::1".parse().unwrap()));
        assert!(admits(lan, "172.19.2.200".parse().unwrap()));
        assert!(admits(lan, "::ffff:172.19.2.7".parse().unwrap()));
        assert!(!admits(lan, "172.19.3.1".parse().unwrap()));
        assert!(!admits(None, "172.19.2.200".parse().unwrap()));
        assert!(parse_network("10.0.0.0/33").is_err());
    }
}
