//! JSON models of the OpenCNPJ admin API (`docs/ACCOUNTS.md`, `crates/opencnpj/src/admin.rs`).
//!
//! Every struct uses `#[serde(default)]` and tolerant field deserializers, so a missing field,
//! a `null` or a slightly different scalar type never breaks parsing of the whole response.
//! Fields whose shape the documentation leaves open are kept as `serde_json::Value`.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// `null` (or a missing field) becomes `T::default()`.
fn nd<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// Any scalar as text: strings as-is, numbers and booleans formatted, `null` empty.
fn text<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Null => String::new(),
        Value::String(s) => s,
        other => other.to_string(),
    })
}

/// Optional text: `null` and empty strings become `None`.
fn opt_text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let s = text(d)?;
    Ok(if s.is_empty() { None } else { Some(s) })
}

/// Optional integer that also accepts floats and numeric strings; anything else is `None`.
fn opt_i64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    Ok(value_i64(&Value::deserialize(d)?))
}

/// Integer, `0` when missing or not numeric.
fn int<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(opt_i64(d)?.unwrap_or(0))
}

/// Unsigned integer, `0` when missing, negative or not numeric.
fn uint<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    Ok(opt_i64(d)?.and_then(|v| u64::try_from(v).ok()).unwrap_or(0))
}

/// Float, `0.0` when missing or not numeric.
fn float<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    })
}

/// Boolean that also accepts `0`/`1` and `"true"`/`"false"`.
fn boolean<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Bool(b) => b,
        Value::Number(n) => n.as_f64().is_some_and(|v| v != 0.0),
        Value::String(s) => matches!(s.as_str(), "true" | "1" | "yes" | "on"),
        _ => false,
    })
}

/// Integer view of a JSON value (numbers, rounded floats, numeric strings).
pub fn value_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => {
            n.as_i64().or_else(|| n.as_u64().map(|u| i64::try_from(u).unwrap_or(i64::MAX))).or_else(|| n.as_f64().map(|f| f as i64))
        }
        Value::String(s) => s.trim().parse::<i64>().ok().or_else(|| s.trim().parse::<f64>().ok().map(|f| f as i64)),
        _ => None,
    }
}

/// Error body of every failing API call: `{"error","message","code"}`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ErrorBody {
    #[serde(deserialize_with = "text")]
    pub error: String,
    #[serde(deserialize_with = "text")]
    pub message: String,
    #[serde(deserialize_with = "opt_i64")]
    pub code: Option<i64>,
}

// ---------------------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct User {
    #[serde(deserialize_with = "int")]
    pub id: i64,
    #[serde(deserialize_with = "text")]
    pub username: String,
    #[serde(deserialize_with = "text")]
    pub email: String,
    #[serde(deserialize_with = "text")]
    pub role: String,
    #[serde(deserialize_with = "text")]
    pub status: String,
    #[serde(deserialize_with = "text")]
    pub plan: String,
    #[serde(deserialize_with = "opt_i64")]
    pub created_at: Option<i64>,
    #[serde(deserialize_with = "opt_i64")]
    pub verified_at: Option<i64>,
    #[serde(deserialize_with = "opt_i64")]
    pub last_seen_at: Option<i64>,
    #[serde(deserialize_with = "opt_i64")]
    pub suspended_until: Option<i64>,
    #[serde(deserialize_with = "opt_text")]
    pub status_reason: Option<String>,
    #[serde(deserialize_with = "text")]
    pub notes: String,
    #[serde(deserialize_with = "opt_i64")]
    pub daily_limit: Option<i64>,
    #[serde(deserialize_with = "opt_i64")]
    pub monthly_limit: Option<i64>,
    #[serde(deserialize_with = "opt_i64")]
    pub per_minute: Option<i64>,
    #[serde(deserialize_with = "opt_text")]
    pub key_prefix: Option<String>,
    /// Only in `GET /admin/api/users` rows.
    #[serde(deserialize_with = "opt_i64")]
    pub usage_today: Option<i64>,
    /// Only in `GET /admin/api/users` rows.
    #[serde(deserialize_with = "opt_i64")]
    pub usage_month: Option<i64>,
}

