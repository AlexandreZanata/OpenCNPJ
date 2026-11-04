//! Public account endpoints under `/api/v1/auth`: register (key by e-mail), login with the
//! key, logout, profile, key rotation and e-mail recovery. See `docs/ACCOUNTS.md`.

// Errors here are ready-made HTTP responses returned on cold paths (rejections).
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Query as UrlQuery, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::{self, Accounts, Principal};
use crate::api::{ApiError, Shared};
use crate::gate::{self, client_ip};
use crate::mail;

fn accounts(st: &Shared) -> Result<Arc<Accounts>, ApiError> {
    st.gate
        .accounts
        .clone()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "accounts_disabled", "this server runs without accounts (--auth off)"))
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Result<T, ApiError> {
    match tokio::task::spawn_blocking(f).await {
        Ok(r) => r.map_err(ApiError::from),
        Err(e) => Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", e.to_string())),
    }
}

/// Per-IP budget for unauthenticated account actions.
fn throttle(st: &Shared, ip: std::net::IpAddr, action: &'static str, per_hour: u32) -> Result<(), Response> {
    if st.gate.throttle(ip, action, per_hour) {
        Ok(())
    } else {
        Err(gate::error(
            StatusCode::TOO_MANY_REQUESTS,
            "too_many_requests",
            &format!("too many {action} requests from this address; retry later"),
            &[(header::RETRY_AFTER, axum::http::HeaderValue::from_static("3600"))],
        ))
    }
}

#[derive(Deserialize)]
struct Register {
    username: String,
    email: String,
}

