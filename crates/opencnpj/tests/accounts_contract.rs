//! Contract tests for accounts, keys, recovery, roles, moderation, plans, quotas, rate
//! limits, usage accounting, the response cache and the admin API (docs/ACCOUNTS.md),
//! driven in-process through the real routers.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use opencnpj::api::{self, ServeArgs, Shared};
use serde_json::{Value, json};
use tower::ServiceExt;

fn fixture() -> &'static Path {
    static DIR: OnceLock<(tempfile::TempDir, PathBuf)> = OnceLock::new();
    &DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        let (p, _) = opencnpj_ingest::testkit::build_fixture(d.path(), "balanced");
        (d, p)
    })
    .1
}

struct Env {
    _dir: tempfile::TempDir,
    outbox: PathBuf,
    st: Shared,
    public: Router,
    admin: Router,
}

/// Fresh server state (own accounts database and outbox) over a copy of the fixture.
fn env(flags: &[&str]) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let snap = dir.path().join("fixture.ocnpj");
    std::fs::copy(fixture(), &snap).unwrap();
    let mut all = vec!["--snapshot", snap.to_str().unwrap(), "--admin-bind", ""];
    all.extend_from_slice(flags);
    let st = api::state(ServeArgs::from_flags(&all).unwrap()).unwrap();
    st.gate.start_background();
    Env { outbox: dir.path().join("outbox"), public: api::router(st.clone()), admin: opencnpj::admin::router(st.clone()), st, _dir: dir }
}

struct Resp {
    status: StatusCode,
    headers: HeaderMap,
    text: String,
}

impl Resp {
    fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or_else(|_| panic!("not JSON ({}): {}", self.status, self.text))
    }
    fn header(&self, k: &str) -> Option<&str> {
        self.headers.get(k).and_then(|v| v.to_str().ok())
    }
}

async fn call(app: &Router, method: Method, uri: &str, auth: Option<&str>, body: Option<Value>, extra: &[(&str, &str)]) -> Resp {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(a) = auth {
        b = b.header(header::AUTHORIZATION, format!("Bearer {a}"));
    }
    for (k, v) in extra {
        b = b.header(*k, *v);
    }
    let req = match body {
        Some(v) => b.header(header::CONTENT_TYPE, "application/json").body(Body::from(v.to_string())).unwrap(),
        None => b.body(Body::empty()).unwrap(),
    };
    let r = app.clone().oneshot(req).await.unwrap();
    let status = r.status();
    let headers = r.headers().clone();
    let bytes = axum::body::to_bytes(r.into_body(), usize::MAX).await.unwrap();
    Resp { status, headers, text: String::from_utf8_lossy(&bytes).into_owned() }
}

async fn get(app: &Router, uri: &str, auth: Option<&str>) -> Resp {
    call(app, Method::GET, uri, auth, None, &[]).await
}

async fn post(app: &Router, uri: &str, auth: Option<&str>, body: Value) -> Resp {
    call(app, Method::POST, uri, auth, Some(body), &[]).await
}

