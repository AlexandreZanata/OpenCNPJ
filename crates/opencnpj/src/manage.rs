//! Admin API for accounts: users, moderation, plans, usage, the search log, the audit log,
//! settings and the response cache. Every route sits behind `gate::staff`, which checks the
//! permission returned by `permission_for`. See `docs/ACCOUNTS.md`.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path as UrlPath, Query as UrlQuery, State};
use axum::http::{Method, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::{self, Accounts, Actor, Perm, Plan, Principal, Role, Status, UserFilter};
use crate::api::{ApiError, ApiResult, Shared};
use crate::gate::Caller;
use crate::mail;

/// Permission needed by an admin route (`None`: any staff account).
pub fn permission_for(method: &Method, route: &str) -> Option<Perm> {
    let read = method == Method::GET;
    let r = route.strip_prefix("/admin/api").unwrap_or(route);
    Some(match r {
        "/whoami" | "/roles" => return None,
        "/users" | "/users/{id}" | "/plans" if read => Perm::UsersRead,
        "/users" => Perm::UsersManage,
        "/users/{id}" if method == Method::DELETE => Perm::UsersManage,
        // PATCH: notes need users.moderate, anything else users.manage (checked in the handler).
        "/users/{id}" => Perm::UsersModerate,
        "/users/{id}/suspend"
        | "/users/{id}/ban"
        | "/users/{id}/reactivate"
        | "/users/{id}/rotate-key"
        | "/users/{id}/revoke-sessions"
        | "/users/{id}/reset-usage" => Perm::UsersModerate,
        "/plans" | "/plans/{id}" => Perm::PlansManage,
        "/usage" | "/searches" => Perm::UsageRead,
        "/audit" => Perm::AuditRead,
        "/settings" if read => Perm::SystemRead,
        "/settings" => Perm::SettingsManage,
        "/metrics" | "/overview" | "/snapshots" | "/index" | "/reports" | "/reports/{id}" | "/jobs" | "/jobs/{id}/log" | "/explain"
        | "/updates" | "/response-cache" | "/cache"
            if read =>
        {
            Perm::SystemRead
        }
        "/snapshots/activate" | "/reload" | "/cache" | "/jobs" | "/jobs/{id}" | "/selftest" | "/updates/check" | "/response-cache" => {
            Perm::SystemOperate
        }
        // Prometheus text on the admin listener.
        "/metrics" => Perm::SystemRead,
        _ => Perm::SettingsManage,
    })
}

struct Ctx {
    acc: Arc<Accounts>,
    actor: Actor,
    me: Arc<Principal>,
}

fn ctx(st: &Shared, caller: Option<Extension<Caller>>) -> Result<Ctx, ApiError> {
    let acc = st
        .gate
        .accounts
        .clone()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "accounts_disabled", "this server runs without accounts (--auth off)"))?;
    let Extension(c) = caller.ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "unauthenticated", "no caller"))?;
    Ok(Ctx { acc, actor: c.actor(), me: c.principal })
}

async fn run<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Result<T, ApiError> {
    match tokio::task::spawn_blocking(f).await {
        Ok(r) => r.map_err(ApiError::from),
        Err(e) => Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", e.to_string())),
    }
}

fn body(b: &Bytes) -> Result<Value, ApiError> {
    if b.is_empty() {
        return Ok(json!({}));
    }
    let v: Value = serde_json::from_slice(b).map_err(|e| ApiError::bad(format!("invalid JSON body: {e}")))?;
    if !v.is_object() {
        return Err(ApiError::bad("expected a JSON object"));
    }
    Ok(v)
}

fn ok(v: Value) -> ApiResult {
    Ok(Json(v).into_response())
}

// ----- identity ------------------------------------------------------------------------

async fn whoami(State(st): State<Shared>, caller: Option<Extension<Caller>>) -> ApiResult {
    let c = ctx(&st, caller)?;
    let id = c.me.id;
    let acc = c.acc.clone();
    let user = run(move || acc.user(id)).await?;
    ok(json!({ "user": user, "permissions": c.me.role.permissions().iter().map(|p| p.name()).collect::<Vec<_>>() }))
}

async fn roles() -> Json<Value> {
    Json(Value::Array(
        Role::ALL
            .iter()
            .map(|r| json!({ "role": r.name(), "description": r.description(), "permissions": r.permissions().iter().map(|p| p.name()).collect::<Vec<_>>() }))
            .collect(),
    ))
}

