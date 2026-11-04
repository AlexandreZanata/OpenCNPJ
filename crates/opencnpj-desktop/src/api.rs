//! Blocking HTTP client for the admin API, plus [`Task`] to run calls off the UI thread.

use std::fmt;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::models::{ErrorBody, LoginResponse, Whoami};

const MAX_BODY: u64 = 64 << 20;

/// An API failure: HTTP status (0 for transport errors), error code and message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn transport(message: impl Into<String>) -> Self {
        Self { status: 0, code: "connection_error".into(), message: message.into() }
    }

    pub fn is_unauthorized(&self) -> bool {
        self.status == 401
    }

    pub fn is_not_found(&self) -> bool {
        self.status == 404
    }

    /// Builds an error from a non-2xx response body (`{"error","message","code"}` or text).
    pub fn from_response(status: u16, body: &str) -> Self {
        if let Ok(b) = serde_json::from_str::<ErrorBody>(body)
            && (!b.error.is_empty() || !b.message.is_empty())
        {
            let message = if b.message.is_empty() { b.error.clone() } else { b.message };
            return Self { status, code: if b.error.is_empty() { format!("http_{status}") } else { b.error }, message };
        }
        let text = body.trim();
        let message = if text.is_empty() {
            format!("HTTP {status}")
        } else {
            let mut t: String = text.chars().take(300).collect();
            if text.chars().count() > 300 {
                t.push('…');
            }
            t
        };
        Self { status, code: format!("http_{status}"), message }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.status == 0 { write!(f, "{}", self.message) } else { write!(f, "{} ({}): {}", self.status, self.code, self.message) }
    }
}

impl std::error::Error for ApiError {}

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Clone, Copy, Debug)]
enum Method {
    Get,
    Post,
    Patch,
    Delete,
}

struct Inner {
    agent: ureq::Agent,
    base: String,
    token: Option<String>,
}

/// Cheap to clone; every clone shares the connection pool.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never print the token.
        f.debug_struct("Client").field("base", &self.inner.base).field("authenticated", &self.inner.token.is_some()).finish()
    }
}

impl Client {
    pub fn new(base: &str) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(8)))
            .timeout_global(Some(Duration::from_secs(180)))
            .user_agent(format!("opencnpj-admin/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Self { inner: Arc::new(Inner { agent, base: base.trim_end_matches('/').to_string(), token: None }) }
    }

    /// Same server and connection pool, authenticated with `token`.
    pub fn with_token(&self, token: String) -> Self {
        Self { inner: Arc::new(Inner { agent: self.inner.agent.clone(), base: self.inner.base.clone(), token: Some(token) }) }
    }

    pub fn base(&self) -> &str {
        &self.inner.base
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.inner.base, path)
    }

    fn send(&self, method: Method, path: &str, body: Option<&Value>) -> ApiResult<(u16, String)> {
        let url = self.url(path);
        let auth = self.inner.token.as_ref().map(|t| format!("Bearer {t}"));
        let a = &self.inner.agent;
        let result = match method {
            Method::Get | Method::Delete => {
                let mut r = if matches!(method, Method::Get) { a.get(&url) } else { a.delete(&url) };
                r = r.header("Accept", "application/json");
                if let Some(h) = &auth {
                    r = r.header("Authorization", h);
                }
                r.call()
            }
            Method::Post | Method::Patch => {
                let mut r = if matches!(method, Method::Post) { a.post(&url) } else { a.patch(&url) };
                r = r.header("Accept", "application/json");
                if let Some(h) = &auth {
                    r = r.header("Authorization", h);
                }
                match body {
                    Some(b) => r.header("Content-Type", "application/json").send(b.to_string()),
                    None => r.send_empty(),
                }
            }
        };
        let mut resp = result.map_err(|e| ApiError::transport(transport_message(&self.inner.base, &e)))?;
        let status = resp.status().as_u16();
        let text = resp.body_mut().with_config().limit(MAX_BODY).read_to_string().map_err(|e| ApiError::transport(e.to_string()))?;
        if (200..300).contains(&status) { Ok((status, text)) } else { Err(ApiError::from_response(status, &text)) }
    }

    fn json<T: DeserializeOwned>(&self, method: Method, path: &str, body: Option<&Value>) -> ApiResult<T> {
        let (status, text) = self.send(method, path, body)?;
        let text = if text.trim().is_empty() { "null" } else { text.as_str() };
        serde_json::from_str(text).map_err(|e| ApiError {
            status,
            code: "invalid_response".into(),
            message: format!("unexpected response from {path}: {e}"),
        })
    }

    pub fn get<T: DeserializeOwned>(&self, path: &str) -> ApiResult<T> {
        self.json(Method::Get, path, None)
    }

    /// GET returning the raw body (job logs are `text/plain`).
    pub fn get_text(&self, path: &str) -> ApiResult<String> {
        self.send(Method::Get, path, None).map(|(_, t)| t)
    }

    pub fn post<T: DeserializeOwned>(&self, path: &str, body: &Value) -> ApiResult<T> {
        self.json(Method::Post, path, Some(body))
    }

    pub fn post_empty<T: DeserializeOwned>(&self, path: &str) -> ApiResult<T> {
        self.json(Method::Post, path, None)
    }

    pub fn patch<T: DeserializeOwned>(&self, path: &str, body: &Value) -> ApiResult<T> {
        self.json(Method::Patch, path, Some(body))
    }

    pub fn delete<T: DeserializeOwned>(&self, path: &str) -> ApiResult<T> {
        self.json(Method::Delete, path, None)
    }

    /// `POST /api/v1/auth/login`, then `GET /admin/api/whoami` with the new session token.
    pub fn login(base: &str, key: &str) -> ApiResult<(Client, Whoami)> {
        let anon = Client::new(base);
        let login: LoginResponse = anon.post("/api/v1/auth/login", &json!({ "key": key }))?;
        if login.token.is_empty() {
            return Err(ApiError { status: 200, code: "invalid_response".into(), message: "login returned no session token".into() });
        }
        let client = anon.with_token(login.token);
        match client.get::<Whoami>("/admin/api/whoami") {
            Ok(me) => Ok((client, me)),
            Err(e) => {
                // Do not leave a session behind when the account cannot use the admin API.
                let _ = client.logout();
                if e.status == 403 {
                    Err(ApiError {
                        message: "this account has no staff role; the admin application needs admin, moderator or analyst".into(),
                        ..e
                    })
                } else {
                    Err(e)
                }
            }
        }
    }

    /// `POST /api/v1/auth/logout` (revokes the session token).
    pub fn logout(&self) -> ApiResult<()> {
        self.send(Method::Post, "/api/v1/auth/logout", None).map(|_| ())
    }
}

