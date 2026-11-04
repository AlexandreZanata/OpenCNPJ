//! Users: searchable, filterable, sortable, paged account list with a detail panel and every
//! moderation action. Calls `GET/POST /admin/api/users`, `GET/PATCH/DELETE
//! /admin/api/users/{id}` and `POST /admin/api/users/{id}/{suspend,ban,reactivate,rotate-key,
//! revoke-sessions,reset-usage}`.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Layout, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use serde_json::{Map, Value, json};

use crate::api::{ApiResult, Client, Task};
use crate::format;
use crate::models::{CreatedUser, Revoked, RotatedKey, User, UserDetail, UserEnvelope, UserList, UserUsageDay};
use crate::state::{Env, STATUSES, perm};
use crate::theme::Palette;
use crate::widgets::{self, OptNum, RevealedKey, Series};

const DEBOUNCE: Duration = Duration::from_millis(350);
const PAGE_SIZES: [u64; 4] = [25, 50, 100, 250];

/// Sort keys accepted by `GET /admin/api/users`.
pub const SORTS: [(&str, &str); 6] = [
    ("-created", "Newest first"),
    ("created", "Oldest first"),
    ("username", "Username"),
    ("-usage_today", "Most requests today"),
    ("usage_today", "Fewest requests today"),
    ("last_seen", "Last seen"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub q: String,
    pub status: String,
    pub role: String,
    pub plan: String,
    pub sort: String,
    pub limit: u64,
    pub offset: u64,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            q: String::new(),
            status: String::new(),
            role: String::new(),
            plan: String::new(),
            sort: "-created".into(),
            limit: 50,
            offset: 0,
        }
    }
}

impl Filter {
    pub fn path(&self) -> String {
        format::with_query(
            "/admin/api/users",
            &[
                ("q", self.q.clone()),
                ("status", self.status.clone()),
                ("role", self.role.clone()),
                ("plan", self.plan.clone()),
                ("sort", self.sort.clone()),
                ("limit", self.limit.to_string()),
                ("offset", self.offset.to_string()),
            ],
        )
    }
}

/// Columns of the account table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Col {
    Id,
    Username,
    Email,
    Role,
    Status,
    Plan,
    Today,
    Month,
    Created,
    LastSeen,
}

impl Col {
    const ALL: [Col; 10] =
        [Col::Id, Col::Username, Col::Email, Col::Role, Col::Status, Col::Plan, Col::Today, Col::Month, Col::Created, Col::LastSeen];
    const COMPACT: [Col; 6] = [Col::Id, Col::Username, Col::Role, Col::Status, Col::Plan, Col::Today];

    fn column(self, compact: bool) -> Column {
        match self {
            Col::Id => Column::auto().at_least(44.0),
            Col::Username if compact => Column::remainder().at_least(120.0).clip(true),
            Col::Username => Column::auto().at_least(120.0).clip(true),
            Col::Email => Column::remainder().at_least(160.0).clip(true),
            Col::Role => Column::auto().at_least(80.0),
            Col::Status => Column::auto().at_least(84.0),
            Col::Plan => Column::auto().at_least(80.0).clip(true),
            Col::Today => Column::initial(70.0),
            Col::Month => Column::initial(84.0),
            Col::Created => Column::auto().at_least(92.0),
            Col::LastSeen => Column::auto().at_least(80.0),
        }
    }
}

/// Profile and limit overrides being edited.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Editor {
    pub for_id: i64,
    pub email: String,
    pub role: String,
    pub plan: String,
    pub daily: OptNum,
    pub monthly: OptNum,
    pub per_minute: OptNum,
    pub notes: String,
}

impl Editor {
    pub fn from_user(u: &User) -> Self {
        Self {
            for_id: u.id,
            email: u.email.clone(),
            role: u.role.clone(),
            plan: u.plan.clone(),
            daily: OptNum::from(u.daily_limit, 1000),
            monthly: OptNum::from(u.monthly_limit, 20_000),
            per_minute: OptNum::from(u.per_minute, 60),
            notes: u.notes.clone(),
        }
    }