/// Waits for an e-mail to `to` containing `needle` and returns the token that starts there.
async fn mailed(outbox: &Path, to: &str, needle: &str, len: usize) -> String {
    for _ in 0..100 {
        for e in std::fs::read_dir(outbox).into_iter().flatten().flatten() {
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            if text.contains(&format!("To: {to}"))
                && let Some(i) = text.find(needle)
            {
                let tok: String = text[i..].chars().take(len).collect();
                if tok.len() == len {
                    let _ = std::fs::remove_file(e.path());
                    return tok;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("no e-mail to {to} containing {needle}");
}

const KEY_LEN: usize = 6 + 10 + 1 + 32;
const TOKEN_LEN: usize = 4 + 43;

/// Creates an account through the CLI path and returns its key.
fn create(e: &Env, username: &str, role: &str, plan: Option<&str>) -> String {
    let acc = e.st.gate.accounts.clone().unwrap();
    let role = opencnpj::accounts::Role::parse(role).unwrap();
    acc.create_user(
        username,
        &format!("{username}@example.com"),
        role,
        plan,
        opencnpj::accounts::Status::Active,
        &opencnpj::accounts::Actor::system("test"),
    )
    .unwrap()
    .1
}

async fn login(e: &Env, key: &str) -> String {
    let r = post(&e.public, "/api/v1/auth/login", None, json!({ "key": key })).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    r.json()["token"].as_str().unwrap().to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn official_sources_and_query_are_exposed() {
    use opencnpj_insights::official::{
        model::{CellStatus, DatasetManifest, Observation},
        store::write_store,
    };
    let official = tempfile::tempdir().unwrap();
    let rows = vec![Observation {
        dataset_id: "ibge-6579".into(),
        variable_id: "populacao_estimada".into(),
        variable_label: "População residente estimada".into(),
        unit: "Pessoas".into(),
        classification_id: String::new(),
        classification_label: String::new(),
        category_id: String::new(),
        category_label: String::new(),
        geography_level: "N6".into(),
        geography_code: "5107925".into(),
        geography_name: "Sorriso".into(),
        period: "2024".into(),
        value: Some(120_985.0),
        status: CellStatus::Observed,
        source_url: "https://servicodados.ibge.gov.br/api/v3/agregados/6579".into(),
    }];
    let manifest = DatasetManifest {
        format_version: 1,
        source_id: "ibge".into(),
        dataset_id: "ibge-6579".into(),
        title: "População residente estimada".into(),
        publisher: "IBGE".into(),
        source_url: "https://servicodados.ibge.gov.br/api/v3/agregados/6579".into(),
        metadata_url: "https://servicodados.ibge.gov.br/api/v3/agregados/6579/metadados".into(),
        licence: "IBGE public statistics".into(),
        licence_url: "https://www.ibge.gov.br/acesso-informacao/termos-de-uso.html".into(),
        licence_status: "review_required".into(),
        attribution: "Fonte: IBGE".into(),
        retrieved_at: "unix:0".into(),
        publication: None,
        coverage: "Mato Grosso".into(),
        unit: "Pessoas".into(),
        period_from: Some("2024".into()),
        period_to: Some("2024".into()),
        raw_sha256: "0".repeat(64),
        raw_bytes: 1,
        row_count: 1,
        accepted_rows: 1,
        rejected_rows: 0,
        duplicate_rows: 0,
        malformed_rows: 0,
        state: "queryable".into(),
        limitations: vec!["A municipal statistic is not a CNPJ fact.".into()],
    };
    write_store(official.path().join("ibge-6579.ocind"), &manifest, rows).unwrap();
    let e = env(&["--official-dir", official.path().to_str().unwrap(), "--auth", "off"]);
    let sources = get(&e.public, "/api/v1/official/sources", None).await;
    assert_eq!(sources.status, StatusCode::OK);
    assert!(sources.json()["datasets"].as_array().unwrap().iter().any(|row| row["dataset_id"] == "ibge-6579"));

    let answer =
        get(&e.public, "/api/v1/official/query?operation=lookup&variable=populacao_estimada&geography=5107925&period=2024", None).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.text);
    let body = answer.json();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["unit"], "Pessoas");
    assert_eq!(body["rows"][0]["value"], 120985.0);
    assert_eq!(body["sources"][0]["licence_status"], "review_required");

    let unavailable = get(&e.public, "/api/v1/official/query?variable=populacao_estimada&geography=5107925&period=2023", None).await;
    assert_eq!(unavailable.status, StatusCode::OK);
    assert_eq!(unavailable.json()["status"], "empty");

    let bad = get(&e.public, "/api/v1/official/query?operation=rank&variable=populacao_estimada&limit=101", None).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread")]
async fn required_auth_guards_data_routes_only() {
    let e = env(&[]);
    let r = get(&e.public, "/api/v1/search?uf=SP", None).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert_eq!(r.json()["error"], "unauthenticated");
    assert!(r.header("www-authenticate").unwrap().starts_with("Bearer"));
    for uri in [
        "/api/v1/cnpj/00000000000191",
        "/api/v1/cpf/12345678901",
        "/api/v1/stats",
        "/api/v1/export?uf=SP",
        "/downloads/fixture.ocnpj",
        "/api/v1/official/query?variable=populacao_estimada",
    ] {
        assert_eq!(get(&e.public, uri, None).await.status, StatusCode::UNAUTHORIZED, "{uri}");
    }
    for uri in [
        "/",
        "/healthz",
        "/readyz",
        "/api/v1/campos",
        "/api/v1/dominios",
        "/api/v1/snapshot",
        "/api/v1/distribution",
        "/api/v1/official/sources",
    ] {
        assert_eq!(get(&e.public, uri, None).await.status, StatusCode::OK, "{uri}");
    }
    assert_eq!(get(&e.public, "/metrics", None).await.status, StatusCode::NOT_FOUND, "metrics move to the admin listener");
    let bad = get(&e.public, "/api/v1/search?uf=SP", Some("ocnpj_0000000000_00000000000000000000000000000000")).await;
    assert_eq!((bad.status, bad.json()["error"].as_str()), (StatusCode::UNAUTHORIZED, Some("invalid_credentials")));
    let r = get(&e.public, "/api/v1/search?uf=SP", Some("ocs_nope")).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_login_me_logout_rotate() {
    let e = env(&[]);
    let r = post(&e.public, "/api/v1/auth/register", None, json!({ "username": "Ana", "email": "Ana@Example.com" })).await;
    assert_eq!(r.status, StatusCode::ACCEPTED, "{}", r.text);
    let key = mailed(&e.outbox, "ana@example.com", "ocnpj_", KEY_LEN).await;
    // Same address again: same answer, and a "use recovery" e-mail instead of a second account.
    let r = post(&e.public, "/api/v1/auth/register", None, json!({ "username": "ana2", "email": "ana@example.com" })).await;
    assert_eq!(r.status, StatusCode::ACCEPTED);
    let _ = mailed(&e.outbox, "ana@example.com", "already has one", 15).await;
    let r = post(&e.public, "/api/v1/auth/register", None, json!({ "username": "ana", "email": "other@example.com" })).await;
    assert_eq!((r.status, r.json()["error"].as_str()), (StatusCode::CONFLICT, Some("username_taken")));
    let r = post(&e.public, "/api/v1/auth/register", None, json!({ "username": "x", "email": "x@example.com" })).await;
    assert_eq!(r.json()["error"], "invalid_username");
    let r = post(&e.public, "/api/v1/auth/register", None, json!({ "username": "valid", "email": "nope" })).await;
    assert_eq!(r.json()["error"], "invalid_email");

    // The key works directly as a bearer credential and proves the address.
    let r = get(&e.public, "/api/v1/search?uf=SP&limit=1", Some(&key)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    assert_eq!(r.header("x-quota-daily-limit"), Some("10"));
    assert_eq!(r.header("x-quota-daily-remaining"), Some("9"));
    assert_eq!(r.header("x-ratelimit-limit"), Some("60"));
    let r = call(&e.public, Method::GET, "/api/v1/search?uf=SP&limit=1", None, None, &[("x-api-key", &key)]).await;
    assert_eq!(r.status, StatusCode::OK);

    let token = login(&e, &key).await;
    assert!(token.starts_with("ocs_"));
    let me = get(&e.public, "/api/v1/auth/me", Some(&token)).await.json();
    assert_eq!(me["user"]["username"], "ana");
    assert_eq!(me["user"]["status"], "active");
    assert_eq!(me["plan"]["name"], "free");
    assert_eq!(me["usage"]["today"], 2);
    assert_eq!(me["limits"]["per_minute"], 60);

    let r = call(&e.public, Method::POST, "/api/v1/auth/logout", Some(&token), None, &[]).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(get(&e.public, "/api/v1/auth/me", Some(&token)).await.status, StatusCode::UNAUTHORIZED);

    let r = call(&e.public, Method::POST, "/api/v1/auth/key/rotate", Some(&key), None, &[]).await;
    assert_eq!(r.status, StatusCode::OK);
    let new = r.json()["key"].as_str().unwrap().to_string();
    assert_eq!(get(&e.public, "/api/v1/search?uf=SP&limit=1", Some(&key)).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(get(&e.public, "/api/v1/search?uf=SP&limit=1", Some(&new)).await.status, StatusCode::OK);
    let _ = mailed(&e.outbox, "ana@example.com", "was replaced", 12).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn recovery_by_email() {
    let e = env(&[]);
    let old = create(&e, "bia", "user", None);
    let r = post(&e.public, "/api/v1/auth/recover", None, json!({ "email": "BIA@example.com" })).await;
    assert_eq!(r.status, StatusCode::ACCEPTED);
    let r = post(&e.public, "/api/v1/auth/recover", None, json!({ "email": "ghost@example.com" })).await;
    assert_eq!(r.status, StatusCode::ACCEPTED, "unknown addresses get the same answer");
    let token = mailed(&e.outbox, "bia@example.com", "ocr_", TOKEN_LEN).await;

    // GET only shows a form: nothing changes.
    let page = get(&e.public, &format!("/api/v1/auth/recover/confirm?token={token}"), None).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.text.contains("<form method=\"post\"") && page.text.contains(&token));
    assert_eq!(get(&e.public, "/api/v1/search?uf=SP&limit=1", Some(&old)).await.status, StatusCode::OK);

    // The HTML form post shows the new key.
    let r = call(&e.public, Method::POST, "/api/v1/auth/recover/confirm", None, None, &[]).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let req = Request::post("/api/v1/auth/recover/confirm")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("token={token}")))
        .unwrap();
    let resp = e.public.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let html = String::from_utf8(axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
    let i = html.find("ocnpj_").unwrap();
    let new: String = html[i..i + KEY_LEN].to_string();
    let mailed_key = mailed(&e.outbox, "bia@example.com", "ocnpj_", KEY_LEN).await;
    assert_eq!(new, mailed_key, "the new key is also e-mailed");
    assert_eq!(get(&e.public, "/api/v1/search?uf=SP&limit=1", Some(&old)).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(get(&e.public, "/api/v1/search?uf=SP&limit=1", Some(&new)).await.status, StatusCode::OK);
    // Single use.
    let r = post(&e.public, "/api/v1/auth/recover/confirm", None, json!({ "token": token })).await;
    assert_eq!((r.status, r.json()["error"].as_str()), (StatusCode::BAD_REQUEST, Some("invalid_token")));
    // JSON flow.
    post(&e.public, "/api/v1/auth/recover", None, json!({ "email": "bia@example.com" })).await;
    let token = mailed(&e.outbox, "bia@example.com", "ocr_", TOKEN_LEN).await;
    let r = post(&e.public, "/api/v1/auth/recover/confirm", None, json!({ "token": token })).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.json()["key"].as_str().unwrap().starts_with("ocnpj_"));
}

#[tokio::test(flavor = "multi_thread")]
async fn plans_quotas_and_rate_limits() {
    let e = env(&[]);
    let root = login(&e, &create(&e, "root", "admin", None)).await;
    let r = post(
        &e.admin,
        "/admin/api/plans",
        Some(&root),
        json!({
            "name": "tiny", "daily_limit": 3, "monthly_limit": null, "per_minute": 100, "burst": 100,
            "max_page_size": 5, "export": false, "max_export_rows": 0, "downloads": false
        }),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text);
    let tiny = r.json()["id"].as_i64().unwrap();
    let key = create(&e, "carl", "user", Some("tiny"));
    for i in 0..3 {
        let r = get(&e.public, &format!("/api/v1/search?uf=SP&limit=1&offset={i}"), Some(&key)).await;
        assert_eq!(r.status, StatusCode::OK);
        assert_eq!(r.header("x-quota-daily-remaining"), Some(["2", "1", "0"][i]));
    }
    let r = get(&e.public, "/api/v1/search?uf=PR&limit=1", Some(&key)).await;
    assert_eq!((r.status, r.json()["error"].as_str()), (StatusCode::TOO_MANY_REQUESTS, Some("quota_exceeded")));
    assert!(r.header("retry-after").unwrap().parse::<u64>().unwrap() <= 86_400);

    // A moderator resets the day; the plan's page size and export rules apply.
    let carl = e.st.gate.accounts.as_ref().unwrap().user_by("username", "carl").unwrap().unwrap().id;
    let r = post(&e.admin, &format!("/admin/api/users/{carl}/reset-usage"), Some(&root), json!({ "scope": "day" })).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    let r = get(&e.public, "/api/v1/search?uf=SP&limit=6", Some(&key)).await;
    assert_eq!((r.status, r.json()["error"].as_str()), (StatusCode::BAD_REQUEST, Some("limit_exceeds_plan")));
    let r = get(&e.public, "/api/v1/export?uf=SP", Some(&key)).await;
    assert_eq!((r.status, r.json()["error"].as_str()), (StatusCode::FORBIDDEN, Some("plan_forbids_export")));
    let r = get(&e.public, "/downloads/fixture.ocnpj", Some(&key)).await;
    assert_eq!(r.json()["error"], "plan_forbids_downloads");

    // Per-user override: 2 per minute with the plan's burst capped by the rate.
    let r = call(
        &e.admin,
        Method::PATCH,
        &format!("/admin/api/plans/{tiny}"),
        Some(&root),
        Some(json!({ "per_minute": 2, "burst": 2, "daily_limit": null })),
        &[],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    e.st.gate.forget(carl);
    assert_eq!(get(&e.public, "/api/v1/stats?uf=SP", Some(&key)).await.status, StatusCode::OK);
    assert_eq!(get(&e.public, "/api/v1/stats?uf=PR", Some(&key)).await.status, StatusCode::OK);
    let r = get(&e.public, "/api/v1/stats?uf=RJ", Some(&key)).await;
    assert_eq!((r.status, r.json()["error"].as_str()), (StatusCode::TOO_MANY_REQUESTS, Some("rate_limited")));
    assert_eq!(r.header("retry-after"), Some("30"));
    let r = call(&e.admin, Method::DELETE, &format!("/admin/api/plans/{tiny}"), Some(&root), None, &[]).await;
    assert_eq!((r.status, r.json()["error"].as_str()), (StatusCode::CONFLICT, Some("plan_in_use")));

    // Pro plan exports with its row cap.
    let pro = create(&e, "dora", "user", Some("pro"));
    let r = get(&e.public, "/api/v1/export?uf=SP&format=csv", Some(&pro)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.text.lines().count() > 100);
}

#[tokio::test(flavor = "multi_thread")]
async fn response_cache_coalesces_and_validates() {
    let e = env(&[]);
    let key = create(&e, "eva", "user", Some("pro"));
    let uri = "/api/v1/search?q=mercadinho&uf=SP&limit=3";
    let a = get(&e.public, uri, Some(&key)).await;
    let b = get(&e.public, uri, Some(&key)).await;
    assert_eq!((a.header("x-cache"), b.header("x-cache")), (Some("MISS"), Some("HIT")));
    assert_eq!(a.text, b.text);
    assert_eq!(b.header("cache-control"), Some("private, max-age=300"));
    // Parameter order does not matter.
    let c = get(&e.public, "/api/v1/search?limit=3&uf=SP&q=mercadinho", Some(&key)).await;
    assert_eq!(c.header("x-cache"), Some("HIT"));
    let etag = b.header("etag").unwrap().to_string();
    let r = call(&e.public, Method::GET, uri, Some(&key), None, &[("if-none-match", &etag)]).await;
    assert_eq!(r.status, StatusCode::NOT_MODIFIED);
    assert!(r.text.is_empty());
    // Concurrent identical requests run once.
    let uri = "/api/v1/stats?uf=PR&group_by=municipio";
    let futs: Vec<_> = (0..16).map(|_| get(&e.public, uri, Some(&key))).collect();
    let rs = futures_util::future::join_all(futs).await;
    assert!(rs.iter().all(|r| r.status == StatusCode::OK));
    assert_eq!(rs.iter().filter(|r| r.header("x-cache") == Some("MISS")).count(), 1);
    // Errors are not cached; a snapshot switch empties the cache.
    assert_eq!(get(&e.public, "/api/v1/search?nope=1", Some(&key)).await.header("x-cache"), Some("MISS"));
    assert_eq!(get(&e.public, "/api/v1/search?nope=1", Some(&key)).await.header("x-cache"), Some("MISS"));
    let path = e.st.snapshot().path().to_path_buf();
    e.st.switch_to(&path).unwrap();
    assert_eq!(get(&e.public, "/api/v1/search?q=mercadinho&uf=SP&limit=3", Some(&key)).await.header("x-cache"), Some("MISS"));
}

#[tokio::test(flavor = "multi_thread")]
async fn roles_and_moderation() {
    let e = env(&[]);
    let root = login(&e, &create(&e, "root", "admin", None)).await;
    let user_key = create(&e, "fred", "user", None);
    let fred = e.st.gate.accounts.as_ref().unwrap().user_by("username", "fred").unwrap().unwrap().id;
    let root_id = e.st.gate.accounts.as_ref().unwrap().user_by("username", "root").unwrap().unwrap().id;

    assert_eq!(get(&e.admin, "/admin/api/users", None).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(get(&e.admin, "/admin/api/users", Some(&user_key)).await.status, StatusCode::FORBIDDEN);
    assert_eq!(get(&e.admin, "/admin/", None).await.status, StatusCode::OK, "the UI page itself is public");
    let who = get(&e.admin, "/admin/api/whoami", Some(&root)).await.json();
    assert_eq!(who["user"]["role"], "admin");
    assert_eq!(who["permissions"].as_array().unwrap().len(), 9);
    assert_eq!(get(&e.admin, "/admin/api/roles", Some(&root)).await.json().as_array().unwrap().len(), 4);

    // Admin creates staff.
    let r = post(
        &e.admin,
        "/admin/api/users",
        Some(&root),
        json!({ "username": "mod", "email": "mod@example.com", "role": "moderator", "send_email": true }),
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text);
    let modk = login(&e, r.json()["key"].as_str().unwrap()).await;
    let _ = mailed(&e.outbox, "mod@example.com", "ocnpj_", KEY_LEN).await;
    let r =
        post(&e.admin, "/admin/api/users", Some(&root), json!({ "username": "ana", "email": "ana@example.com", "role": "analyst" })).await;
    let ana = login(&e, r.json()["key"].as_str().unwrap()).await;

    // Analyst: read-only.
    assert_eq!(get(&e.admin, "/admin/api/users?q=fre", Some(&ana)).await.json()["total"], 1);
    assert_eq!(get(&e.admin, "/admin/api/overview", Some(&ana)).await.status, StatusCode::OK);
    assert_eq!(
        post(&e.admin, &format!("/admin/api/users/{fred}/suspend"), Some(&ana), json!({ "reason": "x" })).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(post(&e.admin, "/admin/api/jobs", Some(&ana), json!({ "kind": "verify" })).await.status, StatusCode::FORBIDDEN);
    assert_eq!(get(&e.admin, "/metrics", Some(&ana)).await.status, StatusCode::OK);

    // Moderator: suspends users, edits notes, but cannot manage accounts or touch staff.
    let r = post(&e.admin, &format!("/admin/api/users/{fred}/suspend"), Some(&modk), json!({ "reason": "scraping" })).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    assert_eq!(r.json()["user"]["status"], "suspended");
    let r = get(&e.public, "/api/v1/search?uf=SP&limit=1", Some(&user_key)).await;
    assert_eq!((r.status, r.json()["error"].as_str()), (StatusCode::FORBIDDEN, Some("account_suspended")));
    assert_eq!(
        post(&e.admin, &format!("/admin/api/users/{fred}/suspend"), Some(&modk), json!({})).await.status,
        StatusCode::BAD_REQUEST,
        "reason required"
    );
    let r = call(&e.admin, Method::PATCH, &format!("/admin/api/users/{fred}"), Some(&modk), Some(json!({ "notes": "warned" })), &[]).await;
    assert_eq!(r.status, StatusCode::OK);
    let r = call(&e.admin, Method::PATCH, &format!("/admin/api/users/{fred}"), Some(&modk), Some(json!({ "plan": "pro" })), &[]).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(
        post(&e.admin, &format!("/admin/api/users/{root_id}/suspend"), Some(&modk), json!({ "reason": "coup" })).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(post(&e.admin, &format!("/admin/api/users/{fred}/reactivate"), Some(&modk), json!({})).await.status, StatusCode::OK);
    assert_eq!(get(&e.public, "/api/v1/search?uf=SP&limit=1", Some(&user_key)).await.status, StatusCode::OK);

    // Bans: moderators ban, only admins lift them.
    assert_eq!(
        post(&e.admin, &format!("/admin/api/users/{fred}/ban"), Some(&modk), json!({ "reason": "fraud" })).await.status,
        StatusCode::OK
    );
    assert_eq!(get(&e.public, "/api/v1/search?uf=SP&limit=1", Some(&user_key)).await.json()["error"], "account_banned");
    assert_eq!(post(&e.admin, &format!("/admin/api/users/{fred}/reactivate"), Some(&modk), json!({})).await.status, StatusCode::FORBIDDEN);
    assert_eq!(post(&e.admin, &format!("/admin/api/users/{fred}/reactivate"), Some(&root), json!({})).await.status, StatusCode::OK);

    // Timed suspension lifts itself.
    let until = opencnpj::accounts::now() + 3600;
    let r =
        post(&e.admin, &format!("/admin/api/users/{fred}/suspend"), Some(&root), json!({ "reason": "cool down", "until": until })).await;
    assert_eq!(r.json()["user"]["suspended_until"], until);
    post(&e.admin, &format!("/admin/api/users/{fred}/reactivate"), Some(&root), json!({})).await;

    // Self-protection and the last admin.
    let r = call(&e.admin, Method::PATCH, &format!("/admin/api/users/{root_id}"), Some(&root), Some(json!({ "role": "user" })), &[]).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(
        call(&e.admin, Method::DELETE, &format!("/admin/api/users/{root_id}"), Some(&root), None, &[]).await.status,
        StatusCode::FORBIDDEN
    );

    // Key rotation by staff revokes the old key; sessions can be revoked.
    let r = post(&e.admin, &format!("/admin/api/users/{fred}/rotate-key"), Some(&modk), json!({ "send_email": true })).await;
    let fred_new = r.json()["key"].as_str().unwrap().to_string();
    assert_eq!(get(&e.public, "/api/v1/search?uf=SP&limit=1", Some(&user_key)).await.status, StatusCode::UNAUTHORIZED);
    let fred_session = login(&e, &fred_new).await;
    let r = post(&e.admin, &format!("/admin/api/users/{fred}/revoke-sessions"), Some(&modk), json!({})).await;
    assert_eq!(r.json()["revoked"], 1);
    assert_eq!(get(&e.public, "/api/v1/auth/me", Some(&fred_session)).await.status, StatusCode::UNAUTHORIZED);

    // Detail, audit trail, delete.
    let d = get(&e.admin, &format!("/admin/api/users/{fred}"), Some(&ana)).await.json();
    assert_eq!(d["user"]["notes"], "warned");
    assert!(d["keys"].as_array().unwrap().len() >= 2);
    let audit = get(&e.admin, &format!("/admin/api/audit?target_id={fred}"), Some(&ana)).await.json();
    let actions: Vec<&str> = audit.as_array().unwrap().iter().map(|a| a["action"].as_str().unwrap()).collect();
    for a in ["user.suspend", "user.ban", "user.reactivate", "user.update", "user.rotate_key", "user.revoke_sessions"] {
        assert!(actions.contains(&a), "{a} missing from {actions:?}");
    }
    assert!(audit[0]["actor"].is_string() && audit[0]["ip"].is_string());
    assert_eq!(
        call(&e.admin, Method::DELETE, &format!("/admin/api/users/{fred}"), Some(&modk), None, &[]).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(call(&e.admin, Method::DELETE, &format!("/admin/api/users/{fred}"), Some(&root), None, &[]).await.status, StatusCode::OK);
    assert_eq!(get(&e.public, "/api/v1/search?uf=SP&limit=1", Some(&fred_new)).await.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test(flavor = "multi_thread")]
async fn usage_search_log_settings_and_updates() {
    let e = env(&[]);
    let root = login(&e, &create(&e, "root", "admin", None)).await;
    let key = create(&e, "gil", "user", None);
    for uri in [
        "/api/v1/search?q=padaria",
        "/api/v1/cnpj/11111111000191",
        "/api/v1/cpf/12345678901",
        "/api/v1/stats?uf=SP",
        "/api/v1/search?q=padaria",
    ] {
        get(&e.public, uri, Some(&key)).await;
    }
    let u = get(&e.admin, "/admin/api/usage?days=7", Some(&root)).await.json();
    assert_eq!(u["by_kind"]["search"], 2);
    assert_eq!(u["by_kind"]["lookup"], 1);
    assert_eq!(u["by_kind"]["cpf"], 1);
    assert_eq!(u["by_kind"]["stats"], 1);
    assert_eq!(u["top_users_today"][0]["username"], "gil");
    assert_eq!(u["daily"][0]["cache_hits"], 1);
    let list = get(&e.admin, "/admin/api/users?sort=-usage_today", Some(&root)).await.json();
    assert_eq!(list["users"][0]["username"], "gil");
    assert_eq!(list["users"][0]["usage_today"], 5);
    let gil = list["users"][0]["id"].as_i64().unwrap();
    let mut logged = Value::Null;
    for _ in 0..100 {
        logged = get(&e.admin, &format!("/admin/api/searches?user_id={gil}"), Some(&root)).await.json();
        if logged.as_array().unwrap().len() == 5 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    assert_eq!(logged.as_array().unwrap().len(), 5);
    assert_eq!(logged[0]["query"], "q=padaria");
    assert_eq!(logged[0]["cache"], true);

    let s = get(&e.admin, "/admin/api/settings", Some(&root)).await.json();
    assert_eq!((s["registration"].as_str(), s["auth"].as_str()), (Some("open"), Some("required")));
    let r = call(
        &e.admin,
        Method::PATCH,
        "/admin/api/settings",
        Some(&root),
        Some(json!({ "registration": "closed", "search_log": false })),
        &[],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    let r = post(&e.public, "/api/v1/auth/register", None, json!({ "username": "late", "email": "late@example.com" })).await;
    assert_eq!(r.json()["error"], "registration_closed");
    let r = call(&e.admin, Method::PATCH, "/admin/api/settings", Some(&root), Some(json!({ "bogus": 1 })), &[]).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    let up = get(&e.admin, "/admin/api/updates", Some(&root)).await.json();
    assert_eq!(up["enabled"], false, "no --data-dir: nothing to update");
    assert_eq!(up["current_competence"], "2026-10");
    let rc = get(&e.admin, "/admin/api/response-cache", Some(&root)).await.json();
    assert_eq!(rc["hits"], 1);
    assert_eq!(call(&e.admin, Method::DELETE, "/admin/api/response-cache", Some(&root), None, &[]).await.json()["enabled"], true);
    let prom = get(&e.admin, "/metrics", Some(&root)).await.text;
    assert!(prom.contains("opencnpj_response_cache_hits") && prom.contains("opencnpj_rejected_total{reason=\"quota_exceeded\"}"));
}

#[tokio::test(flavor = "multi_thread")]
async fn optional_and_off_modes() {
    let e = env(&["--auth", "optional"]);
    let r = get(&e.public, "/api/v1/search?uf=SP&limit=1", None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.header("x-quota-daily-limit"), Some("100"));
    assert_eq!(get(&e.public, "/api/v1/search?uf=SP&limit=21", None).await.json()["error"], "limit_exceeds_plan");
    let key = create(&e, "hal", "user", None);
    assert_eq!(get(&e.public, "/api/v1/search?uf=SP&limit=1", Some(&key)).await.header("x-quota-daily-limit"), Some("10"));

    let off = env(&["--auth", "off"]);
    let r = get(&off.public, "/api/v1/search?uf=SP&limit=1", None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.header("x-quota-daily-limit").is_none());
    assert_eq!(r.header("cache-control"), Some("public, max-age=3600"));
    assert_eq!(post(&off.public, "/api/v1/auth/login", None, json!({ "key": "x" })).await.json()["error"], "accounts_disabled");
    assert_eq!(get(&off.public, "/metrics", None).await.status, StatusCode::OK);
    assert_eq!(get(&off.admin, "/admin/api/overview", None).await.status, StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn platform_assistant_routes_and_web_front_end() {
    let e = env(&[]);
    // What a client learns before logging in.
    let p = get(&e.public, "/api/v1/platform", None).await;
    assert_eq!(p.status, StatusCode::OK);
    let v = p.json();
    assert_eq!((v["auth"].as_str(), v["registration"].as_str()), (Some("required"), Some("open")));
    assert_eq!((v["assistant"].as_bool(), v["search"].as_bool(), v["competence"].as_str()), (Some(false), Some(true), Some("2026-10")));

    // The assistant routes are data routes: a credential first, then the plan, then the service.
    let ask = json!({ "question": "padarias em Sinop/MT" });
    let r = post(&e.public, "/api/v1/assistant/ask", None, ask.clone()).await;
    assert_eq!((r.status, r.json()["error"].as_str()), (StatusCode::UNAUTHORIZED, Some("unauthenticated")));
    let key = create(&e, "ana", "user", None);
    let r = post(&e.public, "/api/v1/assistant/ask", Some(&key), ask).await;
    assert_eq!((r.status, r.json()["error"].as_str()), (StatusCode::SERVICE_UNAVAILABLE, Some("assistant_unavailable")));
    let r = post(&e.public, "/api/v1/assistant/export", Some(&key), json!({ "request": {}, "export": {} })).await;
    assert_eq!((r.status, r.json()["error"].as_str()), (StatusCode::FORBIDDEN, Some("plan_forbids_export")));

    // A user reads their own usage and searches; nobody else's.
    assert_eq!(get(&e.public, "/api/v1/auth/usage", None).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(get(&e.public, "/api/v1/search?uf=SP&limit=1", Some(&key)).await.status, StatusCode::OK);
    let own = get(&e.public, "/api/v1/auth/usage", Some(&key)).await;
    assert_eq!(own.status, StatusCode::OK);
    assert!(own.json()["usage_daily"].is_array() && own.json()["recent_searches"].is_array());

    // Municipalities of one state, for filter forms: reference data, no credential.
    let cities = get(&e.public, "/api/v1/dominios/municipios/sp", None).await;
    assert_eq!(cities.status, StatusCode::OK);
    let rows = cities.json();
    assert!(!rows.as_array().unwrap().is_empty());
    assert!(rows[0]["codigo"].is_string() && rows[0]["descricao"].is_string());
    assert_eq!(get(&e.public, "/api/v1/dominios/municipios/s1", None).await.status, StatusCode::BAD_REQUEST);

    // The built front end owns `/`; the service description moves to `/api/v1`.
    let site = tempfile::tempdir().unwrap();
    std::fs::write(site.path().join("index.html"), "<!doctype html><title>OpenCNPJ</title>").unwrap();
    std::fs::create_dir(site.path().join("js")).unwrap();
    std::fs::write(site.path().join("js/main.js"), "export {};").unwrap();
    std::fs::write(site.path().join(".secret"), "no").unwrap();
    let w = env(&["--web", site.path().to_str().unwrap()]);
    let home = get(&w.public, "/", None).await;
    assert_eq!((home.status, home.header("content-type")), (StatusCode::OK, Some("text/html; charset=utf-8")));
    assert!(home.header("content-security-policy").unwrap().contains("default-src 'self'"));
    assert_eq!(home.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(get(&w.public, "/js/main.js", None).await.header("content-type"), Some("text/javascript; charset=utf-8"));
    for missing in ["/.secret", "/js/../.secret", "/nope.css", "/api/v1/nope", "/admin/nope"] {
        assert_eq!(get(&w.public, missing, None).await.status, StatusCode::NOT_FOUND, "{missing}");
    }
    assert_eq!(get(&w.public, "/api/v1", None).await.json()["service"], "opencnpj");
    assert_eq!(get(&w.admin, "/", None).await.status, StatusCode::OK, "the admin listener serves the application too");
}

fn trial_cookie(response: &Resp) -> &str {
    response.header("set-cookie").unwrap().split(';').next().unwrap()
}

const TRIAL_SEARCH: &str = "/api/v1/search?uf=SP&limit=1";

#[tokio::test]
async fn browser_trial_survives_restart_ip_changes_and_concurrent_requests() {
    let e = env(&["--auth", "trial", "--trust-proxy", "--public-url", "https://example.com"]);
    let status = get(&e.public, "/api/v1/trial", None).await;
    assert_eq!(status.json(), json!({ "limit": 3, "remaining": 3 }));
    let cookie = trial_cookie(&status);
    let set_cookie = status.header("set-cookie").unwrap();
    assert!(set_cookie.contains("HttpOnly") && set_cookie.contains("SameSite=Lax") && set_cookie.ends_with("; Secure"));
    assert_eq!(get(&e.public, "/api/v1/export?uf=SP", None).await.json()["error"], "plan_forbids_export");
    assert_eq!(get(&e.public, "/api/v1/search?uf=SP&limit=1&%6cimit=21", None).await.json()["error"], "limit_exceeds_plan");

    let other = api::router(api::state(e.st.args.clone()).unwrap());
    let addresses = ["192.0.2.1", "192.0.2.2", "192.0.2.3", "192.0.2.4", "192.0.2.5"];
    let responses = futures_util::future::join_all(addresses.iter().enumerate().map(|(i, &ip)| {
        let app = if i % 2 == 0 { &e.public } else { &other };
        async move { call(app, Method::GET, TRIAL_SEARCH, None, None, &[("cookie", cookie), ("x-forwarded-for", ip)]).await }
    }))
    .await;
    assert_eq!(responses.iter().filter(|r| r.status == StatusCode::OK).count(), 3);
    assert_eq!(responses.iter().filter(|r| r.status == StatusCode::TOO_MANY_REQUESTS).count(), 2);

    let restarted = api::router(api::state(e.st.args.clone()).unwrap());
    let r = call(&restarted, Method::GET, TRIAL_SEARCH, None, None, &[("cookie", cookie), ("x-forwarded-for", "192.0.2.6")]).await;
    assert_eq!(r.json()["error"], "trial_exhausted");
    assert_eq!(r.header("x-trial-remaining"), Some("0"));
    assert_eq!(r.header("cache-control"), Some("private, no-store"));
}

#[tokio::test]
async fn trial_network_budget_survives_cookie_reset_and_ipv6_rotation() {
    let e = env(&["--auth", "trial", "--trust-proxy"]);
    for i in 1..=13 {
        let ip = format!("2001:db8:100:2::{i}");
        let r = call(&e.public, Method::GET, TRIAL_SEARCH, None, None, &[("x-forwarded-for", &ip)]).await;
        if i <= 12 {
            assert_eq!(r.status, StatusCode::OK, "{}", r.text);
        } else {
            assert_eq!(r.json()["error"], "trial_exhausted");
        }
    }
}

#[tokio::test]
async fn free_accounts_share_device_budget_and_paid_plans_keep_their_access() {
    let e = env(&["--auth", "trial", "--trust-proxy"]);
    let a = create(&e, "free-one", "user", Some("free"));
    let b = create(&e, "free-two", "user", Some("free"));
    let pro = create(&e, "paid", "user", Some("pro"));
    let status = get(&e.public, "/api/v1/trial", None).await;
    let cookie = trial_cookie(&status);
    for i in 0..10 {
        let key = if i % 2 == 0 { &a } else { &b };
        let r = call(&e.public, Method::GET, TRIAL_SEARCH, Some(key), None, &[("cookie", cookie)]).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text);
        assert_eq!(r.header("x-quota-monthly-limit"), Some("30"));
    }
    let r = call(&e.public, Method::GET, TRIAL_SEARCH, Some(&b), None, &[("cookie", cookie), ("x-forwarded-for", "192.0.2.20")]).await;
    assert_eq!(r.json()["error"], "free_access_limited");
    assert_eq!(get(&e.public, "/api/v1/export?uf=SP", Some(&a)).await.json()["error"], "plan_forbids_export");
    assert_eq!(get(&e.public, "/api/v1/export?uf=SP", Some(&pro)).await.status, StatusCode::OK);
}

#[tokio::test]
async fn registration_is_limited_by_device_and_configured_vpn_networks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vpn.txt");
    std::fs::write(&path, "203.0.113.0/24 # known VPN egress\n").unwrap();
    let e = env(&["--auth", "trial", "--trust-proxy", "--trial-deny-networks", path.to_str().unwrap()]);
    let route = "/api/v1/auth/register";
    let status = get(&e.public, "/api/v1/trial", None).await;
    let cookie = trial_cookie(&status);
    let signup = |i| json!({ "username": format!("guest-{i}"), "email": format!("guest-{i}@example.com") });
    let denied = call(&e.public, Method::POST, route, None, Some(signup(0)), &[("x-forwarded-for", "203.0.113.15")]).await;
    assert_eq!(denied.json()["error"], "trial_network_blocked");
    for i in 1..=3 {
        let ip = format!("192.0.2.{i}");
        let r = call(&e.public, Method::POST, route, None, Some(signup(i)), &[("cookie", cookie), ("x-forwarded-for", &ip)]).await;
        if i <= 2 {
            assert_eq!(r.status, StatusCode::ACCEPTED, "{}", r.text);
        } else {
            assert_eq!(r.json()["error"], "registration_limited");
        }
    }
}

#[tokio::test]
async fn conversations_are_private_and_survive_restart() {
    use opencnpj_ingest::source::sha256_file;
    use opencnpj_insights::agg::{Manifest, Table, write};
    use opencnpj_query::ir::Competence;

    let dir = tempfile::tempdir().unwrap();
    let aggregate = dir.path().join("fixture.ocagg");
    let catalogue = Path::new(env!("CARGO_MANIFEST_DIR")).join("../opencnpj-insights/catalog");
    let municipalities = catalogue.join("municipalities.tsv");
    let sectors = catalogue.join("sectors.toml");
    write(
        std::fs::File::create(&aggregate).unwrap(),
        &Manifest {
            competence: Competence::from_year_month(2026, 10).unwrap(),
            snapshot_sha256: sha256_file(fixture()).unwrap(),
            municipality_sha256: sha256_file(&municipalities).unwrap(),
            tables: Table::ALL.to_vec(),
            establishments: 0,
            missing_registration_date: 0,
            missing_closure_date: 0,
        },
        &Default::default(),
    )
    .unwrap();
    let e = env(&[
        "--assistant-agg",
        aggregate.to_str().unwrap(),
        "--assistant-catalog",
        municipalities.to_str().unwrap(),
        "--assistant-sectors",
        sectors.to_str().unwrap(),
    ]);
    let a = create(&e, "author", "user", Some("pro"));
    let b = create(&e, "another", "user", Some("pro"));
    let root = "/api/v1/conversations";
    assert_eq!(get(&e.public, root, None).await.status, StatusCode::UNAUTHORIZED);
    let created = post(&e.public, root, Some(&a), json!({ "title": "Pesquisa em Cuiabá" })).await;
    assert_eq!(created.status, StatusCode::CREATED);
    let id = created.json()["id"].as_i64().unwrap();
    let conversation = format!("{root}/{id}");
    assert_eq!(get(&e.public, root, Some(&b)).await.json(), json!([]));
    assert_eq!(get(&e.public, &conversation, Some(&b)).await.status, StatusCode::NOT_FOUND);
    assert_eq!(call(&e.public, Method::DELETE, &conversation, Some(&b), None, &[]).await.status, StatusCode::NOT_FOUND);
    let ask = "/api/v1/assistant/ask";
    let question = json!({
        "conversation_id": id,
        "conversation_revision": 0,
        "question": "Liste 5 matrizes de farmácias ativas em Cuiabá MT e Sinop MT, optantes pelo Simples, sem MEI, com telefone e e-mail, ordenadas pelo maior capital."
    });
    assert_eq!(post(&e.public, ask, Some(&b), question.clone()).await.status, StatusCode::NOT_FOUND);
    let first = post(&e.public, ask, Some(&a), question.clone()).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text);
    let mut expected = first.json()["interpretation"]["request"].clone();
    assert_eq!(expected["op"], json!({"kind": "list", "limit": 5, "order": "capital_desc"}));
    assert_eq!(expected["scope"]["contact"], "both");
    assert_eq!(expected["scope"]["head_office"], "yes");
    assert_eq!(expected["scope"]["simples"], "yes");
    assert_eq!(expected["scope"]["mei"], "no");
    let restarted = api::router(api::state(e.st.args.clone()).unwrap());
    assert_eq!(post(&restarted, ask, Some(&a), question).await.json()["error"], "conversation_changed");
    let next = json!({ "conversation_id": id, "conversation_revision": 1, "question": "dessas só as de Sinop" });
    let follow_up = post(&restarted, ask, Some(&a), next).await;
    assert_eq!(follow_up.status, StatusCode::OK, "{}", follow_up.text);
    expected["scope"]["place"] = json!({"municipalities": [5067]});
    assert_eq!(follow_up.json()["interpretation"]["request"], expected);
    let file_turn =
        post(&restarted, ask, Some(&a), json!({ "conversation_id": id, "conversation_revision": 2, "question": "me manda em Excel" }))
            .await;
    assert_eq!(file_turn.status, StatusCode::OK, "{}", file_turn.text);
    let file_reply = file_turn.json();
    assert_eq!(file_reply["interpretation"]["request"], expected);
    let export = &file_reply["interpretation"]["export"];
    assert_eq!(export["format"], "xlsx");
    assert_eq!(export["rows"], 5);
    let download = post(&restarted, "/api/v1/assistant/export", Some(&a), json!({"request": expected, "export": export})).await;
    assert_eq!(download.status, StatusCode::OK);
    assert!(download.header("content-disposition").unwrap().ends_with(".xlsx\""));
    let history = get(&restarted, &conversation, Some(&a)).await;
    assert_eq!(history.header("cache-control"), Some("private, no-store"));
    let turns = history.json();
    assert_eq!(turns.as_array().unwrap().len(), 3);
    assert_eq!(turns[1]["question"], "dessas só as de Sinop");
    assert_eq!(turns[1]["reply"], follow_up.json());
    assert_eq!(turns[2]["question"], "me manda em Excel");
    assert_eq!(turns[2]["reply"], file_reply);
    let renamed = call(&restarted, Method::PATCH, &conversation, Some(&a), Some(json!({ "title": "Empresas em Mato Grosso" })), &[]).await;
    assert_eq!(renamed.json()["title"], "Empresas em Mato Grosso");
    assert_eq!(call(&restarted, Method::DELETE, &conversation, Some(&a), None, &[]).await.status, StatusCode::NO_CONTENT);
    assert_eq!(get(&restarted, &conversation, Some(&a)).await.status, StatusCode::NOT_FOUND);
}
