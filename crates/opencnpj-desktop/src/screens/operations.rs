//! Operations: snapshots, reload, background jobs with a live log tail, update status, caches
//! and server settings.
//!
//! Calls `GET /admin/api/snapshots`, `POST /admin/api/snapshots/activate`, `POST
//! /admin/api/reload`, `GET/POST /admin/api/jobs`, `DELETE /admin/api/jobs/{id}`, `GET
//! /admin/api/jobs/{id}/log`, `GET /admin/api/updates`, `POST /admin/api/updates/check`,
//! `GET/POST/DELETE /admin/api/cache`, `GET/DELETE /admin/api/response-cache` and `GET/PATCH
//! /admin/api/settings`.

use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Layout, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use serde_json::{Map, Value, json};

use crate::api::{ApiResult, Client, Task};
use crate::format;
use crate::models::{CacheStats, Job, JobRequest, JobStarted, ResponseCacheStats, Settings, SnapshotList, UpdateStatus};
use crate::screens::dashboard::stats_grid;
use crate::state::{Env, perm};
use crate::theme::Palette;
use crate::widgets;

const REFRESH: Duration = Duration::from_secs(2);
pub const JOB_KINDS: [&str; 5] = ["verify", "bench", "download", "update", "build"];
pub const PROFILES: [&str; 3] = ["balanced", "fast", "compact"];
const SETTINGS_KEYS: [&str; 6] = ["registration", "default_plan", "search_log", "search_log_days", "auto_update", "update_check_hours"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Snapshots,
    Jobs,
    Updates,
    Caches,
    Settings,
}

impl Tab {
    const ALL: [Tab; 5] = [Tab::Snapshots, Tab::Jobs, Tab::Updates, Tab::Caches, Tab::Settings];

    fn label(self) -> &'static str {
        match self {
            Tab::Snapshots => "Snapshots",
            Tab::Jobs => "Jobs",
            Tab::Updates => "Updates",
            Tab::Caches => "Caches",
            Tab::Settings => "Settings",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobForm {
    pub kind: String,
    pub profile: String,
    pub month: String,
    pub sources: bool,
    pub queries: usize,
}

impl Default for JobForm {
    fn default() -> Self {
        Self { kind: "verify".into(), profile: "balanced".into(), month: String::new(), sources: false, queries: 300 }
    }
}

/// `YYYY-MM` with a plausible month, or empty (server default: latest / current competence).
pub fn valid_month(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return true;
    }
    let b = s.as_bytes();
    b.len() == 7 && b[4] == b'-' && b[..4].iter().chain(&b[5..]).all(u8::is_ascii_digit) && matches!(s[5..].parse::<u8>(), Ok(1..=12))
}

impl JobForm {
    /// Body for `POST /admin/api/jobs`, with only the options the job kind uses.
    pub fn request(&self) -> JobRequest {
        let month = (!self.month.trim().is_empty()).then(|| self.month.trim().to_string());
        let k = self.kind.as_str();
        JobRequest {
            kind: self.kind.clone(),
            profile: matches!(k, "update" | "build").then(|| self.profile.clone()),
            month: if matches!(k, "download" | "build") { month } else { None },
            sources: matches!(k, "verify" | "update") && self.sources,
            queries: (k == "bench").then_some(self.queries),
        }
    }
}

/// PATCH body with the settings that differ.
pub fn settings_patch(old: &Settings, new: &Settings) -> Map<String, Value> {
    let (a, b) = (json!(old), json!(new));
    let mut m = Map::new();
    for k in SETTINGS_KEYS {
        if a.get(k) != b.get(k) {
            m.insert(k.into(), b.get(k).cloned().unwrap_or(Value::Null));
        }
    }
    m
}

enum Confirm {
    Activate(String),
    Reload,
    StopJob(i64),
    ClearPostings,
    ClearResponse,
    StartJob(JobRequest),
    CheckUpdates,
}

enum OpResult {
    Message(String),
    JobStarted(i64),
}

pub struct Operations {
    tab: Tab,
    t_op: Task<OpResult>,
    confirm: Option<Confirm>,

    snapshots: Option<SnapshotList>,
    t_snapshots: Task<SnapshotList>,

