//! Request gate for data routes: authentication, plan checks, token-bucket rate limits,
//! daily/monthly quotas, usage accounting, the search log, and a shared response cache with
//! request coalescing. Everything on the hot path is in memory; SQLite is written in batches
//! by background tasks.

// Errors here are ready-made HTTP responses returned on cold paths (rejections).
#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, MatchedPath, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::accounts::{self, Accounts, Principal, SearchLogRow, UsageDelta};
use crate::api::Shared;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum AuthMode {
    /// No accounts: every route is open (benchmarks, private installs).
    Off,
    /// Credentials are optional; anonymous callers use the `anonymous` plan per IP.
    Optional,
    /// A small persistent browser trial followed by account plans.
    Trial,
    /// Every data route needs a key or session token.
    Required,
}

impl AuthMode {
    pub fn name(self) -> &'static str {
        match self {
            AuthMode::Off => "off",
            AuthMode::Optional => "optional",
            AuthMode::Trial => "trial",
            AuthMode::Required => "required",
        }
    }
}

/// What a handler wants written to the search log instead of the URL query (the question
/// of an assistant turn). Attached to the response.
#[derive(Clone)]
pub struct LoggedQuery(pub String);

/// The authenticated caller, attached to the request for handlers.
#[derive(Clone)]
pub struct Caller {
    pub principal: Arc<Principal>,
    pub ip: IpAddr,
}

impl Caller {
    pub fn actor(&self) -> accounts::Actor {
        accounts::Actor { id: Some(self.principal.id), name: self.principal.username.clone(), ip: Some(self.ip.to_string()) }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Subject {
    User(i64),
    Ip(IpAddr),
}

struct Meter {
    day: i64,
    day_count: i64,
    month_start: i64,
    month_count: i64,
    tokens: f64,
    at: Instant,
}

struct Admission {
    per_minute: Option<(i64, i64)>,
    daily: Option<(i64, i64)>,
    monthly: Option<(i64, i64)>,
}

// ---------------------------------------------------------------------------------------
// Response cache.
// ---------------------------------------------------------------------------------------

pub struct Cached {
    status: StatusCode,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: Bytes,
    etag: HeaderValue,
}

pub struct ResponseCache {
    inner: moka::future::Cache<u128, Arc<Cached>>,
    budget: u64,
    max_entry: usize,
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub coalesced: AtomicU64,
    pub not_modified: AtomicU64,
}

impl ResponseCache {
    pub fn new(budget_mib: u64, ttl_secs: u64) -> ResponseCache {
        let budget = budget_mib << 20;
        ResponseCache {
            inner: moka::future::Cache::builder()
                .max_capacity(budget)
                .weigher(|_k: &u128, v: &Arc<Cached>| u32::try_from(v.body.len() + 256).unwrap_or(u32::MAX))
                .time_to_live(Duration::from_secs(ttl_secs.max(1)))
                .build(),
            budget,
            // One entry may use at most 1/16 of the budget (and never more than 8 MiB).
            max_entry: usize::try_from((budget / 16).min(8 << 20)).unwrap_or(8 << 20),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            coalesced: AtomicU64::new(0),
            not_modified: AtomicU64::new(0),
        }
    }

    pub fn clear(&self) {
        self.inner.invalidate_all();
    }

    /// Writes every entry to `path` (handover to the next process; keys include the
    /// snapshot identity, so entries for another snapshot are never served).
    pub fn export(&self, path: &std::path::Path) -> anyhow::Result<usize> {
        use std::io::Write;
        let tmp = path.with_extension("tmp");
        let mut w = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        w.write_all(b"OCRC1")?;
        let mut n = 0;
        let field = |w: &mut std::io::BufWriter<std::fs::File>, b: &[u8]| -> std::io::Result<()> {
            w.write_all(&u32::try_from(b.len()).unwrap_or(u32::MAX).to_le_bytes())?;
            w.write_all(b)
        };
        for (k, v) in self.inner.iter() {
            w.write_all(&k.to_le_bytes())?;
            w.write_all(&v.status.as_u16().to_le_bytes())?;
            w.write_all(&[u8::try_from(v.headers.len()).unwrap_or(0)])?;
            for (name, value) in v.headers.iter().take(255) {
                field(&mut w, name.as_str().as_bytes())?;
                field(&mut w, value.as_bytes())?;
            }
            field(&mut w, v.etag.as_bytes())?;
            field(&mut w, &v.body)?;
            n += 1;
        }
        w.flush()?;
        drop(w);
        std::fs::rename(&tmp, path)?;
        Ok(n)
    }

    /// Reads (and deletes) a handover file; malformed input stops the import, never panics.
    pub async fn import(&self, path: &std::path::Path) -> anyhow::Result<usize> {
        let p = path.to_path_buf();
        let entries = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<(u128, Arc<Cached>)>> {
            let data = std::fs::read(&p)?;
            let _ = std::fs::remove_file(&p);
            let mut out = Vec::new();
            let mut r = data.strip_prefix(b"OCRC1").ok_or_else(|| anyhow::anyhow!("not a response cache handover"))?;
            fn take<'a>(r: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
                let (a, b) = (r.get(..n)?, r.get(n..)?);
                *r = b;
                Some(a)
            }
            fn field<'a>(r: &mut &'a [u8]) -> Option<&'a [u8]> {
                let n = u32::from_le_bytes(take(r, 4)?.try_into().ok()?) as usize;
                take(r, n)
            }
            let one = |r: &mut &[u8]| -> Option<(u128, Arc<Cached>)> {
                let key = u128::from_le_bytes(take(r, 16)?.try_into().ok()?);
                let status = StatusCode::from_u16(u16::from_le_bytes(take(r, 2)?.try_into().ok()?)).ok()?;
                let nh = take(r, 1)?[0];
                let mut headers = Vec::new();
                for _ in 0..nh {
                    let name = HeaderName::from_bytes(field(r)?).ok()?;
                    let value = HeaderValue::from_bytes(field(r)?).ok()?;
                    headers.push((name, value));
                }
                let etag = HeaderValue::from_bytes(field(r)?).ok()?;
                let body = Bytes::copy_from_slice(field(r)?);
                Some((key, Arc::new(Cached { status, headers, body, etag })))
            };
            while !r.is_empty() {
                match one(&mut r) {
                    Some(e) => out.push(e),
                    None => break,
                }
            }
            Ok(out)
        })
        .await??;
        let n = entries.len();
        for (k, v) in entries {
            self.inner.insert(k, v).await;
        }
        Ok(n)
    }

    pub fn stats(&self) -> serde_json::Value {
        json!({
            "entries": self.inner.entry_count(), "bytes": self.inner.weighted_size(), "budget": self.budget,
            "hits": self.hits.load(Relaxed), "misses": self.misses.load(Relaxed), "coalesced": self.coalesced.load(Relaxed),
            "not_modified": self.not_modified.load(Relaxed),
        })
    }
}

