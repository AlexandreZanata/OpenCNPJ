//! Shared helpers for the HTTP integration tests: one fixture snapshot per test binary and
//! in-process routers driven with `tower::ServiceExt::oneshot`.
//!
//! Every router used by the tests is built by [`app_with`] / [`admin_app_with`], so flags
//! that every test server needs (for example an authentication switch) are added in one
//! place: [`BASE_FLAGS`].

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

/// Flags passed to every test server before the per-test ones.
const BASE_FLAGS: &[&str] = &["--admin-bind", "", "--auth", "off"];

struct Fixture {
    _dir: tempfile::TempDir,
    snapshot: PathBuf,
}

/// Builds the synthetic RFB release (`opencnpj_ingest::testkit`) into a `balanced`
/// snapshot once per test binary.
fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let dir = tempfile::Builder::new().prefix("opencnpj-api-").tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("fixture tempdir");
        let (snapshot, _) = opencnpj_ingest::testkit::build_fixture(dir.path(), "balanced");
        Fixture { _dir: dir, snapshot }
    })
}

/// Path of the fixture snapshot (`<tmp>/balanced.ocnpj`).
pub fn snapshot_path() -> &'static Path {
    &fixture().snapshot
}

fn flags(extra: &[&str]) -> Vec<String> {
    let snap = snapshot_path().to_str().expect("utf-8 path").to_string();
    let mut v = vec!["--snapshot".to_string(), snap];
    v.extend(BASE_FLAGS.iter().map(|s| s.to_string()));
    v.extend(extra.iter().map(|s| s.to_string()));
    v
}

fn state_with(extra: &[&str]) -> opencnpj::api::Shared {
    let args = opencnpj::api::ServeArgs::from_flags(&flags(extra)).expect("serve flags");
    opencnpj::api::state(args).expect("server state")
}

/// Public API router over the fixture snapshot with extra `serve` flags.
pub fn app_with(extra: &[&str]) -> Router {
    opencnpj::api::router(state_with(extra))
}

/// Public API router with default flags.
pub fn app() -> Router {
    app_with(&[])
}

/// Admin router (merges the public one) with extra `serve` flags.
pub fn admin_app_with(extra: &[&str]) -> Router {
    opencnpj::admin::router(state_with(extra))
}

/// A buffered response.
pub struct Resp {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl Resp {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(&self.body)))
    }

    pub fn text(&self) -> String {
        String::from_utf8(self.body.to_vec()).expect("utf-8 body")
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(|v| v.to_str().expect("ascii header"))
    }
}

/// Sends `req` and buffers the whole body. A body stream that fails midway panics.
pub async fn send(app: &Router, req: Request<Body>) -> Resp {
    let r = app.clone().oneshot(req).await.expect("infallible router");
    let (parts, body) = r.into_parts();
    let body = axum::body::to_bytes(body, usize::MAX).await.expect("response body");
    Resp { status: parts.status, headers: parts.headers, body }
}

/// Like [`send`], but returns the body error instead of panicking (streamed responses).
pub async fn try_send(app: &Router, req: Request<Body>) -> (StatusCode, HeaderMap, Result<Bytes, String>) {
    let r = app.clone().oneshot(req).await.expect("infallible router");
    let (parts, body) = r.into_parts();
    let body = axum::body::to_bytes(body, usize::MAX).await.map_err(|e| e.to_string());
    (parts.status, parts.headers, body)
}

pub fn get_request(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).expect("request")
}

/// `GET uri` → status, headers and raw body.
pub async fn get(app: &Router, uri: &str) -> (StatusCode, HeaderMap, Bytes) {
    let r = send(app, get_request(uri)).await;
    (r.status, r.headers, r.body)
}

/// `GET uri` → status, headers and the body parsed as JSON.
pub async fn get_json(app: &Router, uri: &str) -> (StatusCode, HeaderMap, Value) {
    let r = send(app, get_request(uri)).await;
    let v = r.json();
    (r.status, r.headers, v)
}

/// `GET uri` with one extra header.
pub async fn get_with_header(app: &Router, uri: &str, name: &str, value: &str) -> Resp {
    send(app, Request::get(uri).header(name, value).body(Body::empty()).expect("request")).await
}