// ----- users -----------------------------------------------------------------------------

async fn users_list(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlQuery(f): UrlQuery<UserFilter>) -> ApiResult {
    let c = ctx(&st, caller)?;
    ok(run(move || c.acc.list_users(&f)).await?)
}

async fn users_create(State(st): State<Shared>, caller: Option<Extension<Caller>>, b: Bytes) -> ApiResult {
    let c = ctx(&st, caller)?;
    let v = body(&b)?;
    let username = v["username"].as_str().unwrap_or_default().to_string();
    let email = v["email"].as_str().unwrap_or_default().to_string();
    let role = match v["role"].as_str() {
        Some(r) => Role::parse(r).ok_or_else(|| ApiError::bad("role must be admin, moderator, analyst or user"))?,
        None => Role::User,
    };
    let plan = v["plan"].as_str().map(str::to_string);
    let send = v["send_email"].as_bool().unwrap_or(false);
    let (user, key) = run(move || c.acc.create_user(&username, &email, role, plan.as_deref(), Status::Active, &c.actor)).await?;
    if send && let Some(m) = &st.mailer {
        m.spawn(user.email.clone(), "Your OpenCNPJ login key", mail::welcome(&user.username, &key, &m.public_url));
    }
    Ok((StatusCode::CREATED, Json(json!({ "user": user, "key": key }))).into_response())
}

async fn users_get(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlPath(id): UrlPath<i64>) -> ApiResult {
    let c = ctx(&st, caller)?;
    let mut v = run(move || c.acc.user_detail(id)).await?;
    if let Some((today, month)) = st.gate.usage_now(id) {
        v["usage_live"] = json!({ "today": today, "month": month });
    }
    ok(v)
}

/// Loads the target and applies the moderation guards.
async fn target(c: &Ctx, id: i64, removes_admin: bool) -> Result<accounts::User, ApiError> {
    let (acc, me) = (c.acc.clone(), c.me.clone());
    run(move || {
        let t = acc.user(id)?.ok_or_else(|| accounts::denied(404, "not_found", "no such user"))?;
        acc.check_target(&me, &t, removes_admin)?;
        Ok(t)
    })
    .await
}

async fn users_patch(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlPath(id): UrlPath<i64>, b: Bytes) -> ApiResult {
    let c = ctx(&st, caller)?;
    let patch = body(&b)?;
    let only_notes = patch.as_object().is_some_and(|o| o.keys().all(|k| k == "notes"));
    if !only_notes && !c.me.role.can(Perm::UsersManage) {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "forbidden", "requires the users.manage permission"));
    }
    let demotes = patch.get("role").is_some_and(|r| r.as_str() != Some("admin"));
    target(&c, id, demotes).await?;
    let user = run(move || c.acc.update_user(id, &patch, &c.actor)).await?;
    st.gate.forget(id);
    ok(json!({ "user": user }))
}

async fn users_delete(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlPath(id): UrlPath<i64>) -> ApiResult {
    let c = ctx(&st, caller)?;
    target(&c, id, true).await?;
    run(move || c.acc.delete_user(id, &c.actor)).await?;
    st.gate.forget(id);
    ok(json!({ "deleted": true }))
}

#[derive(Deserialize)]
struct Moderation {
    reason: Option<String>,
    until: Option<i64>,
}

async fn suspend(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlPath(id): UrlPath<i64>, b: Bytes) -> ApiResult {
    moderate(st, caller, id, b, Status::Suspended).await
}

async fn ban(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlPath(id): UrlPath<i64>, b: Bytes) -> ApiResult {
    moderate(st, caller, id, b, Status::Banned).await
}

async fn reactivate(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlPath(id): UrlPath<i64>, b: Bytes) -> ApiResult {
    moderate(st, caller, id, b, Status::Active).await
}

async fn moderate(st: Shared, caller: Option<Extension<Caller>>, id: i64, b: Bytes, to: Status) -> ApiResult {
    let c = ctx(&st, caller)?;
    let m: Moderation = serde_json::from_value(body(&b)?).map_err(|e| ApiError::bad(e.to_string()))?;
    let reason = m.reason.map(|r| r.trim().chars().take(500).collect::<String>()).filter(|r| !r.is_empty());
    if to != Status::Active && reason.is_none() {
        return Err(ApiError::bad("a reason is required"));
    }
    if m.until.is_some_and(|t| t <= accounts::now()) {
        return Err(ApiError::bad("until must be in the future"));
    }
    let t = target(&c, id, to != Status::Active).await?;
    if to == Status::Active && t.status == "banned" && c.me.role != Role::Admin {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "forbidden", "only admins can lift a ban"));
    }
    let until = if to == Status::Suspended { m.until } else { None };
    let user = run(move || c.acc.set_status(id, to, reason.as_deref(), until, &c.actor)).await?;
    ok(json!({ "user": user }))
}