/// `POST /api/v1/auth/login`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LoginResponse {
    #[serde(deserialize_with = "text")]
    pub token: String,
    #[serde(deserialize_with = "text")]
    pub token_type: String,
    pub expires_at: Value,
    pub user: User,
}

/// `GET /admin/api/whoami`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Whoami {
    pub user: User,
    #[serde(deserialize_with = "nd")]
    pub permissions: Vec<String>,
}

/// `GET /admin/api/roles` item.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RoleInfo {
    #[serde(deserialize_with = "text")]
    pub role: String,
    #[serde(deserialize_with = "text")]
    pub description: String,
    #[serde(deserialize_with = "nd")]
    pub permissions: Vec<String>,
}

/// `GET /admin/api/users`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UserList {
    #[serde(deserialize_with = "uint")]
    pub total: u64,
    #[serde(deserialize_with = "nd")]
    pub users: Vec<User>,
}

/// `POST /admin/api/users`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CreatedUser {
    pub user: User,
    #[serde(deserialize_with = "text")]
    pub key: String,
}

/// `{"user"}` returned by PATCH, suspend, ban, reactivate and reset-usage.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UserEnvelope {
    pub user: User,
}

/// `POST /admin/api/users/{id}/rotate-key`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RotatedKey {
    #[serde(deserialize_with = "text")]
    pub key: String,
    #[serde(deserialize_with = "text")]
    pub key_prefix: String,
}

/// `POST /admin/api/users/{id}/revoke-sessions`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Revoked {
    #[serde(deserialize_with = "uint")]
    pub revoked: u64,
}

/// One row of `usage_daily` in the user detail.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UserUsageDay {
    #[serde(deserialize_with = "text")]
    pub day: String,
    #[serde(deserialize_with = "text")]
    pub kind: String,
    #[serde(deserialize_with = "uint")]
    pub count: u64,
    #[serde(deserialize_with = "uint")]
    pub cache_hits: u64,
}

/// `GET /admin/api/users/{id}`. Keys and sessions are kept generic: the documentation does
/// not fix their fields, and the UI renders whatever columns the server sends.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UserDetail {
    pub user: User,
    #[serde(deserialize_with = "nd")]
    pub keys: Vec<Value>,
    #[serde(deserialize_with = "nd")]
    pub sessions: Vec<Value>,
    #[serde(deserialize_with = "nd")]
    pub usage_daily: Vec<UserUsageDay>,
    #[serde(deserialize_with = "nd")]
    pub recent_searches: Vec<SearchEntry>,
    /// Optional live counters (`{"today","month"}`), shown when present.
    pub usage_live: Value,
}

// ---------------------------------------------------------------------------------------
// Plans, usage, searches, audit, settings, updates
// ---------------------------------------------------------------------------------------