    /// PATCH body with the profile fields that differ from `u` (`null` clears an override).
    pub fn profile_patch(&self, u: &User) -> Map<String, Value> {
        let mut m = Map::new();
        if self.email.trim() != u.email {
            m.insert("email".into(), json!(self.email.trim()));
        }
        if self.role != u.role {
            m.insert("role".into(), json!(self.role));
        }
        if self.plan != u.plan {
            m.insert("plan".into(), json!(self.plan));
        }
        for (k, v, old) in [
            ("daily_limit", &self.daily, u.daily_limit),
            ("monthly_limit", &self.monthly, u.monthly_limit),
            ("per_minute", &self.per_minute, u.per_minute),
        ] {
            if v.get() != old {
                m.insert(k.into(), v.json());
            }
        }
        m
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unit {
    Hours,
    Days,
}

impl Unit {
    pub fn seconds(self) -> i64 {
        match self {
            Unit::Hours => 3600,
            Unit::Days => 86_400,
        }
    }
}

enum Dialog {
    Suspend { reason: String, indefinite: bool, amount: u32, unit: Unit },
    Ban { reason: String },
    Reactivate,
    Rotate { send_email: bool },
    RevokeSessions,
    ResetUsage(&'static str),
    Delete { confirm: String },
}

struct CreateForm {
    username: String,
    email: String,
    role: String,
    plan: String,
    send_email: bool,
}

/// A moderation call to run in the background.
type Action = Box<dyn FnOnce(&Client) -> ApiResult<Outcome> + Send>;

enum Outcome {
    User(Box<User>, String),
    Key(String, RotatedKey),
    Message(String),
    Deleted(String),
}

#[derive(Default)]
pub struct Users {
    filter: Filter,
    applied: Option<Filter>,
    q_edited: Option<Instant>,
    list: Option<UserList>,
    t_list: Task<UserList>,
    selected: Option<i64>,
    detail: Option<UserDetail>,
    t_detail: Task<UserDetail>,
    editor: Editor,
    t_action: Task<Outcome>,
    dialog: Option<Dialog>,
    create: Option<CreateForm>,
    t_create: Task<CreatedUser>,
    revealed: Option<RevealedKey>,
}

/// Day labels and per-kind counts (stacked chart data) from `usage_daily`.
pub fn usage_by_day(rows: &[UserUsageDay], max_days: usize) -> (Vec<String>, Vec<(String, Vec<f64>)>) {
    let days: BTreeSet<&str> = rows.iter().map(|r| r.day.as_str()).collect();
    let days: Vec<String> = days.into_iter().rev().take(max_days).collect::<Vec<_>>().into_iter().rev().map(str::to_string).collect();
    let kinds: BTreeSet<&str> = rows.iter().map(|r| r.kind.as_str()).collect();
    let series = kinds
        .into_iter()
        .map(|k| {
            let mut v = vec![0.0; days.len()];
            for r in rows.iter().filter(|r| r.kind == k) {
                if let Some(i) = days.iter().position(|d| *d == r.day) {
                    v[i] += r.count as f64;
                }
            }
            (if k.is_empty() { "requests".to_string() } else { k.to_string() }, v)
        })
        .collect();
    (days, series)
}

impl Users {
    /// Selects `id` and loads its detail (used by other screens).
    pub fn open(&mut self, id: i64, env: &Env<'_>) {
        self.selected = Some(id);
        self.load_detail(env);
    }

    fn load_list(&mut self, env: &Env<'_>) {
        let path = self.filter.path();
        self.applied = Some(self.filter.clone());
        self.t_list = Task::call(&env.ctx, env.client(), move |c| c.get(&path));
    }

    fn load_detail(&mut self, env: &Env<'_>) {
        if let Some(id) = self.selected {
            self.t_detail = Task::call(&env.ctx, env.client(), move |c| c.get(&format!("/admin/api/users/{id}")));
        }
    }

    fn act(&mut self, env: &Env<'_>, f: impl FnOnce(&Client) -> ApiResult<Outcome> + Send + 'static) {
        self.t_action = Task::call(&env.ctx, env.client(), f);
    }

    pub fn poll(&mut self, env: &mut Env<'_>) {
        if let Some(r) = self.t_list.poll()
            && let Some(l) = env.check(r)
        {
            self.list = Some(l);
        }
        if let Some(r) = self.t_detail.poll() {
            match r {
                Ok(d) => {
                    if self.editor.for_id != d.user.id || self.detail.as_ref().is_none_or(|old| old.user != d.user) {
                        self.editor = Editor::from_user(&d.user);
                    }
                    self.detail = Some(d);
                }
                Err(e) => {
                    if e.is_not_found() {
                        self.selected = None;
                        self.detail = None;
                    }
                    env.report(&e);
                }
            }
        }
        if let Some(r) = self.t_create.poll()
            && let Some(c) = env.check(r)
        {
            env.toasts.success(format!("Created account {}", c.user.username));
            self.create = None;
            self.revealed = Some(RevealedKey { title: format!("Login key for {}", c.user.username), key: c.key });
            self.selected = Some(c.user.id);
            self.load_detail(env);
            self.load_list(env);
        }
        if let Some(r) = self.t_action.poll()
            && let Some(o) = env.check(r)
        {
            match o {
                Outcome::User(u, msg) => {
                    env.toasts.success(msg);
                    self.editor = Editor::from_user(&u);
                    if let Some(d) = &mut self.detail {
                        d.user = *u;
                    }
                }
                Outcome::Key(username, k) => {
                    env.toasts.success(format!("Issued a new key for {username} (prefix {})", k.key_prefix));
                    self.revealed = Some(RevealedKey { title: format!("New login key for {username}"), key: k.key });
                }
                Outcome::Message(m) => env.toasts.success(m),
                Outcome::Deleted(name) => {
                    env.toasts.success(format!("Deleted account {name}"));
                    self.selected = None;
                    self.detail = None;
                }
            }
            self.load_detail(env);
            self.load_list(env);
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
        if self.applied.is_none() && !self.t_list.running() {
            self.load_list(env);
        }
        if let Some(t) = self.q_edited
            && t.elapsed() >= DEBOUNCE
        {
            self.q_edited = None;
            self.filter.offset = 0;
            self.load_list(env);
        } else if self.q_edited.is_some() {
            ui.ctx().request_repaint_after(DEBOUNCE);
        }

        let total = self.list.as_ref().map_or(0, |l| l.total);
        if widgets::header(ui, "Users", &format!("{} accounts", format::int(total)), self.t_list.running()) {
            self.load_list(env);
            self.load_detail(env);
        }
        self.filter_bar(ui, env);
        ui.add_space(4.0);

        if self.selected.is_some() {
            egui::SidePanel::right("user-detail").resizable(true).default_width(480.0).min_width(380.0).show_inside(ui, |ui| {
                self.detail_panel(ui, env);
            });
        }
        egui::TopBottomPanel::bottom("users-pager").show_inside(ui, |ui| self.pager(ui, env));
        egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(ui, |ui| self.table(ui, env));

        self.dialogs(ui.ctx(), env);
        self.create_dialog(ui.ctx(), env);
        widgets::key_modal(ui.ctx(), &mut self.revealed);
    }

    fn filter_bar(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
        let roles = env.session.role_names();
        let plans = env.session.plan_names();
        let statuses: Vec<String> = STATUSES.iter().map(|s| s.to_string()).collect();
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.filter.q).hint_text("🔍 username or e-mail").desired_width(220.0));
            if r.changed() {
                self.q_edited = Some(Instant::now());
            }
            ui.label("Status");
            changed |= widgets::combo(ui, "f-status", &mut self.filter.status, &statuses, Some("any"));
            ui.label("Role");
            changed |= widgets::combo(ui, "f-role", &mut self.filter.role, &roles, Some("any"));
            ui.label("Plan");
            changed |= widgets::combo(ui, "f-plan", &mut self.filter.plan, &plans, Some("any"));
            ui.label("Sort");
            let label = SORTS.iter().find(|(k, _)| *k == self.filter.sort).map_or("custom", |(_, l)| l);
            egui::ComboBox::from_id_salt("f-sort").selected_text(label).width(170.0).show_ui(ui, |ui| {
                for (k, l) in SORTS {
                    changed |= ui.selectable_value(&mut self.filter.sort, k.to_string(), l).changed();
                }
            });
            if ui.button("Clear").clicked() {
                self.filter = Filter { limit: self.filter.limit, ..Filter::default() };
                changed = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let can = env.can(perm::USERS_MANAGE);
                let b = widgets::primary(ui, can, "➕ New user").on_disabled_hover_text("Requires users.manage");
                if b.clicked() {
                    self.create = Some(CreateForm {
                        username: String::new(),
                        email: String::new(),
                        role: "user".into(),
                        plan: String::new(),
                        send_email: false,
                    });
                }
            });
        });
        if changed {
            self.filter.offset = 0;
            self.load_list(env);
        }
    }

    fn pager(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
        let total = self.list.as_ref().map_or(0, |l| l.total);
        let limit = self.filter.limit.max(1);
        let pages = total.div_ceil(limit).max(1);
        let page = self.filter.offset / limit + 1;
        let mut reload = false;
        ui.horizontal(|ui| {
            if ui.add_enabled(self.filter.offset > 0, egui::Button::new("◀ Previous")).clicked() {
                self.filter.offset = self.filter.offset.saturating_sub(limit);
                reload = true;
            }
            ui.label(widgets::mono(format!("page {page} of {pages}")));
            if ui.add_enabled(page < pages, egui::Button::new("Next ▶")).clicked() {
                self.filter.offset += limit;
                reload = true;
            }
            ui.separator();
            ui.label("Rows per page");
            egui::ComboBox::from_id_salt("page-size").selected_text(limit.to_string()).width(70.0).show_ui(ui, |ui| {
                for s in PAGE_SIZES {
                    if ui.selectable_value(&mut self.filter.limit, s, s.to_string()).changed() {
                        self.filter.offset = 0;
                        reload = true;
                    }
                }
            });
            let shown = self.list.as_ref().map_or(0, |l| l.users.len()) as u64;
            if shown > 0 {
                widgets::muted(ui, format!("showing {}–{} of {}", self.filter.offset + 1, self.filter.offset + shown, format::int(total)));
            }
        });
        if reload {
            self.load_list(env);
        }
    }

    fn sort_header(&mut self, ui: &mut Ui, label: &str, asc: Option<&str>, desc: Option<&str>) -> bool {
        let cur = self.filter.sort.as_str();
        let arrow = if asc == Some(cur) {
            " ⏶"
        } else if desc == Some(cur) {
            " ⏷"
        } else {
            ""
        };
        let r =
            ui.add(egui::Label::new(RichText::new(format!("{label}{arrow}")).strong()).sense(egui::Sense::click())).on_hover_text("Sort");
        if r.clicked() {
            let next = match (asc, desc) {
                (Some(a), Some(d)) => {
                    if cur == d {
                        a
                    } else {
                        d
                    }
                }
                (Some(a), None) => a,
                (None, Some(d)) => d,
                (None, None) => return false,
            };
            self.filter.sort = next.to_string();
            self.filter.offset = 0;
            return true;
        }
        false
    }

    fn table(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
        let Some(list) = self.list.take() else {
            widgets::muted(ui, if self.t_list.running() { "Loading…" } else { "No data." });
            return;
        };
        if list.users.is_empty() {
            widgets::muted(ui, "No accounts match these filters.");
        }
        let p = Palette::get(ui);
        let mut resort = false;
        let mut clicked = None;
        let now = format::now_unix();
        // With the detail panel open, keep the columns that matter for triage.
        let compact = self.selected.is_some();
        let cols: &[Col] = if compact { &Col::COMPACT } else { &Col::ALL };
        let mut tb = TableBuilder::new(ui)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .id_salt(("users-table", cols.len()))
            .striped(true)
            .sense(egui::Sense::click())
            .auto_shrink(false);
        for c in cols {
            tb = tb.column(c.column(compact));
        }
        tb.header(widgets::ROW_H, |mut h| {
            for c in cols {
                h.col(|ui| match c {
                    Col::Id => {
                        ui.strong("ID");
                    }
                    Col::Username => resort |= self.sort_header(ui, "Username", Some("username"), None),
                    Col::Email => {
                        ui.strong("E-mail");
                    }
                    Col::Role => {
                        ui.strong("Role");
                    }
                    Col::Status => {
                        ui.strong("Status");
                    }
                    Col::Plan => {
                        ui.strong("Plan");
                    }
                    Col::Today => {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            resort |= self.sort_header(ui, "Today", Some("usage_today"), Some("-usage_today"));
                        });
                    }
                    Col::Month => widgets::num_header(ui, "Month"),
                    Col::Created => resort |= self.sort_header(ui, "Created", Some("created"), Some("-created")),
                    Col::LastSeen => resort |= self.sort_header(ui, "Last seen", None, Some("last_seen")),
                });
            }
        })
        .body(|body| {
            body.rows(widgets::ROW_H, list.users.len(), |mut row| {
                let u = &list.users[row.index()];
                row.set_selected(self.selected == Some(u.id));
                for c in cols {
                    row.col(|ui| match c {
                        Col::Id => {
                            ui.label(widgets::mono(u.id.to_string()));
                        }
                        Col::Username => {
                            ui.label(RichText::new(&u.username).strong());
                        }
                        Col::Email => {
                            ui.label(&u.email);
                        }
                        Col::Role => {
                            let color = if u.role == "user" { p.muted } else { p.accent };
                            widgets::pill(ui, &u.role, color);
                        }
                        Col::Status => {
                            widgets::status_pill(ui, &u.status);
                        }
                        Col::Plan => {
                            ui.label(&u.plan);
                        }
                        Col::Today => widgets::num(ui, u.usage_today.map_or("–".into(), format::signed)),
                        Col::Month => widgets::num(ui, u.usage_month.map_or("–".into(), format::signed)),
                        Col::Created => {
                            ui.label(widgets::mono(u.created_at.map_or("–".into(), format::date)));
                        }
                        Col::LastSeen => {
                            ui.label(widgets::mono(u.last_seen_at.map_or("never".into(), |t| format::ago(t, now))))
                                .on_hover_text(format::opt_ts(u.last_seen_at));
                        }
                    });
                }
                if row.response().clicked() {
                    clicked = Some(u.id);
                }
            });
        });
        self.list = Some(list);
        if resort {
            self.load_list(env);
        }
        if let Some(id) = clicked {
            if self.selected == Some(id) {
                self.selected = None;
                self.detail = None;
            } else {
                self.open(id, env);
            }
        }
    }

    fn detail_panel(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
        ui.add_space(6.0);
        let Some(d) = self.detail.take() else {
            ui.horizontal(|ui| {
                ui.spinner();
                widgets::muted(ui, "Loading account…");
            });
            return;
        };
        if d.user.id != self.selected.unwrap_or_default() {
            ui.spinner();
            self.detail = Some(d);
            return;
        }
        let u = &d.user;
        let p = Palette::get(ui);
        let busy = self.t_action.running() || self.t_detail.running();
        ui.horizontal(|ui| {
            ui.heading(&u.username);
            widgets::status_pill(ui, &u.status);
            if u.id == env.session.user.id {
                widgets::pill(ui, "you", p.accent);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("✖").on_hover_text("Close").clicked() {
                    self.selected = None;
                }
                if busy {
                    ui.spinner();
                }
            });
        });
        egui::ScrollArea::vertical().id_salt("user-detail-scroll").auto_shrink(false).show(ui, |ui| {
            self.profile(ui, &d);
            ui.add_space(8.0);
            self.actions(ui, u, env, busy);
            ui.add_space(8.0);
            self.editor_ui(ui, u, env, busy);
            ui.add_space(8.0);
            widgets::section(ui, "Usage, last 30 days", |ui| {
                let (days, series) = usage_by_day(&d.usage_daily, 30);
                if days.is_empty() {
                    widgets::muted(ui, "No usage recorded.");
                } else {
                    let colors = [p.accent, p.accent2, p.ok, p.warn, p.bad, p.muted];
                    let s: Vec<Series<'_>> = series
                        .iter()
                        .enumerate()
                        .map(|(i, (k, v))| Series { name: k, values: v.clone(), color: colors[i % colors.len()] })
                        .collect();
                    let short: Vec<String> = days.iter().map(|d| d.get(5..).unwrap_or(d).to_string()).collect();
                    widgets::bar_chart(ui, "user-usage", &short, &s, true, 170.0);
                    let total: u64 = d.usage_daily.iter().map(|r| r.count).sum();
                    let hits: u64 = d.usage_daily.iter().map(|r| r.cache_hits).sum();
                    widgets::muted(
                        ui,
                        format!(
                            "{} requests, {} cache hits ({})",
                            format::int(total),
                            format::int(hits),
                            format::percent((total > 0).then(|| hits as f64 / total as f64))
                        ),
                    );
                }
            });
            ui.add_space(8.0);
            egui::CollapsingHeader::new(format!("Keys ({})", d.keys.len())).id_salt("keys").default_open(true).show(ui, |ui| {
                widgets::json_table(ui, "keys", &d.keys);
            });
            egui::CollapsingHeader::new(format!("Sessions ({})", d.sessions.len())).id_salt("sessions").default_open(false).show(
                ui,
                |ui| {
                    widgets::json_table(ui, "sessions", &d.sessions);
                },
            );
            egui::CollapsingHeader::new(format!("Recent searches ({})", d.recent_searches.len()))
                .id_salt("recent")
                .default_open(false)
                .show(ui, |ui| {
                    if d.recent_searches.is_empty() {
                        widgets::muted(ui, "none");
                    }
                    egui::Grid::new("recent-grid").striped(true).num_columns(4).show(ui, |ui| {
                        for s in &d.recent_searches {
                            ui.label(widgets::mono(format::opt_ts(s.ts)));
                            ui.label(widgets::mono(&s.status));
                            ui.label(widgets::mono(format!("{} ms", format::ms(s.ms))));
                            ui.label(format!("{} {}", s.route, format::value(&s.query)));
                            ui.end_row();
                        }
                    });
                });
        });
        self.detail = Some(d);
    }

    fn profile(&self, ui: &mut Ui, d: &UserDetail) {
        let u = &d.user;
        let p = Palette::get(ui);
        widgets::section(ui, "Profile", |ui| {
            egui::Grid::new("profile").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                let mut row = |k: &str, v: String| {
                    ui.label(RichText::new(k).color(p.muted));
                    ui.label(widgets::mono(v));
                    ui.end_row();
                };
                row("ID", u.id.to_string());
                row("E-mail", u.email.clone());
                row("Role", u.role.clone());
                row("Plan", u.plan.clone());
                row("Key prefix", u.key_prefix.clone().unwrap_or_else(|| "–".into()));
                row("Created", format::opt_ts(u.created_at));
                row("Verified", u.verified_at.map_or("not yet (pending)".into(), format::ts));
                row("Last seen", format::opt_ts(u.last_seen_at));
                row(
                    "Limits",
                    format!(
                        "day {} · month {} · {}/min",
                        u.daily_limit.map_or("plan".into(), format::signed),
                        u.monthly_limit.map_or("plan".into(), format::signed),
                        u.per_minute.map_or("plan".into(), format::signed)
                    ),
                );
                // Live counters, when the server includes them (not in the documented shape).
                let live = |k: &str| d.usage_live.get(k).and_then(crate::models::value_i64);
                if let (Some(today), Some(month)) = (live("today"), live("month")) {
                    row("Requests now", format!("today {} · month {}", format::signed(today), format::signed(month)));
                }
            });
            if u.status == "suspended" || u.status == "banned" || u.status_reason.is_some() {
                ui.add_space(6.0);
                let c = p.status(&u.status);
                ui.label(RichText::new(format!("Status: {}", u.status)).color(c).strong());
                if let Some(r) = &u.status_reason {
                    ui.label(format!("Reason: {r}"));
                }
                if let Some(t) = u.suspended_until {
                    ui.label(format!("Until: {} UTC ({})", format::ts(t), format::ago(t, format::now_unix())));
                }
            }
        });
    }

    fn actions(&mut self, ui: &mut Ui, u: &User, env: &Env<'_>, busy: bool) {
        let s = env.session;
        let is_self = u.id == s.user.id;
        let manage = s.can(perm::USERS_MANAGE);
        let moderate = s.can_moderate(u);
        let moderate_any = s.can(perm::USERS_MODERATE) && (manage || u.role == "user");
        let why = if is_self {
            "Not available on your own account"
        } else if !s.can(perm::USERS_MODERATE) {
            "Requires users.moderate"
        } else {
            "Moderators act only on user accounts"
        };
        widgets::section(ui, "Actions", |ui| {
            ui.horizontal_wrapped(|ui| {
                let suspended = u.status == "suspended";
                let banned = u.status == "banned";
                if ui
                    .add_enabled(!busy && moderate && !suspended && !banned, egui::Button::new("⏸ Suspend…"))
                    .on_disabled_hover_text(why)
                    .clicked()
                {
                    self.dialog = Some(Dialog::Suspend { reason: String::new(), indefinite: false, amount: 7, unit: Unit::Days });
                }
                if ui.add_enabled(!busy && moderate && !banned, egui::Button::new("⛔ Ban…")).on_disabled_hover_text(why).clicked() {
                    self.dialog = Some(Dialog::Ban { reason: String::new() });
                }
                let can_reactivate = moderate && (suspended || banned) && (!banned || manage);
                if ui
                    .add_enabled(!busy && can_reactivate, egui::Button::new("▶ Reactivate"))
                    .on_disabled_hover_text("Only suspended or banned accounts; banned accounts need an admin")
                    .clicked()
                {
                    self.dialog = Some(Dialog::Reactivate);
                }
                if ui.add_enabled(!busy && moderate, egui::Button::new("🔑 Rotate key…")).on_disabled_hover_text(why).clicked() {
                    self.dialog = Some(Dialog::Rotate { send_email: false });
                }
                if ui.add_enabled(!busy && moderate, egui::Button::new("Revoke sessions…")).on_disabled_hover_text(why).clicked() {
                    self.dialog = Some(Dialog::RevokeSessions);
                }
                if ui.add_enabled(!busy && moderate_any, egui::Button::new("Reset usage (day)…")).on_disabled_hover_text(why).clicked() {
                    self.dialog = Some(Dialog::ResetUsage("day"));
                }
                if ui.add_enabled(!busy && moderate_any, egui::Button::new("Reset usage (month)…")).on_disabled_hover_text(why).clicked()
                {
                    self.dialog = Some(Dialog::ResetUsage("month"));
                }
                let can_delete = manage && !is_self;
                if widgets::danger(ui, !busy && can_delete, "🗑 Delete…")
                    .on_disabled_hover_text(if is_self { "You cannot delete your own account" } else { "Requires users.manage" })
                    .clicked()
                {
                    self.dialog = Some(Dialog::Delete { confirm: String::new() });
                }
            });
        });
    }

    fn editor_ui(&mut self, ui: &mut Ui, u: &User, env: &Env<'_>, busy: bool) {
        let s = env.session;
        let manage = s.can(perm::USERS_MANAGE);
        let is_self = u.id == s.user.id;
        let notes_ok = s.can(perm::USERS_MODERATE) && (manage || u.role == "user");
        let roles = s.role_names();
        let plans = s.plan_names();
        let mut save_profile = false;
        let mut save_notes = false;
        widgets::section(ui, "Role, plan and limit overrides", |ui| {
            ui.add_enabled_ui(manage, |ui| {
                egui::Grid::new("user-editor").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("E-mail");
                    ui.add(egui::TextEdit::singleline(&mut self.editor.email).desired_width(260.0));
                    ui.end_row();
                    ui.label("Role");
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(!is_self, |ui| {
                            widgets::combo(ui, "ed-role", &mut self.editor.role, &roles, None);
                        });
                        if is_self {
                            widgets::muted(ui, "you cannot change your own role");
                        }
                    });
                    ui.end_row();
                    ui.label("Plan");
                    widgets::combo(ui, "ed-plan", &mut self.editor.plan, &plans, None);
                    ui.end_row();
                    ui.label("Daily limit");
                    self.editor.daily.ui(ui, "override", "plan default");
                    ui.end_row();
                    ui.label("Monthly limit");
                    self.editor.monthly.ui(ui, "override", "plan default");
                    ui.end_row();
                    ui.label("Per minute");
                    self.editor.per_minute.ui(ui, "override", "plan default");
                    ui.end_row();
                });
                let patch = self.editor.profile_patch(u);
                ui.horizontal(|ui| {
                    if widgets::primary(ui, !busy && !patch.is_empty(), "Save changes").clicked() {
                        save_profile = true;
                    }
                    if ui.add_enabled(!patch.is_empty(), egui::Button::new("Discard")).clicked() {
                        let notes = std::mem::take(&mut self.editor.notes);
                        self.editor = Editor { notes, ..Editor::from_user(u) };
                    }
                    if !patch.is_empty() {
                        widgets::muted(ui, format!("{} field(s) changed", patch.len()));
                    }
                });
            });
            if !manage {
                widgets::muted(ui, "Editing the profile requires users.manage.");
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Notes (staff only)").strong());
            ui.add_enabled(notes_ok, egui::TextEdit::multiline(&mut self.editor.notes).desired_rows(3).desired_width(f32::INFINITY));
            ui.horizontal(|ui| {
                let changed = self.editor.notes != u.notes;
                if ui.add_enabled(notes_ok && changed && !busy, egui::Button::new("Save notes")).clicked() {
                    save_notes = true;
                }
                if !notes_ok {
                    widgets::muted(ui, "Requires users.moderate");
                }
            });
        });
        let id = u.id;
        if save_profile {
            let body = Value::Object(self.editor.profile_patch(u));
            self.act(env, move |c| {
                let r: UserEnvelope = c.patch(&format!("/admin/api/users/{id}"), &body)?;
                Ok(Outcome::User(Box::new(r.user), "Profile saved".into()))
            });
        }
        if save_notes {
            let body = json!({ "notes": self.editor.notes });
            self.act(env, move |c| {
                let r: UserEnvelope = c.patch(&format!("/admin/api/users/{id}"), &body)?;
                Ok(Outcome::User(Box::new(r.user), "Notes saved".into()))
            });
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context, env: &Env<'_>) {
        let Some(u) = self.detail.as_ref().map(|d| d.user.clone()) else {
            self.dialog = None;
            return;
        };
        let Some(dialog) = &mut self.dialog else { return };
        let id = u.id;
        let name = u.username.clone();
        let mut close = false;
        let mut run: Option<Action> = None;
        match dialog {
            Dialog::Suspend { reason, indefinite, amount, unit } => {
                let r = egui::Modal::new(egui::Id::new("suspend")).show(ctx, |ui| {
                    ui.set_width(440.0);
                    ui.heading(format!("Suspend {name}"));
                    ui.add_space(6.0);
                    ui.label("Reason (shown in the audit log and to the user)");
                    ui.add(egui::TextEdit::multiline(reason).desired_rows(2).desired_width(f32::INFINITY));
                    ui.checkbox(indefinite, "Until reactivated");
                    ui.add_enabled_ui(!*indefinite, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Duration");
                            ui.add(egui::DragValue::new(amount).range(1..=3650));
                            ui.selectable_value(unit, Unit::Hours, "hours");
                            ui.selectable_value(unit, Unit::Days, "days");
                        });
                    });
                    if !*indefinite {
                        let until = format::now_unix() + i64::from(*amount) * unit.seconds();
                        widgets::muted(ui, format!("Lifts itself at {} UTC", format::ts(until)));
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::danger(ui, !reason.trim().is_empty(), "Suspend").clicked() {
                                let mut body = json!({ "reason": reason.trim() });
                                if !*indefinite {
                                    body["until"] = json!(format::now_unix() + i64::from(*amount) * unit.seconds());
                                }
                                let name = name.clone();
                                run = Some(Box::new(move |c| {
                                    let r: UserEnvelope = c.post(&format!("/admin/api/users/{id}/suspend"), &body)?;
                                    Ok(Outcome::User(Box::new(r.user), format!("Suspended {name}")))
                                }));
                            }
                        });
                    });
                });
                close |= r.should_close();
            }
            Dialog::Ban { reason } => {
                let r = egui::Modal::new(egui::Id::new("ban")).show(ctx, |ui| {
                    ui.set_width(440.0);
                    ui.heading(format!("Ban {name}"));
                    ui.add_space(6.0);
                    ui.label("Banned accounts are refused on every request until an admin reactivates them.");
                    ui.label("Reason");
                    ui.add(egui::TextEdit::multiline(reason).desired_rows(2).desired_width(f32::INFINITY));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::danger(ui, !reason.trim().is_empty(), "Ban").clicked() {
                                let body = json!({ "reason": reason.trim() });
                                let name = name.clone();
                                run = Some(Box::new(move |c| {
                                    let r: UserEnvelope = c.post(&format!("/admin/api/users/{id}/ban"), &body)?;
                                    Ok(Outcome::User(Box::new(r.user), format!("Banned {name}")))
                                }));
                            }
                        });
                    });
                });
                close |= r.should_close();
            }
            Dialog::Rotate { send_email } => {
                let r = egui::Modal::new(egui::Id::new("rotate")).show(ctx, |ui| {
                    ui.set_width(440.0);
                    ui.heading(format!("Rotate the key of {name}"));
                    ui.add_space(6.0);
                    ui.label("The current key and every session are revoked immediately. The new key is shown once.");
                    ui.checkbox(send_email, "Also e-mail the new key to the account address");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::danger(ui, true, "Rotate key").clicked() {
                                let body = json!({ "send_email": *send_email });
                                let name = name.clone();
                                run = Some(Box::new(move |c| {
                                    let k: RotatedKey = c.post(&format!("/admin/api/users/{id}/rotate-key"), &body)?;
                                    Ok(Outcome::Key(name, k))
                                }));
                            }
                        });
                    });
                });
                close |= r.should_close();
            }
            Dialog::Delete { confirm } => {
                let r = egui::Modal::new(egui::Id::new("delete")).show(ctx, |ui| {
                    let p = Palette::get(ui);
                    ui.set_width(440.0);
                    ui.heading(format!("Delete {name}"));
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("This permanently deletes the account, its keys, sessions and usage. It cannot be undone.")
                            .color(p.bad),
                    );
                    ui.label(format!("Type the username ({name}) to confirm:"));
                    ui.add(egui::TextEdit::singleline(confirm).desired_width(f32::INFINITY));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::danger(ui, confirm.trim() == name, "Delete account").clicked() {
                                let name = name.clone();
                                run = Some(Box::new(move |c| {
                                    let _: Value = c.delete(&format!("/admin/api/users/{id}"))?;
                                    Ok(Outcome::Deleted(name))
                                }));
                            }
                        });
                    });
                });
                close |= r.should_close();
            }
            Dialog::Reactivate => match widgets::confirm(
                ctx,
                &format!("Reactivate {name}"),
                "The account can log in and use the API again.",
                "Reactivate",
                false,
            ) {
                Some(true) => {
                    run = Some(Box::new(move |c| {
                        let r: UserEnvelope = c.post_empty(&format!("/admin/api/users/{id}/reactivate"))?;
                        Ok(Outcome::User(Box::new(r.user), format!("Reactivated {name}")))
                    }))
                }
                Some(false) => close = true,
                None => {}
            },
            Dialog::RevokeSessions => {
                match widgets::confirm(
                    ctx,
                    &format!("Revoke sessions of {name}"),
                    "Every session token of this account stops working. The login key stays valid.",
                    "Revoke",
                    true,
                ) {
                    Some(true) => {
                        run = Some(Box::new(move |c| {
                            let r: Revoked = c.post_empty(&format!("/admin/api/users/{id}/revoke-sessions"))?;
                            Ok(Outcome::Message(format!("Revoked {} session(s) of {name}", r.revoked)))
                        }))
                    }
                    Some(false) => close = true,
                    None => {}
                }
            }
            Dialog::ResetUsage(scope) => {
                let scope = *scope;
                let body = format!("Sets the {scope} request counter of {name} to zero.");
                match widgets::confirm(ctx, &format!("Reset {scope} usage"), &body, "Reset", false) {
                    Some(true) => {
                        run = Some(Box::new(move |c| {
                            let r: UserEnvelope = c.post(&format!("/admin/api/users/{id}/reset-usage"), &json!({ "scope": scope }))?;
                            Ok(Outcome::User(Box::new(r.user), format!("Reset {scope} usage of {name}")))
                        }))
                    }
                    Some(false) => close = true,
                    None => {}
                }
            }
        }
        if let Some(f) = run {
            self.act(env, f);
            close = true;
        }
        if close {
            self.dialog = None;
        }
    }

    fn create_dialog(&mut self, ctx: &egui::Context, env: &Env<'_>) {
        let Some(f) = &mut self.create else { return };
        let roles = env.session.role_names();
        let plans = env.session.plan_names();
        let busy = self.t_create.running();
        let mut close = false;
        let mut submit = false;
        let r = egui::Modal::new(egui::Id::new("create-user")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.heading("New user");
            ui.add_space(6.0);
            egui::Grid::new("create-grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label("Username");
                ui.add(egui::TextEdit::singleline(&mut f.username).hint_text("3–32 of a-z 0-9 . _ -").desired_width(260.0));
                ui.end_row();
                ui.label("E-mail");
                ui.add(egui::TextEdit::singleline(&mut f.email).hint_text("name@example.com").desired_width(260.0));
                ui.end_row();
                ui.label("Role");
                widgets::combo(ui, "create-role", &mut f.role, &roles, None);
                ui.end_row();
                ui.label("Plan");
                widgets::combo(ui, "create-plan", &mut f.plan, &plans, Some("server default"));
                ui.end_row();
                ui.label("");
                ui.checkbox(&mut f.send_email, "E-mail the key to the user");
                ui.end_row();
            });
            widgets::muted(ui, "The account starts active. The login key is shown once after creation.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    close = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ok = !busy && valid_username(f.username.trim()) && f.email.contains('@');
                    if widgets::primary(ui, ok, "Create").clicked() {
                        submit = true;
                    }
                    if busy {
                        ui.spinner();
                    }
                });
            });
        });
        if submit {
            let mut body = json!({ "username": f.username.trim(), "email": f.email.trim(), "role": f.role, "send_email": f.send_email });
            if !f.plan.is_empty() {
                body["plan"] = json!(f.plan);
            }
            self.t_create = Task::call(&env.ctx, env.client(), move |c| c.post("/admin/api/users", &body));
        }
        if (close || r.should_close()) && !busy {
            self.create = None;
        }
    }
}