async fn rotate_key(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlPath(id): UrlPath<i64>, b: Bytes) -> ApiResult {
    let c = ctx(&st, caller)?;
    let send = body(&b)?["send_email"].as_bool().unwrap_or(false);
    let t = target(&c, id, false).await?;
    let key = run(move || c.acc.rotate_key(id, &c.actor)).await?;
    if send && let Some(m) = &st.mailer {
        m.spawn(t.email.clone(), "Your new OpenCNPJ login key", mail::new_key(&t.username, &key, "replaced by an administrator"));
    }
    let prefix = accounts::split_key(&key).map(|x| x.0.to_string()).unwrap_or_default();
    ok(json!({ "key": key, "key_prefix": prefix }))
}

async fn revoke_sessions(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlPath(id): UrlPath<i64>) -> ApiResult {
    let c = ctx(&st, caller)?;
    target(&c, id, false).await?;
    let n = run(move || c.acc.revoke_sessions(id, &c.actor)).await?;
    ok(json!({ "revoked": n }))
}

async fn reset_usage(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlPath(id): UrlPath<i64>, b: Bytes) -> ApiResult {
    let c = ctx(&st, caller)?;
    let month = match body(&b)?["scope"].as_str().unwrap_or("day") {
        "day" => false,
        "month" => true,
        _ => return Err(ApiError::bad("scope must be day or month")),
    };
    target(&c, id, false).await?;
    // Write pending increments first so the reset removes them too.
    let g = st.clone();
    run(move || {
        g.gate.flush();
        Ok(())
    })
    .await?;
    let acc = c.acc.clone();
    let user = run(move || {
        acc.reset_usage(id, month, &c.actor)?;
        acc.user(id)
    })
    .await?;
    st.gate.forget(id);
    ok(json!({ "user": user }))
}

// ----- plans -----------------------------------------------------------------------------

async fn plans_list(State(st): State<Shared>, caller: Option<Extension<Caller>>) -> ApiResult {
    let c = ctx(&st, caller)?;
    ok(run(move || c.acc.plans_json()).await?)
}

async fn plans_create(State(st): State<Shared>, caller: Option<Extension<Caller>>, b: Bytes) -> ApiResult {
    let c = ctx(&st, caller)?;
    let p: Plan = serde_json::from_value(body(&b)?).map_err(|e| ApiError::bad(format!("invalid plan: {e}")))?;
    let plan = run(move || c.acc.create_plan(&p, &c.actor)).await?;
    Ok((StatusCode::CREATED, Json(json!(*plan))).into_response())
}

async fn plans_patch(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlPath(id): UrlPath<i64>, b: Bytes) -> ApiResult {
    let c = ctx(&st, caller)?;
    let patch = body(&b)?;
    let plan = run(move || c.acc.update_plan(id, &patch, &c.actor)).await?;
    ok(json!(*plan))
}

async fn plans_delete(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlPath(id): UrlPath<i64>) -> ApiResult {
    let c = ctx(&st, caller)?;
    run(move || c.acc.delete_plan(id, &c.actor)).await?;
    ok(json!({ "deleted": true }))
}

// ----- reports ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Days {
    days: Option<i64>,
}

async fn usage(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlQuery(d): UrlQuery<Days>) -> ApiResult {
    let c = ctx(&st, caller)?;
    let g = st.clone();
    let mut v = run(move || {
        g.gate.flush();
        c.acc.usage_report(d.days.unwrap_or(30))
    })
    .await?;
    v["gate"] = st.gate.stats();
    ok(v)
}

#[derive(Deserialize)]
struct LogQuery {
    user_id: Option<i64>,
    actor_id: Option<i64>,
    target_id: Option<i64>,
    limit: Option<i64>,
    before: Option<i64>,
}

async fn searches(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlQuery(q): UrlQuery<LogQuery>) -> ApiResult {
    let c = ctx(&st, caller)?;
    ok(run(move || c.acc.searches(q.user_id, q.limit.unwrap_or(100), q.before)).await?)
}