/// Plan (`null` limits mean unlimited). `users` is only present in `GET /admin/api/plans`.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Plan {
    #[serde(deserialize_with = "int")]
    pub id: i64,
    #[serde(deserialize_with = "text")]
    pub name: String,
    #[serde(deserialize_with = "opt_i64")]
    pub daily_limit: Option<i64>,
    #[serde(deserialize_with = "opt_i64")]
    pub monthly_limit: Option<i64>,
    #[serde(deserialize_with = "opt_i64")]
    pub per_minute: Option<i64>,
    #[serde(deserialize_with = "opt_i64")]
    pub burst: Option<i64>,
    #[serde(deserialize_with = "opt_i64")]
    pub max_page_size: Option<i64>,
    #[serde(deserialize_with = "boolean")]
    pub export: bool,
    #[serde(deserialize_with = "opt_i64")]
    pub max_export_rows: Option<i64>,
    #[serde(deserialize_with = "boolean")]
    pub downloads: bool,
    #[serde(deserialize_with = "opt_i64")]
    pub users: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UsageDay {
    #[serde(deserialize_with = "text")]
    pub day: String,
    #[serde(deserialize_with = "uint")]
    pub requests: u64,
    #[serde(deserialize_with = "uint")]
    pub users: u64,
    #[serde(deserialize_with = "uint")]
    pub cache_hits: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TopUser {
    #[serde(deserialize_with = "int")]
    pub id: i64,
    #[serde(deserialize_with = "text")]
    pub username: String,
    #[serde(deserialize_with = "uint")]
    pub count: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UsageTotals {
    #[serde(deserialize_with = "uint")]
    pub users: u64,
    #[serde(deserialize_with = "uint")]
    pub active_users_today: u64,
    #[serde(deserialize_with = "uint")]
    pub requests_today: u64,
    #[serde(deserialize_with = "uint")]
    pub requests_month: u64,
    /// Not in the documented shape; shown when the server sends it.
    #[serde(deserialize_with = "nd")]
    pub users_by_status: BTreeMap<String, u64>,
}

/// `GET /admin/api/usage`. `by_kind` values are numbers or objects (left open by the docs).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UsageReport {
    #[serde(deserialize_with = "nd")]
    pub daily: Vec<UsageDay>,
    #[serde(deserialize_with = "nd")]
    pub by_kind: BTreeMap<String, Value>,
    #[serde(deserialize_with = "nd")]
    pub top_users_today: Vec<TopUser>,
    pub totals: UsageTotals,
}

/// One search log row (`GET /admin/api/searches`, `recent_searches`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SearchEntry {
    #[serde(deserialize_with = "int")]
    pub id: i64,
    #[serde(deserialize_with = "opt_i64")]
    pub ts: Option<i64>,
    #[serde(deserialize_with = "opt_i64")]
    pub user_id: Option<i64>,
    #[serde(deserialize_with = "text")]
    pub username: String,
    #[serde(deserialize_with = "text")]
    pub route: String,
    /// Query string or an object of parameters.
    pub query: Value,
    #[serde(deserialize_with = "text")]
    pub status: String,
    #[serde(deserialize_with = "float")]
    pub ms: f64,
    /// `"HIT"`/`"MISS"`, a boolean, or absent.
    pub cache: Value,
    #[serde(deserialize_with = "text")]
    pub ip: String,
}

impl SearchEntry {
    /// `"hit"`, `"miss"` or empty, whatever the server's representation.
    pub fn cache_text(&self) -> &'static str {
        match &self.cache {
            Value::Bool(true) => "hit",
            Value::Bool(false) => "miss",
            Value::String(s) if s.eq_ignore_ascii_case("hit") => "hit",
            Value::String(s) if s.eq_ignore_ascii_case("miss") => "miss",
            _ => "",
        }
    }
}

/// One audit log row.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AuditEntry {
    #[serde(deserialize_with = "int")]
    pub id: i64,
    #[serde(deserialize_with = "opt_i64")]
    pub ts: Option<i64>,
    #[serde(deserialize_with = "opt_i64")]
    pub actor_id: Option<i64>,
    #[serde(deserialize_with = "text")]
    pub actor: String,
    #[serde(deserialize_with = "text")]
    pub action: String,
    #[serde(deserialize_with = "opt_i64")]
    pub target_id: Option<i64>,
    #[serde(deserialize_with = "text")]
    pub target: String,
    /// Free-form object or string.
    pub details: Value,
    #[serde(deserialize_with = "text")]
    pub ip: String,
}

/// `GET/PATCH /admin/api/settings`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    #[serde(deserialize_with = "text")]
    pub registration: String,
    #[serde(deserialize_with = "text")]
    pub default_plan: String,
    #[serde(deserialize_with = "boolean")]
    pub search_log: bool,
    #[serde(deserialize_with = "int")]
    pub search_log_days: i64,
    #[serde(deserialize_with = "boolean")]
    pub auto_update: bool,
    #[serde(deserialize_with = "int")]
    pub update_check_hours: i64,
}

/// `GET /admin/api/updates`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UpdateStatus {
    #[serde(deserialize_with = "boolean")]
    pub enabled: bool,
    #[serde(deserialize_with = "opt_i64")]
    pub interval_hours: Option<i64>,
    #[serde(deserialize_with = "opt_i64")]
    pub last_check: Option<i64>,
    pub last_result: Value,
    pub latest_remote: Value,
    #[serde(deserialize_with = "text")]
    pub current_competence: String,
    #[serde(deserialize_with = "opt_i64")]
    pub next_check: Option<i64>,
    /// Job object (or id) of the running update, `null` when idle.
    pub job: Value,
    #[serde(deserialize_with = "boolean")]
    pub checking: bool,
}