/// Username rule from docs/ACCOUNTS.md: 3–32 of `a-z 0-9 . _ -`.
pub fn valid_username(s: &str) -> bool {
    (3..=32).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> User {
        User {
            id: 7,
            username: "ana".into(),
            email: "ana@example.com".into(),
            role: "user".into(),
            plan: "free".into(),
            daily_limit: None,
            monthly_limit: Some(5000),
            per_minute: None,
            ..User::default()
        }
    }

    #[test]
    fn filter_query_string() {
        let f = Filter { q: "ana maria".into(), status: "active".into(), ..Filter::default() };
        assert_eq!(f.path(), "/admin/api/users?q=ana%20maria&status=active&sort=-created&limit=50&offset=0");
    }

    #[test]
    fn editor_patch_contains_only_changes() {
        let u = user();
        let mut e = Editor::from_user(&u);
        assert!(e.profile_patch(&u).is_empty());
        e.plan = "pro".into();
        e.daily.set = true;
        e.daily.value = 500;
        e.monthly.set = false;
        let p = e.profile_patch(&u);
        assert_eq!(Value::Object(p), json!({ "plan": "pro", "daily_limit": 500, "monthly_limit": null }));
        // Notes are saved separately.
        e = Editor::from_user(&u);
        e.notes = "called support".into();
        assert!(e.profile_patch(&u).is_empty());
    }

    #[test]
    fn usernames() {
        assert!(valid_username("ana"));
        assert!(valid_username("ana.maria_01-x"));
        assert!(!valid_username("an"));
        assert!(!valid_username("Ana"));
        assert!(!valid_username("ana maria"));
        assert!(!valid_username(&"a".repeat(33)));
    }

    #[test]
    fn usage_chart_data() {
        let rows = vec![
            UserUsageDay { day: "2026-11-02".into(), kind: "search".into(), count: 5, cache_hits: 0 },
            UserUsageDay { day: "2026-11-03".into(), kind: "search".into(), count: 7, cache_hits: 1 },
            UserUsageDay { day: "2026-11-03".into(), kind: "cnpj".into(), count: 2, cache_hits: 0 },
            UserUsageDay { day: "2026-11-01".into(), kind: "cnpj".into(), count: 1, cache_hits: 0 },
        ];
        let (days, series) = usage_by_day(&rows, 30);
        assert_eq!(days, vec!["2026-11-01", "2026-11-02", "2026-11-03"]);
        assert_eq!(series[0], ("cnpj".to_string(), vec![1.0, 0.0, 2.0]));
        assert_eq!(series[1], ("search".to_string(), vec![0.0, 5.0, 7.0]));
        let (days, _) = usage_by_day(&rows, 2);
        assert_eq!(days, vec!["2026-11-02", "2026-11-03"]);
    }
}
