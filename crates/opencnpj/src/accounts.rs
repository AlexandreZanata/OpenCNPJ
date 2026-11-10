//! Accounts: users, single login keys (Argon2id), sessions, recovery tokens, plans, roles,
//! settings, the audit log, the search log and usage counters, in one embedded SQLite file
//! (WAL). The hot path never touches SQLite: verified credentials and principals are cached
//! in memory and invalidated on every change. See `docs/ACCOUNTS.md`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::{Algorithm, Argon2, Params, Version};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------------------
// Time helpers (UTC days since 1970-01-01).
// ---------------------------------------------------------------------------------------

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

pub fn day_of(ts: i64) -> i64 {
    ts.div_euclid(86_400)
}

/// `(year, month, day)` of a day number (Howard Hinnant's civil-from-days).
pub fn civil(day: i64) -> (i64, u32, u32) {
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Day number of a civil date.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// First day of the month containing `day`, and the first day of the next month.
pub fn month_bounds(day: i64) -> (i64, i64) {
    let (y, m, _) = civil(day);
    let next = if m == 12 { days_from_civil(y + 1, 1, 1) } else { days_from_civil(y, m + 1, 1) };
    (days_from_civil(y, m, 1), next)
}

pub fn day_string(day: i64) -> String {
    let (y, m, d) = civil(day);
    format!("{y:04}-{m:02}-{d:02}")
}

// ---------------------------------------------------------------------------------------
// Roles, permissions, statuses.
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Perm {
    UsersRead,
    UsersModerate,
    UsersManage,
    PlansManage,
    UsageRead,
    AuditRead,
    SystemRead,
    SystemOperate,
    SettingsManage,
}

impl Perm {
    pub const ALL: [Perm; 9] = [
        Perm::UsersRead,
        Perm::UsersModerate,
        Perm::UsersManage,
        Perm::PlansManage,
        Perm::UsageRead,
        Perm::AuditRead,
        Perm::SystemRead,
        Perm::SystemOperate,
        Perm::SettingsManage,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Perm::UsersRead => "users.read",
            Perm::UsersModerate => "users.moderate",
            Perm::UsersManage => "users.manage",
            Perm::PlansManage => "plans.manage",
            Perm::UsageRead => "usage.read",
            Perm::AuditRead => "audit.read",
            Perm::SystemRead => "system.read",
            Perm::SystemOperate => "system.operate",
            Perm::SettingsManage => "settings.manage",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Admin,
    Moderator,
    Analyst,
    User,
}

impl Role {
    pub const ALL: [Role; 4] = [Role::Admin, Role::Moderator, Role::Analyst, Role::User];

    pub fn parse(s: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.name() == s)
    }

    pub fn name(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Moderator => "moderator",
            Role::Analyst => "analyst",
            Role::User => "user",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Role::Admin => "Owns the service: accounts, roles, plans, settings, snapshots and jobs.",
            Role::Moderator => {
                "Handles abuse and support: suspends, bans, reactivates and rotates keys of user accounts, resets usage, writes notes."
            }
            Role::Analyst => "Read-only operations and usage reporting.",
            Role::User => "API access under their plan.",
        }
    }

    pub fn permissions(self) -> &'static [Perm] {
        match self {
            Role::Admin => &Perm::ALL,
            Role::Moderator => &[Perm::UsersRead, Perm::UsersModerate, Perm::UsageRead, Perm::AuditRead, Perm::SystemRead],
            Role::Analyst => &[Perm::UsersRead, Perm::UsageRead, Perm::AuditRead, Perm::SystemRead],
            Role::User => &[],
        }
    }

    pub fn is_staff(self) -> bool {
        self != Role::User
    }

    pub fn can(self, p: Perm) -> bool {
        self.permissions().contains(&p)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pending,
    Active,
    Suspended,
    Banned,
}

impl Status {
    pub fn parse(s: &str) -> Option<Status> {
        [Status::Pending, Status::Active, Status::Suspended, Status::Banned].into_iter().find(|x| x.name() == s)
    }

    pub fn name(self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Active => "active",
            Status::Suspended => "suspended",
            Status::Banned => "banned",
        }
    }
}