    jobs: Vec<Job>,
    t_jobs: Task<Vec<Job>>,
    jobs_last: Option<Instant>,
    job_form: JobForm,
    log_job: Option<i64>,
    log: String,
    t_log: Task<String>,
    log_last: Option<Instant>,
    follow_log: bool,

    updates: Option<UpdateStatus>,
    t_updates: Task<UpdateStatus>,

    postings: Option<CacheStats>,
    t_postings: Task<CacheStats>,
    budget_mib: u64,
    response: Option<ResponseCacheStats>,
    response_missing: bool,
    t_response: Task<ResponseCacheStats>,

    settings_raw: Option<Value>,
    settings: Option<Settings>,
    edit: Settings,
    t_settings: Task<Value>,

    loaded: [bool; 5],
}

impl Default for Operations {
    fn default() -> Self {
        Self {
            tab: Tab::Snapshots,
            t_op: Task::default(),
            confirm: None,
            snapshots: None,
            t_snapshots: Task::default(),
            jobs: Vec::new(),
            t_jobs: Task::default(),
            jobs_last: None,
            job_form: JobForm::default(),
            log_job: None,
            log: String::new(),
            t_log: Task::default(),
            log_last: None,
            follow_log: true,
            updates: None,
            t_updates: Task::default(),
            postings: None,
            t_postings: Task::default(),
            budget_mib: 256,
            response: None,
            response_missing: false,
            t_response: Task::default(),
            settings_raw: None,
            settings: None,
            edit: Settings::default(),
            t_settings: Task::default(),
            loaded: [false; 5],
        }
    }
}

impl Operations {
    fn tab_index(t: Tab) -> usize {
        Tab::ALL.iter().position(|x| *x == t).unwrap_or(0)
    }

    fn load(&mut self, tab: Tab, env: &Env<'_>) {
        self.loaded[Self::tab_index(tab)] = true;
        let (ctx, c) = (&env.ctx, env.client());
        match tab {
            Tab::Snapshots => self.t_snapshots = Task::call(ctx, c, |c| c.get("/admin/api/snapshots")),
            Tab::Jobs => {
                self.jobs_last = Some(Instant::now());
                self.t_jobs = Task::call(ctx, c, |c| c.get("/admin/api/jobs"));
                self.load_log(env);
            }
            Tab::Updates => self.t_updates = Task::call(ctx, c, |c| c.get("/admin/api/updates")),
            Tab::Caches => {
                self.t_postings = Task::call(ctx, c, |c| c.get("/admin/api/cache"));
                if !self.response_missing {
                    self.t_response = Task::call(ctx, c, |c| c.get("/admin/api/response-cache"));
                }
            }
            Tab::Settings => self.t_settings = Task::call(ctx, c, |c| c.get("/admin/api/settings")),
        }
    }

    fn load_log(&mut self, env: &Env<'_>) {
        if let Some(id) = self.log_job
            && !self.t_log.running()
        {
            self.log_last = Some(Instant::now());
            self.t_log = Task::call(&env.ctx, env.client(), move |c| c.get_text(&format!("/admin/api/jobs/{id}/log?lines=500")));
        }
    }

    fn op(&mut self, env: &Env<'_>, f: impl FnOnce(&Client) -> ApiResult<OpResult> + Send + 'static) {
        self.t_op = Task::call(&env.ctx, env.client(), f);
    }

