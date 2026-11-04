//! HTTP client against a minimal in-process server (std only): login flow, bearer tokens,
//! verbs, bodies and error mapping.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use opencnpj_desktop::api::{ApiResult, Client};
use opencnpj_desktop::models::{UserEnvelope, UserList};
use serde_json::{Value, json};

#[derive(Debug, Clone)]
struct Request {
    method: String,
    path: String,
    auth: Option<String>,
    body: String,
}

/// `(method, path with query, status, content type, body)`.
type Route = (&'static str, &'static str, u16, &'static str, String);

fn serve(routes: Vec<Route>) -> (String, Arc<Mutex<Vec<Request>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let log = Arc::new(Mutex::new(Vec::new()));
    let log2 = log.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                continue;
            }
            let mut parts = line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default().to_string();
            let (mut len, mut auth) = (0usize, None);
            loop {
                let mut h = String::new();
                reader.read_line(&mut h).unwrap();
                let h = h.trim_end();
                if h.is_empty() {
                    break;
                }
                let (k, v) = h.split_once(':').unwrap();
                match k.to_ascii_lowercase().as_str() {
                    "content-length" => len = v.trim().parse().unwrap(),
                    "authorization" => auth = Some(v.trim().to_string()),
                    _ => {}
                }
            }
            let mut body = vec![0; len];
            reader.read_exact(&mut body).unwrap();
            log2.lock().unwrap().push(Request { method: method.clone(), path: path.clone(), auth, body: String::from_utf8(body).unwrap() });
            let (status, ctype, text) = routes.iter().find(|r| r.0 == method && r.1 == path).map(|r| (r.2, r.3, r.4.clone())).unwrap_or((
                404,
                "application/json",
                json!({"error":"not_found","message":"no route","code":404}).to_string(),
            ));
            let head =
                format!("HTTP/1.1 {status} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", text.len());
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(text.as_bytes());
        }
    });
    (base, log)
}

const JSON: &str = "application/json";

fn user(id: i64, role: &str) -> Value {
    json!({ "id": id, "username": "admin", "email": "admin@example.com", "role": role, "status": "active", "plan": "unlimited",
            "created_at": 1793750000, "verified_at": null, "last_seen_at": null, "suspended_until": null, "status_reason": null,
            "notes": "", "daily_limit": null, "monthly_limit": null, "per_minute": null, "key_prefix": "Ab3dE5gH9k" })
}

#[test]
fn login_then_whoami_with_bearer_token() {
    let (base, log) = serve(vec![
        (
            "POST",
            "/api/v1/auth/login",
            200,
            JSON,
            json!({"token":"ocs_t","token_type":"Bearer","expires_at":1,"user":user(1,"admin")}).to_string(),
        ),
        ("GET", "/admin/api/whoami", 200, JSON, json!({"user":user(1,"admin"),"permissions":["users.read","system.read"]}).to_string()),
    ]);
    let (client, me) = Client::login(&base, "ocnpj_key").unwrap();
    assert_eq!(me.user.role, "admin");
    assert_eq!(me.permissions, vec!["users.read", "system.read"]);
    assert_eq!(client.base(), base);
    let log = log.lock().unwrap();
    assert_eq!(log[0].method, "POST");
    assert_eq!(serde_json::from_str::<Value>(&log[0].body).unwrap(), json!({"key":"ocnpj_key"}));
    assert_eq!(log[0].auth, None);
    assert_eq!(log[1].path, "/admin/api/whoami");
    assert_eq!(log[1].auth.as_deref(), Some("Bearer ocs_t"));
}

#[test]
fn invalid_key_is_a_401() {
    let (base, _) = serve(vec![(
        "POST",
        "/api/v1/auth/login",
        401,
        JSON,
        json!({"error":"invalid_key","message":"invalid login key","code":401}).to_string(),
    )]);
    let e = Client::login(&base, "nope").unwrap_err();
    assert!(e.is_unauthorized());
    assert_eq!(e.code, "invalid_key");
}

#[test]
fn non_staff_account_is_refused_and_logged_out() {
    let (base, log) = serve(vec![
        (
            "POST",
            "/api/v1/auth/login",
            200,
            JSON,
            json!({"token":"ocs_u","token_type":"Bearer","expires_at":1,"user":user(9,"user")}).to_string(),
        ),
        ("GET", "/admin/api/whoami", 403, JSON, json!({"error":"forbidden","message":"staff only","code":403}).to_string()),
        ("POST", "/api/v1/auth/logout", 204, JSON, String::new()),
    ]);
    let e = Client::login(&base, "k").unwrap_err();
    assert_eq!(e.status, 403);
    assert!(e.message.contains("staff"), "{e}");
    let log = log.lock().unwrap();
    let logout = log.iter().find(|r| r.path == "/api/v1/auth/logout").expect("logout called");
    assert_eq!(logout.auth.as_deref(), Some("Bearer ocs_u"));
}

#[test]
fn verbs_bodies_and_text() {
    let (base, log) = serve(vec![
        ("POST", "/api/v1/auth/login", 200, JSON, json!({"token":"ocs_t","user":user(1,"admin")}).to_string()),
        ("GET", "/admin/api/whoami", 200, JSON, json!({"user":user(1,"admin"),"permissions":[]}).to_string()),
        ("GET", "/admin/api/users?q=ana&limit=50", 200, JSON, json!({"total":1,"users":[user(7,"user")]}).to_string()),
        ("PATCH", "/admin/api/users/7", 200, JSON, json!({"user":user(7,"moderator")}).to_string()),
        ("DELETE", "/admin/api/users/7", 200, JSON, json!({"deleted":true}).to_string()),
        ("POST", "/admin/api/users/7/reactivate", 200, JSON, json!({"user":user(7,"user")}).to_string()),
        ("POST", "/admin/api/updates/check", 202, JSON, String::new()),
        ("GET", "/admin/api/jobs/3/log?lines=500", 200, "text/plain; charset=utf-8", "line 1\nline 2".into()),
    ]);
    let (c, _) = Client::login(&base, "k").unwrap();
    let list: UserList = c.get("/admin/api/users?q=ana&limit=50").unwrap();
    assert_eq!(list.total, 1);
    let patched: UserEnvelope = c.patch("/admin/api/users/7", &json!({"role":"moderator","daily_limit":null})).unwrap();
    assert_eq!(patched.user.role, "moderator");
    let deleted: Value = c.delete("/admin/api/users/7").unwrap();
    assert_eq!(deleted["deleted"], true);
    let _: UserEnvelope = c.post_empty("/admin/api/users/7/reactivate").unwrap();
    // An empty 2xx body parses as JSON null.
    let empty: Value = c.post_empty("/admin/api/updates/check").unwrap();
    assert!(empty.is_null());
    assert_eq!(c.get_text("/admin/api/jobs/3/log?lines=500").unwrap(), "line 1\nline 2");
    let missing: ApiResult<Value> = c.get("/admin/api/nope");
    assert!(missing.unwrap_err().is_not_found());

    let log = log.lock().unwrap();
    let patch = log.iter().find(|r| r.method == "PATCH").unwrap();
    assert_eq!(serde_json::from_str::<Value>(&patch.body).unwrap(), json!({"role":"moderator","daily_limit":null}));
    assert!(log.iter().skip(2).all(|r| r.auth.as_deref() == Some("Bearer ocs_t")));
}