/// `GET /admin/api/response-cache`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ResponseCacheStats {
    #[serde(deserialize_with = "uint")]
    pub entries: u64,
    #[serde(deserialize_with = "uint")]
    pub bytes: u64,
    #[serde(deserialize_with = "uint")]
    pub budget: u64,
    #[serde(deserialize_with = "uint")]
    pub hits: u64,
    #[serde(deserialize_with = "uint")]
    pub misses: u64,
    #[serde(deserialize_with = "uint")]
    pub coalesced: u64,
    /// `false` when the server runs without a response cache; absent on servers that always have one.
    pub enabled: Option<bool>,
}

// ---------------------------------------------------------------------------------------
// Operations (admin.rs / metrics.rs)
// ---------------------------------------------------------------------------------------

/// Postings cache (`/admin/api/cache`, `overview.cache`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CacheStats {
    #[serde(deserialize_with = "uint")]
    pub entries: u64,
    #[serde(deserialize_with = "uint")]
    pub bytes: u64,
    #[serde(deserialize_with = "uint")]
    pub budget: u64,
    #[serde(deserialize_with = "uint")]
    pub hits: u64,
    #[serde(deserialize_with = "uint")]
    pub misses: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> Option<f64> {
        let total = self.hits + self.misses;
        (total > 0).then(|| self.hits as f64 / total as f64)
    }
}

impl ResponseCacheStats {
    pub fn hit_rate(&self) -> Option<f64> {
        let total = self.hits + self.misses;
        (total > 0).then(|| self.hits as f64 / total as f64)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProcessStats {
    #[serde(deserialize_with = "uint")]
    pub rss_anon_bytes: u64,
    #[serde(deserialize_with = "uint")]
    pub rss_file_bytes: u64,
    #[serde(deserialize_with = "float")]
    pub cpu_seconds: f64,
    #[serde(deserialize_with = "uint")]
    pub threads: u64,
    #[serde(deserialize_with = "uint")]
    pub peak_rss_bytes: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SnapshotCounts {
    #[serde(deserialize_with = "uint")]
    pub documents: u64,
    #[serde(deserialize_with = "uint")]
    pub companies: u64,
    #[serde(deserialize_with = "uint")]
    pub rejected_lines: u64,
    #[serde(deserialize_with = "nd")]
    pub rows: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SnapshotInfo {
    #[serde(deserialize_with = "text")]
    pub path: String,
    #[serde(deserialize_with = "uint")]
    pub file_bytes: u64,
    #[serde(deserialize_with = "nd")]
    pub format_version: Vec<u32>,
    #[serde(deserialize_with = "text")]
    pub competence: String,
    #[serde(deserialize_with = "text")]
    pub profile: String,
    pub counts: SnapshotCounts,
    #[serde(deserialize_with = "uint")]
    pub terms: u64,
    #[serde(deserialize_with = "uint")]
    pub blocks: u64,
}

/// `GET /admin/api/overview`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Overview {
    #[serde(deserialize_with = "text")]
    pub version: String,
    #[serde(deserialize_with = "float")]
    pub uptime_s: f64,
    #[serde(deserialize_with = "uint")]
    pub requests_total: u64,
    #[serde(deserialize_with = "uint")]
    pub timeouts_total: u64,
    #[serde(deserialize_with = "uint")]
    pub overload_total: u64,
    #[serde(deserialize_with = "uint")]
    pub reloads_total: u64,
    pub process: ProcessStats,
    pub cache: CacheStats,
    pub snapshot: SnapshotInfo,
    #[serde(deserialize_with = "text")]
    pub sha256: String,
    #[serde(deserialize_with = "nd")]
    pub numeric_columns: Vec<String>,
    pub config: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RouteStats {
    #[serde(deserialize_with = "text")]
    pub route: String,
    #[serde(deserialize_with = "uint")]
    pub count: u64,
    #[serde(deserialize_with = "float")]
    pub mean_ms: f64,
    #[serde(deserialize_with = "float")]
    pub p50_ms: f64,
    #[serde(deserialize_with = "float")]
    pub p90_ms: f64,
    #[serde(deserialize_with = "float")]
    pub p95_ms: f64,
    #[serde(deserialize_with = "float")]
    pub p99_ms: f64,
    #[serde(deserialize_with = "float")]
    pub max_ms: f64,
    #[serde(deserialize_with = "nd")]
    pub status: BTreeMap<String, u64>,
}

impl RouteStats {
    pub fn class(&self, class: &str) -> u64 {
        self.status.get(class).copied().unwrap_or(0)
    }
}

/// One second of the series: `[unix_second, requests, errors, mean_ms]`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SeriesPoint {
    pub second: i64,
    pub requests: f64,
    pub errors: f64,
    pub mean_ms: f64,
}

impl<'de> Deserialize<'de> for SeriesPoint {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Vec::<Value>::deserialize(d)?;
        let f = |i: usize| v.get(i).and_then(Value::as_f64).unwrap_or(0.0);
        Ok(Self { second: v.first().and_then(value_i64).unwrap_or(0), requests: f(1), errors: f(2), mean_ms: f(3) })
    }
}

/// `GET /admin/api/metrics?seconds=N`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MetricsResponse {
    #[serde(deserialize_with = "nd")]
    pub routes: Vec<RouteStats>,
    #[serde(deserialize_with = "nd")]
    pub series: Vec<SeriesPoint>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SnapshotFile {
    #[serde(deserialize_with = "text")]
    pub name: String,
    #[serde(deserialize_with = "uint")]
    pub bytes: u64,
    #[serde(deserialize_with = "opt_text")]
    pub sha256: Option<String>,
    #[serde(deserialize_with = "opt_i64")]
    pub modified: Option<i64>,
    #[serde(deserialize_with = "boolean")]
    pub active: bool,
}

/// `GET /admin/api/snapshots`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SnapshotList {
    #[serde(deserialize_with = "text")]
    pub dir: String,
    #[serde(deserialize_with = "nd")]
    pub files: Vec<SnapshotFile>,
}

/// `GET /admin/api/jobs` item.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Job {
    #[serde(deserialize_with = "int")]
    pub id: i64,
    #[serde(deserialize_with = "text")]
    pub kind: String,
    #[serde(deserialize_with = "nd")]
    pub args: Vec<String>,
    #[serde(deserialize_with = "text")]
    pub status: String,
    #[serde(deserialize_with = "opt_i64")]
    pub exit_code: Option<i64>,
    #[serde(deserialize_with = "opt_i64")]
    pub started: Option<i64>,
    #[serde(deserialize_with = "opt_i64")]
    pub finished: Option<i64>,
    #[serde(deserialize_with = "float")]
    pub elapsed_s: f64,
    #[serde(deserialize_with = "opt_text")]
    pub log: Option<String>,
}

