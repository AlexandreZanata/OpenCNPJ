//! Formatting helpers: numbers, bytes, durations, UTC timestamps and URL query strings.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

/// Current unix time in seconds.
pub fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// `1234567` -> `"1,234,567"`.
pub fn int(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Signed variant of [`int`].
pub fn signed(n: i64) -> String {
    if n < 0 { format!("-{}", int(n.unsigned_abs())) } else { int(n.unsigned_abs()) }
}

/// A limit where `None` means unlimited.
pub fn limit(v: Option<i64>) -> String {
    v.map_or_else(|| "unlimited".to_string(), signed)
}

/// Decimal byte units, as the web dashboard shows them.
pub fn bytes(b: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1000.0 && i < UNITS.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 { format!("{b} B") } else { format!("{v:.2} {}", UNITS[i]) }
}

/// Milliseconds with precision that depends on magnitude.
pub fn ms(v: f64) -> String {
    if !v.is_finite() {
        "–".into()
    } else if v < 1.0 {
        format!("{v:.3}")
    } else if v < 100.0 {
        format!("{v:.2}")
    } else {
        format!("{v:.0}")
    }
}

/// Ratio in `[0, 1]` as a percentage.
pub fn percent(r: Option<f64>) -> String {
    r.map_or_else(|| "–".to_string(), |r| format!("{:.1}%", r * 100.0))
}

/// Compact duration: `"45s"`, `"12m 05s"`, `"3h 07m"`, `"2d 04h"`.
pub fn duration(secs: f64) -> String {
    let s = if secs.is_finite() && secs > 0.0 { secs as u64 } else { 0 };
    let (d, h, m, sec) = (s / 86_400, (s / 3600) % 24, (s / 60) % 60, s % 60);
    if d > 0 {
        format!("{d}d {h:02}h")
    } else if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m {sec:02}s")
    } else {
        format!("{sec}s")
    }
}

