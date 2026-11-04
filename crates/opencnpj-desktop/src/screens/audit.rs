//! Audit: the audit log with actor/target filters and "load more". Calls
//! `GET /admin/api/audit?actor_id=&target_id=&limit=&before=`.

use eframe::egui::{self, RichText, Ui};
use egui_extras::{Column, TableBuilder};

use crate::format;
use crate::models::AuditEntry;
use crate::screens::paging::{Pager, page_path, valid_id};
use crate::state::Env;
use crate::theme::Palette;
use crate::widgets;

#[derive(Default)]
pub struct Audit {
    actor_id: String,
    target_id: String,
    action: String,
    pager: Pager<AuditEntry>,
}

impl Audit {
    fn path(&self, before: Option<i64>) -> String {
        page_path(
            "/admin/api/audit",
            &[("actor_id", self.actor_id.clone()), ("target_id", self.target_id.clone())],
            self.pager.limit,
            before,
        )
    }

    fn reload(&mut self, env: &Env<'_>) {
        let path = self.path(None);
        self.pager.fetch(env, path, false);
    }

    pub fn poll(&mut self, env: &mut Env<'_>) {
        self.pager.poll(env);
    }

    pub fn ui(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
        if !self.pager.started {
            self.reload(env);
        }
        if widgets::header(ui, "Audit log", "every change made through the admin API", self.pager.task.running()) {
            self.reload(env);
        }
        let mut reload = false;
        ui.horizontal(|ui| {
            let ok = valid_id(&self.actor_id) && valid_id(&self.target_id);
            ui.label("Actor ID");
            let a = ui.add(egui::TextEdit::singleline(&mut self.actor_id).hint_text("any").desired_width(80.0));
            ui.label("Target ID");
            let t = ui.add(egui::TextEdit::singleline(&mut self.target_id).hint_text("any").desired_width(80.0));
            if (a.lost_focus() || t.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter)) && ok {
                reload = true;
            }
            if ui.add_enabled(ok, egui::Button::new("Apply")).clicked() {
                reload = true;
            }
            if (!self.actor_id.is_empty() || !self.target_id.is_empty()) && ui.button("Clear").clicked() {
                self.actor_id.clear();
                self.target_id.clear();
                reload = true;
            }
            reload |= self.pager.limit_combo(ui, "audit-limit");
            ui.separator();
            ui.label("Action contains");
            ui.add(egui::TextEdit::singleline(&mut self.action).hint_text("e.g. user.suspend").desired_width(160.0));
        });
        if reload {
            self.reload(env);
        }
        ui.add_space(4.0);
        let mut more = false;
        egui::TopBottomPanel::bottom("audit-footer").show_inside(ui, |ui| more = self.pager.footer(ui));
        let mut pick: Option<(bool, i64)> = None;
        let mut open_user = None;
        egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(ui, |ui| (pick, open_user) = self.table(ui));
        if more {
            let before = self.pager.rows.iter().map(|r| r.id).min();
            let path = self.path(before);
            self.pager.fetch(env, path, true);
        }
        if let Some((actor, id)) = pick {
            if actor {
                self.actor_id = id.to_string();
            } else {
                self.target_id = id.to_string();
            }
            self.reload(env);
        }
        if open_user.is_some() {
            env.signals.open_user = open_user;
        }
    }

    /// Returns a filter request `(is_actor, id)` and/or a user to open.
    fn table(&self, ui: &mut Ui) -> (Option<(bool, i64)>, Option<i64>) {
        let p = Palette::get(ui);
        let f = self.action.to_lowercase();
        let rows: Vec<&AuditEntry> = self.pager.rows.iter().filter(|r| f.is_empty() || r.action.to_lowercase().contains(&f)).collect();
        if rows.is_empty() {
            widgets::muted(ui, if self.pager.task.running() { "Loading…" } else { "No audit entries." });
            return (None, None);
        }
        let mut pick = None;
        let mut open = None;
        TableBuilder::new(ui)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .id_salt("audit-table")
            .striped(true)
            .auto_shrink(false)
            .column(Column::auto().at_least(56.0))
            .column(Column::auto().at_least(150.0))
            .column(Column::auto().at_least(110.0).clip(true))
            .column(Column::auto().at_least(150.0).clip(true))
            .column(Column::auto().at_least(110.0).clip(true))
            .column(Column::remainder().at_least(220.0).clip(true))
            .column(Column::auto().at_least(100.0))
            .header(widgets::ROW_H, |mut h| {
                for t in ["ID", "Time (UTC)", "Actor", "Action", "Target", "Details", "IP"] {
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
                    row.col(|ui| match r.actor_id {
                        Some(id) => {
                            let name = if r.actor.is_empty() { format!("#{id}") } else { r.actor.clone() };
                            if ui.link(name).on_hover_text(format!("Filter by actor {id}")).clicked() {
                                pick = Some((true, id));
                            }
                        }
                        None => {
                            widgets::muted(ui, if r.actor.is_empty() { "system" } else { r.actor.as_str() });
                        }
                    });
                    row.col(|ui| {
                        let c = if r.action.contains("delete") || r.action.contains("ban") || r.action.contains("suspend") {
                            p.bad
                        } else {
                            p.accent
                        };
                        ui.label(RichText::new(&r.action).monospace().color(c));
                    });
                    row.col(|ui| match r.target_id {
                        Some(id) => {
                            let name = if r.target.is_empty() { format!("#{id}") } else { r.target.clone() };
                            let resp = ui.link(name).on_hover_text(format!("Filter by target {id} (right click: open user)"));
                            if resp.clicked() {
                                pick = Some((false, id));
                            }
                            if resp.secondary_clicked() {
                                open = Some(id);
                            }
                        }
                        None => {
                            widgets::muted(ui, "–");
                        }
                    });
                    row.col(|ui| {
                        let d = format::value(&r.details);
                        let pretty = serde_json::to_string_pretty(&r.details).unwrap_or_else(|_| d.clone());
                        ui.label(widgets::mono(&d)).on_hover_text(widgets::mono(pretty));
                    });
                    row.col(|ui| {
                        ui.label(widgets::mono(&r.ip));
                    });
                });
            });
        (pick, open)
    }
}