impl Job {
    pub fn is_running(&self) -> bool {
        matches!(self.status.as_str(), "running" | "killing")
    }
}

/// `POST /admin/api/jobs` response.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct JobStarted {
    #[serde(deserialize_with = "int")]
    pub id: i64,
}

/// `POST /admin/api/jobs` body (see `JobRequest` in admin.rs).
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct JobRequest {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub month: Option<String>,
    pub sources: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queries: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse<T: for<'de> Deserialize<'de>>(s: &str) -> T {
        serde_json::from_str(s).unwrap_or_else(|e| panic!("{e}: {s}"))
    }

    const USER_DOC: &str = r#"{ "id": 7, "username": "ana", "email": "ana@example.com", "role": "user", "status": "active",
  "plan": "free", "created_at": 1793750000, "verified_at": 1793750100, "last_seen_at": 1793760000,
  "suspended_until": null, "status_reason": null, "notes": "",
  "daily_limit": null, "monthly_limit": null, "per_minute": null, "key_prefix": "Ab3dE5gH9k" }"#;

    #[test]
    fn user_object_from_docs() {
        let u: User = parse(USER_DOC);
        assert_eq!(u.id, 7);
        assert_eq!(u.username, "ana");
        assert_eq!(u.role, "user");
        assert_eq!(u.status, "active");
        assert_eq!(u.plan, "free");
        assert_eq!(u.created_at, Some(1_793_750_000));
        assert_eq!(u.verified_at, Some(1_793_750_100));
        assert_eq!(u.suspended_until, None);
        assert_eq!(u.status_reason, None);
        assert_eq!(u.daily_limit, None);
        assert_eq!(u.key_prefix.as_deref(), Some("Ab3dE5gH9k"));
        assert_eq!(u.usage_today, None);
    }

    #[test]
    fn null_and_missing_fields_do_not_break_parsing() {
        let u: User = parse(r#"{"id": 3, "username": null, "notes": null, "status_reason": "spam", "per_minute": 120.0}"#);
        assert_eq!(u.id, 3);
        assert_eq!(u.username, "");
        assert_eq!(u.notes, "");
        assert_eq!(u.status_reason.as_deref(), Some("spam"));
        assert_eq!(u.per_minute, Some(120));
        let empty: UserDetail = parse("{}");
        assert!(empty.keys.is_empty() && empty.recent_searches.is_empty());
        let nulls: UserDetail = parse(r#"{"user": {"id": 1}, "keys": null, "sessions": null, "usage_daily": null}"#);
        assert_eq!(nulls.user.id, 1);
    }

    #[test]
    fn login_and_whoami() {
        let l: LoginResponse = parse(&format!(r#"{{"token":"ocs_abc","token_type":"Bearer","expires_at":1794354800,"user":{USER_DOC}}}"#));
        assert_eq!(l.token, "ocs_abc");
        assert_eq!(l.token_type, "Bearer");
        assert_eq!(l.user.username, "ana");
        let w: Whoami = parse(&format!(
            r#"{{"user":{USER_DOC},"permissions":["users.read","users.moderate","usage.read","audit.read","system.read"]}}"#
        ));
        assert_eq!(w.permissions.len(), 5);
        assert!(w.permissions.iter().any(|p| p == "users.moderate"));
        let roles: Vec<RoleInfo> =
            parse(r#"[{"role":"analyst","description":"Read-only operations","permissions":["users.read","usage.read"]}]"#);
        assert_eq!(roles[0].role, "analyst");
        assert_eq!(roles[0].permissions.len(), 2);
    }

    #[test]
    fn users_list_create_and_actions() {
        let mut row: Value = serde_json::from_str(USER_DOC).unwrap();
        row["usage_today"] = json!(12);
        row["usage_month"] = json!(310);
        let list: UserList = parse(&json!({ "total": 41, "users": [row] }).to_string());
        assert_eq!(list.total, 41);
        assert_eq!(list.users[0].usage_today, Some(12));
        assert_eq!(list.users[0].usage_month, Some(310));

        let created: CreatedUser = parse(&format!(r#"{{"user":{USER_DOC},"key":"ocnpj_Ab3dE5gH9k_0123456789abcdefghijABCDEFGHIJ01"}}"#));
        assert!(created.key.starts_with("ocnpj_"));
        let rotated: RotatedKey = parse(r#"{"key":"ocnpj_x_y","key_prefix":"Zz9yX8wV7u"}"#);
        assert_eq!(rotated.key_prefix, "Zz9yX8wV7u");
        let revoked: Revoked = parse(r#"{"revoked": 3}"#);
        assert_eq!(revoked.revoked, 3);
        let env: UserEnvelope = parse(&format!(r#"{{"user":{USER_DOC}}}"#));
        assert_eq!(env.user.email, "ana@example.com");
    }

    #[test]
    fn user_detail() {
        let d: UserDetail = parse(&format!(
            r#"{{"user":{USER_DOC},
                "keys":[{{"id":1,"prefix":"Ab3dE5gH9k","created_at":1793750000,"revoked_at":null}}],
                "sessions":[{{"id":9,"created_at":1793750100,"expires_at":1794354900,"ip":"10.0.0.2"}}],
                "usage_daily":[{{"day":"2026-11-03","kind":"search","count":12,"cache_hits":4}},
                               {{"day":"2026-11-03","kind":"cnpj","count":3,"cache_hits":0}}],
                "recent_searches":[{{"id":100,"ts":1793760000,"user_id":7,"username":"ana","route":"/api/v1/search","query":"q=padaria","status":200,"ms":3.2,"cache":"HIT"}}]}}"#
        ));
        assert_eq!(d.keys.len(), 1);
        assert_eq!(d.sessions[0]["ip"], "10.0.0.2");
        assert_eq!(d.usage_daily[0].count, 12);
        assert_eq!(d.usage_daily[1].kind, "cnpj");
        assert_eq!(d.recent_searches[0].status, "200");
        assert_eq!(d.recent_searches[0].cache, json!("HIT"));
    }

    #[test]
    fn plans() {
        let plans: Vec<Plan> = parse(
            r#"[{"id":1,"name":"free","daily_limit":1000,"monthly_limit":20000,"per_minute":60,"burst":30,"max_page_size":100,"export":false,"max_export_rows":0,"downloads":false,"users":12},
                {"id":4,"name":"unlimited","daily_limit":null,"monthly_limit":null,"per_minute":null,"burst":null,"max_page_size":1000,"export":true,"max_export_rows":null,"downloads":true}]"#,
        );
        assert_eq!(plans[0].daily_limit, Some(1000));
        assert_eq!(plans[0].users, Some(12));
        assert!(!plans[0].export);
        assert_eq!(plans[1].daily_limit, None);
        assert!(plans[1].downloads);
        assert_eq!(plans[1].users, None);
    }

    #[test]
    fn usage_report() {
        let u: UsageReport = parse(
            r#"{"daily":[{"day":"2026-11-03","requests":1200,"users":14,"cache_hits":300}],
                "by_kind":{"search":900,"cnpj":300},
                "top_users_today":[{"id":7,"username":"ana","count":120}],
                "totals":{"users":41,"active_users_today":9,"requests_today":1300,"requests_month":21000}}"#,
        );
        assert_eq!(u.daily[0].requests, 1200);
        assert_eq!(u.by_kind["search"], json!(900));
        assert_eq!(u.top_users_today[0].username, "ana");
        assert_eq!(u.totals.requests_month, 21000);
    }

    #[test]
    fn searches_and_audit() {
        let s: Vec<SearchEntry> = parse(
            r#"[{"id":5,"ts":1793760000,"user_id":null,"username":null,"route":"/api/v1/cnpj/{cnpj}","query":{"cnpj":"33000167000101"},"status":404,"ms":0.4,"cache":false}]"#,
        );
        assert_eq!(s[0].user_id, None);
        assert_eq!(s[0].status, "404");
        assert!(s[0].query.is_object());
        assert_eq!(s[0].cache_text(), "miss");
        let hit: SearchEntry = parse(r#"{"id":6,"cache":"HIT","ip":"10.0.0.9"}"#);
        assert_eq!(hit.cache_text(), "hit");
        assert_eq!(hit.ip, "10.0.0.9");
        let a: Vec<AuditEntry> = parse(
            r#"[{"id":2,"ts":1793760000,"actor_id":1,"actor":"admin","action":"user.suspend","target_id":7,"target":"ana","details":{"reason":"abuse","until":1793850000},"ip":"127.0.0.1"}]"#,
        );
        assert_eq!(a[0].action, "user.suspend");
        assert_eq!(a[0].target_id, Some(7));
        assert_eq!(a[0].details["reason"], "abuse");
    }

    #[test]
    fn settings_updates_and_response_cache() {
        let s: Settings = parse(
            r#"{"registration":"open","default_plan":"free","search_log":true,"search_log_days":30,"auto_update":false,"update_check_hours":24}"#,
        );
        assert_eq!(s.registration, "open");
        assert!(s.search_log);
        assert_eq!(s.update_check_hours, 24);
        let u: UpdateStatus = parse(
            r#"{"enabled":true,"interval_hours":24,"last_check":1793760000,"last_result":"up to date","latest_remote":"2026-10","current_competence":"2026-10","next_check":1793846400,"job":null}"#,
        );
        assert!(u.enabled);
        assert_eq!(u.next_check, Some(1_793_846_400));
        assert!(u.job.is_null());
        let c: ResponseCacheStats = parse(r#"{"entries":10,"bytes":2048,"budget":67108864,"hits":30,"misses":10,"coalesced":2}"#);
        assert_eq!(c.hit_rate(), Some(0.75));
        assert_eq!(c.enabled, None);
        let off: ResponseCacheStats = parse(r#"{"enabled":false}"#);
        assert_eq!(off.enabled, Some(false));
        assert_eq!(off.hit_rate(), None);
    }

    #[test]
    fn overview_from_admin_rs() {
        let o: Overview = parse(
            r#"{"version":"0.1.0","uptime_s":3600.5,"requests_total":1000,"timeouts_total":0,"overload_total":0,"reloads_total":1,
                "process":{"rss_anon_bytes":104857600,"rss_file_bytes":2147483648,"cpu_seconds":12.5,"threads":17,"peak_rss_bytes":0},
                "cache":{"entries":3,"bytes":1024,"budget":268435456,"hits":9,"misses":1},
                "snapshot":{"path":"/data/snapshots/2026-10-balanced.ocnpj","file_bytes":9000000000,"format_version":[1,2],"competence":"2026-10",
                            "source_snapshot":"2026-10","profile":"balanced","block_codec":{"zstd":3},"block_target_bytes":65536,"zstd_dictionary_bytes":0,
                            "counts":{"rows":{"empresas":60000000},"rejected_lines":0,"companies":60000000,"documents":64000000,"blocks":1},
                            "sections":{"docs":1},"blocks":100,"terms":5000},
                "sha256":"abc","numeric_columns":["capital_social"],"config":{"authentication":"none"}}"#,
        );
        assert_eq!(o.snapshot.competence, "2026-10");
        assert_eq!(o.snapshot.counts.documents, 64_000_000);
        assert_eq!(o.snapshot.format_version, vec![1, 2]);
        assert_eq!(o.process.threads, 17);
        assert_eq!(o.cache.hit_rate(), Some(0.9));
        assert_eq!(o.config["authentication"], "none");
    }

    #[test]
    fn metrics_from_metrics_rs() {
        let m: MetricsResponse = parse(
            r#"{"routes":[{"route":"/api/v1/search","count":100,"mean_ms":2.5,"p50_ms":1.1,"p90_ms":4.0,"p95_ms":5.2,"p99_ms":9.9,"max_ms":20.0,
                           "status":{"1xx":0,"2xx":98,"3xx":0,"4xx":1,"5xx":1}}],
                "series":[[1793760000,0,0,0.0],[1793760001,12,1,3.25]]}"#,
        );
        assert_eq!(m.routes[0].class("4xx"), 1);
        assert_eq!(m.routes[0].class("5xx"), 1);
        assert_eq!(m.series.len(), 2);
        assert_eq!(m.series[1], SeriesPoint { second: 1_793_760_001, requests: 12.0, errors: 1.0, mean_ms: 3.25 });
    }

    #[test]
    fn snapshots_and_jobs_from_admin_rs() {
        let s: SnapshotList = parse(
            r#"{"dir":"/data/snapshots","files":[{"name":"2026-10-balanced.ocnpj","bytes":9000,"sha256":null,"modified":1793760000,"active":true}]}"#,
        );
        assert!(s.files[0].active);
        assert_eq!(s.files[0].sha256, None);
        let j: Vec<Job> = parse(
            r#"[{"id":2,"kind":"verify","args":["verify","/x.ocnpj"],"status":"running","exit_code":null,"started":1793760000,"finished":null,"elapsed_s":12.3,"log":"1793760000-0002-verify.log"},
                {"id":1,"kind":"bench","args":[],"status":"ok","exit_code":0,"started":1793750000,"finished":1793750100,"elapsed_s":100.0,"log":null}]"#,
        );
        assert!(j[0].is_running());
        assert!(!j[1].is_running());
        assert_eq!(j[1].exit_code, Some(0));
        let started: JobStarted = parse(r#"{"id":3}"#);
        assert_eq!(started.id, 3);
        let body = JobRequest { kind: "build".into(), profile: Some("fast".into()), month: None, sources: false, queries: None };
        assert_eq!(serde_json::to_value(&body).unwrap(), json!({"kind":"build","profile":"fast","sources":false}));
    }

    #[test]
    fn error_body() {
        let e: ErrorBody = parse(r#"{"error":"forbidden","message":"missing permission users.manage","code":403}"#);
        assert_eq!(e.error, "forbidden");
        assert_eq!(e.code, Some(403));
    }
}