// ---------------------------------------------------------------------------------------
// Gate state.
// ---------------------------------------------------------------------------------------

pub struct Gate {
    pub mode: AuthMode,
    pub accounts: Option<Arc<Accounts>>,
    pub cache: Option<ResponseCache>,
    pub trust_proxy: bool,
    meters: moka::sync::Cache<Subject, Arc<Mutex<Meter>>>,
    pending: Mutex<HashMap<(i64, i64, &'static str), UsageDelta>>,
    log_tx: Mutex<Option<std::sync::mpsc::SyncSender<SearchLogRow>>>,
    failures: moka::sync::Cache<IpAddr, Arc<AtomicU32>>,
    /// Bounds concurrent Argon2 verifications (19 MiB each).
    pub argon: tokio::sync::Semaphore,
    throttles: moka::sync::Cache<(IpAddr, &'static str), Arc<AtomicU32>>,
    handover: Option<std::path::PathBuf>,
    /// Set once this process has handed its state over: it must not take it back.
    leaving: std::sync::atomic::AtomicBool,
    pub dropped_logs: AtomicU64,
    pub rejected_rate: AtomicU64,
    pub rejected_quota: AtomicU64,
    pub rejected_auth: AtomicU64,
}

/// Failed credential attempts allowed per IP per minute.
const MAX_FAILURES: u32 = 30;

impl Gate {
    pub fn new(mode: AuthMode, accounts: Option<Arc<Accounts>>, cache: Option<ResponseCache>, trust_proxy: bool) -> Gate {
        Gate {
            mode,
            accounts,
            cache,
            trust_proxy,
            meters: moka::sync::Cache::builder().max_capacity(2_000_000).time_to_idle(Duration::from_secs(2 * 86_400)).build(),
            pending: Mutex::new(HashMap::new()),
            log_tx: Mutex::new(None),
            failures: moka::sync::Cache::builder().max_capacity(1_000_000).time_to_live(Duration::from_secs(60)).build(),
            argon: tokio::sync::Semaphore::new(crate::default_threads().max(2)),
            throttles: moka::sync::Cache::builder().max_capacity(1_000_000).time_to_live(Duration::from_secs(3600)).build(),
            handover: None,
            leaving: std::sync::atomic::AtomicBool::new(false),
            dropped_logs: AtomicU64::new(0),
            rejected_rate: AtomicU64::new(0),
            rejected_quota: AtomicU64::new(0),
            rejected_auth: AtomicU64::new(0),
        }
    }

    pub fn enabled(&self) -> bool {
        self.mode != AuthMode::Off && self.accounts.is_some()
    }

    pub fn stats(&self) -> serde_json::Value {
        json!({
            "auth": self.mode.name(), "meters": self.meters.entry_count(),
            "rejected": { "rate_limited": self.rejected_rate.load(Relaxed), "quota_exceeded": self.rejected_quota.load(Relaxed), "auth": self.rejected_auth.load(Relaxed) },
            "search_log_dropped": self.dropped_logs.load(Relaxed),
            "pending_usage_rows": self.pending.lock().map(|p| p.len()).unwrap_or(0),
        })
    }

    /// Counts an unauthenticated account action from `ip`; `false` once `per_hour` is used.
    pub fn throttle(&self, ip: IpAddr, action: &'static str, per_hour: u32) -> bool {
        self.throttles.get_with((ip, action), || Arc::new(AtomicU32::new(0))).fetch_add(1, Relaxed) < per_hour
    }

    /// Live `(today, month)` request counts of a user, when metered by this process.
    pub fn usage_now(&self, user_id: i64) -> Option<(i64, i64)> {
        let m = self.meters.get(&Subject::User(user_id))?;
        let m = m.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        (m.day == accounts::day_of(accounts::now())).then_some((m.day_count, m.month_count))
    }

    /// Forget the in-memory counters of a user (after an admin reset).
    pub fn forget(&self, user_id: i64) {
        self.meters.invalidate(&Subject::User(user_id));
    }

    /// Resolves the credential in `headers`. `Ok(None)`: no credential sent.
    pub async fn authenticate(&self, headers: &HeaderMap, ip: IpAddr) -> Result<Option<Arc<Principal>>, Response> {
        let Some(acc) = self.accounts.clone() else { return Ok(None) };
        let Some(cred) = credential(headers) else { return Ok(None) };
        if self.failures.get(&ip).is_some_and(|n| n.load(Relaxed) >= MAX_FAILURES) {
            self.rejected_auth.fetch_add(1, Relaxed);
            return Err(error(
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_failures",
                "too many invalid credentials; retry in a minute",
                &[(header::RETRY_AFTER, HeaderValue::from_static("60"))],
            ));
        }
        let user_id = if cred.starts_with("ocs_") {
            let r = match acc.cached_session(&cred) {
                Some(r) => r,
                None => {
                    let (a, c) = (acc.clone(), cred.clone());
                    blocking(move || a.session(&c)).await?
                }
            };
            r.map(|x| x.0)
        } else {
            match acc.cached_key(&cred) {
                Some(r) => Some(r.0),
                None => {
                    let _permit = self.argon.acquire().await.map_err(|_| internal("closed"))?;
                    let (a, c) = (acc.clone(), cred.clone());
                    blocking(move || a.verify_key(&c)).await?.map(|x| x.0)
                }
            }
        };
        let Some(user_id) = user_id else {
            self.failures.get_with(ip, || Arc::new(AtomicU32::new(0))).fetch_add(1, Relaxed);
            self.rejected_auth.fetch_add(1, Relaxed);
            return Err(unauthorized("invalid_credentials", "invalid or expired key or session token"));
        };
        let p = match acc.cached_principal(user_id) {
            Some(p) => p,
            None => {
                let a = acc.clone();
                blocking(move || a.principal(user_id))
                    .await?
                    .ok_or_else(|| unauthorized("invalid_credentials", "account no longer exists"))?
            }
        };
        if p.status == accounts::Status::Pending {
            let a = acc.clone();
            blocking(move || a.activate_if_pending(user_id)).await?;
        }
        if let Err(e) = p.check_status(accounts::now()) {
            self.rejected_auth.fetch_add(1, Relaxed);
            return Err(crate::api::ApiError::from(e).into_response());
        }
        Ok(Some(p))
    }

    async fn admit(&self, subject: Subject, p: &Principal) -> Result<Admission, Response> {
        let today = accounts::day_of(accounts::now());
        let meter = match self.meters.get(&subject) {
            Some(m) => m,
            None => {
                let (d, m) = match (&subject, &self.accounts) {
                    (Subject::User(id), Some(acc)) => {
                        let (a, id) = (acc.clone(), *id);
                        blocking(move || a.counts(id, today)).await?
                    }
                    _ => (0, 0),
                };
                self.meters.get_with(subject, || {
                    Arc::new(Mutex::new(Meter {
                        day: today,
                        day_count: d,
                        month_start: accounts::month_bounds(today).0,
                        month_count: m,
                        tokens: f64::MAX,
                        at: Instant::now(),
                    }))
                })
            }
        };
        let mut m = meter.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if m.day != today {
            m.day = today;
            m.day_count = 0;
            let (m0, _) = accounts::month_bounds(today);
            if m0 != m.month_start {
                m.month_start = m0;
                m.month_count = 0;
            }
        }
        let mut rate = None;
        if let Some(pm) = p.per_minute {
            let pm = pm.max(1);
            let cap = p.plan.burst.filter(|b| *b > 0).unwrap_or(pm) as f64;
            let now = Instant::now();
            let refill = now.duration_since(m.at).as_secs_f64() * pm as f64 / 60.0;
            m.tokens = (if m.tokens == f64::MAX { cap } else { m.tokens + refill }).min(cap);
            m.at = now;
            if m.tokens < 1.0 {
                self.rejected_rate.fetch_add(1, Relaxed);
                let wait = ((1.0 - m.tokens) * 60.0 / pm as f64).ceil().max(1.0) as u64;
                return Err(limited("rate_limited", format!("rate limit of {pm} requests per minute exceeded"), wait, pm, 0));
            }
            rate = Some(pm);
        }
        let now_s = accounts::now();
        if let Some(d) = p.daily_limit
            && m.day_count >= d
        {
            self.rejected_quota.fetch_add(1, Relaxed);
            let wait = (today + 1) * 86_400 - now_s;
            return Err(limited(
                "quota_exceeded",
                format!("daily quota of {d} requests used up; resets at 00:00 UTC"),
                wait.max(1) as u64,
                0,
                0,
            ));
        }
        if let Some(mo) = p.monthly_limit
            && m.month_count >= mo
        {
            self.rejected_quota.fetch_add(1, Relaxed);
            let wait = accounts::month_bounds(today).1 * 86_400 - now_s;
            return Err(limited("quota_exceeded", format!("monthly quota of {mo} requests used up"), wait.max(1) as u64, 0, 0));
        }
        if rate.is_some() {
            m.tokens -= 1.0;
        }
        m.day_count += 1;
        m.month_count += 1;
        Ok(Admission {
            per_minute: rate.map(|pm| (pm, m.tokens.floor() as i64)),
            daily: p.daily_limit.map(|d| (d, d - m.day_count)),
            monthly: p.monthly_limit.map(|x| (x, x - m.month_count)),
        })
    }

    fn refund(&self, subject: &Subject) {
        if let Some(m) = self.meters.get(subject) {
            let mut m = m.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            m.day_count = (m.day_count - 1).max(0);
            m.month_count = (m.month_count - 1).max(0);
        }
    }

    fn record(&self, user: i64, kind: &'static str, status: u16, cache_hit: bool, ms: f64) {
        let day = accounts::day_of(accounts::now());
        let mut p = self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let d = p.entry((user, day, kind)).or_default();
        if status < 500 {
            d.count += 1;
        }
        d.cache_hits += i64::from(cache_hit);
        d.errors += i64::from(status >= 400);
        d.ms_total += ms;
    }

    fn log(&self, row: SearchLogRow) {
        let tx = self.log_tx.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(tx) = tx.as_ref()
            && tx.try_send(row).is_err()
        {
            self.dropped_logs.fetch_add(1, Relaxed);
        }
    }

    pub fn set_handover_path(&mut self, p: std::path::PathBuf) {
        self.handover = Some(p);
    }

    fn cache_handover(&self) -> Option<std::path::PathBuf> {
        self.handover.as_ref().map(|p| p.with_file_name(".response-cache"))
    }

    /// Imports verified keys left by a process that just stopped (no Argon2 storm after a
    /// blue/green restart).
    pub fn take_handover(&self) {
        if let (Some(acc), Some(p)) = (&self.accounts, &self.handover)
            && !self.leaving.load(Relaxed)
            && p.exists()
        {
            match acc.import_verified(p) {
                Ok(n) if n > 0 => eprintln!("imported {n} verified keys from the previous process"),
                Err(e) => eprintln!("key handover import failed: {e:#}"),
                _ => {}
            }
        }
    }

    /// Imports the response cache left by a process that just stopped.
    pub async fn take_cache_handover(&self) {
        if let (Some(c), Some(p)) = (&self.cache, self.cache_handover())
            && !self.leaving.load(Relaxed)
            && p.exists()
        {
            match c.import(&p).await {
                Ok(n) => eprintln!("imported {n} cached responses from the previous process"),
                Err(e) => eprintln!("response cache handover import failed: {e:#}"),
            }
        }
    }

    /// Called when shutdown starts: verified keys and cached responses go to the successor.
    pub fn give_handover(&self) {
        self.leaving.store(true, Relaxed);
        if let (Some(acc), Some(p)) = (&self.accounts, &self.handover) {
            match acc.export_verified(p) {
                Ok(n) => eprintln!("handed {n} verified keys over to the next process"),
                Err(e) => eprintln!("key handover export failed: {e:#}"),
            }
        }
        if let (Some(c), Some(p)) = (&self.cache, self.cache_handover()) {
            match c.export(&p) {
                Ok(n) => eprintln!("handed {n} cached responses over to the next process"),
                Err(e) => eprintln!("response cache handover export failed: {e:#}"),
            }
        }
    }

    /// Writes pending usage increments to SQLite.
    pub fn flush(&self) {
        let Some(acc) = &self.accounts else { return };
        let batch = std::mem::take(&mut *self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
        if let Err(e) = acc.flush_usage(&batch) {
            eprintln!("usage flush failed ({e:#}); retrying later");
            let mut p = self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            for (k, d) in batch {
                let x = p.entry(k).or_default();
                x.count += d.count;
                x.cache_hits += d.cache_hits;
                x.errors += d.errors;
                x.ms_total += d.ms_total;
            }
        }
    }

    /// Starts the search-log writer thread and the periodic usage flush / purge task.
    pub fn start_background(self: &Arc<Self>) {
        if self.accounts.is_none() && self.handover.is_some() {
            let g = self.clone();
            tokio::spawn(async move {
                loop {
                    g.take_cache_handover().await;
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
            });
        }
        let Some(acc) = self.accounts.clone() else { return };
        let (tx, rx) = std::sync::mpsc::sync_channel::<SearchLogRow>(50_000);
        *self.log_tx.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(tx);
        let a = acc.clone();
        std::thread::Builder::new()
            .name("search-log".into())
            .spawn(move || {
                let mut buf = Vec::with_capacity(1000);
                loop {
                    match rx.recv_timeout(Duration::from_secs(1)) {
                        Ok(r) => buf.push(r),
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                    while buf.len() < 1000
                        && let Ok(r) = rx.try_recv()
                    {
                        buf.push(r);
                    }
                    if !buf.is_empty() {
                        if let Err(e) = a.insert_search_logs(&buf) {
                            eprintln!("search log write failed: {e:#}");
                        }
                        buf.clear();
                    }
                }
            })
            .expect("spawn search-log thread");
        if let Some(p) = self.handover.clone() {
            let g = self.clone();
            tokio::spawn(async move {
                loop {
                    if p.exists() {
                        let g2 = g.clone();
                        let _ = tokio::task::spawn_blocking(move || g2.take_handover()).await;
                    }
                    g.take_cache_handover().await;
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
            });
        }
        let g = self.clone();
        tokio::spawn(async move {
            let mut tick = 0u64;
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                tick += 1;
                let g2 = g.clone();
                let purge = tick % 720 == 1;
                let _ = tokio::task::spawn_blocking(move || {
                    g2.flush();
                    if purge && let Some(a) = &g2.accounts {
                        match a.purge() {
                            Ok(n) if n > 0 => eprintln!("purged {n} expired log rows and credentials"),
                            Err(e) => eprintln!("purge failed: {e:#}"),
                            _ => {}
                        }
                    }
                })
                .await;
            }
        });
    }
}

// ---------------------------------------------------------------------------------------
// Helpers.
// ---------------------------------------------------------------------------------------

fn credential(h: &HeaderMap) -> Option<String> {
    if let Some(v) = h.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        let v = v.trim();
        let t = v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")).unwrap_or(v).trim();
        return (!t.is_empty() && t.len() <= 200).then(|| t.to_string());
    }
    h.get("x-api-key").and_then(|v| v.to_str().ok()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s.len() <= 200)
}

pub fn client_ip(req: &Request, trust_proxy: bool) -> IpAddr {
    if trust_proxy
        && let Some(ip) = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .and_then(|s| s.trim().parse::<IpAddr>().ok())
    {
        return ip;
    }
    req.extensions().get::<ConnectInfo<SocketAddr>>().map_or(IpAddr::V4(Ipv4Addr::LOCALHOST), |c| c.0.ip())
}

pub(crate) async fn blocking<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Result<T, Response> {
    match tokio::task::spawn_blocking(f).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(crate::api::ApiError::from(e).into_response()),
        Err(e) => Err(internal(&e.to_string())),
    }
}

pub fn error(status: StatusCode, code: &str, message: &str, headers: &[(HeaderName, HeaderValue)]) -> Response {
    let mut r = (status, axum::Json(json!({ "error": code, "message": message, "code": status.as_u16() }))).into_response();
    for (k, v) in headers {
        r.headers_mut().insert(k.clone(), v.clone());
    }
    r
}

fn internal(m: &str) -> Response {
    error(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", m, &[])
}

pub fn unauthorized(code: &str, message: &str) -> Response {
    error(StatusCode::UNAUTHORIZED, code, message, &[(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer realm=\"opencnpj\""))])
}

fn limited(code: &str, message: String, retry_after: u64, limit: i64, remaining: i64) -> Response {
    let mut h = vec![(header::RETRY_AFTER, HeaderValue::from(retry_after))];
    if limit > 0 {
        h.push((HeaderName::from_static("x-ratelimit-limit"), HeaderValue::from(limit)));
        h.push((HeaderName::from_static("x-ratelimit-remaining"), HeaderValue::from(remaining)));
    }
    error(StatusCode::TOO_MANY_REQUESTS, code, &message, &h)
}

/// Usage category of a data route.
pub fn kind_of(route: &str) -> &'static str {
    if route == "/api/v1/assistant/export" {
        "export"
    } else if route.starts_with("/api/v1/assistant") {
        "assistant"
    } else if route.starts_with("/api/v1/cpf") {
        "cpf"
    } else if route.ends_with("/search") || route == "/api/v1/busca" {
        "search"
    } else if route == "/api/v1/stats" {
        "stats"
    } else if route == "/api/v1/export" {
        "export"
    } else if route.starts_with("/downloads") {
        "download"
    } else {
        "lookup"
    }
}

/// Plan rules that depend on the request: export, downloads, page size.
fn plan_check(p: &Principal, kind: &str, uri: &axum::http::Uri, preview: bool) -> Result<(), Response> {
    let plan = &p.plan;
    if kind == "export" && (preview || !plan.export) {
        return Err(error(StatusCode::FORBIDDEN, "plan_forbids_export", &format!("the {} plan does not include export", plan.name), &[]));
    }
    if kind == "download" && (preview || !plan.downloads) {
        return Err(error(
            StatusCode::FORBIDDEN,
            "plan_forbids_downloads",
            &format!("the {} plan does not include snapshot downloads", plan.name),
            &[],
        ));
    }
    let pairs = axum::extract::Query::<Vec<(String, String)>>::try_from_uri(uri).map(|query| query.0).unwrap_or_default();
    for (key, value) in pairs {
        if matches!(key.as_str(), "limit" | "estab_limit") && value.parse::<u64>().is_ok_and(|limit| limit > plan.max_page_size as u64) {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "limit_exceeds_plan",
                &format!("{key} above {} (the {} plan maximum)", plan.max_page_size, plan.name),
                &[],
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Middleware.
// ---------------------------------------------------------------------------------------

/// Authentication, plan checks, limits and accounting for data routes.
pub async fn gate(State(st): State<Shared>, mut req: Request, next: Next) -> Response {
    let g = &st.gate;
    if !g.enabled() {
        return next.run(req).await;
    }
    let route = req.extensions().get::<MatchedPath>().map_or_else(|| req.uri().path().to_string(), |m| m.as_str().to_string());
    let kind = kind_of(&route);
    let ip = client_ip(&req, g.trust_proxy);
    let query = req.uri().query().unwrap_or("").to_string();
    let principal = match g.authenticate(req.headers(), ip).await {
        Ok(Some(p)) => p,
        Ok(None) if matches!(g.mode, AuthMode::Optional | AuthMode::Trial) => {
            let Some(plan) = g.accounts.as_ref().and_then(|a| a.plan("anonymous")) else {
                return unauthorized("unauthenticated", "anonymous plan is missing");
            };
            Arc::new(Principal::anonymous(plan))
        }
        Ok(None) => {
            return unauthorized(
                "unauthenticated",
                "send `Authorization: Bearer <key>`; register at POST /api/v1/auth/register to receive a key by e-mail",
            );
        }
        Err(r) => return r,
    };
    if let Err(r) = plan_check(&principal, kind, req.uri(), st.trial.is_some() && principal.id == 0) {
        return r;
    }
    let subject = if principal.id == 0 { Subject::Ip(ip) } else { Subject::User(principal.id) };
    let adm = match g.admit(subject.clone(), &principal).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let ticket = if let Some(trial) = &st.trial {
        use crate::trial::Activity;
        let activity = if principal.id == 0 {
            Some(Activity::Preview)
        } else if principal.role == accounts::Role::User && principal.plan.name == "free" {
            Some(Activity::Free)
        } else {
            None
        };
        if let Some(activity) = activity {
            let preview = matches!(activity, Activity::Preview);
            let ticket = match trial.ticket(req.headers(), ip, activity) {
                Ok(ticket) => Arc::new(ticket),
                Err(response) => {
                    g.refund(&subject);
                    return response;
                }
            };
            match trial.reserve(ticket.clone()).await {
                Ok(Some(remaining)) => Some((trial, ticket, preview, remaining)),
                Ok(None) => {
                    g.refund(&subject);
                    g.rejected_quota.fetch_add(1, Relaxed);
                    let (code, message) = if preview {
                        ("trial_exhausted", "free preview used up; create an account or sign in to continue")
                    } else {
                        ("free_access_limited", "shared free usage limit reached; see account plans for higher limits")
                    };
                    let mut response = error(StatusCode::TOO_MANY_REQUESTS, code, message, &[]);
                    ticket.decorate(&mut response, preview.then_some(0));
                    return response;
                }
                Err(response) => {
                    g.refund(&subject);
                    return response;
                }
            }
        } else {
            None
        }
    } else {
        None
    };
    req.extensions_mut().insert(Caller { principal: principal.clone(), ip });
    let t0 = Instant::now();
    let mut resp = next.run(req).await;
    let ms = t0.elapsed().as_secs_f64() * 1e3;
    let status = resp.status().as_u16();
    let hit = resp.headers().get("x-cache").is_some_and(|v| v != "MISS");
    if status >= 500 {
        g.refund(&subject);
    }
    if let Some((trial, ticket, preview, mut remaining)) = ticket {
        if status >= 500
            && let Ok(refunded) = trial.refund(ticket.clone()).await
        {
            remaining = refunded;
        }
        ticket.decorate(&mut resp, preview.then_some(remaining));
    }
    g.record(principal.id, kind, status, hit, ms);
    if g.accounts.as_ref().is_some_and(|a| a.settings_cached_search_log()) {
        g.log(SearchLogRow {
            ts: accounts::now(),
            user_id: (principal.id > 0).then_some(principal.id),
            route,
            query: resp.extensions().get::<LoggedQuery>().map_or(query, |q| q.0.clone()).chars().take(1000).collect(),
            status,
            ms,
            cache: hit,
            ip: Some(ip.to_string()),
        });
    }
    let h = resp.headers_mut();
    let mut put = |name: &'static str, v: i64| {
        h.insert(HeaderName::from_static(name), HeaderValue::from(v.max(0)));
    };
    if let Some((l, r)) = adm.per_minute {
        put("x-ratelimit-limit", l);
        put("x-ratelimit-remaining", r);
    }
    if let Some((l, r)) = adm.daily {
        put("x-quota-daily-limit", l);
        put("x-quota-daily-remaining", r);
    }
    if let Some((l, r)) = adm.monthly {
        put("x-quota-monthly-limit", l);
        put("x-quota-monthly-remaining", r);
    }
    resp
}

fn cache_key(st: &Shared, req: &Request) -> u128 {
    let mut pairs: Vec<(String, String)> = req
        .uri()
        .query()
        .map(|q| {
            q.split('&')
                .filter(|s| !s.is_empty())
                .map(|kv| {
                    let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
                    (k.to_string(), v.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    pairs.sort();
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    h.update(&st.generation().to_le_bytes());
    h.update(req.uri().path().as_bytes());
    for (k, v) in &pairs {
        h.update(&[0]);
        h.update(k.as_bytes());
        h.update(&[1]);
        h.update(v.as_bytes());
    }
    h.update(&[u8::from(st.privacy.hide_socios), u8::from(st.privacy.hide_contacts)]);
    if let Some(caller) = req.extensions().get::<Caller>() {
        h.update(&caller.principal.plan.max_page_size.to_le_bytes());
    }
    h.digest128()
}

/// Shared response cache for idempotent JSON routes. Identical concurrent requests are
/// coalesced into one computation; `If-None-Match` gets `304`.
pub async fn cache(State(st): State<Shared>, req: Request, next: Next) -> Response {
    let Some(c) = st.gate.cache.as_ref() else { return next.run(req).await };
    let key = cache_key(&st, &req);
    let etag = HeaderValue::from_str(&format!("\"{key:032x}\"")).expect("hex etag");
    let inm = req.headers().get(header::IF_NONE_MATCH).cloned();
    let private = st.gate.enabled();
    let finish = |entry: &Cached, state: &'static str| -> Response {
        if inm.as_ref() == Some(&entry.etag) && entry.status == StatusCode::OK {
            c.not_modified.fetch_add(1, Relaxed);
            let mut r = StatusCode::NOT_MODIFIED.into_response();
            r.headers_mut().insert(header::ETAG, entry.etag.clone());
            r.headers_mut().insert("x-cache", HeaderValue::from_static("HIT"));
            return r;
        }
        let mut r = Response::new(Body::from(entry.body.clone()));
        *r.status_mut() = entry.status;
        for (k, v) in &entry.headers {
            r.headers_mut().insert(k.clone(), v.clone());
        }
        r.headers_mut().insert(header::ETAG, entry.etag.clone());
        if private {
            r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=300"));
            r.headers_mut().insert(header::VARY, HeaderValue::from_static("Authorization, X-API-Key"));
        }
        r.headers_mut().insert("x-cache", HeaderValue::from_static(state));
        r
    };
    if let Some(hit) = c.inner.get(&key).await {
        c.hits.fetch_add(1, Relaxed);
        return finish(&hit, "HIT");
    }
    let max_entry = c.max_entry;
    let tag = etag.clone();
    let res = c
        .inner
        .entry(key)
        .or_try_insert_with(async move {
            let resp = next.run(req).await;
            let (parts, body) = resp.into_parts();
            let body = axum::body::to_bytes(body, usize::MAX).await.unwrap_or_default();
            let keep = [header::CONTENT_TYPE, header::CACHE_CONTROL];
            let headers = keep.iter().filter_map(|k| parts.headers.get(k).map(|v| (k.clone(), v.clone()))).collect();
            let entry = Arc::new(Cached { status: parts.status, headers, body, etag: tag });
            let cacheable = matches!(parts.status, StatusCode::OK | StatusCode::NOT_FOUND) && entry.body.len() <= max_entry;
            if cacheable { Ok(entry) } else { Err(entry) }
        })
        .await;
    match res {
        Ok(e) if e.is_fresh() => {
            c.misses.fetch_add(1, Relaxed);
            finish(e.value(), "MISS")
        }
        Ok(e) => {
            c.coalesced.fetch_add(1, Relaxed);
            finish(e.value(), "COALESCED")
        }
        // Not cacheable (error status or too large): returned to every waiter, not stored.
        Err(e) => {
            c.misses.fetch_add(1, Relaxed);
            finish(&e, "MISS")
        }
    }
}

/// Staff-only admin routes: resolves the caller and checks the route's permission.
pub async fn staff(State(st): State<Shared>, mut req: Request, next: Next) -> Response {
    let g = &st.gate;
    if !g.enabled() {
        return next.run(req).await;
    }
    let ip = client_ip(&req, g.trust_proxy);
    let route = req.extensions().get::<MatchedPath>().map_or_else(|| req.uri().path().to_string(), |m| m.as_str().to_string());
    let p = match g.authenticate(req.headers(), ip).await {
        Ok(Some(p)) => p,
        Ok(None) => return unauthorized("unauthenticated", "log in with POST /api/v1/auth/login and send the session token"),
        Err(r) => return r,
    };
    if !p.role.is_staff() {
        return error(StatusCode::FORBIDDEN, "forbidden", "staff accounts only", &[]);
    }
    if let Some(perm) = crate::manage::permission_for(req.method(), &route)
        && !p.role.can(perm)
    {
        return error(StatusCode::FORBIDDEN, "forbidden", &format!("requires the {} permission", perm.name()), &[]);
    }
    req.extensions_mut().insert(Caller { principal: p, ip });
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn response_cache_handover_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("rc");
        let a = ResponseCache::new(16, 60);
        for k in 0..50u128 {
            let v = Arc::new(Cached {
                status: StatusCode::OK,
                headers: vec![(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))],
                body: Bytes::from(format!("{{\"k\":{k}}}")),
                etag: HeaderValue::from_static("\"x\""),
            });
            a.inner.insert(k, v).await;
        }
        a.inner.run_pending_tasks().await;
        assert_eq!(a.export(&path).unwrap(), 50);
        let b = ResponseCache::new(16, 60);
        assert_eq!(b.import(&path).await.unwrap(), 50);
        assert!(!path.exists());
        let v = b.inner.get(&7).await.unwrap();
        assert_eq!((v.status, &v.body[..]), (StatusCode::OK, &b"{\"k\":7}"[..]));
        std::fs::write(&path, b"OCRC1\x01\x02").unwrap();
        assert_eq!(b.import(&path).await.unwrap(), 0, "truncated input is ignored");
    }

    #[test]
    fn credentials_and_kinds() {
        let mut h = HeaderMap::new();
        assert_eq!(credential(&h), None);
        h.insert("x-api-key", HeaderValue::from_static(" ocnpj_a_b "));
        assert_eq!(credential(&h).as_deref(), Some("ocnpj_a_b"));
        h.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer ocs_token"));
        assert_eq!(credential(&h).as_deref(), Some("ocs_token"));
        assert_eq!(kind_of("/api/v1/cnpj/{cnpj}"), "lookup");
        assert_eq!(kind_of("/api/v1/empresas/search"), "search");
        assert_eq!(kind_of("/api/v1/busca"), "search");
        assert_eq!(kind_of("/api/v1/cpf/{cpf}"), "cpf");
        assert_eq!(kind_of("/downloads/{name}"), "download");
        assert_eq!(kind_of("/api/v1/assistant/ask"), "assistant");
        assert_eq!(kind_of("/api/v1/assistant/export"), "export");
    }
}
