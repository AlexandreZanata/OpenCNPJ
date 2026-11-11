//! Persistent browser and network budgets for the public trial and free tier.

// HTTP rejections carry their response directly.
#![allow(clippy::result_large_err)]

use std::net::IpAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use ipnet::IpNet;
use ring::hmac;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::json;

use crate::{accounts, api::Shared, gate};

pub const TRIAL_LIMIT: i64 = 3;
pub const FREE_DAILY_LIMIT: i64 = 10;
pub const FREE_MONTHLY_LIMIT: i64 = 30;
const COOKIE: &str = "opencnpj_trial";
const DAY: i64 = 86_400;
const COOKIE_LIFETIME: i64 = 365 * DAY;

pub struct Trial {
    db: Mutex<Connection>,
    key: hmac::Key,
    denied: Vec<IpNet>,
    secure: bool,
}

#[derive(Clone, Copy)]
pub enum Activity {
    Preview,
    Free,
    Register,
}

struct Budget {
    key: [u8; 32],
    expires: i64,
    limit: i64,
}

pub struct Ticket {
    budgets: Vec<Budget>,
    now: i64,
    cookie: Option<HeaderValue>,
}

impl Trial {
    pub fn open(db: &Path, denied_file: Option<&Path>, secure: bool) -> Result<Self> {
        let denied = if let Some(path) = denied_file {
            std::fs::read_to_string(path)?
                .lines()
                .map(|line| line.split('#').next().unwrap_or_default().trim())
                .filter(|line| !line.is_empty())
                .map(|line| line.parse().with_context(|| format!("invalid trial network: {line}")))
                .collect::<Result<_>>()?
        } else {
            Vec::new()
        };
        let conn = Connection::open(db)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS trial_secret (id INTEGER PRIMARY KEY CHECK(id = 1), secret BLOB NOT NULL);
             CREATE TABLE IF NOT EXISTS trial_usage (
                subject BLOB NOT NULL, expires INTEGER NOT NULL, used INTEGER NOT NULL,
                PRIMARY KEY(subject, expires)
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS trial_usage_expiry ON trial_usage(expires);",
        )?;
        let mut secret = [0; 32];
        getrandom::fill(&mut secret).map_err(|e| anyhow::anyhow!("trial secret: {e}"))?;
        conn.execute("INSERT OR IGNORE INTO trial_secret VALUES (1, ?1)", [secret.as_slice()])?;
        let secret: Vec<u8> = conn.query_row("SELECT secret FROM trial_secret WHERE id=1", [], |row| row.get(0))?;
        Ok(Self { db: Mutex::new(conn), key: hmac::Key::new(hmac::HMAC_SHA256, &secret), denied, secure })
    }

    fn tag(&self, value: &[u8]) -> [u8; 32] {
        hmac::sign(&self.key, value).as_ref().try_into().expect("SHA-256 tag")
    }

