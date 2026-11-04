//! Login: server admin URL and login key. Calls `POST /api/v1/auth/login` and
//! `GET /admin/api/whoami`. The key lives only in memory and is cleared after the attempt.

use eframe::egui::{self, Align, Layout, RichText};

use crate::api::{Client, Task};
use crate::config::{self, Config};
use crate::format;
use crate::models::Whoami;
use crate::theme::Palette;
use crate::widgets;

pub struct Login {
    pub url: String,
    key: String,
    pub remember_username: bool,
    pub last_username: Option<String>,
    show_key: bool,
    task: Task<(Client, Whoami)>,
    pub error: Option<String>,
}

impl Login {
    pub fn new(c: &Config) -> Self {
        Self {
            url: c.server_url.clone(),
            key: String::new(),
            remember_username: c.username.is_some(),
            last_username: c.username.clone(),
            show_key: false,
            task: Task::default(),
            error: None,
        }
    }

    fn submit(&mut self, ctx: &egui::Context) {
        let url = format::normalize_url(&self.url);
        let key = self.key.trim().to_string();
        self.url = url.clone();
        self.error = None;
        self.task = Task::spawn(ctx, move || Client::login(&url, &key));
    }

    /// Draws the login screen; returns the session once signed in.
    pub fn ui(&mut self, ctx: &egui::Context, logo: Option<&egui::TextureHandle>) -> Option<(Client, Whoami)> {
        let mut done = None;
        if let Some(r) = self.task.poll() {
            self.key.clear();
            match r {
                Ok(v) => done = Some(v),
                Err(e) => self.error = Some(if e.status == 401 { "Invalid login key.".into() } else { e.to_string() }),
            }
        }
        let busy = self.task.running();
        egui::CentralPanel::default().show(ctx, |ui| {
            let p = Palette::get(ui);
            let top = ((ui.available_height() - 470.0) / 2.0).max(16.0);
            ui.add_space(top);
            ui.vertical_centered(|ui| {
                ui.set_max_width(440.0);
                widgets::card(ui, |ui| {
                    ui.set_width(400.0);
                    ui.vertical_centered(|ui| {
                        if let Some(t) = logo {
                            ui.add(egui::Image::new(egui::load::SizedTexture::new(t.id(), [56.0, 56.0])));
                        }
                        ui.add_space(4.0);
                        ui.heading(RichText::new("OpenCNPJ Admin").strong());
                        widgets::muted(ui, "Sign in with a staff login key");
                    });
                    ui.add_space(14.0);
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.label("Server admin URL");
                        ui.add_enabled(
                            !busy,
                            egui::TextEdit::singleline(&mut self.url).hint_text(config::DEFAULT_URL).desired_width(f32::INFINITY),
                        );
                        ui.add_space(6.0);
                        ui.label("Login key");
                        let mut enter = false;
                        ui.horizontal(|ui| {
                            let w = ui.available_width() - 36.0;
                            let r = ui.add_enabled(
                                !busy,
                                egui::TextEdit::singleline(&mut self.key)
                                    .password(!self.show_key)
                                    .hint_text("ocnpj_…")
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(w),
                            );
                            enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if ui.selectable_label(self.show_key, "👁").on_hover_text("Show or hide the key").clicked() {
                                self.show_key = !self.show_key;
                            }
                        });
                        ui.add_space(6.0);
                        ui.checkbox(&mut self.remember_username, "Remember my username on this computer");
                        ui.add_space(10.0);
                        let ready = !busy && !self.key.trim().is_empty() && !format::normalize_url(&self.url).is_empty();
                        ui.horizontal(|ui| {
                            if widgets::primary(ui, ready, "Sign in").clicked() || (enter && ready) {
                                self.submit(ui.ctx());
                            }
                            if busy {
                                ui.spinner();
                                widgets::muted(ui, "Signing in…");
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if let Some(u) = &self.last_username {
                                    widgets::muted(ui, format!("last: {u}"));
                                }
                            });
                        });
                        if let Some(e) = &self.error {
                            ui.add_space(8.0);
                            ui.label(RichText::new(e).color(p.bad));
                        }
                        ui.add_space(10.0);
                        let path = config::config_path().map_or_else(|| "(no config directory)".to_string(), |p| p.display().to_string());
                        ui.label(
                            RichText::new(format!(
                                "The key and the session token are kept in memory only. The server URL is saved to {path}."
                            ))
                            .small()
                            .color(p.muted),
                        );
                    });
                });
            });
        });
        done
    }
}