async fn audit(State(st): State<Shared>, caller: Option<Extension<Caller>>, UrlQuery(q): UrlQuery<LogQuery>) -> ApiResult {
    let c = ctx(&st, caller)?;
    ok(run(move || c.acc.audit_list(q.actor_id, q.target_id, q.limit.unwrap_or(100), q.before)).await?)
}

// ----- settings and caches ---------------------------------------------------------------

async fn settings_get(State(st): State<Shared>, caller: Option<Extension<Caller>>) -> ApiResult {
    let c = ctx(&st, caller)?;
    let mut v = json!(c.acc.settings());
    v["auth"] = json!(st.gate.mode.name());
    v["mail"] = json!(st.mailer.as_ref().map(|m| m.describe()));
    ok(v)
}

async fn settings_patch(State(st): State<Shared>, caller: Option<Extension<Caller>>, b: Bytes) -> ApiResult {
    let c = ctx(&st, caller)?;
    let patch = body(&b)?;
    let s = run(move || c.acc.patch_settings(&patch, &c.actor)).await?;
    st.wake_updater();
    ok(json!(s))
}

async fn response_cache_get(State(st): State<Shared>) -> ApiResult {
    ok(st.gate.cache.as_ref().map_or(json!({ "enabled": false }), |c| {
        let mut v = c.stats();
        v["enabled"] = json!(true);
        v
    }))
}

async fn response_cache_clear(State(st): State<Shared>, caller: Option<Extension<Caller>>) -> ApiResult {
    if let Some(c) = &st.gate.cache {
        c.clear();
    }
    if let (Some(acc), Some(Extension(c))) = (&st.gate.accounts, caller) {
        let (acc, actor) = (acc.clone(), c.actor());
        run(move || {
            acc.audit(&actor, "response_cache.clear", None, json!({}));
            Ok(())
        })
        .await?;
    }
    response_cache_get(State(st)).await
}

pub fn router() -> Router<Shared> {
    Router::new()
        .route("/admin/api/whoami", get(whoami))
        .route("/admin/api/roles", get(roles))
        .route("/admin/api/users", get(users_list).post(users_create))
        .route("/admin/api/users/{id}", get(users_get).patch(users_patch).delete(users_delete))
        .route("/admin/api/users/{id}/suspend", post(suspend))
        .route("/admin/api/users/{id}/ban", post(ban))
        .route("/admin/api/users/{id}/reactivate", post(reactivate))
        .route("/admin/api/users/{id}/rotate-key", post(rotate_key))
        .route("/admin/api/users/{id}/revoke-sessions", post(revoke_sessions))
        .route("/admin/api/users/{id}/reset-usage", post(reset_usage))
        .route("/admin/api/plans", get(plans_list).post(plans_create))
        .route("/admin/api/plans/{id}", axum::routing::patch(plans_patch).delete(plans_delete))
        .route("/admin/api/usage", get(usage))
        .route("/admin/api/searches", get(searches))
        .route("/admin/api/audit", get(audit))
        .route("/admin/api/settings", get(settings_get).patch(settings_patch))
        .route("/admin/api/response-cache", get(response_cache_get).delete(response_cache_clear))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissions_fail_closed() {
        assert_eq!(permission_for(&Method::GET, "/admin/api/whoami"), None);
        assert_eq!(permission_for(&Method::GET, "/admin/api/users"), Some(Perm::UsersRead));
        assert_eq!(permission_for(&Method::POST, "/admin/api/users"), Some(Perm::UsersManage));
        assert_eq!(permission_for(&Method::DELETE, "/admin/api/users/{id}"), Some(Perm::UsersManage));
        assert_eq!(permission_for(&Method::POST, "/admin/api/users/{id}/ban"), Some(Perm::UsersModerate));
        assert_eq!(permission_for(&Method::GET, "/admin/api/overview"), Some(Perm::SystemRead));
        assert_eq!(permission_for(&Method::POST, "/admin/api/jobs"), Some(Perm::SystemOperate));
        assert_eq!(permission_for(&Method::PATCH, "/admin/api/settings"), Some(Perm::SettingsManage));
        assert_eq!(permission_for(&Method::POST, "/admin/api/something-new"), Some(Perm::SettingsManage));
        assert!(!Role::Moderator.can(Perm::UsersManage) && Role::Moderator.can(Perm::UsersModerate));
        assert!(!Role::Analyst.can(Perm::SystemOperate) && Role::Analyst.can(Perm::SystemRead));
    }
}
