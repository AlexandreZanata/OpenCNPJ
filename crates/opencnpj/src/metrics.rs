//! In-process request metrics: per-route latency histograms (log-spaced buckets, ~9%
//! resolution), status classes, and a per-second time series for the last 15 minutes.
//! Lock-free on the request path except for the first request of a new route.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, RwLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// Buckets cover 1 µs .. ~2^32 µs (≈72 min) with 8 sub-buckets per power of two.
const SUB: u32 = 8;
const BUCKETS: usize = 33 * SUB as usize;
const SERIES: usize = 900;

pub struct RouteStats {
    count: AtomicU64,
    sum_us: AtomicU64,
    max_us: AtomicU64,
    status: [AtomicU64; 5],
    hist: Box<[AtomicU64]>,
}

impl RouteStats {
    fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            sum_us: AtomicU64::new(0),
            max_us: AtomicU64::new(0),
            status: Default::default(),
            hist: (0..BUCKETS).map(|_| AtomicU64::new(0)).collect(),
        }
    }

    fn record(&self, us: u64, status: u16) {
        self.count.fetch_add(1, Relaxed);
        self.sum_us.fetch_add(us, Relaxed);
        self.max_us.fetch_max(us, Relaxed);
        let class = (usize::from(status / 100)).clamp(1, 5) - 1;
        self.status[class].fetch_add(1, Relaxed);
        self.hist[bucket(us)].fetch_add(1, Relaxed);
    }

    fn percentile(counts: &[u64], total: u64, p: f64) -> f64 {
        if total == 0 {
            return 0.0;
        }
        let target = ((total as f64) * p).ceil().max(1.0) as u64;
        let mut acc = 0;
        for (i, &c) in counts.iter().enumerate() {
            acc += c;
            if acc >= target {
                return upper_bound_us(i) as f64 / 1000.0;
            }
        }
        upper_bound_us(BUCKETS - 1) as f64 / 1000.0
    }

    fn snapshot(&self) -> Value {
        let counts: Vec<u64> = self.hist.iter().map(|c| c.load(Relaxed)).collect();
        let total: u64 = counts.iter().sum();
        let n = self.count.load(Relaxed);
        json!({
            "count": n,
            "mean_ms": if n > 0 { self.sum_us.load(Relaxed) as f64 / n as f64 / 1000.0 } else { 0.0 },
            "p50_ms": Self::percentile(&counts, total, 0.50),
            "p90_ms": Self::percentile(&counts, total, 0.90),
            "p95_ms": Self::percentile(&counts, total, 0.95),
            "p99_ms": Self::percentile(&counts, total, 0.99),
            "max_ms": self.max_us.load(Relaxed) as f64 / 1000.0,
            "status": {
                "1xx": self.status[0].load(Relaxed), "2xx": self.status[1].load(Relaxed), "3xx": self.status[2].load(Relaxed),
                "4xx": self.status[3].load(Relaxed), "5xx": self.status[4].load(Relaxed),
            },
        })
    }
}

fn bucket(us: u64) -> usize {
    let us = us.max(1);
    let exp = 63 - us.leading_zeros();
    let frac = if exp >= 3 { ((us >> (exp - 3)) & 7) as u32 } else { ((us << (3 - exp)) & 7) as u32 };
    ((exp * SUB + frac) as usize).min(BUCKETS - 1)
}

fn upper_bound_us(i: usize) -> u64 {
    let exp = i as u32 / SUB;
    let frac = u64::from(i as u32 % SUB);
    let base = 1u64 << exp;
    base + (base * (frac + 1)) / u64::from(SUB)
}

struct Slot {
    second: AtomicU64,
    requests: AtomicU64,
    errors: AtomicU64,
    sum_us: AtomicU64,
}

pub struct Metrics {
    pub started: Instant,
    routes: RwLock<HashMap<String, Arc<RouteStats>>>,
    series: Box<[Slot]>,
    pub timeouts: AtomicU64,
    pub overload: AtomicU64,
    pub reloads: AtomicU64,
}

fn now_s() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            routes: RwLock::new(HashMap::new()),
            series: (0..SERIES)
                .map(|_| Slot {
                    second: AtomicU64::new(0),
                    requests: AtomicU64::new(0),
                    errors: AtomicU64::new(0),
                    sum_us: AtomicU64::new(0),
                })
                .collect(),
            timeouts: AtomicU64::new(0),
            overload: AtomicU64::new(0),
            reloads: AtomicU64::new(0),
        }
    }
}