    pub fn poll(&mut self, env: &mut Env<'_>) {
        if let Some(r) = self.t_snapshots.poll()
            && let Some(s) = env.check(r)
        {
            self.snapshots = Some(s);
        }
        if let Some(r) = self.t_jobs.poll()
            && let Some(j) = env.check(r)
        {
            self.jobs = j;
        }
        match self.t_log.poll() {
            Some(Ok(t)) => self.log = if t.is_empty() { "(empty)".into() } else { t },
            Some(Err(e)) => self.log = format!("Cannot read the log: {e}"),
            None => {}
        }
        if let Some(r) = self.t_updates.poll()
            && let Some(u) = env.check(r)
        {
            self.updates = Some(u);
        }
        if let Some(r) = self.t_postings.poll()
            && let Some(c) = env.check(r)
        {
            self.budget_mib = c.budget >> 20;
            self.postings = Some(c);
        }
        match self.t_response.poll() {
            Some(Ok(c)) => self.response = Some(c),
            Some(Err(e)) if e.is_not_found() => self.response_missing = true,
            Some(Err(e)) => env.report(&e),
            None => {}
        }
        if let Some(r) = self.t_settings.poll()
            && let Some(v) = env.check(r)
        {
            let s: Settings = serde_json::from_value(v.clone()).unwrap_or_default();
            self.edit = s.clone();
            self.settings = Some(s);
            self.settings_raw = Some(v);
        }
        if let Some(r) = self.t_op.poll()
            && let Some(o) = env.check(r)
        {
            match o {
                OpResult::Message(m) => env.toasts.success(m),
                OpResult::JobStarted(id) => {
                    env.toasts.success(format!("Started job #{id}"));
                    self.log_job = Some(id);
                    self.log.clear();
                }
            }
            self.load(self.tab, env);
            if self.tab == Tab::Updates {
                self.load(Tab::Jobs, env);
            }
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
        if !self.loaded[Self::tab_index(self.tab)] {
            self.load(self.tab, env);
        }
        let busy = self.t_op.running()
            || match self.tab {
                Tab::Snapshots => self.t_snapshots.running(),
                Tab::Jobs => self.t_jobs.running(),
                Tab::Updates => self.t_updates.running(),
                Tab::Caches => self.t_postings.running(),
                Tab::Settings => self.t_settings.running(),
            };
        if widgets::header(ui, "Operations", "", busy) {
            self.load(self.tab, env);
        }
        ui.horizontal(|ui| {
            for t in Tab::ALL {
                if ui.selectable_label(self.tab == t, RichText::new(t.label()).size(15.0)).clicked() && self.tab != t {
                    self.tab = t;
                    self.load(t, env);
                }
            }
        });
        ui.separator();
        let operate = env.can(perm::SYSTEM_OPERATE);
        match self.tab {
            Tab::Snapshots => self.snapshots_ui(ui, operate),
            Tab::Jobs => self.jobs_ui(ui, env, operate),
            Tab::Updates => self.updates_ui(ui, operate),
            Tab::Caches => self.caches_ui(ui, env, operate),
            Tab::Settings => self.settings_ui(ui, env),
        }
        self.confirm_ui(ui.ctx(), env);
    }

    fn snapshots_ui(&mut self, ui: &mut Ui, operate: bool) {
        let p = Palette::get(ui);
        ui.horizontal(|ui| {
            if let Some(s) = &self.snapshots {
                widgets::muted(ui, format!("Directory: {}", s.dir));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(operate && !self.t_op.running(), egui::Button::new("⟲ Reload current"))
                    .on_hover_text("Re-open the snapshot the `current` pointer names")
                    .on_disabled_hover_text("Requires system.operate")
                    .clicked()
                {
                    self.confirm = Some(Confirm::Reload);
                }
            });
        });
        let Some(s) = &self.snapshots else {
            widgets::muted(ui, "Loading…");
            return;
        };
        if s.files.is_empty() {
            widgets::muted(ui, "No .ocnpj files in the snapshots directory.");
            return;
        }
        let mut activate = None;
        let busy = self.t_op.running();
        TableBuilder::new(ui)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .id_salt("snapshots")
            .striped(true)
            .auto_shrink(false)
            .column(Column::remainder().at_least(240.0).clip(true))
            .column(Column::initial(90.0))
            .column(Column::auto().at_least(150.0))
            .column(Column::auto().at_least(150.0))
            .column(Column::auto().at_least(100.0))
            .header(widgets::ROW_H, |mut h| {
                for t in ["File", "Size", "SHA-256", "Modified (UTC)", ""] {
                    h.col(|ui| {
                        ui.strong(t);
                    });
                }
            })
            .body(|body| {
                body.rows(widgets::ROW_H + 4.0, s.files.len(), |mut row| {
                    let f = &s.files[row.index()];
                    row.col(|ui| {
                        ui.label(widgets::mono(&f.name));
                        if f.active {
                            widgets::pill(ui, "active", p.ok);
                        }
                    });
                    row.col(|ui| widgets::num(ui, format::bytes(f.bytes)));
                    row.col(|ui| match &f.sha256 {
                        Some(h) => {
                            let short: String = h.chars().take(16).collect();
                            ui.label(widgets::mono(format!("{short}…"))).on_hover_text(h);
                        }
                        None => {
                            widgets::muted(ui, "no sidecar");
                        }
                    });
                    row.col(|ui| {
                        ui.label(widgets::mono(format::opt_ts(f.modified)));
                    });
                    row.col(|ui| {
                        if !f.active
                            && ui
                                .add_enabled(operate && !busy, egui::Button::new("Activate"))
                                .on_disabled_hover_text("Requires system.operate")
                                .clicked()
                        {
                            activate = Some(f.name.clone());
                        }
                    });
                });
            });
        if let Some(n) = activate {
            self.confirm = Some(Confirm::Activate(n));
        }
    }

