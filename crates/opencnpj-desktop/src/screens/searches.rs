//! Searches: the search log with a user filter and "load more". Calls
//! `GET /admin/api/searches?user_id=&limit=&before=`.

use eframe::egui::{self, RichText, Ui};
use egui_extras::{Column, TableBuilder};

use crate::format;
use crate::models::SearchEntry;
use crate::screens::paging::{Pager, page_path, valid_id};
use crate::state::Env;
use crate::theme::Palette;
use crate::widgets;

#[derive(Default)]
pub struct Searches {
    user_id: String,
    contains: String,
    pager: Pager<SearchEntry>,
}

impl Searches {
    fn path(&self, before: Option<i64>) -> String {
        page_path("/admin/api/searches", &[("user_id", self.user_id.clone())], self.pager.limit, before)
    }

    fn reload(&mut self, env: &Env<'_>) {
        let path = self.path(None);
        self.pager.fetch(env, path, false);
    }

    pub fn filter_user(&mut self, id: i64, env: &Env<'_>) {
        self.user_id = id.to_string();
        self.reload(env);
    }

    pub fn poll(&mut self, env: &mut Env<'_>) {
        self.pager.poll(env);
    }

    pub fn ui(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
        if !self.pager.started {
            self.reload(env);
        }
        if widgets::header(ui, "Searches", "search log, newest first", self.pager.task.running()) {
            self.reload(env);
        }
        let mut reload = false;
        ui.horizontal(|ui| {
            ui.label("User ID");
            let ok = valid_id(&self.user_id);
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.user_id)
                    .hint_text("all users")
                    .desired_width(90.0)
                    .text_color_opt((!ok).then(|| Palette::get(ui).bad)),
            );
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && ok {
                reload = true;
            }
            if ui.add_enabled(ok, egui::Button::new("Apply")).clicked() {
                reload = true;
            }
            if !self.user_id.is_empty() && ui.button("All users").clicked() {
                self.user_id.clear();
                reload = true;
            }
            reload |= self.pager.limit_combo(ui, "searches-limit");
            ui.separator();
            ui.label("Contains");
            ui.add(egui::TextEdit::singleline(&mut self.contains).hint_text("route or query (loaded rows)").desired_width(200.0));
        });
        if reload {
            self.reload(env);
        }
        ui.add_space(4.0);
        let mut more = false;
        egui::TopBottomPanel::bottom("searches-footer").show_inside(ui, |ui| more = self.pager.footer(ui));
        let mut pick_user = None;
        egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(ui, |ui| pick_user = self.table(ui));
        if more {
            let before = self.pager.rows.iter().map(|r| r.id).min();
            let path = self.path(before);
            self.pager.fetch(env, path, true);
        }
        if let Some(id) = pick_user {
            self.filter_user(id, env);
        }
    }

    fn table(&self, ui: &mut Ui) -> Option<i64> {
        let p = Palette::get(ui);
        let f = self.contains.to_lowercase();
        let rows: Vec<&SearchEntry> = self
            .pager
            .rows
            .iter()
            .filter(|r| f.is_empty() || r.route.to_lowercase().contains(&f) || format::value(&r.query).to_lowercase().contains(&f))
            .collect();
        if rows.is_empty() {
            widgets::muted(ui, if self.pager.task.running() { "Loading…" } else { "No searches logged." });
            return None;
        }
        let mut pick = None;
        TableBuilder::new(ui)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .id_salt("searches-table")
            .striped(true)
            .auto_shrink(false)
            .column(Column::auto().at_least(60.0))
            .column(Column::auto().at_least(150.0))
            .column(Column::auto().at_least(110.0).clip(true))
            .column(Column::auto().at_least(170.0).clip(true))
            .column(Column::remainder().at_least(200.0).clip(true))
            .column(Column::auto().at_least(54.0))
            .column(Column::initial(70.0))
            .column(Column::auto().at_least(50.0))
            .column(Column::auto().at_least(100.0))
            .header(widgets::ROW_H, |mut h| {
                for t in ["ID", "Time (UTC)", "User", "Route", "Query", "Status", "ms", "Cache", "IP"] {
                    h.col(|ui| {
                        ui.strong(t);
                    });
                }
            })
            .body(|body| {
                body.rows(widgets::ROW_H, rows.len(), |mut row| {
                    let r = rows[row.index()];
                    row.col(|ui| {
                        ui.label(widgets::mono(r.id.to_string()));
                    });
                    row.col(|ui| {
                        ui.label(widgets::mono(format::opt_ts(r.ts)));
                    });
                    row.col(|ui| match r.user_id {
                        Some(id) => {
                            let name = if r.username.is_empty() { format!("#{id}") } else { r.username.clone() };
                            if ui.link(name).on_hover_text(format!("Only searches of user {id}")).clicked() {
                                pick = Some(id);
                            }
                        }
                        None => {
                            widgets::muted(ui, "anonymous");
                        }
                    });
                    row.col(|ui| {
                        ui.label(widgets::mono(&r.route));
                    });
                    row.col(|ui| {
                        let q = format::value(&r.query);
                        ui.label(widgets::mono(&q)).on_hover_text(q);
                    });
                    row.col(|ui| {
                        let code: u16 = r.status.parse().unwrap_or(0);
                        let c = match code {
                            200..=299 => p.ok,
                            400..=499 => p.warn,
                            500.. => p.bad,
                            _ => p.muted,
                        };
                        ui.label(RichText::new(&r.status).monospace().color(c));
                    });
                    row.col(|ui| widgets::num(ui, format::ms(r.ms)));
                    row.col(|ui| {
                        let t = r.cache_text();
                        if !t.is_empty() {
                            widgets::pill(ui, t, if t == "hit" { p.ok } else { p.muted });
                        }
                    });
                    row.col(|ui| {
                        ui.label(widgets::mono(&r.ip));
                    });
                });
            });
        pick
    }
}