impl Metrics {
    pub fn record(&self, route: &str, us: u64, status: u16) {
        let stats = self.routes.read().expect("lock").get(route).cloned();
        let stats = match stats {
            Some(s) => s,
            None => self.routes.write().expect("lock").entry(route.to_string()).or_insert_with(|| Arc::new(RouteStats::new())).clone(),
        };
        stats.record(us, status);
        let sec = now_s();
        let slot = &self.series[(sec % SERIES as u64) as usize];
        if slot.second.swap(sec, Relaxed) != sec {
            slot.requests.store(0, Relaxed);
            slot.errors.store(0, Relaxed);
            slot.sum_us.store(0, Relaxed);
        }
        slot.requests.fetch_add(1, Relaxed);
        slot.sum_us.fetch_add(us, Relaxed);
        if status >= 500 {
            slot.errors.fetch_add(1, Relaxed);
        }
    }

    pub fn total_requests(&self) -> u64 {
        self.routes.read().expect("lock").values().map(|s| s.count.load(Relaxed)).sum()
    }

    pub fn routes_json(&self) -> Value {
        let routes = self.routes.read().expect("lock");
        let mut v: Vec<(&String, &Arc<RouteStats>)> = routes.iter().collect();
        v.sort_by(|a, b| b.1.count.load(Relaxed).cmp(&a.1.count.load(Relaxed)));
        Value::Array(
            v.into_iter()
                .map(|(r, s)| {
                    let mut o = s.snapshot();
                    o["route"] = json!(r);
                    o
                })
                .collect(),
        )
    }

    /// Last `seconds` seconds, oldest first: `[unix_second, requests, errors, mean_ms]`.
    pub fn series_json(&self, seconds: usize) -> Value {
        let now = now_s();
        let seconds = seconds.min(SERIES - 1) as u64;
        Value::Array(
            (0..=seconds)
                .rev()
                .map(|ago| {
                    let sec = now - ago;
                    let slot = &self.series[(sec % SERIES as u64) as usize];
                    if slot.second.load(Relaxed) != sec {
                        return json!([sec, 0, 0, 0.0]);
                    }
                    let n = slot.requests.load(Relaxed);
                    let mean = if n > 0 { slot.sum_us.load(Relaxed) as f64 / n as f64 / 1000.0 } else { 0.0 };
                    json!([sec, n, slot.errors.load(Relaxed), mean])
                })
                .collect(),
        )
    }

    /// Prometheus text format.
    pub fn prometheus(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("opencnpj_uptime_seconds {}\n", self.started.elapsed().as_secs_f64()));
        out.push_str(&format!("opencnpj_timeouts_total {}\n", self.timeouts.load(Relaxed)));
        out.push_str(&format!("opencnpj_overload_rejections_total {}\n", self.overload.load(Relaxed)));
        out.push_str(&format!("opencnpj_snapshot_reloads_total {}\n", self.reloads.load(Relaxed)));
        for (route, s) in self.routes.read().expect("lock").iter() {
            let l = format!("route=\"{route}\"");
            out.push_str(&format!("opencnpj_requests_total{{{l}}} {}\n", s.count.load(Relaxed)));
            out.push_str(&format!("opencnpj_request_seconds_sum{{{l}}} {}\n", s.sum_us.load(Relaxed) as f64 / 1e6));
            for (i, class) in ["1xx", "2xx", "3xx", "4xx", "5xx"].iter().enumerate() {
                out.push_str(&format!("opencnpj_responses_total{{{l},class=\"{class}\"}} {}\n", s.status[i].load(Relaxed)));
            }
            let mut acc = 0;
            for (i, c) in s.hist.iter().enumerate() {
                let c = c.load(Relaxed);
                if c == 0 {
                    continue;
                }
                acc += c;
                out.push_str(&format!("opencnpj_request_seconds_bucket{{{l},le=\"{}\"}} {acc}\n", upper_bound_us(i) as f64 / 1e6));
            }
            out.push_str(&format!("opencnpj_request_seconds_bucket{{{l},le=\"+Inf\"}} {acc}\n"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_are_monotonic_and_tight() {
        let mut prev = 0;
        for us in [1u64, 2, 3, 7, 8, 9, 100, 1000, 1234, 50_000, 10_000_000] {
            let b = bucket(us);
            assert!(b >= prev);
            prev = b;
            let ub = upper_bound_us(b);
            assert!(ub >= us && ub as f64 <= us as f64 * 1.26 + 1.0, "{us} -> {ub}");
        }
    }

    #[test]
    fn percentiles() {
        let m = Metrics::default();
        for i in 1..=100u64 {
            m.record("/x", i * 1000, if i == 100 { 500 } else { 200 });
        }
        let r = &m.routes_json()[0];
        assert_eq!(r["count"], 100);
        assert_eq!(r["status"]["5xx"], 1);
        let p50 = r["p50_ms"].as_f64().unwrap();
        assert!((50.0..=56.0).contains(&p50), "{p50}");
        assert!(m.prometheus().contains("opencnpj_requests_total{route=\"/x\"} 100"));
        assert_eq!(m.series_json(5).as_array().unwrap().len(), 6);
    }
}