    fn jobs_ui(&mut self, ui: &mut Ui, env: &Env<'_>, operate: bool) {
        if self.jobs_last.is_none_or(|t| t.elapsed() >= REFRESH) && !self.t_jobs.running() {
            self.jobs_last = Some(Instant::now());
            self.t_jobs = Task::call(&env.ctx, env.client(), |c| c.get("/admin/api/jobs"));
        }
        let log_running = self.log_job.is_some_and(|id| self.jobs.iter().any(|j| j.id == id && j.is_running()));
        if log_running && self.log_last.is_none_or(|t| t.elapsed() >= REFRESH) {
            self.load_log(env);
        }
        ui.ctx().request_repaint_after(REFRESH);
        let p = Palette::get(ui);

        widgets::section(ui, "Start a job", |ui| {
            let f = &mut self.job_form;
            ui.horizontal_wrapped(|ui| {
                ui.label("Kind");
                let kinds: Vec<String> = JOB_KINDS.iter().map(|s| s.to_string()).collect();
                widgets::combo(ui, "job-kind", &mut f.kind, &kinds, None);
                let k = f.kind.clone();
                if matches!(k.as_str(), "update" | "build") {
                    ui.label("Profile");
                    let profiles: Vec<String> = PROFILES.iter().map(|s| s.to_string()).collect();
                    widgets::combo(ui, "job-profile", &mut f.profile, &profiles, None);
                }
                if matches!(k.as_str(), "download" | "build") {
                    ui.label("Month");
                    let ok = valid_month(&f.month);
                    ui.add(
                        egui::TextEdit::singleline(&mut f.month)
                            .hint_text(if k == "build" { "current" } else { "latest" })
                            .desired_width(80.0)
                            .text_color_opt((!ok).then_some(p.bad)),
                    );
                }
                if matches!(k.as_str(), "verify" | "update") {
                    ui.checkbox(&mut f.sources, "verify against sources");
                }
                if k == "bench" {
                    ui.label("Queries");
                    ui.add(egui::DragValue::new(&mut f.queries).range(10..=100_000));
                }
            });
            ui.add_space(4.0);
            let ok = valid_month(&f.month);
            let description = match f.kind.as_str() {
                "verify" => "Checks the active snapshot (and, with sources, every source row round-trips).",
                "bench" => "Runs the benchmark harness against the active snapshot (1, 4 and 16 threads).",
                "download" => "Downloads the Receita Federal ZIP files of a month into the data directory.",
                "update" => "Download, build, verify and publish the newest month.",
                "build" => "Builds a snapshot from downloaded files.",
                _ => "",
            };
            ui.horizontal(|ui| {
                if widgets::primary(ui, operate && ok && !self.t_op.running(), "▶ Start")
                    .on_disabled_hover_text("Requires system.operate")
                    .clicked()
                {
                    self.confirm = Some(Confirm::StartJob(f.request()));
                }
                widgets::muted(ui, description);
            });
            widgets::muted(ui, "Download, build and update need the server started with --data-dir.");
        });
        ui.add_space(8.0);

        let mut stop = None;
        let mut show = None;
        let avail = ui.available_height();
        widgets::section(ui, "Jobs", |ui| {
            if self.jobs.is_empty() {
                widgets::muted(ui, "No jobs since the server started.");
                return;
            }
            TableBuilder::new(ui)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .id_salt("jobs")
                .striped(true)
                .sense(egui::Sense::click())
                .max_scroll_height((avail * 0.35).max(120.0))
                .column(Column::auto().at_least(36.0))
                .column(Column::auto().at_least(80.0))
                .column(Column::auto().at_least(100.0))
                .column(Column::auto().at_least(150.0))
                .column(Column::initial(80.0))
                .column(Column::remainder().at_least(200.0).clip(true))
                .column(Column::auto().at_least(70.0))
                .header(widgets::ROW_H, |mut h| {
                    for t in ["#", "Kind", "Status", "Started (UTC)", "Elapsed", "Arguments", ""] {
                        h.col(|ui| {
                            ui.strong(t);
                        });
                    }
                })
                .body(|body| {
                    body.rows(widgets::ROW_H + 2.0, self.jobs.len(), |mut row| {
                        let j = &self.jobs[row.index()];
                        row.set_selected(self.log_job == Some(j.id));
                        row.col(|ui| {
                            ui.label(widgets::mono(j.id.to_string()));
                        });
                        row.col(|ui| {
                            ui.label(&j.kind);
                        });
                        row.col(|ui| {
                            widgets::status_pill(ui, &j.status);
                            if let Some(c) = j.exit_code {
                                ui.label(widgets::mono(c.to_string()));
                            }
                        });
                        row.col(|ui| {
                            ui.label(widgets::mono(format::opt_ts(j.started)));
                        });
                        row.col(|ui| widgets::num(ui, format::duration(j.elapsed_s)));
                        row.col(|ui| {
                            let a = j.args.join(" ");
                            ui.label(RichText::new(&a).monospace().small()).on_hover_text(a);
                        });
                        row.col(|ui| {
                            if j.is_running() && widgets::danger(ui, operate, "Stop").clicked() {
                                stop = Some(j.id);
                            }
                        });
                        if row.response().clicked() {
                            show = Some(j.id);
                        }
                    });
                });
        });
        if let Some(id) = stop {
            self.confirm = Some(Confirm::StopJob(id));
        }
        if let Some(id) = show {
            self.log_job = Some(id);
            self.log = "loading…".into();
            self.load_log(env);
        }
        ui.add_space(8.0);
        widgets::section(ui, "Log", |ui| {
            ui.horizontal(|ui| {
                match self.log_job {
                    Some(id) => ui.label(RichText::new(format!("job #{id}, last 500 lines")).strong()),
                    None => widgets::muted(ui, "Select a job to see its log."),
                };
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add_enabled(self.log_job.is_some(), egui::Button::new("Copy")).clicked() {
                        ui.ctx().copy_text(self.log.clone());
                    }
                    ui.checkbox(&mut self.follow_log, "follow");
                    if log_running {
                        widgets::pill(ui, "live", p.warn);
                    }
                });
            });
            let mut scroll = egui::ScrollArea::both().id_salt("job-log").auto_shrink(false).max_height(ui.available_height().max(160.0));
            if self.follow_log {
                scroll = scroll.stick_to_bottom(true);
            }
            scroll.show(ui, |ui| {
                ui.add(egui::Label::new(RichText::new(&self.log).monospace().size(12.0)).extend());
            });
        });
    }

    fn updates_ui(&mut self, ui: &mut Ui, operate: bool) {
        let p = Palette::get(ui);
        ui.horizontal(|ui| {
            widgets::muted(ui, "Checks the Receita Federal site for a newer month and runs an update job when one is published.");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let checking = self.updates.as_ref().is_some_and(|u| u.checking);
                if widgets::primary(ui, operate && !self.t_op.running() && !checking, "Check now")
                    .on_disabled_hover_text("Requires system.operate")
                    .clicked()
                {
                    self.confirm = Some(Confirm::CheckUpdates);
                }
            });
        });
        ui.add_space(6.0);
        let Some(u) = &self.updates else {
            widgets::muted(ui, "Loading…");
            return;
        };
        let now = format::now_unix();
        widgets::section(ui, "Update status", |ui| {
            let when = |t: Option<i64>| t.map_or("–".to_string(), |t| format!("{} ({})", format::ts(t), format::ago(t, now)));
            stats_grid(
                ui,
                "updates",
                &[
                    ("Automatic updates", if u.enabled { "enabled".into() } else { "disabled".into() }),
                    ("Check interval", u.interval_hours.map_or("–".into(), |h| format!("{h} h"))),
                    ("Current competence", if u.current_competence.is_empty() { "–".into() } else { u.current_competence.clone() }),
                    ("Latest on the source", format::value(&u.latest_remote)),
                    ("Last check (UTC)", when(u.last_check)),
                    ("Last result", format::value(&u.last_result)),
                    ("Next check (UTC)", when(u.next_check)),
                    ("Checking now", if u.checking { "yes".into() } else { "no".into() }),
                ],
            );
            if !u.job.is_null() {
                ui.add_space(6.0);
                ui.label(RichText::new("Update job").strong().color(p.warn));
                widgets::json_table(ui, "update-job", std::slice::from_ref(&u.job));
            }
        });
    }

    fn caches_ui(&mut self, ui: &mut Ui, env: &Env<'_>, operate: bool) {
        let busy = self.t_op.running();
        let mut set_budget = None;
        ui.columns(2, |cols| {
            widgets::section(&mut cols[0], "Postings cache", |ui| {
                widgets::muted(ui, "Decoded large postings lists (hot filter values, common tokens).");
                match &self.postings {
                    Some(c) => stats_grid(
                        ui,
                        "op-postings",
                        &[
                            ("Entries", format::int(c.entries)),
                            ("Used", format::bytes(c.bytes)),
                            ("Budget", format::bytes(c.budget)),
                            ("Hits / misses", format!("{} / {}", format::int(c.hits), format::int(c.misses))),
                            ("Hit rate", format::percent(c.hit_rate())),
                        ],
                    ),
                    None => {
                        widgets::muted(ui, "Loading…");
                    }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Budget");
                    ui.add_enabled(operate, egui::DragValue::new(&mut self.budget_mib).range(0..=1_048_576).suffix(" MiB"));
                    if ui
                        .add_enabled(operate && !busy, egui::Button::new("Apply"))
                        .on_disabled_hover_text("Requires system.operate")
                        .clicked()
                    {
                        set_budget = Some(self.budget_mib);
                    }
                    if ui
                        .add_enabled(operate && !busy, egui::Button::new("Clear"))
                        .on_disabled_hover_text("Requires system.operate")
                        .clicked()
                    {
                        self.confirm = Some(Confirm::ClearPostings);
                    }
                });
            });
            widgets::section(&mut cols[1], "Response cache", |ui| {
                widgets::muted(ui, "Whole responses of repeated data requests.");
                match &self.response {
                    Some(c) if c.enabled == Some(false) => {
                        widgets::muted(ui, "Disabled on this server.");
                    }
                    Some(c) => {
                        stats_grid(
                            ui,
                            "op-response",
                            &[
                                ("Entries", format::int(c.entries)),
                                ("Used", format::bytes(c.bytes)),
                                ("Budget", format::bytes(c.budget)),
                                ("Hits / misses", format!("{} / {}", format::int(c.hits), format::int(c.misses))),
                                ("Hit rate", format::percent(c.hit_rate())),
                                ("Coalesced", format::int(c.coalesced)),
                            ],
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            let clear = ui.add_enabled(operate && !busy, egui::Button::new("Clear"));
                            if clear.on_disabled_hover_text("Requires system.operate").clicked() {
                                self.confirm = Some(Confirm::ClearResponse);
                            }
                        });
                    }
                    None if self.response_missing => {
                        widgets::muted(ui, "Not available on this server.");
                    }
                    None => {
                        widgets::muted(ui, "Loading…");
                    }
                }
            });
        });
        if let Some(mib) = set_budget {
            self.op(env, move |c| {
                let s: CacheStats = c.post("/admin/api/cache", &json!({ "budget_mib": mib }))?;
                Ok(OpResult::Message(format!("Postings cache budget set to {}", format::bytes(s.budget))))
            });
        }
    }

    fn settings_ui(&mut self, ui: &mut Ui, env: &Env<'_>) {
        let can = env.can(perm::SETTINGS_MANAGE);
        let Some(saved) = self.settings.clone() else {
            widgets::muted(ui, "Loading…");
            return;
        };
        let plans = env.session.plan_names();
        let busy = self.t_op.running();
        widgets::section(ui, "Server settings", |ui| {
            if !can {
                widgets::muted(ui, "Read-only: changing settings requires settings.manage.");
            }
            ui.add_enabled_ui(can, |ui| {
                egui::Grid::new("settings").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                    ui.label("Registration");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.edit.registration, "open".to_string(), "open");
                        ui.selectable_value(&mut self.edit.registration, "closed".to_string(), "closed");
                    });
                    ui.end_row();
                    ui.label("Default plan");
                    widgets::combo(ui, "default-plan", &mut self.edit.default_plan, &plans, None);
                    ui.end_row();
                    ui.label("Search log");
                    ui.checkbox(&mut self.edit.search_log, "record every data request");
                    ui.end_row();
                    ui.label("Search log retention");
                    ui.add_enabled(
                        self.edit.search_log,
                        egui::DragValue::new(&mut self.edit.search_log_days).range(1..=3650).suffix(" days"),
                    );
                    ui.end_row();
                    ui.label("Automatic updates");
                    ui.checkbox(&mut self.edit.auto_update, "download, build and publish new months");
                    ui.end_row();
                    ui.label("Update check interval");
                    ui.add_enabled(
                        self.edit.auto_update,
                        egui::DragValue::new(&mut self.edit.update_check_hours).range(1..=720).suffix(" hours"),
                    );
                    ui.end_row();
                });
            });
            let patch = settings_patch(&saved, &self.edit);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if widgets::primary(ui, can && !busy && !patch.is_empty(), "Save settings").clicked() {
                    let body = Value::Object(patch.clone());
                    self.t_op = Task::call(&env.ctx, env.client(), move |c| {
                        let _: Value = c.patch("/admin/api/settings", &body)?;
                        Ok(OpResult::Message("Settings saved".into()))
                    });
                }
                if ui.add_enabled(!patch.is_empty(), egui::Button::new("Discard")).clicked() {
                    self.edit = saved.clone();
                }
                if !patch.is_empty() {
                    widgets::muted(ui, format!("{} change(s)", patch.len()));
                }
            });
        });
        if let Some(Value::Object(raw)) = &self.settings_raw {
            let extra: Vec<(&str, String)> =
                raw.iter().filter(|(k, _)| !SETTINGS_KEYS.contains(&k.as_str())).map(|(k, v)| (k.as_str(), format::value(v))).collect();
            if !extra.is_empty() {
                ui.add_space(8.0);
                widgets::section(ui, "Server configuration (read-only)", |ui| stats_grid(ui, "settings-extra", &extra));
            }
        }
    }

    fn confirm_ui(&mut self, ctx: &egui::Context, env: &Env<'_>) {
        let Some(confirm) = &self.confirm else { return };
        let (title, body, action, danger): (String, String, &str, bool) = match confirm {
            Confirm::Activate(n) => (
                format!("Activate {n}"),
                "The server switches to this snapshot without dropping requests and moves the `current` pointer.".into(),
                "Activate",
                false,
            ),
            Confirm::Reload => ("Reload snapshot".into(), "Re-opens the snapshot named by the `current` pointer.".into(), "Reload", false),
            Confirm::StopJob(id) => {
                (format!("Stop job #{id}"), "The child process is killed; partial output stays on disk.".into(), "Stop job", true)
            }
            Confirm::ClearPostings => {
                ("Clear postings cache".into(), "Queries get slower until the cache warms up again.".into(), "Clear", true)
            }
            Confirm::ClearResponse => {
                ("Clear response cache".into(), "Repeated requests are recomputed until the cache warms up again.".into(), "Clear", true)
            }
            Confirm::StartJob(r) => {
                let mut desc = format!("kind: {}", r.kind);
                if let Some(p) = &r.profile {
                    desc.push_str(&format!(", profile: {p}"));
                }
                if let Some(m) = &r.month {
                    desc.push_str(&format!(", month: {m}"));
                }
                if r.sources {
                    desc.push_str(", verify against sources");
                }
                if let Some(q) = r.queries {
                    desc.push_str(&format!(", queries: {q}"));
                }
                (format!("Start {} job", r.kind), format!("Runs on the server as a child process ({desc})."), "Start", false)
            }
            Confirm::CheckUpdates => (
                "Check for updates".into(),
                "Asks the source for a newer month now; an update job starts if one is found and automatic updates are on.".into(),
                "Check now",
                false,
            ),
        };
        match widgets::confirm(ctx, &title, &body, action, danger) {
            Some(true) => {
                let Some(c) = self.confirm.take() else { return };
                match c {
                    Confirm::Activate(name) => self.op(env, move |c| {
                        let _: Value = c.post("/admin/api/snapshots/activate", &json!({ "name": name }))?;
                        Ok(OpResult::Message(format!("Activated {name}")))
                    }),
                    Confirm::Reload => self.op(env, |c| {
                        let v: Value = c.post_empty("/admin/api/reload")?;
                        Ok(OpResult::Message(format!("Reloaded {}", format::value(&v["reloaded"]))))
                    }),
                    Confirm::StopJob(id) => self.op(env, move |c| {
                        let v: Value = c.delete(&format!("/admin/api/jobs/{id}"))?;
                        Ok(OpResult::Message(if v["killed"] == json!(true) {
                            format!("Stopping job #{id}")
                        } else {
                            format!("Job #{id} was not running")
                        }))
                    }),
                    Confirm::ClearPostings => self.op(env, |c| {
                        let _: Value = c.delete("/admin/api/cache")?;
                        Ok(OpResult::Message("Postings cache cleared".into()))
                    }),
                    Confirm::ClearResponse => self.op(env, |c| {
                        let _: Value = c.delete("/admin/api/response-cache")?;
                        Ok(OpResult::Message("Response cache cleared".into()))
                    }),
                    Confirm::StartJob(req) => self.op(env, move |c| {
                        let body = serde_json::to_value(&req).unwrap_or(Value::Null);
                        let r: JobStarted = c.post("/admin/api/jobs", &body)?;
                        Ok(OpResult::JobStarted(r.id))
                    }),
                    Confirm::CheckUpdates => self.op(env, |c| {
                        let _: Value = c.post_empty("/admin/api/updates/check")?;
                        Ok(OpResult::Message("Update check started".into()))
                    }),
                }
            }
            Some(false) => self.confirm = None,
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months() {
        assert!(valid_month(""));
        assert!(valid_month("2026-10"));
        assert!(valid_month(" 2026-01 "));
        assert!(!valid_month("2026-13"));
        assert!(!valid_month("2026-00"));
        assert!(!valid_month("2026/10"));
        assert!(!valid_month("26-10"));
    }

    #[test]
    fn job_requests_carry_only_relevant_options() {
        let mut f = JobForm { kind: "verify".into(), profile: "fast".into(), month: "2026-10".into(), sources: true, queries: 50 };
        assert_eq!(serde_json::to_value(f.request()).unwrap(), json!({"kind":"verify","sources":true}));
        f.kind = "bench".into();
        assert_eq!(serde_json::to_value(f.request()).unwrap(), json!({"kind":"bench","sources":false,"queries":50}));
        f.kind = "build".into();
        assert_eq!(serde_json::to_value(f.request()).unwrap(), json!({"kind":"build","profile":"fast","month":"2026-10","sources":false}));
        f.kind = "download".into();
        f.month.clear();
        assert_eq!(serde_json::to_value(f.request()).unwrap(), json!({"kind":"download","sources":false}));
        f.kind = "update".into();
        assert_eq!(serde_json::to_value(f.request()).unwrap(), json!({"kind":"update","profile":"fast","sources":true}));
    }

    #[test]
    fn settings_diff() {
        let a = Settings {
            registration: "open".into(),
            default_plan: "free".into(),
            search_log: true,
            search_log_days: 30,
            auto_update: false,
            update_check_hours: 24,
        };
        assert!(settings_patch(&a, &a).is_empty());
        let mut b = a.clone();
        b.registration = "closed".into();
        b.update_check_hours = 6;
        assert_eq!(Value::Object(settings_patch(&a, &b)), json!({"registration":"closed","update_check_hours":6}));
    }
}
