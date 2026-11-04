//! Automatic updates: checks the RFB share every `update_check_hours`, and when a release
//! differs from the served snapshot (new competence, or files republished with other sizes)
//! runs `opencnpj update` as a low-priority child process. The update publishes `current`
//! atomically and the server's reload loop swaps to it; in-flight requests finish on the old
//! snapshot, so there is no downtime.

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::Serialize;
use serde_json::{Value, json};

use crate::accounts;
use crate::api::{ApiResult, Shared};

#[derive(Default, Clone, Serialize)]
pub struct UpdateState {
    pub last_check: Option<i64>,
    pub last_result: Option<String>,
    pub latest_remote: Option<String>,
    pub next_check: Option<i64>,
    pub job: Option<u64>,
    pub checking: bool,
}

#[derive(Default)]
pub struct Updater {
    pub notify: tokio::sync::Notify,
    pub state: Mutex<UpdateState>,
}

/// `(enabled, interval hours)` from the settings (accounts) or `--auto-update-hours`.
fn schedule(st: &Shared) -> (bool, i64) {
    if st.args.data_dir.is_none() || st.args.no_auto_update {
        return (false, 0);
    }
    match &st.gate.accounts {
        Some(a) => {
            let s = a.settings();
            (s.auto_update, s.update_check_hours.max(1))
        }
        None => (st.args.auto_update_hours > 0, st.args.auto_update_hours.max(1)),
    }
}

/// Same release? Compares competence and the (file name, size) list.
pub fn differs(served: &str, sources: &[(String, u64)], competence: &str, remote: &[(String, u64)]) -> bool {
    if served != competence {
        return served < competence;
    }
    let name = |p: &str| Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut have: Vec<(String, u64)> = sources.iter().map(|(p, b)| (name(p), *b)).collect();
    let mut want: Vec<(String, u64)> = remote.to_vec();
    have.sort();
    want.sort();
    have != want
}

/// One check: discovers the latest complete release and starts an update job if needed.
pub async fn check(st: &Shared) -> Result<String> {
    let dir = st.args.data_dir.clone().context("auto update needs --data-dir")?;
    {
        let mut s = st.updater.state.lock().expect("lock");
        if s.checking {
            return Ok("a check is already running".into());
        }
        s.checking = true;
    }
    let result = async {
        let args = crate::download::DownloadArgs::defaults(dir.join("raw"));
        let client = reqwest::Client::builder().timeout(Duration::from_secs(120)).build()?;
        let (month, files) = crate::download::discover(&client, &args).await?;
        let remote: Vec<(String, u64)> = files.iter().map(|f| (f.name.clone(), f.bytes)).collect();
        st.updater.state.lock().expect("lock").latest_remote = Some(month.clone());
        let snap = st.snapshot();
        let sources: Vec<(String, u64)> = snap.manifest.sources.iter().map(|s| (s.path.clone(), s.bytes)).collect();
        if !differs(&snap.manifest.competence, &sources, &month, &remote) {
            return Ok(format!("up to date: serving {} and the latest release is {month}", snap.manifest.competence));
        }
        if st.jobs.running("update") {
            return Ok(format!("release {month} found; an update job is already running"));
        }
        let profile = snap.manifest.profile.clone();
        let profile = if ["fast", "balanced", "compact"].contains(&profile.as_str()) { profile } else { "balanced".into() };
        let id = st.jobs.start_low_priority(
            "update",
            vec!["update".into(), "--data-dir".into(), dir.display().to_string(), "--profile".into(), profile],
        )?;
        st.updater.state.lock().expect("lock").job = Some(id);
        if let Some(a) = &st.gate.accounts {
            let a = a.clone();
            let m = month.clone();
            tokio::task::spawn_blocking(move || {
                a.audit(&accounts::Actor::system("auto-update"), "update.start", None, json!({ "release": m, "job": id }))
            });
        }
        Ok(format!("release {month} differs from {}; update job #{id} started", snap.manifest.competence))
    }
    .await;
    let mut s = st.updater.state.lock().expect("lock");
    s.checking = false;
    s.last_check = Some(accounts::now());
    s.last_result = Some(match &result {
        Ok(m) => m.clone(),
        Err(e) => format!("check failed: {e:#}"),
    });
    result
}

/// Background loop; wakes up early when settings change or a check is requested.
pub async fn run(st: Shared) {
    // Let the server settle before the first check.
    tokio::time::sleep(Duration::from_secs(60)).await;
    loop {
        let (enabled, hours) = schedule(&st);
        if !enabled {
            st.updater.state.lock().expect("lock").next_check = None;
            let _ = tokio::time::timeout(Duration::from_secs(3600), st.updater.notify.notified()).await;
            continue;
        }
        let last = st.updater.state.lock().expect("lock").last_check;
        let due = last.map_or(0, |t| t + hours * 3600);
        let wait = (due - accounts::now()).max(0);
        st.updater.state.lock().expect("lock").next_check = Some(accounts::now() + wait);
        // Woken early by a settings change or a manual check request: re-evaluate.
        if wait > 0 && tokio::time::timeout(Duration::from_secs(wait as u64), st.updater.notify.notified()).await.is_ok() {
            continue;
        }
        match check(&st).await {
            Ok(m) => eprintln!("auto update: {m}"),
            Err(e) => eprintln!("auto update: check failed: {e:#}"),
        }
    }
}

pub async fn status(State(st): State<Shared>) -> ApiResult {
    let (enabled, hours) = schedule(&st);
    let s = st.updater.state.lock().expect("lock").clone();
    let snap = st.snapshot();
    let job: Option<Value> = s.job.and_then(|id| st.jobs.get(id));
    Ok(Json(json!({
        "enabled": enabled, "interval_hours": hours, "last_check": s.last_check, "last_result": s.last_result,
        "latest_remote": s.latest_remote, "current_competence": snap.manifest.competence, "next_check": s.next_check,
        "checking": s.checking, "job": job,
        "zero_downtime": "the update builds and verifies a new snapshot at idle CPU/IO priority, then switches `current`; the server swaps snapshots without dropping requests",
    }))
    .into_response())
}

pub async fn check_now(State(st): State<Shared>) -> ApiResult {
    let st2 = st.clone();
    tokio::spawn(async move {
        if let Err(e) = check(&st2).await {
            eprintln!("auto update: check failed: {e:#}");
        }
    });
    Ok((axum::http::StatusCode::ACCEPTED, Json(json!({ "status": "checking" }))).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_comparison() {
        let served = vec![("raw/2026-10/Empresas0.zip".to_string(), 10)];
        let same = vec![("Empresas0.zip".to_string(), 10)];
        assert!(!differs("2026-10", &served, "2026-10", &same));
        assert!(differs("2026-10", &served, "2026-10", &[("Empresas0.zip".into(), 11)]), "republished file");
        assert!(differs("2026-10", &served, "2026-11", &same), "newer month");
        assert!(!differs("2026-10", &served, "2026-09", &same), "never go back in time");
    }
}