fn transport_message(base: &str, e: &ureq::Error) -> String {
    match e {
        ureq::Error::Timeout(_) => format!("request to {base} timed out"),
        ureq::Error::HostNotFound => format!("host not found: {base}"),
        ureq::Error::Io(io) if io.kind() == std::io::ErrorKind::ConnectionRefused => {
            format!("connection refused by {base}; is the admin listener running?")
        }
        other => format!("cannot reach {base}: {other}"),
    }
}

/// Result of one background call, polled from the UI thread.
pub struct Task<T> {
    rx: Option<Receiver<ApiResult<T>>>,
}

impl<T> Default for Task<T> {
    fn default() -> Self {
        Self { rx: None }
    }
}

impl<T: Send + 'static> Task<T> {
    /// Runs `f` on a new thread and repaints `ctx` when it finishes.
    pub fn spawn(ctx: &eframe::egui::Context, f: impl FnOnce() -> ApiResult<T> + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("opencnpj-admin-http".into())
            .spawn(move || {
                let _ = tx.send(f());
                ctx.request_repaint();
            })
            .map(|_| Self { rx: Some(rx) })
            .unwrap_or_else(|e| {
                let (tx, rx) = mpsc::channel();
                let _ = tx.send(Err(ApiError::transport(format!("cannot start worker thread: {e}"))));
                Self { rx: Some(rx) }
            })
    }

    /// Runs `f(client)` in the background.
    pub fn call(ctx: &eframe::egui::Context, client: &Client, f: impl FnOnce(&Client) -> ApiResult<T> + Send + 'static) -> Self {
        let c = client.clone();
        Self::spawn(ctx, move || f(&c))
    }

    pub fn running(&self) -> bool {
        self.rx.is_some()
    }

    /// The result, once, when the call has finished.
    pub fn poll(&mut self) -> Option<ApiResult<T>> {
        let rx = self.rx.as_ref()?;
        match rx.try_recv() {
            Ok(r) => {
                self.rx = None;
                Some(r)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.rx = None;
                Some(Err(ApiError::transport("worker thread ended without a result")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_bodies() {
        let e = ApiError::from_response(403, r#"{"error":"forbidden","message":"missing permission users.manage","code":403}"#);
        assert_eq!(e.status, 403);
        assert_eq!(e.code, "forbidden");
        assert_eq!(e.message, "missing permission users.manage");
        assert_eq!(e.to_string(), "403 (forbidden): missing permission users.manage");
        assert!(ApiError::from_response(401, r#"{"error":"unauthenticated","message":"","code":401}"#).is_unauthorized());
        let e = ApiError::from_response(502, "Bad Gateway");
        assert_eq!(e.code, "http_502");
        assert_eq!(e.message, "Bad Gateway");
        assert_eq!(ApiError::from_response(500, "").message, "HTTP 500");
        assert_eq!(ApiError::from_response(500, &"x".repeat(1000)).message.chars().count(), 301);
        assert_eq!(ApiError::transport("down").to_string(), "down");
    }

    #[test]
    fn client_debug_hides_token() {
        let c = Client::new("http://127.0.0.1:1/").with_token("ocs_secret".into());
        assert_eq!(c.base(), "http://127.0.0.1:1");
        assert!(!format!("{c:?}").contains("ocs_secret"));
    }

    #[test]
    fn task_reports_result_once() {
        let ctx = eframe::egui::Context::default();
        let mut t: Task<u32> = Task::spawn(&ctx, || Ok(7));
        let mut got = None;
        for _ in 0..500 {
            if let Some(r) = t.poll() {
                got = Some(r);
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(got, Some(Ok(7)));
        assert!(!t.running());
        assert!(t.poll().is_none());
    }

    #[test]
    fn transport_error_when_server_is_down() {
        // Port 1 on loopback is closed on CI machines; the call must fail without panicking.
        let r: ApiResult<Value> = Client::new("http://127.0.0.1:1").get("/admin/api/whoami");
        let e = r.unwrap_err();
        assert_eq!(e.status, 0);
    }
}
