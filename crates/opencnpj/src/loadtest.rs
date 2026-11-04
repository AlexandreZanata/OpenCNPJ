//! `opencnpj loadtest`: many concurrent virtual users, each with its own login key and its
//! own keep-alive connection, replaying a URL list against a running server. Measures what
//! one machine serves to thousands of simultaneous authenticated users (authentication,
//! quotas, accounting and caches included).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Args;
use serde_json::json;

#[derive(Args)]
pub struct LoadArgs {
    /// Server base URL.
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    pub base_url: String,
    /// One request path per line (e.g. from `bench --write-urls`).
    #[arg(long)]
    pub urls: PathBuf,
    /// Login keys, one per line (e.g. from `accounts seed`); virtual user i uses key i.
    #[arg(long)]
    pub keys: Option<PathBuf>,
    /// Simultaneous virtual users (each keeps one connection open).
    #[arg(long, default_value_t = 1000)]
    pub users: usize,
    #[arg(long, default_value_t = 30)]
    pub seconds: u64,
    /// Pause between requests of one virtual user, in milliseconds (0 = closed loop).
    #[arg(long, default_value_t = 0)]
    pub think_ms: u64,
    /// Write the JSON report here.
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(Default)]
struct Tally {
    lat_us: Vec<u32>,
    status: BTreeMap<u16, u64>,
    cache: BTreeMap<String, u64>,
    errors: u64,
    bytes: u64,
}

fn pct(sorted: &[u32], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() as f64 * p).ceil() as usize).clamp(1, sorted.len()) - 1;
    f64::from(sorted[i]) / 1000.0
}

pub fn run(a: LoadArgs) -> Result<()> {
    let urls: Vec<String> = std::fs::read_to_string(&a.urls)
        .with_context(|| format!("reading {}", a.urls.display()))?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| if l.starts_with("http") { l.to_string() } else { format!("{}{l}", a.base_url.trim_end_matches('/')) })
        .collect();
    anyhow::ensure!(!urls.is_empty(), "no URLs");
    let keys: Vec<String> = match &a.keys {
        Some(p) => std::fs::read_to_string(p)?.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect(),
        None => Vec::new(),
    };
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let report = rt.block_on(async move {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(a.users)
            .pool_idle_timeout(Duration::from_secs(120))
            .timeout(Duration::from_secs(120))
            .build()?;
        let urls = Arc::new(urls);
        let keys = Arc::new(keys);
        let start = Instant::now();
        let deadline = start + Duration::from_secs(a.seconds);
        let mut tasks = Vec::with_capacity(a.users);
        for u in 0..a.users {
            let (client, urls, keys) = (client.clone(), urls.clone(), keys.clone());
            let think = Duration::from_millis(a.think_ms);
            tasks.push(tokio::spawn(async move {
                let mut t = Tally::default();
                let key = (!keys.is_empty()).then(|| keys[u % keys.len()].clone());
                // Spread users over the URL list so they do not all ask the same thing.
                let mut i = u.wrapping_mul(7919) % urls.len();
                while Instant::now() < deadline {
                    let mut req = client.get(&urls[i]);
                    if let Some(k) = &key {
                        req = req.bearer_auth(k);
                    }
                    let t0 = Instant::now();
                    match req.send().await {
                        Ok(r) => {
                            let status = r.status().as_u16();
                            let cache = r.headers().get("x-cache").and_then(|v| v.to_str().ok()).unwrap_or("none").to_string();
                            let body = r.bytes().await.map(|b| b.len()).unwrap_or(0);
                            t.lat_us.push(u32::try_from(t0.elapsed().as_micros()).unwrap_or(u32::MAX));
                            *t.status.entry(status).or_default() += 1;
                            *t.cache.entry(cache).or_default() += 1;
                            t.bytes += body as u64;
                        }
                        Err(_) => t.errors += 1,
                    }
                    i = (i + 1) % urls.len();
                    if !think.is_zero() {
                        tokio::time::sleep(think).await;
                    }
                }
                t
            }));
        }
        let mut all = Tally::default();
        for task in tasks {
            let t = task.await?;
            all.lat_us.extend(t.lat_us);
            for (k, v) in t.status {
                *all.status.entry(k).or_default() += v;
            }
            for (k, v) in t.cache {
                *all.cache.entry(k).or_default() += v;
            }
            all.errors += t.errors;
            all.bytes += t.bytes;
        }
        let wall = start.elapsed().as_secs_f64();
        all.lat_us.sort_unstable();
        let n = all.lat_us.len();
        anyhow::Ok(json!({
            "base_url": a.base_url, "virtual_users": a.users, "distinct_keys": keys.len().min(a.users), "urls": urls.len(),
            "seconds": wall, "think_ms": a.think_ms, "requests": n, "requests_per_second": n as f64 / wall,
            "transport_errors": all.errors, "bytes": all.bytes,
            "latency_ms": { "p50": pct(&all.lat_us, 0.50), "p90": pct(&all.lat_us, 0.90), "p95": pct(&all.lat_us, 0.95),
                            "p99": pct(&all.lat_us, 0.99), "max": all.lat_us.last().map_or(0.0, |x| f64::from(*x) / 1000.0) },
            "status": all.status.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
            "x_cache": all.cache,
        }))
    })?;
    let text = serde_json::to_string_pretty(&report)?;
    if let Some(p) = &a.out {
        std::fs::write(p, &text)?;
    }
    println!("{text}");
    Ok(())
}