/// Days since 1970-01-01 to a civil date (proleptic Gregorian, Howard Hinnant's algorithm).
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Unix seconds (or milliseconds, when the value is clearly in ms) as `YYYY-MM-DD HH:MM:SS` UTC.
pub fn ts(t: i64) -> String {
    let t = if t.abs() >= 100_000_000_000 { t / 1000 } else { t };
    let (days, rem) = (t.div_euclid(86_400), t.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", rem / 3600, (rem / 60) % 60, rem % 60)
}

/// Unix seconds as `YYYY-MM-DD` UTC.
pub fn date(t: i64) -> String {
    let (y, m, d) = civil_from_days(t.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Optional timestamp, `"–"` when absent.
pub fn opt_ts(t: Option<i64>) -> String {
    t.map_or_else(|| "–".to_string(), ts)
}

/// Relative time: `"just now"`, `"5m ago"`, `"in 2h"`.
pub fn ago(t: i64, now: i64) -> String {
    let diff = now - t;
    if diff.abs() < 30 {
        return "just now".into();
    }
    let d = duration(diff.unsigned_abs() as f64);
    let short = d.split(' ').next().unwrap_or(&d).to_string();
    if diff > 0 { format!("{short} ago") } else { format!("in {short}") }
}

/// Short text for an arbitrary JSON value (tables of loosely specified objects).
pub fn value(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// Like [`value`], but fields that look like unix timestamps are rendered as dates.
pub fn field(key: &str, v: &Value) -> String {
    let is_time = key == "ts" || key.ends_with("_at") || key == "expires" || key == "until" || key == "modified" || key == "started";
    match (is_time, crate::models::value_i64(v)) {
        (true, Some(t)) if t > 0 => ts(t),
        _ => value(v),
    }
}

/// Percent-encodes one query string component (RFC 3986 unreserved characters kept).
pub fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Builds `path?k=v&...`, skipping empty values.
pub fn with_query(path: &str, params: &[(&str, String)]) -> String {
    let q: Vec<String> =
        params.iter().filter(|(_, v)| !v.trim().is_empty()).map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v.trim()))).collect();
    if q.is_empty() { path.to_string() } else { format!("{path}?{}", q.join("&")) }
}

/// Normalizes a server URL typed by the user: trims, adds `http://`, drops trailing slashes.
pub fn normalize_url(s: &str) -> String {
    let s = s.trim().trim_end_matches('/');
    if s.is_empty() {
        return String::new();
    }
    if s.contains("://") { s.to_string() } else { format!("http://{s}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn integers() {
        assert_eq!(int(0), "0");
        assert_eq!(int(999), "999");
        assert_eq!(int(1000), "1,000");
        assert_eq!(int(1_234_567), "1,234,567");
        assert_eq!(signed(-20_000), "-20,000");
        assert_eq!(limit(None), "unlimited");
        assert_eq!(limit(Some(100_000)), "100,000");
    }

    #[test]
    fn byte_units() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(999), "999 B");
        assert_eq!(bytes(1500), "1.50 KB");
        assert_eq!(bytes(2_147_483_648), "2.15 GB");
    }

    #[test]
    fn milliseconds_and_percent() {
        assert_eq!(ms(0.1234), "0.123");
        assert_eq!(ms(12.345), "12.35");
        assert_eq!(ms(1234.5), "1234");
        assert_eq!(ms(f64::NAN), "–");
        assert_eq!(percent(Some(0.756)), "75.6%");
        assert_eq!(percent(None), "–");
    }

    #[test]
    fn durations() {
        assert_eq!(duration(0.0), "0s");
        assert_eq!(duration(45.9), "45s");
        assert_eq!(duration(725.0), "12m 05s");
        assert_eq!(duration(3.0 * 3600.0 + 7.0 * 60.0), "3h 07m");
        assert_eq!(duration(2.0 * 86_400.0 + 4.0 * 3600.0), "2d 04h");
        assert_eq!(duration(-5.0), "0s");
    }

    #[test]
    fn timestamps() {
        assert_eq!(ts(0), "1970-01-01 00:00:00");
        assert_eq!(ts(951_782_400), "2000-02-29 00:00:00");
        assert_eq!(ts(1_793_750_000), "2026-11-03 23:53:20");
        assert_eq!(ts(1_793_750_000_000), "2026-11-03 23:53:20");
        assert_eq!(date(1_793_750_000), "2026-11-03");
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(opt_ts(None), "–");
        assert_eq!(ago(100, 110), "just now");
        assert_eq!(ago(0, 3600), "1h ago");
        assert_eq!(ago(7200, 0), "in 2h");
    }

    #[test]
    fn json_values() {
        assert_eq!(value(&json!(null)), "");
        assert_eq!(value(&json!("x")), "x");
        assert_eq!(value(&json!(3)), "3");
        assert_eq!(value(&json!({"a":1})), r#"{"a":1}"#);
        assert_eq!(field("created_at", &json!(0)), "0");
        assert_eq!(field("created_at", &json!(1_793_750_000)), "2026-11-03 23:53:20");
        assert_eq!(field("ip", &json!("10.0.0.1")), "10.0.0.1");
    }

    #[test]
    fn query_strings() {
        assert_eq!(urlencode("ana maria@x.com"), "ana%20maria%40x.com");
        assert_eq!(urlencode("são"), "s%C3%A3o");
        assert_eq!(with_query("/admin/api/users", &[]), "/admin/api/users");
        assert_eq!(
            with_query("/admin/api/users", &[("q", " ana ".into()), ("status", String::new()), ("sort", "-created".into())]),
            "/admin/api/users?q=ana&sort=-created"
        );
    }

    #[test]
    fn urls() {
        assert_eq!(normalize_url(" 127.0.0.1:8081/ "), "http://127.0.0.1:8081");
        assert_eq!(normalize_url("https://admin.example.com//"), "https://admin.example.com");
        assert_eq!(normalize_url("  "), "");
    }
}