/// A refusal with an HTTP status and a stable error code (mapped by `api::ApiError`).
#[derive(Debug)]
pub struct Denied {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

impl std::fmt::Display for Denied {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Denied {}

pub fn denied(status: u16, code: &'static str, message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(Denied { status, code, message: message.into() })
}

// ---------------------------------------------------------------------------------------
// Records.
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    pub daily_limit: Option<i64>,
    pub monthly_limit: Option<i64>,
    pub per_minute: Option<i64>,
    pub burst: Option<i64>,
    pub max_page_size: i64,
    pub export: bool,
    pub max_export_rows: Option<i64>,
    pub downloads: bool,
}

impl Plan {
    fn from_row(r: &Row) -> rusqlite::Result<Plan> {
        Ok(Plan {
            id: r.get("id")?,
            name: r.get("name")?,
            daily_limit: r.get("daily_limit")?,
            monthly_limit: r.get("monthly_limit")?,
            per_minute: r.get("per_minute")?,
            burst: r.get("burst")?,
            max_page_size: r.get("max_page_size")?,
            export: r.get("export")?,
            max_export_rows: r.get("max_export_rows")?,
            downloads: r.get("downloads")?,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub plan: String,
    pub plan_id: i64,
    pub created_at: i64,
    pub verified_at: Option<i64>,
    pub last_seen_at: Option<i64>,
    pub suspended_until: Option<i64>,
    pub status_reason: Option<String>,
    pub notes: String,
    pub daily_limit: Option<i64>,
    pub monthly_limit: Option<i64>,
    pub per_minute: Option<i64>,
    pub key_prefix: Option<String>,
}

const USER_SELECT: &str = "SELECT u.*, p.name AS plan_name,
    (SELECT prefix FROM keys k WHERE k.user_id = u.id AND k.revoked_at IS NULL ORDER BY k.id DESC LIMIT 1) AS key_prefix
    FROM users u JOIN plans p ON p.id = u.plan_id";

impl User {
    fn from_row(r: &Row) -> rusqlite::Result<User> {
        Ok(User {
            id: r.get("id")?,
            username: r.get("username")?,
            email: r.get("email")?,
            role: r.get("role")?,
            status: r.get("status")?,
            plan: r.get("plan_name")?,
            plan_id: r.get("plan_id")?,
            created_at: r.get("created_at")?,
            verified_at: r.get("verified_at")?,
            last_seen_at: r.get("last_seen_at")?,
            suspended_until: r.get("suspended_until")?,
            status_reason: r.get("status_reason")?,
            notes: r.get("notes")?,
            daily_limit: r.get("daily_limit")?,
            monthly_limit: r.get("monthly_limit")?,
            per_minute: r.get("per_minute")?,
            key_prefix: r.get("key_prefix")?,
        })
    }

    pub fn role(&self) -> Role {
        Role::parse(&self.role).unwrap_or(Role::User)
    }
}

/// What the request path needs to know about a caller (cached in memory).
#[derive(Debug, Clone)]
pub struct Principal {
    pub id: i64,
    pub username: String,
    pub role: Role,
    pub status: Status,
    pub suspended_until: Option<i64>,
    pub plan: Arc<Plan>,
    pub daily_limit: Option<i64>,
    pub monthly_limit: Option<i64>,
    pub per_minute: Option<i64>,
}

impl Principal {
    pub fn anonymous(plan: Arc<Plan>) -> Principal {
        Principal {
            id: 0,
            username: "anonymous".into(),
            role: Role::User,
            status: Status::Active,
            suspended_until: None,
            daily_limit: plan.daily_limit,
            monthly_limit: plan.monthly_limit,
            per_minute: plan.per_minute,
            plan,
        }
    }

    /// `Err` when the account may not use the API right now.
    pub fn check_status(&self, now: i64) -> Result<()> {
        match self.status {
            Status::Banned => Err(denied(403, "account_banned", "this account is banned")),
            Status::Suspended if self.suspended_until.is_none_or(|t| t > now) => {
                let until = self.suspended_until.map(|t| format!(" until {}", day_string(day_of(t)))).unwrap_or_default();
                Err(denied(403, "account_suspended", format!("this account is suspended{until}")))
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub registration: String,
    pub default_plan: String,
    pub search_log: bool,
    pub search_log_days: i64,
    pub auto_update: bool,
    pub update_check_hours: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            registration: "open".into(),
            default_plan: "free".into(),
            search_log: true,
            search_log_days: 30,
            auto_update: true,
            update_check_hours: 6,
        }
    }
}

/// One aggregated usage increment (flushed in batches).
#[derive(Debug, Clone, Copy, Default)]
pub struct UsageDelta {
    pub count: i64,
    pub cache_hits: i64,
    pub errors: i64,
    pub ms_total: f64,
}

#[derive(Debug, Clone)]
pub struct SearchLogRow {
    pub ts: i64,
    pub user_id: Option<i64>,
    pub route: String,
    pub query: String,
    pub status: u16,
    pub ms: f64,
    pub cache: bool,
    pub ip: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UserFilter {
    pub q: Option<String>,
    pub status: Option<String>,
    pub role: Option<String>,
    pub plan: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ---------------------------------------------------------------------------------------
// Secrets.
// ---------------------------------------------------------------------------------------

const B62: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
pub const KEY_PREFIX_LEN: usize = 10;
pub const KEY_SECRET_LEN: usize = 32;

/// `n` uniformly random base62 characters (rejection sampling).
pub fn random_b62(n: usize) -> String {
    let mut out = String::with_capacity(n);
    let mut buf = [0u8; 64];
    while out.len() < n {
        getrandom::fill(&mut buf).expect("system random source");
        for &b in &buf {
            if b < 248 && out.len() < n {
                out.push(B62[usize::from(b % 62)] as char);
            }
        }
    }
    out
}

pub fn sha256(s: &str) -> [u8; 32] {
    Sha256::digest(s.as_bytes()).into()
}

fn argon2() -> Argon2<'static> {
    // OWASP baseline for Argon2id: 19 MiB, 2 passes, 1 lane.
    Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::new(19 * 1024, 2, 1, None).expect("argon2 params"))
}

pub fn hash_secret(secret: &str) -> Result<String> {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|e| anyhow::anyhow!("random: {e}"))?;
    let h = argon2().hash_password_with_salt(secret.as_bytes(), &salt).map_err(|e| anyhow::anyhow!("argon2: {e}"))?;
    Ok(h.to_string())
}

pub fn verify_secret(secret: &str, phc: &str) -> bool {
    argon2().verify_password(secret.as_bytes(), phc).is_ok()
}

/// `(prefix, secret)` of a well-formed login key.
pub fn split_key(key: &str) -> Option<(&str, &str)> {
    let rest = key.strip_prefix("ocnpj_")?;
    let (p, s) = rest.split_once('_')?;
    let ok = |x: &str, n: usize| x.len() == n && x.bytes().all(|b| b.is_ascii_alphanumeric());
    (ok(p, KEY_PREFIX_LEN) && ok(s, KEY_SECRET_LEN)).then_some((p, s))
}

fn valid_username(u: &str) -> bool {
    (3..=32).contains(&u.len()) && u.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".-_".contains(&b))
}

fn valid_email(e: &str) -> bool {
    let Some((local, domain)) = e.rsplit_once('@') else { return false };
    e.len() <= 254
        && !local.is_empty()
        && local.len() <= 64
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !e.chars().any(|c| c.is_whitespace() || c.is_control() || "<>,;:\"()[]\\".contains(c))
}

// ---------------------------------------------------------------------------------------
// Store.
// ---------------------------------------------------------------------------------------

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS plans (
    id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL,
    daily_limit INTEGER, monthly_limit INTEGER, per_minute INTEGER, burst INTEGER,
    max_page_size INTEGER NOT NULL, export INTEGER NOT NULL, max_export_rows INTEGER, downloads INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY, username TEXT UNIQUE NOT NULL, email TEXT UNIQUE NOT NULL,
    role TEXT NOT NULL, status TEXT NOT NULL, plan_id INTEGER NOT NULL REFERENCES plans(id),
    created_at INTEGER NOT NULL, verified_at INTEGER, last_seen_at INTEGER,
    suspended_until INTEGER, status_reason TEXT, notes TEXT NOT NULL DEFAULT '',
    daily_limit INTEGER, monthly_limit INTEGER, per_minute INTEGER);
CREATE TABLE IF NOT EXISTS keys (
    id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    prefix TEXT UNIQUE NOT NULL, hash TEXT NOT NULL, created_at INTEGER NOT NULL,
    revoked_at INTEGER, last_used_at INTEGER);
CREATE INDEX IF NOT EXISTS keys_user ON keys(user_id);
CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash BLOB UNIQUE NOT NULL, created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL,
    revoked_at INTEGER, ip TEXT, user_agent TEXT);
CREATE INDEX IF NOT EXISTS sessions_user ON sessions(user_id);
CREATE TABLE IF NOT EXISTS recovery (
    token_hash BLOB PRIMARY KEY, user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL, used_at INTEGER);
CREATE TABLE IF NOT EXISTS usage_daily (
    user_id INTEGER NOT NULL, day INTEGER NOT NULL, kind TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 0, cache_hits INTEGER NOT NULL DEFAULT 0,
    errors INTEGER NOT NULL DEFAULT 0, ms_total REAL NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, day, kind)) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS usage_day ON usage_daily(day);
CREATE TABLE IF NOT EXISTS search_log (
    id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, user_id INTEGER, route TEXT NOT NULL,
    query TEXT NOT NULL, status INTEGER NOT NULL, ms REAL NOT NULL, cache INTEGER NOT NULL, ip TEXT);
CREATE INDEX IF NOT EXISTS search_log_user ON search_log(user_id, id);
CREATE INDEX IF NOT EXISTS search_log_ts ON search_log(ts);
CREATE TABLE IF NOT EXISTS audit (
    id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, actor_id INTEGER, actor TEXT,
    action TEXT NOT NULL, target_id INTEGER, target TEXT, details TEXT NOT NULL, ip TEXT);
CREATE INDEX IF NOT EXISTS audit_actor ON audit(actor_id, id);
CREATE INDEX IF NOT EXISTS audit_target ON audit(target_id, id);
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL, updated_at INTEGER NOT NULL, turn_count INTEGER NOT NULL DEFAULT 0,
    context TEXT NOT NULL DEFAULT '{}');
CREATE INDEX IF NOT EXISTS conversations_user ON conversations(user_id, updated_at DESC, id DESC);
CREATE TABLE IF NOT EXISTS conversation_turns (
    conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    position INTEGER NOT NULL, question TEXT NOT NULL, reply TEXT NOT NULL,
    PRIMARY KEY (conversation_id, position)
) WITHOUT ROWID;
";

fn seed_plans() -> Vec<Plan> {
    let p = |name: &str, d, m, pm, b, page, export, rows, dl| Plan {
        id: 0,
        name: name.into(),
        daily_limit: d,
        monthly_limit: m,
        per_minute: pm,
        burst: b,
        max_page_size: page,
        export,
        max_export_rows: rows,
        downloads: dl,
    };
    vec![
        p("anonymous", Some(100), Some(1_000), Some(10), Some(10), 20, false, Some(0), false),
        p(
            "free",
            Some(crate::trial::FREE_DAILY_LIMIT),
            Some(crate::trial::FREE_MONTHLY_LIMIT),
            Some(60),
            Some(30),
            100,
            false,
            Some(0),
            false,
        ),
        p("pro", Some(100_000), Some(2_000_000), Some(600), Some(200), 1_000, true, Some(100_000), true),
        p("unlimited", None, None, None, None, 10_000, true, None, true),
    ]
}

/// Who did something, for the audit log.
#[derive(Debug, Clone, Default)]
pub struct Actor {
    pub id: Option<i64>,
    pub name: String,
    pub ip: Option<String>,
}

impl Actor {
    pub fn system(name: &str) -> Actor {
        Actor { id: None, name: name.into(), ip: None }
    }
}

pub struct Accounts {
    db: Mutex<Connection>,
    /// SHA-256 of a verified key → (user id, key id).
    keys: moka::sync::Cache<[u8; 32], (i64, i64)>,
    /// SHA-256 of a session token → (user id, session id, expires at).
    sessions: moka::sync::Cache<[u8; 32], (i64, i64, i64)>,
    principals: moka::sync::Cache<i64, Arc<Principal>>,
    plans: RwLock<HashMap<String, Arc<Plan>>>,
    settings: RwLock<Settings>,
    pub session_secs: i64,
}

fn rerr(e: rusqlite::Error) -> anyhow::Error {
    if let rusqlite::Error::SqliteFailure(f, Some(msg)) = &e
        && f.code == rusqlite::ErrorCode::ConstraintViolation
    {
        if msg.contains("users.username") {
            return denied(409, "username_taken", "username is already taken");
        }
        if msg.contains("users.email") {
            return denied(409, "email_taken", "e-mail address already has an account");
        }
        if msg.contains("plans.name") {
            return denied(409, "plan_exists", "a plan with this name exists");
        }
    }
    anyhow::Error::new(e)
}

