//! Application shell: login, left navigation, status bar, notifications and theme.

use std::time::Duration;

use eframe::egui::{self, Align, Align2, Color32, Layout, RichText};

use crate::api::{ApiResult, Client, Task};
use crate::config::Config;
use crate::icon;
use crate::models::{Plan, RoleInfo, Whoami};
use crate::screens::login::Login;
use crate::screens::{Screen, Screens};
use crate::state::{Env, Session, Signals, ToastKind, Toasts, perm};
use crate::theme::{self, Palette};
use crate::widgets;

pub struct AdminApp {
    config: Config,
    login: Login,
    session: Option<Session>,
    screens: Screens,
    current: Screen,
    toasts: Toasts,
    signals: Signals,
    logo: Option<egui::TextureHandle>,
    t_roles: Task<Vec<RoleInfo>>,
    t_plans: Task<Vec<Plan>>,
}

impl AdminApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        let size = 128;
        let image = egui::ColorImage::from_rgba_unmultiplied([size, size], &icon::rgba(size as u32));
        let logo = Some(cc.egui_ctx.load_texture("opencnpj-logo", image, egui::TextureOptions::LINEAR));
        let config = Config::load();
        Self {
            login: Login::new(&config),
            config,
            session: None,
            screens: Screens::default(),
            current: Screen::Dashboard,
            toasts: Toasts::default(),
            signals: Signals::default(),
            logo,
            t_roles: Task::default(),
            t_plans: Task::default(),
        }
    }

    /// Starts already signed in on `screen` (embedding, scripted UI checks). Unlike the login
    /// screen, this writes no preferences.
    pub fn with_session(cc: &eframe::CreationContext<'_>, client: Client, me: Whoami, screen: Screen) -> Self {
        let mut app = Self::new(cc);
        app.session = Some(Session::new(client, me));
        app.current = screen;
        app.load_meta(&cc.egui_ctx);
        app
    }

    /// Opens the Users screen with account `id` selected on the next frame.
    pub fn open_user(mut self, id: i64) -> Self {
        self.signals.open_user = Some(id);
        self
    }

    fn start_session(&mut self, ctx: &egui::Context, client: Client, me: Whoami) {
        self.config.server_url = client.base().to_string();
        self.config.username = self.login.remember_username.then(|| me.user.username.clone());
        self.login.last_username = self.config.username.clone();
        if let Err(e) = self.config.save() {
            self.toasts.error(format!("Cannot save preferences: {e}"));
        }
        let session = Session::new(client, me);
        self.current = Screen::ALL.into_iter().find(|s| session.can(s.permission())).unwrap_or(Screen::Dashboard);
        self.toasts.success(format!("Signed in as {} ({})", session.user.username, session.user.role));
        self.screens = Screens::default();
        self.session = Some(session);
        self.load_meta(ctx);
    }

    fn load_meta(&mut self, ctx: &egui::Context) {
        let Some(s) = &self.session else { return };
        self.t_roles = Task::call(ctx, &s.client, |c| c.get("/admin/api/roles"));
        if s.can(perm::USERS_READ) {
            self.t_plans = Task::call(ctx, &s.client, |c| c.get("/admin/api/plans"));
        }
    }

    /// Drops the session. `revoke` also revokes the token on the server.
    fn end_session(&mut self, revoke: bool, message: Option<String>) {
        if let Some(s) = self.session.take()
            && revoke
        {
            let c = s.client.clone();
            // Fire and forget: the UI does not wait for the server.
            let _ = std::thread::Builder::new().name("opencnpj-admin-logout".into()).spawn(move || -> ApiResult<()> { c.logout() });
        }
        self.screens = Screens::default();
        self.t_roles = Task::default();
        self.t_plans = Task::default();
        self.signals = Signals::default();
        self.login.error = message;
    }

    fn poll_meta(&mut self) {
        let Some(s) = &mut self.session else { return };
        match self.t_roles.poll() {
            Some(Ok(r)) => s.roles = r,
            Some(Err(e)) => {
                if e.is_unauthorized() {
                    self.signals.unauthorized = true;
                }
                self.toasts.error(format!("Cannot load roles: {e}"));
            }
            None => {}
        }
        match self.t_plans.poll() {
            Some(Ok(p)) => s.plans = p,
            Some(Err(e)) => {
                if e.is_unauthorized() {
                    self.signals.unauthorized = true;
                }
                self.toasts.error(format!("Cannot load plans: {e}"));
            }
            None => {}
        }
    }

    fn nav(&mut self, ctx: &egui::Context) {
        let Some(s) = &self.session else { return };
        let mut logout = false;
        egui::SidePanel::left("nav").exact_width(206.0).resizable(false).show(ctx, |ui| {
            let p = Palette::get(ui);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if let Some(t) = &self.logo {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(t.id(), [28.0, 28.0])));
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new("OpenCNPJ").strong().size(16.0));
                    ui.label(RichText::new("Admin").small().color(p.muted));
                });
            });
            ui.add_space(14.0);
            for screen in Screen::ALL {
                let allowed = s.can(screen.permission());
                let text = RichText::new(format!("{}  {}", screen.icon(), screen.label())).size(15.0);
                let b = egui::Button::selectable(self.current == screen, text).min_size(egui::vec2(ui.available_width(), 34.0));
                let r = ui.add_enabled(allowed, b).on_disabled_hover_text(format!("Requires {}", screen.permission()));
                if r.clicked() {
                    self.current = screen;
                }
            }
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                ui.add_space(10.0);
                if ui.add(egui::Button::new("🚪  Sign out").min_size(egui::vec2(ui.available_width(), 30.0))).clicked() {
                    logout = true;
                }
                let dark = ui.visuals().dark_mode;
                let label = if dark { "☀  Light theme" } else { "🌙  Dark theme" };
                if ui.add(egui::Button::new(label).min_size(egui::vec2(ui.available_width(), 30.0))).clicked() {
                    ui.ctx().set_theme(if dark { egui::ThemePreference::Light } else { egui::ThemePreference::Dark });
                }
                ui.add_space(6.0);
                ui.label(RichText::new(format!("{} permissions", s.permissions.len())).small().color(p.muted))
                    .on_hover_text(s.permissions.iter().cloned().collect::<Vec<_>>().join("\n"));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&s.user.username).strong());
                    widgets::pill(ui, &s.user.role, p.accent);
                });
                ui.separator();
            });
        });
        if logout {
            self.end_session(true, Some("Signed out.".into()));
        }
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        let Some(s) = &self.session else { return };
        egui::TopBottomPanel::bottom("status").exact_height(26.0).show(ctx, |ui| {
            let p = Palette::get(ui);
            ui.horizontal_centered(|ui| {
                ui.label(RichText::new("⏺").color(p.ok));
                ui.label(RichText::new(s.client.base()).monospace().small());
                ui.separator();
                ui.label(RichText::new(format!("{} · {}", s.user.username, s.user.role)).small());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).small().color(p.muted));
                    if let Some(e) = self.toasts.last_error.clone() {
                        if ui.small_button("✖").on_hover_text("Dismiss").clicked() {
                            self.toasts.last_error = None;
                        }
                        let short: String = e.chars().take(110).collect();
                        ui.label(RichText::new(format!("⚠ {short}")).small().color(p.bad)).on_hover_text(e);
                    }
                });
            });
        });
    }

    fn toasts_ui(&mut self, ctx: &egui::Context) {
        self.toasts.expire();
        if self.toasts.items.is_empty() {
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(250));
        let dark = ctx.style().visuals.dark_mode;
        let p = Palette::of(dark);
        let mut dismiss = None;
        egui::Area::new(egui::Id::new("toasts")).anchor(Align2::RIGHT_BOTTOM, [-16.0, -38.0]).order(egui::Order::Foreground).show(
            ctx,
            |ui| {
                ui.set_max_width(420.0);
                for (i, t) in self.toasts.items.iter().enumerate().rev() {
                    let color = match t.kind {
                        ToastKind::Info => p.accent,
                        ToastKind::Success => p.ok,
                        ToastKind::Error => p.bad,
                    };
                    let r = egui::Frame::new()
                        .fill(p.card)
                        .stroke(egui::Stroke::new(1.0_f32, color))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .shadow(egui::Shadow { offset: [0, 2], blur: 10, spread: 0, color: Color32::from_black_alpha(60) })
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("⏺").color(color));
                                ui.add(egui::Label::new(&t.text).wrap());
                            });
                        })
                        .response
                        .interact(egui::Sense::click())
                        .on_hover_text("Click to dismiss");
                    if r.clicked() {
                        dismiss = Some(i);
                    }
                    ui.add_space(6.0);
                }
            },
        );
        if let Some(i) = dismiss {
            self.toasts.items.remove(i);
        }
    }

    fn main_ui(&mut self, ctx: &egui::Context) {
        self.poll_meta();
        let Self { session: Some(session), screens, toasts, signals, current, .. } = self else { return };
        let mut env = Env { ctx: ctx.clone(), session, toasts, signals };
        screens.dashboard.poll(&mut env);
        screens.users.poll(&mut env);
        screens.plans.poll(&mut env);
        screens.usage.poll(&mut env);
        screens.searches.poll(&mut env);
        screens.audit.poll(&mut env);
        screens.operations.poll(&mut env);
        if let Some(id) = env.signals.open_user.take()
            && env.can(perm::USERS_READ)
        {
            *current = Screen::Users;
            screens.users.open(id, &env);
        }
        let screen = *current;
        drop(env);

        self.nav(ctx);
        self.status_bar(ctx);
        let Self { session: Some(session), screens, toasts, signals, .. } = self else { return };
        egui::CentralPanel::default().frame(egui::Frame::central_panel(&ctx.style()).inner_margin(egui::Margin::same(16))).show(
            ctx,
            |ui| {
                let mut env = Env { ctx: ctx.clone(), session, toasts, signals };
                if !env.can(screen.permission()) {
                    widgets::muted(ui, format!("Your role cannot open {} (needs {}).", screen.label(), screen.permission()));
                    return;
                }
                match screen {
                    Screen::Dashboard => screens.dashboard.ui(ui, &mut env),
                    Screen::Users => screens.users.ui(ui, &mut env),
                    Screen::Plans => screens.plans.ui(ui, &mut env),
                    Screen::Usage => screens.usage.ui(ui, &mut env),
                    Screen::Searches => screens.searches.ui(ui, &mut env),
                    Screen::Audit => screens.audit.ui(ui, &mut env),
                    Screen::Operations => screens.operations.ui(ui, &mut env),
                }
            },
        );

        if std::mem::take(&mut self.signals.plans_changed) {
            self.load_meta(ctx);
        }
        if self.signals.unauthorized {
            self.end_session(false, Some("Your session expired or was revoked. Sign in again.".into()));
        }
    }
}

impl eframe::App for AdminApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.session.is_none() {
            if let Some((client, me)) = self.login.ui(ctx, self.logo.as_ref()) {
                self.start_session(ctx, client, me);
            }
        } else {
            self.main_ui(ctx);
        }
        self.toasts_ui(ctx);
    }
}