    fn read_cookie<'a>(&self, headers: &'a HeaderMap, now: i64) -> Option<(&'a str, i64)> {
        let value = headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(';'))
            .filter_map(|pair| pair.trim().split_once('='))
            .find_map(|(name, value)| (name == COOKIE).then_some(value))?;
        let (payload, signature) = value.rsplit_once('.')?;
        let (id, expires) = payload.split_once('.')?;
        let expires: i64 = expires.parse().ok()?;
        if id.len() != 32 || signature.len() != 64 || expires <= now || expires > now + COOKIE_LIFETIME {
            return None;
        }
        let signature = hex::decode(signature).ok()?;
        hmac::verify(&self.key, payload.as_bytes(), &signature).ok()?;
        Some((id, expires))
    }

    pub fn ticket(&self, headers: &HeaderMap, ip: IpAddr, activity: Activity) -> Result<Ticket, Response> {
        let ip = ip.to_canonical();
        if self.denied.iter().any(|network| network.contains(&ip)) {
            return Err(gate::error(StatusCode::FORBIDDEN, "trial_network_blocked", "free access is unavailable from this network", &[]));
        }
        if headers.get("sec-fetch-site").is_some_and(|value| value == "cross-site") {
            return Err(gate::error(StatusCode::FORBIDDEN, "trial_cross_site", "free access requires a same-site request", &[]));
        }
        let now = accounts::now();
        let today = accounts::day_of(now);
        let day = (today + 1) * DAY;
        let month = accounts::month_bounds(today).1 * DAY;
        let (device, expiry, cookie) = if let Some((id, expiry)) = self.read_cookie(headers, now) {
            (id.to_owned(), expiry, None)
        } else {
            let mut id = [0; 16];
            if getrandom::fill(&mut id).is_err() {
                return Err(StatusCode::SERVICE_UNAVAILABLE.into_response());
            }
            let id = hex::encode(id);
            let expiry = now + COOKIE_LIFETIME;
            let payload = format!("{id}.{expiry}");
            let sig = hex::encode(self.tag(payload.as_bytes()));
            let secure = if self.secure { "; Secure" } else { "" };
            let cookie = format!("{COOKIE}={payload}.{sig}; Path=/; Max-Age={COOKIE_LIFETIME}; HttpOnly; SameSite=Lax{secure}");
            (id, expiry, HeaderValue::from_str(&cookie).ok())
        };
        // IPv6 privacy addresses commonly rotate inside a single /64.
        let network = match ip {
            IpAddr::V4(ip) => ip.to_string(),
            IpAddr::V6(ip) => std::net::Ipv6Addr::from((u128::from(ip) >> 64) << 64).to_string(),
        };
        let budget = |scope, identity, expires, limit| Budget { key: self.tag(format!("{scope}:{identity}").as_bytes()), expires, limit };
        let (device, network) = (device.as_str(), network.as_str());
        let budgets = match activity {
            Activity::Preview => vec![budget("preview-device", device, expiry, TRIAL_LIMIT), budget("preview-network", network, day, 12)],
            Activity::Free => vec![
                budget("free-device-day", device, day, FREE_DAILY_LIMIT),
                budget("free-device-month", device, month, FREE_MONTHLY_LIMIT),
                budget("free-network-day", network, day, 40),
                budget("free-network-month", network, month, 120),
            ],
            Activity::Register => vec![budget("register-device", device, day, 2), budget("register-network", network, day, 6)],
        };
        Ok(Ticket { budgets, now, cookie })
    }

    fn remaining(conn: &Connection, ticket: &Ticket) -> Result<i64> {
        let mut remaining = i64::MAX;
        let mut statement = conn.prepare_cached("SELECT used FROM trial_usage WHERE subject=?1 AND expires=?2")?;
        for budget in &ticket.budgets {
            let used: Option<i64> = statement.query_row(params![budget.key.as_slice(), budget.expires], |row| row.get(0)).optional()?;
            remaining = remaining.min((budget.limit - used.unwrap_or(0)).max(0));
        }
        Ok(remaining)
    }

    pub async fn status(self: &Arc<Self>, ticket: Arc<Ticket>) -> Result<i64, Response> {
        let trial = self.clone();
        gate::blocking(move || Self::remaining(&trial.db.lock().unwrap_or_else(std::sync::PoisonError::into_inner), &ticket)).await
    }

    /// Checks and increments all budgets together, including when two server processes overlap.
    pub async fn reserve(self: &Arc<Self>, ticket: Arc<Ticket>) -> Result<Option<i64>, Response> {
        let trial = self.clone();
        gate::blocking(move || {
            let mut conn = trial.db.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let remaining = Self::remaining(&tx, &ticket)?;
            if remaining == 0 {
                return Ok(None);
            }
            tx.execute(
                "DELETE FROM trial_usage WHERE (subject, expires) IN
                 (SELECT subject, expires FROM trial_usage WHERE expires <= ?1 ORDER BY expires LIMIT 256)",
                [ticket.now],
            )?;
            for budget in &ticket.budgets {
                tx.execute(
                    "INSERT INTO trial_usage VALUES (?1, ?2, 1)
                     ON CONFLICT(subject, expires) DO UPDATE SET used=used+1",
                    params![budget.key.as_slice(), budget.expires],
                )?;
            }
            tx.commit()?;
            Ok(Some(remaining - 1))
        })
        .await
    }

    pub async fn refund(self: &Arc<Self>, ticket: Arc<Ticket>) -> Result<i64, Response> {
        let trial = self.clone();
        gate::blocking(move || {
            let mut conn = trial.db.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            for budget in &ticket.budgets {
                tx.execute(
                    "UPDATE trial_usage SET used=used-1 WHERE subject=?1 AND expires=?2 AND used>0",
                    params![budget.key.as_slice(), budget.expires],
                )?;
            }
            let remaining = Self::remaining(&tx, &ticket)?;
            tx.commit()?;
            Ok(remaining)
        })
        .await
    }
}

impl Ticket {
    pub fn decorate(&self, response: &mut Response, remaining: Option<i64>) {
        let headers = response.headers_mut();
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
        if let Some(cookie) = &self.cookie {
            headers.append(header::SET_COOKIE, cookie.clone());
        }
        if let Some(remaining) = remaining {
            headers.insert("x-trial-limit", HeaderValue::from(TRIAL_LIMIT));
            headers.insert("x-trial-remaining", HeaderValue::from(remaining));
        }
    }
}

pub async fn status(State(st): State<Shared>, request: Request) -> Response {
    let Some(trial) = &st.trial else { return StatusCode::NOT_FOUND.into_response() };
    let ip = gate::client_ip(&request, st.gate.trust_proxy);
    let ticket = match trial.ticket(request.headers(), ip, Activity::Preview) {
        Ok(ticket) => Arc::new(ticket),
        Err(response) => return response,
    };
    match trial.status(ticket.clone()).await {
        Ok(remaining) => {
            let mut response = axum::Json(json!({ "limit": TRIAL_LIMIT, "remaining": remaining })).into_response();
            ticket.decorate(&mut response, Some(remaining));
            response
        }
        Err(response) => response,
    }
}