async fn register(State(st): State<Shared>, req: Request) -> Response {
    let ip = client_ip(&req, st.gate.trust_proxy);
    let r: Register = match parse_body(req).await {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    if let Err(r) = throttle(&st, ip, "register", 10) {
        return r;
    }
    let acc = match accounts(&st) {
        Ok(a) => a,
        Err(e) => return e.into_response(),
    };
    let email = r.email.trim().to_ascii_lowercase();
    let (a, u, e) = (acc.clone(), r.username.clone(), email.clone());
    let res = blocking(move || {
        let existing = a.user_by("email", &e)?;
        Ok((a.register(&u, &e, Some(ip.to_string()))?, existing))
    })
    .await;
    match res {
        Ok((Some((user, key)), _)) => {
            if let Some(m) = &st.mailer {
                m.spawn(user.email.clone(), "Your OpenCNPJ login key", mail::welcome(&user.username, &key, &m.public_url));
            }
        }
        Ok((None, existing)) => {
            if let (Some(m), Some(u)) = (&st.mailer, existing) {
                m.spawn(u.email.clone(), "OpenCNPJ account already exists", mail::already_registered(&u.username, &m.public_url));
            }
        }
        Err(e) => return e.into_response(),
    }
    (
        StatusCode::ACCEPTED,
        Json(json!({ "status": "pending", "message": "if the address can receive mail, a login key is on its way to it" })),
    )
        .into_response()
}

#[derive(Deserialize)]
struct Login {
    key: String,
}

async fn login(State(st): State<Shared>, req: Request) -> Response {
    let ip = client_ip(&req, st.gate.trust_proxy);
    let ua = req.headers().get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(str::to_string);
    let l: Login = match parse_body(req).await {
        Ok(l) => l,
        Err(e) => return e.into_response(),
    };
    if let Err(r) = throttle(&st, ip, "login", 120) {
        return r;
    }
    let acc = match accounts(&st) {
        Ok(a) => a,
        Err(e) => return e.into_response(),
    };
    let Ok(_permit) = st.gate.argon.acquire().await else {
        return gate::error(StatusCode::SERVICE_UNAVAILABLE, "overloaded", "closed", &[]);
    };
    let key = l.key.trim().to_string();
    match blocking(move || acc.login(&key, Some(ip.to_string()), ua)).await {
        Ok((token, exp, user)) => Json(json!({ "token": token, "token_type": "Bearer", "expires_at": exp, "user": user })).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn caller(st: &Shared, headers: &HeaderMap, ip: std::net::IpAddr) -> Result<Arc<Principal>, Response> {
    match st.gate.authenticate(headers, ip).await? {
        Some(p) => Ok(p),
        None => Err(gate::unauthorized("unauthenticated", "send `Authorization: Bearer <key or session token>`")),
    }
}

async fn logout(State(st): State<Shared>, req: Request) -> Response {
    let Some(acc) = st.gate.accounts.clone() else { return StatusCode::NOT_FOUND.into_response() };
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
        .filter(|t| t.starts_with("ocs_"));
    let Some(token) = token else {
        return gate::unauthorized("unauthenticated", "send the session token as `Authorization: Bearer ocs_...`");
    };
    match blocking(move || acc.logout(&token)).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn me(State(st): State<Shared>, req: Request) -> Response {
    let ip = client_ip(&req, st.gate.trust_proxy);
    let p = match caller(&st, req.headers(), ip).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    let Ok(acc) = accounts(&st) else { return StatusCode::NOT_FOUND.into_response() };
    let id = p.id;
    let res = blocking(move || {
        let u = acc.user(id)?;
        let counts = acc.counts(id, accounts::day_of(accounts::now()))?;
        Ok((u, counts))
    })
    .await;
    match res {
        Ok((Some(u), db_counts)) => {
            let (today, month) = st.gate.usage_now(id).unwrap_or(db_counts);
            Json(json!({
                "user": u,
                "plan": *p.plan,
                "limits": { "daily_limit": p.daily_limit, "monthly_limit": p.monthly_limit, "per_minute": p.per_minute },
                "usage": { "today": today, "month": month },
                "permissions": p.role.permissions().iter().map(|x| x.name()).collect::<Vec<_>>(),
            }))
            .into_response()
        }
        Ok((None, _)) => gate::unauthorized("invalid_credentials", "account no longer exists"),
        Err(e) => e.into_response(),
    }
}

async fn rotate(State(st): State<Shared>, req: Request) -> Response {
    let ip = client_ip(&req, st.gate.trust_proxy);
    let p = match caller(&st, req.headers(), ip).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    let Ok(acc) = accounts(&st) else { return StatusCode::NOT_FOUND.into_response() };
    let actor = accounts::Actor { id: Some(p.id), name: p.username.clone(), ip: Some(ip.to_string()) };
    let id = p.id;
    let a = acc.clone();
    match blocking(move || {
        let key = a.rotate_key(id, &actor)?;
        Ok((key, a.user(id)?))
    })
    .await
    {
        Ok((key, user)) => {
            let prefix = accounts::split_key(&key).map(|x| x.0.to_string()).unwrap_or_default();
            if let (Some(m), Some(u)) = (&st.mailer, user) {
                m.spawn(u.email, "Your OpenCNPJ login key was replaced", mail::key_rotated_notice(&u.username, &prefix));
            }
            Json(json!({ "key": key, "key_prefix": prefix, "note": "the previous key and every session were revoked; log in again with this key" }))
                .into_response()
        }
        Err(e) => e.into_response(),
    }
}

#[derive(Deserialize)]
struct Recover {
    email: String,
}

async fn recover(State(st): State<Shared>, req: Request) -> Response {
    let ip = client_ip(&req, st.gate.trust_proxy);
    let r: Recover = match parse_body(req).await {
        Ok(r) => r,
        Err(e) => return e.into_response(),
    };
    if let Err(resp) = throttle(&st, ip, "recover", 10) {
        return resp;
    }
    let acc = match accounts(&st) {
        Ok(a) => a,
        Err(e) => return e.into_response(),
    };
    match blocking(move || acc.start_recovery(&r.email)).await {
        Ok(Some((u, token))) => {
            if let Some(m) = &st.mailer {
                m.spawn(u.email.clone(), "Replace your OpenCNPJ login key", mail::recovery(&u.username, &token, &m.public_url));
            }
        }
        Ok(None) => {}
        Err(e) => return e.into_response(),
    }
    (
        StatusCode::ACCEPTED,
        Json(
            json!({ "status": "sent", "message": "if the address has an account, a confirmation link valid for 30 minutes was e-mailed" }),
        ),
    )
        .into_response()
}

#[derive(Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

fn page(title: &str, body: &str) -> Html<String> {
    Html(format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{title}</title><style>body{{font:16px/1.5 system-ui,sans-serif;max-width:560px;margin:10vh auto;padding:0 16px;color:#1d2433}}\
         code{{display:block;padding:12px;background:#f1f3f7;border-radius:6px;word-break:break-all;font-size:15px}}\
         button{{font:inherit;padding:10px 18px;border:0;border-radius:6px;background:#1f5eff;color:#fff;cursor:pointer}}\
         @media (prefers-color-scheme:dark){{body{{background:#12151c;color:#e6e9ef}}code{{background:#1d222c}}}}</style></head>\
         <body><h1>{title}</h1>{body}</body></html>"
    ))
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// GET shows a confirmation form; nothing changes until the form is posted.
async fn confirm_page(UrlQuery(q): UrlQuery<TokenQuery>) -> Html<String> {
    let token = escape(q.token.as_deref().unwrap_or(""));
    page(
        "Replace your login key",
        &format!(
            "<p>Confirming revokes your current OpenCNPJ login key and every open session, and issues a new key. \
             The new key is shown on the next page and e-mailed to you.</p>\
             <form method=\"post\" action=\"confirm\"><input type=\"hidden\" name=\"token\" value=\"{token}\"><button>Replace my key</button></form>"
        ),
    )
}

async fn confirm(State(st): State<Shared>, req: Request) -> Response {
    let ip = client_ip(&req, st.gate.trust_proxy);
    let is_form = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|c| c.starts_with("application/x-www-form-urlencoded"));
    let bytes = match axum::body::to_bytes(req.into_body(), 16 << 10).await {
        Ok(b) => b,
        Err(_) => return ApiError::bad("body too large").into_response(),
    };
    let token = if is_form {
        form_value(&bytes, "token")
    } else {
        serde_json::from_slice::<Value>(&bytes).ok().and_then(|v| v["token"].as_str().map(str::to_string))
    };
    let Some(token) = token.filter(|t| !t.is_empty()) else {
        return ApiError::new(StatusCode::BAD_REQUEST, "invalid_token", "missing token").into_response();
    };
    if let Err(r) = throttle(&st, ip, "recover-confirm", 30) {
        return r;
    }
    let acc = match accounts(&st) {
        Ok(a) => a,
        Err(e) => return e.into_response(),
    };
    match blocking(move || acc.confirm_recovery(&token, Some(ip.to_string()))).await {
        Ok((u, key)) => {
            if let Some(m) = &st.mailer {
                m.spawn(u.email.clone(), "Your new OpenCNPJ login key", mail::new_key(&u.username, &key, "recovery by e-mail"));
            }
            let prefix = accounts::split_key(&key).map(|x| x.0.to_string()).unwrap_or_default();
            if is_form {
                page(
                    "Your new login key",
                    &format!(
                        "<p>Hello {}, this is your new key. It was also e-mailed to you. Your previous key no longer works.</p><code>{}</code>",
                        escape(&u.username),
                        escape(&key)
                    ),
                )
                .into_response()
            } else {
                Json(json!({ "key": key, "key_prefix": prefix, "user": u })).into_response()
            }
        }
        Err(e) if is_form => (
            StatusCode::BAD_REQUEST,
            page("Link not valid", &format!("<p>{}</p><p>Request a new link and try again.</p>", escape(e.message()))),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

fn form_value(body: &Bytes, key: &str) -> Option<String> {
    std::str::from_utf8(body).ok()?.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then(|| v.replace('+', " "))
    })
}

async fn parse_body<T: serde::de::DeserializeOwned>(req: Request) -> Result<T, ApiError> {
    let bytes = axum::body::to_bytes(req.into_body(), 16 << 10).await.map_err(|_| ApiError::bad("body too large"))?;
    serde_json::from_slice(&bytes).map_err(|e| ApiError::bad(format!("invalid JSON body: {e}")))
}

pub fn router() -> Router<Shared> {
    Router::new()
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(me))
        .route("/api/v1/auth/key/rotate", post(rotate))
        .route("/api/v1/auth/recover", post(recover))
        .route("/api/v1/auth/recover/confirm", get(confirm_page).post(confirm))
}