impl Accounts {
    pub fn open(path: &Path, session_hours: i64) -> Result<Accounts> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        conn.busy_timeout(Duration::from_secs(10))?;
        conn.execute_batch(SCHEMA)?;
        for p in seed_plans() {
            conn.execute(
                "INSERT OR IGNORE INTO plans (name, daily_limit, monthly_limit, per_minute, burst, max_page_size, export, max_export_rows, downloads)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![p.name, p.daily_limit, p.monthly_limit, p.per_minute, p.burst, p.max_page_size, p.export, p.max_export_rows, p.downloads],
            )?;
        }
        conn.execute(
            "UPDATE plans SET daily_limit=?1, monthly_limit=?2 WHERE name='free' AND daily_limit=1000 AND monthly_limit=20000
             AND per_minute=60 AND burst=30 AND max_page_size=100 AND export=0 AND max_export_rows=0 AND downloads=0",
            params![crate::trial::FREE_DAILY_LIMIT, crate::trial::FREE_MONTHLY_LIMIT],
        )?;
        let settings = {
            let mut s = Settings::default();
            let stored: Option<String> =
                conn.query_row("SELECT value FROM settings WHERE key = 'settings'", [], |r| r.get(0)).optional()?;
            if let Some(v) = stored
                && let Ok(x) = serde_json::from_str::<Settings>(&v)
            {
                s = x;
            }
            s
        };
        let a = Accounts {
            db: Mutex::new(conn),
            keys: moka::sync::Cache::builder()
                .max_capacity(1_000_000)
                .time_to_live(Duration::from_secs(3600))
                .support_invalidation_closures()
                .build(),
            sessions: moka::sync::Cache::builder()
                .max_capacity(1_000_000)
                .time_to_live(Duration::from_secs(300))
                .support_invalidation_closures()
                .build(),
            principals: moka::sync::Cache::builder().max_capacity(1_000_000).time_to_live(Duration::from_secs(60)).build(),
            plans: RwLock::new(HashMap::new()),
            settings: RwLock::new(settings),
            session_secs: session_hours.max(1) * 3600,
        };
        a.reload_plans()?;
        Ok(a)
    }

    pub(crate) fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    // ----- plans and settings ------------------------------------------------------------

    fn reload_plans(&self) -> Result<()> {
        let conn = self.conn();
        let mut st = conn.prepare("SELECT * FROM plans ORDER BY id")?;
        let plans: Vec<Plan> = st.query_map([], Plan::from_row)?.collect::<rusqlite::Result<_>>()?;
        *self.plans.write().expect("lock") = plans.into_iter().map(|p| (p.name.clone(), Arc::new(p))).collect();
        self.principals.invalidate_all();
        Ok(())
    }

    pub fn plan(&self, name: &str) -> Option<Arc<Plan>> {
        self.plans.read().expect("lock").get(name).cloned()
    }

    pub fn plans_json(&self) -> Result<Value> {
        let conn = self.conn();
        let counts: HashMap<i64, i64> = conn
            .prepare("SELECT plan_id, COUNT(*) FROM users GROUP BY plan_id")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut plans: Vec<Arc<Plan>> = self.plans.read().expect("lock").values().cloned().collect();
        plans.sort_by_key(|p| p.id);
        Ok(Value::Array(
            plans
                .iter()
                .map(|p| {
                    let mut v = json!(**p);
                    v["users"] = json!(counts.get(&p.id).copied().unwrap_or(0));
                    v
                })
                .collect(),
        ))
    }

    pub fn create_plan(&self, p: &Plan, actor: &Actor) -> Result<Arc<Plan>> {
        check_plan(p)?;
        self.conn()
            .execute(
                "INSERT INTO plans (name, daily_limit, monthly_limit, per_minute, burst, max_page_size, export, max_export_rows, downloads)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    p.name,
                    p.daily_limit,
                    p.monthly_limit,
                    p.per_minute,
                    p.burst,
                    p.max_page_size,
                    p.export,
                    p.max_export_rows,
                    p.downloads
                ],
            )
            .map_err(rerr)?;
        self.reload_plans()?;
        self.audit(actor, "plan.create", None, json!(p));
        self.plan(&p.name).context("plan vanished")
    }

    /// Applies a JSON patch of plan fields.
    pub fn update_plan(&self, id: i64, patch: &Value, actor: &Actor) -> Result<Arc<Plan>> {
        let cur = self.plans.read().expect("lock").values().find(|p| p.id == id).cloned();
        let cur = cur.ok_or_else(|| denied(404, "not_found", "no such plan"))?;
        let mut v = json!(*cur);
        let obj = patch.as_object().ok_or_else(|| denied(400, "invalid_request", "expected a JSON object"))?;
        for (k, x) in obj {
            if k == "id" || v.get(k).is_none() {
                return Err(denied(400, "invalid_request", format!("unknown plan field {k}")));
            }
            v[k] = x.clone();
        }
        let p: Plan = serde_json::from_value(v).map_err(|e| denied(400, "invalid_request", e.to_string()))?;
        check_plan(&p)?;
        if cur.name == "anonymous" && p.name != "anonymous" {
            return Err(denied(400, "invalid_request", "the anonymous plan cannot be renamed"));
        }
        self.conn()
            .execute(
                "UPDATE plans SET name=?2, daily_limit=?3, monthly_limit=?4, per_minute=?5, burst=?6, max_page_size=?7,
                 export=?8, max_export_rows=?9, downloads=?10 WHERE id=?1",
                params![
                    id,
                    p.name,
                    p.daily_limit,
                    p.monthly_limit,
                    p.per_minute,
                    p.burst,
                    p.max_page_size,
                    p.export,
                    p.max_export_rows,
                    p.downloads
                ],
            )
            .map_err(rerr)?;
        self.reload_plans()?;
        self.audit(actor, "plan.update", None, json!({ "plan": id, "changes": patch }));
        self.plan(&p.name).context("plan vanished")
    }

    pub fn delete_plan(&self, id: i64, actor: &Actor) -> Result<()> {
        let name = self.plans.read().expect("lock").values().find(|p| p.id == id).map(|p| p.name.clone());
        let name = name.ok_or_else(|| denied(404, "not_found", "no such plan"))?;
        if name == "anonymous" || name == self.settings().default_plan {
            return Err(denied(409, "plan_in_use", "the anonymous and default plans cannot be deleted"));
        }
        {
            let conn = self.conn();
            let n: i64 = conn.query_row("SELECT COUNT(*) FROM users WHERE plan_id = ?1", [id], |r| r.get(0))?;
            if n > 0 {
                return Err(denied(409, "plan_in_use", format!("{n} users are on this plan")));
            }
            conn.execute("DELETE FROM plans WHERE id = ?1", [id])?;
        }
        self.reload_plans()?;
        self.audit(actor, "plan.delete", None, json!({ "plan": id, "name": name }));
        Ok(())
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().expect("lock").clone()
    }

    pub fn patch_settings(&self, patch: &Value, actor: &Actor) -> Result<Settings> {
        let mut v = json!(self.settings());
        let obj = patch.as_object().ok_or_else(|| denied(400, "invalid_request", "expected a JSON object"))?;
        for (k, x) in obj {
            if v.get(k).is_none() {
                return Err(denied(400, "invalid_request", format!("unknown setting {k}")));
            }
            v[k] = x.clone();
        }
        let s: Settings = serde_json::from_value(v).map_err(|e| denied(400, "invalid_request", e.to_string()))?;
        if s.registration != "open" && s.registration != "closed" {
            return Err(denied(400, "invalid_request", "registration must be open or closed"));
        }
        if self.plan(&s.default_plan).is_none() {
            return Err(denied(400, "invalid_request", "default_plan must name an existing plan"));
        }
        if !(1..=3650).contains(&s.search_log_days) || !(1..=720).contains(&s.update_check_hours) {
            return Err(denied(400, "invalid_request", "search_log_days must be 1..3650 and update_check_hours 1..720"));
        }
        self.conn().execute(
            "INSERT INTO settings (key, value) VALUES ('settings', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [serde_json::to_string(&s)?],
        )?;
        *self.settings.write().expect("lock") = s.clone();
        self.audit(actor, "settings.update", None, patch.clone());
        Ok(s)
    }

    // ----- users ---------------------------------------------------------------------------

    pub fn user(&self, id: i64) -> Result<Option<User>> {
        let conn = self.conn();
        Ok(conn.query_row(&format!("{USER_SELECT} WHERE u.id = ?1"), [id], User::from_row).optional()?)
    }

    pub fn user_by(&self, column: &str, value: &str) -> Result<Option<User>> {
        anyhow::ensure!(column == "username" || column == "email", "bad column");
        let conn = self.conn();
        Ok(conn.query_row(&format!("{USER_SELECT} WHERE u.{column} = ?1"), [value], User::from_row).optional()?)
    }

    fn require_user(&self, id: i64) -> Result<User> {
        self.user(id)?.ok_or_else(|| denied(404, "not_found", "no such user"))
    }

    /// Creates a user with a fresh key; returns the user and the key (shown once).
    pub fn create_user(
        &self,
        username: &str,
        email: &str,
        role: Role,
        plan: Option<&str>,
        status: Status,
        actor: &Actor,
    ) -> Result<(User, String)> {
        let username = username.trim().to_ascii_lowercase();
        let email = email.trim().to_ascii_lowercase();
        if !valid_username(&username) {
            return Err(denied(400, "invalid_username", "username must be 3-32 characters of a-z, 0-9, '.', '_' or '-'"));
        }
        if !valid_email(&email) {
            return Err(denied(400, "invalid_email", "invalid e-mail address"));
        }
        let plan_name = plan
            .map(str::to_string)
            .unwrap_or_else(|| if role.is_staff() { "unlimited".to_string() } else { self.settings().default_plan });
        let plan = self.plan(&plan_name).ok_or_else(|| denied(400, "invalid_request", format!("unknown plan {plan_name}")))?;
        let (prefix, secret, hash) = new_key()?;
        let t = now();
        let id = {
            let mut conn = self.conn();
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO users (username, email, role, status, plan_id, created_at, verified_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![username, email, role.name(), status.name(), plan.id, t, (status == Status::Active).then_some(t)],
            )
            .map_err(rerr)?;
            let id = tx.last_insert_rowid();
            tx.execute("INSERT INTO keys (user_id, prefix, hash, created_at) VALUES (?1, ?2, ?3, ?4)", params![id, prefix, hash, t])?;
            tx.commit()?;
            id
        };
        self.audit(
            actor,
            "user.create",
            Some(id),
            json!({ "username": username, "email": email, "role": role.name(), "plan": plan.name, "status": status.name() }),
        );
        Ok((self.require_user(id)?, format!("ocnpj_{prefix}_{secret}")))
    }

    /// Self-service registration. `Ok(None)` when the e-mail already has an account.
    pub fn register(&self, username: &str, email: &str, ip: Option<String>) -> Result<Option<(User, String)>> {
        if self.settings().registration != "open" {
            return Err(denied(403, "registration_closed", "registration is closed on this server"));
        }
        let actor = Actor { id: None, name: "self-registration".into(), ip };
        if self.user_by("email", &email.trim().to_ascii_lowercase())?.is_some() {
            return Ok(None);
        }
        match self.create_user(username, email, Role::User, None, Status::Pending, &actor) {
            Ok(x) => Ok(Some(x)),
            Err(e) if e.downcast_ref::<Denied>().is_some_and(|d| d.code == "email_taken") => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn principal(&self, id: i64) -> Result<Option<Arc<Principal>>> {
        if let Some(p) = self.principals.get(&id) {
            return Ok(Some(p));
        }
        let Some(u) = self.user(id)? else { return Ok(None) };
        let plan = self.plans.read().expect("lock").values().find(|p| p.id == u.plan_id).cloned().context("user plan missing")?;
        let p = Arc::new(Principal {
            id: u.id,
            role: u.role(),
            status: Status::parse(&u.status).unwrap_or(Status::Suspended),
            suspended_until: u.suspended_until,
            daily_limit: u.daily_limit.or(plan.daily_limit),
            monthly_limit: u.monthly_limit.or(plan.monthly_limit),
            per_minute: u.per_minute.or(plan.per_minute),
            username: u.username,
            plan,
        });
        self.principals.insert(id, p.clone());
        Ok(Some(p))
    }

    fn changed(&self, id: i64) {
        self.principals.invalidate(&id);
    }

    /// First successful login with the e-mailed key proves the address.
    pub fn activate_if_pending(&self, id: i64) -> Result<()> {
        let n = self
            .conn()
            .execute("UPDATE users SET status = 'active', verified_at = ?2 WHERE id = ?1 AND status = 'pending'", params![id, now()])?;
        if n > 0 {
            self.changed(id);
            self.audit(&Actor::system("login"), "user.verify", Some(id), json!({}));
        }
        Ok(())
    }

    // ----- credentials ---------------------------------------------------------------------

    /// Cached check (no Argon2): `Some((user, key id))` for a key verified recently.
    pub fn cached_key(&self, key: &str) -> Option<(i64, i64)> {
        self.keys.get(&sha256(key))
    }

    /// Cached session check: `None` when not cached, `Some(None)` when expired.
    pub fn cached_session(&self, token: &str) -> Option<Option<(i64, i64)>> {
        let (u, s, exp) = self.sessions.get(&sha256(token))?;
        Some((exp > now()).then_some((u, s)))
    }

    pub fn cached_principal(&self, id: i64) -> Option<Arc<Principal>> {
        self.principals.get(&id)
    }

    pub fn settings_cached_search_log(&self) -> bool {
        self.settings.read().map(|s| s.search_log).unwrap_or(false)
    }

    /// Hands verified keys to the process that takes over (blue/green restart): writes
    /// `sha256(key) user key_id` lines to `path` (mode 0600). Keys are 190-bit random
    /// secrets, so their SHA-256 reveals nothing guessable; the importer re-checks
    /// revocation. Returns the number of entries written.
    pub fn export_verified(&self, path: &Path) -> Result<usize> {
        let lines: Vec<String> = self.keys.iter().map(|(h, (u, k))| format!("{} {u} {k}", hex::encode(*h))).collect();
        let tmp = path.with_extension("tmp");
        crate::accounts_cli::write_secret(&tmp, &(lines.join("\n") + "\n"))?;
        std::fs::rename(&tmp, path)?;
        Ok(lines.len())
    }

    /// Imports (and deletes) a handover file younger than a day; skips revoked keys.
    pub fn import_verified(&self, path: &Path) -> Result<usize> {
        let fresh = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age < Duration::from_secs(86_400));
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let _ = std::fs::remove_file(path);
        if !fresh {
            return Ok(0);
        }
        let active: std::collections::HashSet<i64> = {
            let conn = self.conn();
            let mut st = conn.prepare("SELECT id FROM keys WHERE revoked_at IS NULL")?;
            st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
        };
        let mut n = 0;
        for l in text.lines() {
            let mut it = l.split(' ');
            let (Some(h), Some(u), Some(k)) = (it.next(), it.next(), it.next()) else { continue };
            let (Ok(h), Ok(u), Ok(k)) = (hex::decode(h), u.parse::<i64>(), k.parse::<i64>()) else { continue };
            if let Ok(h) = <[u8; 32]>::try_from(h)
                && active.contains(&k)
            {
                self.keys.insert(h, (u, k));
                n += 1;
            }
        }
        Ok(n)
    }

    /// Full verification (Argon2id). Call from a blocking thread.
    pub fn verify_key(&self, key: &str) -> Result<Option<(i64, i64)>> {
        if let Some(hit) = self.cached_key(key) {
            return Ok(Some(hit));
        }
        let Some((prefix, secret)) = split_key(key) else { return Ok(None) };
        let row: Option<(i64, i64, String)> = self
            .conn()
            .query_row("SELECT id, user_id, hash FROM keys WHERE prefix = ?1 AND revoked_at IS NULL", [prefix], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()?;
        let Some((key_id, user_id, hash)) = row else { return Ok(None) };
        if !verify_secret(secret, &hash) {
            return Ok(None);
        }
        self.conn().execute("UPDATE keys SET last_used_at = ?2 WHERE id = ?1", params![key_id, now()])?;
        self.keys.insert(sha256(key), (user_id, key_id));
        Ok(Some((user_id, key_id)))
    }

    pub fn session(&self, token: &str) -> Result<Option<(i64, i64)>> {
        let h = sha256(token);
        let t = now();
        if let Some((u, s, exp)) = self.sessions.get(&h) {
            return Ok((exp > t).then_some((u, s)));
        }
        let row: Option<(i64, i64, i64)> = self
            .conn()
            .query_row("SELECT user_id, id, expires_at FROM sessions WHERE token_hash = ?1 AND revoked_at IS NULL", [&h[..]], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()?;
        let Some((u, s, exp)) = row else { return Ok(None) };
        self.sessions.insert(h, (u, s, exp));
        Ok((exp > t).then_some((u, s)))
    }

    /// Exchanges a key for a session token.
    pub fn login(&self, key: &str, ip: Option<String>, user_agent: Option<String>) -> Result<(String, i64, User)> {
        let (user_id, _) = self.verify_key(key)?.ok_or_else(|| denied(401, "invalid_key", "invalid login key"))?;
        let p = self.principal(user_id)?.ok_or_else(|| denied(401, "invalid_key", "invalid login key"))?;
        p.check_status(now())?;
        self.activate_if_pending(user_id)?;
        let token = format!("ocs_{}", random_b62(43));
        let t = now();
        let exp = t + self.session_secs;
        self.conn().execute(
            "INSERT INTO sessions (user_id, token_hash, created_at, expires_at, ip, user_agent) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![user_id, &sha256(&token)[..], t, exp, ip, user_agent.map(|s| s.chars().take(200).collect::<String>())],
        )?;
        self.conn().execute("UPDATE users SET last_seen_at = ?2 WHERE id = ?1", params![user_id, t])?;
        Ok((token, exp, self.require_user(user_id)?))
    }

    pub fn logout(&self, token: &str) -> Result<bool> {
        let h = sha256(token);
        self.sessions.invalidate(&h);
        Ok(self
            .conn()
            .execute("UPDATE sessions SET revoked_at = ?2 WHERE token_hash = ?1 AND revoked_at IS NULL", params![&h[..], now()])?
            > 0)
    }

    pub fn revoke_sessions(&self, user_id: i64, actor: &Actor) -> Result<usize> {
        let n = self
            .conn()
            .execute("UPDATE sessions SET revoked_at = ?2 WHERE user_id = ?1 AND revoked_at IS NULL", params![user_id, now()])?;
        self.sessions.invalidate_entries_if(move |_, v| v.0 == user_id).ok();
        self.audit(actor, "user.revoke_sessions", Some(user_id), json!({ "revoked": n }));
        Ok(n)
    }

    /// Revokes every key and session of the user and issues a new key.
    pub fn rotate_key(&self, user_id: i64, actor: &Actor) -> Result<String> {
        self.require_user(user_id)?;
        let (prefix, secret, hash) = new_key()?;
        let t = now();
        {
            let mut conn = self.conn();
            let tx = conn.transaction()?;
            tx.execute("UPDATE keys SET revoked_at = ?2 WHERE user_id = ?1 AND revoked_at IS NULL", params![user_id, t])?;
            tx.execute("UPDATE sessions SET revoked_at = ?2 WHERE user_id = ?1 AND revoked_at IS NULL", params![user_id, t])?;
            tx.execute("INSERT INTO keys (user_id, prefix, hash, created_at) VALUES (?1, ?2, ?3, ?4)", params![user_id, prefix, hash, t])?;
            tx.commit()?;
        }
        self.keys.invalidate_entries_if(move |_, v| v.0 == user_id).ok();
        self.sessions.invalidate_entries_if(move |_, v| v.0 == user_id).ok();
        self.audit(actor, "user.rotate_key", Some(user_id), json!({ "key_prefix": prefix }));
        Ok(format!("ocnpj_{prefix}_{secret}"))
    }

    /// Starts recovery for an address; `None` when it has no account.
    pub fn start_recovery(&self, email: &str) -> Result<Option<(User, String)>> {
        let Some(u) = self.user_by("email", &email.trim().to_ascii_lowercase())? else { return Ok(None) };
        let token = format!("ocr_{}", random_b62(43));
        let t = now();
        let conn = self.conn();
        // At most five live tokens per account.
        conn.execute(
            "DELETE FROM recovery WHERE user_id = ?1 AND token_hash NOT IN
             (SELECT token_hash FROM recovery WHERE user_id = ?1 AND used_at IS NULL AND expires_at > ?2 ORDER BY created_at DESC LIMIT 4)",
            params![u.id, t],
        )?;
        conn.execute(
            "INSERT INTO recovery (token_hash, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
            params![&sha256(&token)[..], u.id, t, t + 1800],
        )?;
        Ok(Some((u, token)))
    }

    /// Consumes a recovery token: revokes the old key and sessions, returns a new key.
    pub fn confirm_recovery(&self, token: &str, ip: Option<String>) -> Result<(User, String)> {
        let h = sha256(token.trim());
        let t = now();
        let user_id: Option<i64> = {
            let conn = self.conn();
            let id: Option<i64> = conn
                .query_row(
                    "SELECT user_id FROM recovery WHERE token_hash = ?1 AND used_at IS NULL AND expires_at > ?2",
                    params![&h[..], t],
                    |r| r.get(0),
                )
                .optional()?;
            if id.is_some() {
                conn.execute("UPDATE recovery SET used_at = ?2 WHERE token_hash = ?1", params![&h[..], t])?;
            }
            id
        };
        let user_id = user_id.ok_or_else(|| denied(400, "invalid_token", "this recovery link is invalid, used or expired"))?;
        let actor = Actor { id: Some(user_id), name: "recovery".into(), ip };
        let key = self.rotate_key(user_id, &actor)?;
        self.activate_if_pending(user_id)?;
        Ok((self.require_user(user_id)?, key))
    }

    // ----- moderation and management --------------------------------------------------------

    fn active_admins(&self) -> Result<i64> {
        Ok(self
            .conn()
            .query_row("SELECT COUNT(*) FROM users WHERE role = 'admin' AND status IN ('active', 'pending')", [], |r| r.get(0))?)
    }

    /// Guards shared by every moderation action.
    pub fn check_target(&self, actor: &Principal, target: &User, removes_admin: bool) -> Result<()> {
        if actor.id == target.id && removes_admin {
            return Err(denied(403, "forbidden", "you cannot do this to your own account"));
        }
        if actor.role != Role::Admin && target.role() != Role::User {
            return Err(denied(403, "forbidden", "only admins can act on staff accounts"));
        }
        if removes_admin && target.role() == Role::Admin && target.status == "active" && self.active_admins()? <= 1 {
            return Err(denied(409, "last_admin", "this is the last active admin"));
        }
        Ok(())
    }

    pub fn set_status(&self, id: i64, status: Status, reason: Option<&str>, until: Option<i64>, actor: &Actor) -> Result<User> {
        let n = self.conn().execute(
            "UPDATE users SET status = ?2, status_reason = ?3, suspended_until = ?4 WHERE id = ?1",
            params![id, status.name(), reason, until],
        )?;
        if n == 0 {
            return Err(denied(404, "not_found", "no such user"));
        }
        if status != Status::Active {
            // Live sessions end now; the key keeps working once the account is reactivated.
            self.conn().execute("UPDATE sessions SET revoked_at = ?2 WHERE user_id = ?1 AND revoked_at IS NULL", params![id, now()])?;
            self.sessions.invalidate_entries_if(move |_, v| v.0 == id).ok();
        }
        self.changed(id);
        let action = format!(
            "user.{}",
            match status {
                Status::Active => "reactivate",
                Status::Suspended => "suspend",
                Status::Banned => "ban",
                Status::Pending => "pending",
            }
        );
        self.audit(actor, &action, Some(id), json!({ "reason": reason, "until": until }));
        self.require_user(id)
    }

    /// Applies a JSON patch: `email`, `role`, `plan`, `daily_limit`, `monthly_limit`,
    /// `per_minute`, `notes`.
    pub fn update_user(&self, id: i64, patch: &Value, actor: &Actor) -> Result<User> {
        let obj = patch.as_object().ok_or_else(|| denied(400, "invalid_request", "expected a JSON object"))?;
        let cur = self.require_user(id)?;
        let mut sets: Vec<(&str, rusqlite::types::Value)> = Vec::new();
        let opt_int = |k: &str, v: &Value| -> Result<rusqlite::types::Value> {
            match v {
                Value::Null => Ok(rusqlite::types::Value::Null),
                Value::Number(n) if n.as_i64().is_some_and(|x| x >= 0) => Ok(rusqlite::types::Value::Integer(n.as_i64().unwrap_or(0))),
                _ => Err(denied(400, "invalid_request", format!("{k} must be a non-negative integer or null"))),
            }
        };
        for (k, v) in obj {
            match k.as_str() {
                "email" => {
                    let e = v.as_str().unwrap_or_default().trim().to_ascii_lowercase();
                    if !valid_email(&e) {
                        return Err(denied(400, "invalid_email", "invalid e-mail address"));
                    }
                    sets.push(("email", e.into()));
                }
                "role" => {
                    let r = v
                        .as_str()
                        .and_then(Role::parse)
                        .ok_or_else(|| denied(400, "invalid_request", "role must be admin, moderator, analyst or user"))?;
                    if cur.role() == Role::Admin && r != Role::Admin && self.active_admins()? <= 1 {
                        return Err(denied(409, "last_admin", "this is the last active admin"));
                    }
                    sets.push(("role", r.name().to_string().into()));
                }
                "plan" => {
                    let name = v.as_str().unwrap_or_default();
                    let p = self.plan(name).ok_or_else(|| denied(400, "invalid_request", format!("unknown plan {name}")))?;
                    sets.push(("plan_id", p.id.into()));
                }
                "daily_limit" | "monthly_limit" | "per_minute" => sets.push((
                    match k.as_str() {
                        "daily_limit" => "daily_limit",
                        "monthly_limit" => "monthly_limit",
                        _ => "per_minute",
                    },
                    opt_int(k, v)?,
                )),
                "notes" => sets.push(("notes", v.as_str().unwrap_or_default().chars().take(10_000).collect::<String>().into())),
                _ => return Err(denied(400, "invalid_request", format!("unknown user field {k}"))),
            }
        }
        if sets.is_empty() {
            return Ok(cur);
        }
        let sql = format!(
            "UPDATE users SET {} WHERE id = ?1",
            sets.iter().enumerate().map(|(i, (c, _))| format!("{c} = ?{}", i + 2)).collect::<Vec<_>>().join(", ")
        );
        let mut values: Vec<rusqlite::types::Value> = vec![id.into()];
        values.extend(sets.into_iter().map(|(_, v)| v));
        self.conn().execute(&sql, rusqlite::params_from_iter(values)).map_err(rerr)?;
        self.changed(id);
        self.audit(actor, "user.update", Some(id), patch.clone());
        self.require_user(id)
    }

    pub fn delete_user(&self, id: i64, actor: &Actor) -> Result<()> {
        let u = self.require_user(id)?;
        {
            let mut conn = self.conn();
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM search_log WHERE user_id = ?1", [id])?;
            tx.execute("DELETE FROM usage_daily WHERE user_id = ?1", [id])?;
            tx.execute("DELETE FROM users WHERE id = ?1", [id])?;
            tx.commit()?;
        }
        self.changed(id);
        self.keys.invalidate_entries_if(move |_, v| v.0 == id).ok();
        self.sessions.invalidate_entries_if(move |_, v| v.0 == id).ok();
        self.audit(actor, "user.delete", Some(id), json!({ "username": u.username, "email": u.email }));
        Ok(())
    }

    /// Removes recorded usage of the current UTC day or month (the in-memory meter is
    /// reset by the caller).
    pub fn reset_usage(&self, id: i64, month: bool, actor: &Actor) -> Result<()> {
        let today = day_of(now());
        let from = if month { month_bounds(today).0 } else { today };
        self.conn().execute("DELETE FROM usage_daily WHERE user_id = ?1 AND day >= ?2", params![id, from])?;
        self.audit(actor, "user.reset_usage", Some(id), json!({ "scope": if month { "month" } else { "day" } }));
        Ok(())
    }

    pub fn list_users(&self, f: &UserFilter) -> Result<Value> {
        let today = day_of(now());
        let (m0, _) = month_bounds(today);
        let mut cond = vec!["1 = 1".to_string()];
        let mut args: Vec<rusqlite::types::Value> = vec![today.into(), m0.into()];
        if let Some(q) = f.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
            args.push(format!("%{}%", q.to_ascii_lowercase().replace(['%', '_'], "")).into());
            cond.push(format!("(u.username LIKE ?{0} OR u.email LIKE ?{0})", args.len()));
        }
        for (col, v) in [("u.status", &f.status), ("u.role", &f.role), ("p.name", &f.plan)] {
            if let Some(v) = v.as_deref().filter(|v| !v.is_empty()) {
                args.push(v.to_string().into());
                cond.push(format!("{col} = ?{}", args.len()));
            }
        }
        let order = match f.sort.as_deref().unwrap_or("-created") {
            "created" => "u.id ASC",
            "username" => "u.username ASC",
            "usage_today" => "usage_today ASC, u.id",
            "-usage_today" => "usage_today DESC, u.id",
            "last_seen" => "u.last_seen_at DESC NULLS LAST, u.id",
            _ => "u.id DESC",
        };
        let limit = f.limit.unwrap_or(50).clamp(1, 500);
        let offset = f.offset.unwrap_or(0).max(0);
        let where_ = cond.join(" AND ");
        let conn = self.conn();
        // ?1 and ?2 (today, month start) are only used by the usage columns below.
        let total: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM users u JOIN plans p ON p.id = u.plan_id WHERE ?1 IS NOT NULL AND ?2 IS NOT NULL AND {where_}"),
            rusqlite::params_from_iter(args.iter()),
            |r| r.get(0),
        )?;
        let sql = format!(
            "SELECT * FROM (SELECT u.*, p.name AS plan_name,
                (SELECT prefix FROM keys k WHERE k.user_id = u.id AND k.revoked_at IS NULL ORDER BY k.id DESC LIMIT 1) AS key_prefix,
                COALESCE((SELECT SUM(count) FROM usage_daily d WHERE d.user_id = u.id AND d.day = ?1), 0) AS usage_today,
                COALESCE((SELECT SUM(count) FROM usage_daily d WHERE d.user_id = u.id AND d.day >= ?2), 0) AS usage_month
             FROM users u JOIN plans p ON p.id = u.plan_id WHERE {where_}) u ORDER BY {order} LIMIT {limit} OFFSET {offset}"
        );
        let mut st = conn.prepare(&sql)?;
        let users: Vec<Value> = st
            .query_map(rusqlite::params_from_iter(args.iter()), |r| {
                let mut v = json!(User::from_row(r)?);
                v["usage_today"] = json!(r.get::<_, i64>("usage_today")?);
                v["usage_month"] = json!(r.get::<_, i64>("usage_month")?);
                Ok(v)
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(json!({ "total": total, "users": users }))
    }

    pub fn user_detail(&self, id: i64) -> Result<Value> {
        let u = self.require_user(id)?;
        let since = day_of(now()) - 29;
        let conn = self.conn();
        let keys: Vec<Value> = conn
            .prepare("SELECT id, prefix, created_at, revoked_at, last_used_at FROM keys WHERE user_id = ?1 ORDER BY id DESC LIMIT 50")?
            .query_map([id], |r| {
                Ok(json!({ "id": r.get::<_, i64>(0)?, "prefix": r.get::<_, String>(1)?, "created_at": r.get::<_, i64>(2)?,
                           "revoked_at": r.get::<_, Option<i64>>(3)?, "last_used_at": r.get::<_, Option<i64>>(4)? }))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let sessions: Vec<Value> = conn
            .prepare(
                "SELECT id, created_at, expires_at, revoked_at, ip, user_agent FROM sessions WHERE user_id = ?1 ORDER BY id DESC LIMIT 50",
            )?
            .query_map([id], |r| {
                Ok(json!({ "id": r.get::<_, i64>(0)?, "created_at": r.get::<_, i64>(1)?, "expires_at": r.get::<_, i64>(2)?,
                           "revoked_at": r.get::<_, Option<i64>>(3)?, "ip": r.get::<_, Option<String>>(4)?, "user_agent": r.get::<_, Option<String>>(5)? }))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let usage: Vec<Value> = conn
            .prepare("SELECT day, kind, count, cache_hits, errors FROM usage_daily WHERE user_id = ?1 AND day >= ?2 ORDER BY day, kind")?
            .query_map(params![id, since], |r| {
                Ok(json!({ "day": day_string(r.get(0)?), "kind": r.get::<_, String>(1)?, "count": r.get::<_, i64>(2)?,
                           "cache_hits": r.get::<_, i64>(3)?, "errors": r.get::<_, i64>(4)? }))
            })?
            .collect::<rusqlite::Result<_>>()?;
        drop(conn);
        let recent = self.searches(Some(id), 50, None)?;
        Ok(json!({ "user": u, "keys": keys, "sessions": sessions, "usage_daily": usage, "recent_searches": recent }))
    }

    // ----- usage, logs, audit ---------------------------------------------------------------

    /// `(today, this month)` request counts recorded for a user.
    pub fn counts(&self, user_id: i64, day: i64) -> Result<(i64, i64)> {
        let (m0, _) = month_bounds(day);
        Ok(self.conn().query_row(
            "SELECT COALESCE(SUM(CASE WHEN day = ?2 THEN count END), 0), COALESCE(SUM(count), 0)
             FROM usage_daily WHERE user_id = ?1 AND day >= ?3 AND day <= ?2",
            params![user_id, day, m0],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    }

    /// Writes aggregated usage increments in one transaction.
    pub fn flush_usage(&self, deltas: &HashMap<(i64, i64, &'static str), UsageDelta>) -> Result<()> {
        if deltas.is_empty() {
            return Ok(());
        }
        let t = now();
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        {
            let mut up = tx.prepare_cached(
                "INSERT INTO usage_daily (user_id, day, kind, count, cache_hits, errors, ms_total) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(user_id, day, kind) DO UPDATE SET count = count + excluded.count, cache_hits = cache_hits + excluded.cache_hits,
                 errors = errors + excluded.errors, ms_total = ms_total + excluded.ms_total",
            )?;
            let mut seen = tx.prepare_cached("UPDATE users SET last_seen_at = ?2 WHERE id = ?1")?;
            let mut users: Vec<i64> = Vec::new();
            for ((user, day, kind), d) in deltas {
                up.execute(params![user, day, kind, d.count, d.cache_hits, d.errors, d.ms_total])?;
                users.push(*user);
            }
            users.sort_unstable();
            users.dedup();
            for u in users.into_iter().filter(|u| *u > 0) {
                seen.execute(params![u, t])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn insert_search_logs(&self, rows: &[SearchLogRow]) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        {
            let mut ins = tx.prepare_cached(
                "INSERT INTO search_log (ts, user_id, route, query, status, ms, cache, ip) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for r in rows {
                ins.execute(params![r.ts, r.user_id, r.route, r.query, r.status, r.ms, r.cache, r.ip])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Deletes search-log rows older than the retention and expired credentials.
    pub fn purge(&self) -> Result<usize> {
        let t = now();
        let days = self.settings().search_log_days;
        let conn = self.conn();
        let mut n = conn.execute("DELETE FROM search_log WHERE ts < ?1", [t - days * 86_400])?;
        n += conn.execute("DELETE FROM recovery WHERE expires_at < ?1", [t - 86_400])?;
        n += conn.execute("DELETE FROM sessions WHERE expires_at < ?1 OR revoked_at < ?1", [t - 30 * 86_400])?;
        Ok(n)
    }

    pub fn searches(&self, user_id: Option<i64>, limit: i64, before: Option<i64>) -> Result<Value> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT s.id, s.ts, s.user_id, u.username, s.route, s.query, s.status, s.ms, s.cache, s.ip FROM search_log s
             LEFT JOIN users u ON u.id = s.user_id
             WHERE (?1 IS NULL OR s.user_id = ?1) AND (?2 IS NULL OR s.id < ?2) ORDER BY s.id DESC LIMIT ?3",
        )?;
        let rows: Vec<Value> = st
            .query_map(params![user_id, before, limit.clamp(1, 1000)], |r| {
                Ok(json!({ "id": r.get::<_, i64>(0)?, "ts": r.get::<_, i64>(1)?, "user_id": r.get::<_, Option<i64>>(2)?,
                           "username": r.get::<_, Option<String>>(3)?, "route": r.get::<_, String>(4)?, "query": r.get::<_, String>(5)?,
                           "status": r.get::<_, i64>(6)?, "ms": r.get::<_, f64>(7)?, "cache": r.get::<_, bool>(8)?, "ip": r.get::<_, Option<String>>(9)? }))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Value::Array(rows))
    }

    pub fn usage_report(&self, days: i64) -> Result<Value> {
        let today = day_of(now());
        let since = today - days.clamp(1, 400) + 1;
        let (m0, _) = month_bounds(today);
        let conn = self.conn();
        let daily: Vec<Value> = conn
            .prepare(
                "SELECT day, SUM(count), COUNT(DISTINCT user_id), SUM(cache_hits), SUM(errors) FROM usage_daily WHERE day >= ?1 GROUP BY day ORDER BY day",
            )?
            .query_map([since], |r| {
                Ok(json!({ "day": day_string(r.get(0)?), "requests": r.get::<_, i64>(1)?, "users": r.get::<_, i64>(2)?,
                           "cache_hits": r.get::<_, i64>(3)?, "errors": r.get::<_, i64>(4)? }))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let by_kind: serde_json::Map<String, Value> = conn
            .prepare("SELECT kind, SUM(count) FROM usage_daily WHERE day >= ?1 GROUP BY kind ORDER BY 2 DESC")?
            .query_map([since], |r| Ok((r.get::<_, String>(0)?, json!(r.get::<_, i64>(1)?))))?
            .collect::<rusqlite::Result<_>>()?;
        let top: Vec<Value> = conn
            .prepare(
                "SELECT d.user_id, COALESCE(u.username, CASE d.user_id WHEN 0 THEN 'anonymous' END), SUM(d.count) AS n FROM usage_daily d
                 LEFT JOIN users u ON u.id = d.user_id WHERE d.day = ?1 GROUP BY d.user_id ORDER BY n DESC LIMIT 20",
            )?
            .query_map([today], |r| {
                Ok(json!({ "id": r.get::<_, i64>(0)?, "username": r.get::<_, Option<String>>(1)?, "count": r.get::<_, i64>(2)? }))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let users: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
        let (active, req_today): (i64, i64) =
            conn.query_row("SELECT COUNT(DISTINCT user_id), COALESCE(SUM(count), 0) FROM usage_daily WHERE day = ?1", [today], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        let req_month: i64 = conn.query_row("SELECT COALESCE(SUM(count), 0) FROM usage_daily WHERE day >= ?1", [m0], |r| r.get(0))?;
        let by_status: serde_json::Map<String, Value> = conn
            .prepare("SELECT status, COUNT(*) FROM users GROUP BY status")?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, json!(r.get::<_, i64>(1)?))))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(json!({
            "daily": daily, "by_kind": by_kind, "top_users_today": top,
            "totals": { "users": users, "users_by_status": by_status, "active_users_today": active, "requests_today": req_today, "requests_month": req_month },
            "note": "usage is flushed from memory every 5 seconds",
        }))
    }

    /// `VACUUM INTO`: a consistent copy taken while readers and writers keep going.
    pub fn backup(&self, out: &Path) -> Result<()> {
        anyhow::ensure!(!out.exists(), "{} already exists", out.display());
        self.conn().execute("VACUUM INTO ?1", [out.to_string_lossy()])?;
        Ok(())
    }

    pub fn audit(&self, actor: &Actor, action: &str, target: Option<i64>, details: Value) {
        let target_name =
            target.and_then(|id| self.conn().query_row("SELECT username FROM users WHERE id = ?1", [id], |r| r.get::<_, String>(0)).ok());
        let r = self.conn().execute(
            "INSERT INTO audit (ts, actor_id, actor, action, target_id, target, details, ip) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![now(), actor.id, actor.name, action, target, target_name, details.to_string(), actor.ip],
        );
        if let Err(e) = r {
            eprintln!("audit log write failed: {e}");
        }
    }

    pub fn audit_list(&self, actor_id: Option<i64>, target_id: Option<i64>, limit: i64, before: Option<i64>) -> Result<Value> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT id, ts, actor_id, actor, action, target_id, target, details, ip FROM audit
             WHERE (?1 IS NULL OR actor_id = ?1) AND (?2 IS NULL OR target_id = ?2) AND (?3 IS NULL OR id < ?3) ORDER BY id DESC LIMIT ?4",
        )?;
        let rows: Vec<Value> = st
            .query_map(params![actor_id, target_id, before, limit.clamp(1, 1000)], |r| {
                let details: String = r.get(7)?;
                Ok(json!({ "id": r.get::<_, i64>(0)?, "ts": r.get::<_, i64>(1)?, "actor_id": r.get::<_, Option<i64>>(2)?,
                           "actor": r.get::<_, Option<String>>(3)?, "action": r.get::<_, String>(4)?, "target_id": r.get::<_, Option<i64>>(5)?,
                           "target": r.get::<_, Option<String>>(6)?, "details": serde_json::from_str::<Value>(&details).unwrap_or(Value::String(details)),
                           "ip": r.get::<_, Option<String>>(8)? }))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Value::Array(rows))
    }
}

fn check_plan(p: &Plan) -> Result<()> {
    let name_ok =
        (1..=40).contains(&p.name.len()) && p.name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_".contains(&b));
    let non_neg = [p.daily_limit, p.monthly_limit, p.per_minute, p.burst, p.max_export_rows].iter().flatten().all(|x| *x >= 0);
    if !name_ok || !non_neg || !(1..=100_000).contains(&p.max_page_size) {
        return Err(denied(400, "invalid_request", "plan name must be 1-40 of a-z 0-9 - _, limits non-negative, max_page_size 1..100000"));
    }
    Ok(())
}

fn new_key() -> Result<(String, String, String)> {
    let prefix = random_b62(KEY_PREFIX_LEN);
    let secret = random_b62(KEY_SECRET_LEN);
    let hash = hash_secret(&secret)?;
    Ok((prefix, secret, hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Accounts) {
        let d = tempfile::tempdir().unwrap();
        let a = Accounts::open(&d.path().join("a.sqlite"), 24).unwrap();
        (d, a)
    }

    #[test]
    fn calendar() {
        for day in [-800_000i64, -1, 0, 1, 59, 10_957, 20_000, 20_759, 2_000_000] {
            let (y, m, d) = civil(day);
            assert_eq!(days_from_civil(y, m, d), day);
        }
        assert_eq!(day_string(0), "1970-01-01");
        assert_eq!(day_string(days_from_civil(2024, 2, 29)), "2024-02-29");
        let (a, b) = month_bounds(days_from_civil(2026, 12, 15));
        assert_eq!((day_string(a), day_string(b)), ("2026-12-01".into(), "2027-01-01".into()));
    }

    #[test]
    fn keys_are_well_formed_and_verified() {
        let k = format!("ocnpj_{}_{}", random_b62(10), random_b62(32));
        assert!(split_key(&k).is_some());
        assert!(split_key("ocnpj_short_x").is_none());
        let h = hash_secret("secret-value").unwrap();
        assert!(h.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
        assert!(verify_secret("secret-value", &h));
        assert!(!verify_secret("secret-valuf", &h));
    }

    #[test]
    fn verified_key_handover() {
        let (d, a) = store();
        let sys = Actor::system("test");
        let (u1, k1) = a.create_user("one", "one@example.com", Role::User, None, Status::Active, &sys).unwrap();
        let (_, k2) = a.create_user("two", "two@example.com", Role::User, None, Status::Active, &sys).unwrap();
        a.verify_key(&k1).unwrap().unwrap();
        a.verify_key(&k2).unwrap().unwrap();
        let path = d.path().join("handover");
        assert_eq!(a.export_verified(&path).unwrap(), 2);
        let b = Accounts::open(&d.path().join("a.sqlite"), 24).unwrap();
        b.rotate_key(u1.id, &sys).unwrap();
        assert_eq!(b.import_verified(&path).unwrap(), 1, "revoked keys are skipped");
        assert!(!path.exists(), "consumed");
        assert!(b.cached_key(&k2).is_some() && b.cached_key(&k1).is_none());
    }

    #[test]
    fn lifecycle() {
        let (_d, a) = store();
        let sys = Actor::system("test");
        let (admin, _) = a.create_user("root", "root@example.com", Role::Admin, None, Status::Active, &sys).unwrap();
        assert_eq!(admin.plan, "unlimited");
        let (u, key) = a.register("Ana", "ANA@example.com", None).unwrap().unwrap();
        assert_eq!(
            (u.username.as_str(), u.email.as_str(), u.status.as_str(), u.plan.as_str()),
            ("ana", "ana@example.com", "pending", "free")
        );
        assert!(a.register("other", "ana@example.com", None).unwrap().is_none(), "duplicate e-mail is silent");
        let e = a.register("ana", "x@example.com", None).unwrap_err();
        assert_eq!(e.downcast_ref::<Denied>().unwrap().code, "username_taken");
        assert_eq!(a.register("a b", "y@example.com", None).unwrap_err().downcast_ref::<Denied>().unwrap().code, "invalid_username");
        assert_eq!(a.register("bob", "no-at-sign", None).unwrap_err().downcast_ref::<Denied>().unwrap().code, "invalid_email");

        let (token, _, me) = a.login(&key, None, None).unwrap();
        assert_eq!(me.status, "active");
        assert!(me.verified_at.is_some());
        assert_eq!(a.session(&token).unwrap().unwrap().0, u.id);
        assert!(a.login("ocnpj_0000000000_00000000000000000000000000000000", None, None).is_err());

        let new = a.rotate_key(u.id, &sys).unwrap();
        assert!(a.verify_key(&key).unwrap().is_none(), "old key revoked");
        assert!(a.session(&token).unwrap().is_none(), "sessions revoked");
        assert!(a.verify_key(&new).unwrap().is_some());

        let (_, rt) = a.start_recovery("ana@example.com").unwrap().unwrap();
        let (_, recovered) = a.confirm_recovery(&rt, None).unwrap();
        assert!(a.verify_key(&new).unwrap().is_none());
        assert!(a.verify_key(&recovered).unwrap().is_some());
        assert!(a.confirm_recovery(&rt, None).is_err(), "single use");
        assert!(a.start_recovery("nobody@example.com").unwrap().is_none());

        let p = a.principal(u.id).unwrap().unwrap();
        assert_eq!((p.daily_limit, p.per_minute), (Some(10), Some(60)));
        a.update_user(u.id, &json!({ "daily_limit": 5, "plan": "pro", "notes": "vip" }), &sys).unwrap();
        let p = a.principal(u.id).unwrap().unwrap();
        assert_eq!((p.daily_limit, p.per_minute, p.plan.name.as_str()), (Some(5), Some(600), "pro"));

        a.set_status(u.id, Status::Suspended, Some("abuse"), None, &sys).unwrap();
        let e = a.principal(u.id).unwrap().unwrap().check_status(now()).unwrap_err();
        assert_eq!(e.downcast_ref::<Denied>().unwrap().code, "account_suspended");
        a.set_status(u.id, Status::Suspended, Some("cool down"), Some(now() - 1), &sys).unwrap();
        assert!(a.principal(u.id).unwrap().unwrap().check_status(now()).is_ok(), "expired suspension");

        let root = a.principal(admin.id).unwrap().unwrap();
        assert_eq!(
            a.update_user(admin.id, &json!({ "role": "user" }), &sys).unwrap_err().downcast_ref::<Denied>().unwrap().code,
            "last_admin"
        );
        assert!(a.check_target(&root, &a.user(admin.id).unwrap().unwrap(), true).is_err(), "cannot suspend self");

        let mut deltas = HashMap::new();
        let today = day_of(now());
        deltas.insert((u.id, today, "search"), UsageDelta { count: 3, cache_hits: 1, errors: 0, ms_total: 2.0 });
        deltas.insert((u.id, today - 40, "search"), UsageDelta { count: 7, ..Default::default() });
        a.flush_usage(&deltas).unwrap();
        a.flush_usage(&deltas).unwrap();
        assert_eq!(a.counts(u.id, today).unwrap().0, 6);
        let list = a.list_users(&UserFilter { q: Some("an".into()), sort: Some("-usage_today".into()), ..Default::default() }).unwrap();
        assert_eq!(list["total"], 1);
        assert_eq!(list["users"][0]["usage_today"], 6);
        let report = a.usage_report(30).unwrap();
        assert_eq!(report["totals"]["requests_today"], 6);

        a.insert_search_logs(&[SearchLogRow {
            ts: now(),
            user_id: Some(u.id),
            route: "/api/v1/search".into(),
            query: "q=x".into(),
            status: 200,
            ms: 1.0,
            cache: false,
            ip: None,
        }])
        .unwrap();
        assert_eq!(a.user_detail(u.id).unwrap()["recent_searches"].as_array().unwrap().len(), 1);
        assert!(a.audit_list(None, Some(u.id), 100, None).unwrap().as_array().unwrap().len() >= 5);

        let pro = a.plan("pro").unwrap().id;
        assert_eq!(a.delete_plan(pro, &sys).unwrap_err().downcast_ref::<Denied>().unwrap().code, "plan_in_use");
        a.delete_user(u.id, &sys).unwrap();
        a.delete_plan(pro, &sys).unwrap();
        assert!(a.plan("pro").is_none());

        let s = a.patch_settings(&json!({ "registration": "closed" }), &sys).unwrap();
        assert_eq!(s.registration, "closed");
        assert_eq!(a.register("zed", "zed@example.com", None).unwrap_err().downcast_ref::<Denied>().unwrap().code, "registration_closed");
    }
}
