//! Usage: daily requests, active users and cache-hit rate, requests by kind, top users today and
//! totals. Calls `GET /admin/api/usage?days=N`.

use eframe::egui::{self, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use serde_json::Value;

use crate::api::Task;
use crate::format;
use crate::models::{UsageReport, value_i64};
use crate::state::Env;
use crate::theme::Palette;
use crate::widgets::{self, Series};

const PRESETS: [u32; 5] = [7, 30, 90, 180, 365];

pub struct Usage {
    days: u32,
    report: Option<UsageReport>,
    loaded_days: Option<u32>,
    task: Task<UsageReport>,
}

impl Default for Usage {
    fn default() -> Self {
        Self { days: 30, report: None, loaded_days: None, task: Task::default() }
    }
}

/// Total for one `by_kind` entry: a number, or the `requests`/`count` field of an object.
pub fn kind_total(v: &Value) -> Option<i64> {
    value_i64(v).or_else(|| ["requests", "count", "total"].iter().find_map(|k| v.get(k).and_then(value_i64)))
}

impl Usage {
    fn load(&mut self, env: &Env<'_>) {
        let days = self.days.clamp(1, 400);
        self.loaded_days = Some(days);
        self.task = Task::call(&env.ctx, env.client(), move |c| c.get(&format!("/admin/api/usage?days={days}")));
    }

    pub fn poll(&mut self, env: &mut Env<'_>) {
        if let Some(r) = self.task.poll()
            && let Some(u) = env.check(r)
        {
            self.report = Some(u);
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
        if self.loaded_days.is_none() {
            self.load(env);
        }
        if widgets::header(ui, "Usage", "UTC days", self.task.running()) {
            self.load(env);
        }
        let mut reload = false;
        ui.horizontal(|ui| {
            ui.label("Period");
            for d in PRESETS {
                if ui.selectable_label(self.days == d, format!("{d} days")).clicked() {
                    self.days = d;
                    reload = true;
                }
            }
            ui.add(egui::DragValue::new(&mut self.days).range(1..=400).suffix(" days"));
            if Some(self.days) != self.loaded_days && ui.button("Apply").clicked() {
                reload = true;
            }
        });
        if reload {
            self.load(env);
        }
        ui.add_space(6.0);
        let Some(r) = self.report.take() else {
            widgets::muted(ui, "Loading…");
            return;
        };
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            Self::body(ui, &r, env);
        });
        self.report = Some(r);
    }

    fn body(ui: &mut Ui, r: &UsageReport, env: &mut Env<'_>) {
        let p = Palette::get(ui);
        let t = &r.totals;
        let mut items = vec![
            ("Users", format::int(t.users)),
            ("Active users today", format::int(t.active_users_today)),
            ("Requests today", format::int(t.requests_today)),
            ("Requests this month", format::int(t.requests_month)),
        ];
        let period_requests: u64 = r.daily.iter().map(|d| d.requests).sum();
        let period_hits: u64 = r.daily.iter().map(|d| d.cache_hits).sum();
        items.push(("Requests in period", format::int(period_requests)));
        items.push(("Cache hits in period", format::percent((period_requests > 0).then(|| period_hits as f64 / period_requests as f64))));
        let by_status: Vec<(String, String)> = t.users_by_status.iter().map(|(k, v)| (format!("Users {k}"), format::int(*v))).collect();
        let mut all: Vec<(&str, String)> = items;
        all.extend(by_status.iter().map(|(k, v)| (k.as_str(), v.clone())));
        widgets::kpis(ui, &all);
        ui.add_space(10.0);

        let labels: Vec<String> = r.daily.iter().map(|d| d.day.get(5..).unwrap_or(&d.day).to_string()).collect();
        widgets::section(ui, "Requests per day", |ui| {
            if r.daily.is_empty() {
                widgets::muted(ui, "No usage in this period.");
                return;
            }
            let hits: Vec<f64> = r.daily.iter().map(|d| d.cache_hits as f64).collect();
            let misses: Vec<f64> = r.daily.iter().map(|d| d.requests.saturating_sub(d.cache_hits) as f64).collect();
            widgets::bar_chart(
                ui,
                "usage-requests",
                &labels,
                &[
                    Series { name: "cache hits", values: hits, color: p.ok },
                    Series { name: "other requests", values: misses, color: p.accent },
                ],
                true,
                220.0,
            );
        });
        ui.add_space(10.0);
        let xs: Vec<f64> = (0..r.daily.len()).map(|i| i as f64).collect();
        let label = |x: f64| {
            let i = x.round();
            if (x - i).abs() > 0.01 || i < 0.0 { String::new() } else { labels.get(i as usize).cloned().unwrap_or_default() }
        };
        ui.columns(2, |cols| {
            widgets::section(&mut cols[0], "Active users per day", |ui| {
                let v: Vec<f64> = r.daily.iter().map(|d| d.users as f64).collect();
                widgets::line_chart(ui, "usage-users", &xs, &[Series { name: "users", values: v, color: p.accent2 }], &label, 180.0);
            });
            widgets::section(&mut cols[1], "Cache-hit rate per day, %", |ui| {
                let v: Vec<f64> =
                    r.daily.iter().map(|d| if d.requests > 0 { 100.0 * d.cache_hits as f64 / d.requests as f64 } else { 0.0 }).collect();
                widgets::line_chart(ui, "usage-hits", &xs, &[Series { name: "hit rate %", values: v, color: p.ok }], &label, 180.0);
            });
        });
        ui.add_space(10.0);
        ui.columns(2, |cols| {
            widgets::section(&mut cols[0], "Requests by kind", |ui| {
                let mut kinds: Vec<(&String, &Value)> = r.by_kind.iter().collect();
                kinds.sort_by_key(|(_, v)| std::cmp::Reverse(kind_total(v).unwrap_or(0)));
                if kinds.is_empty() {
                    widgets::muted(ui, "none");
                    return;
                }
                let max = kinds.iter().filter_map(|(_, v)| kind_total(v)).max().unwrap_or(1).max(1) as f32;
                egui::Grid::new("by-kind").num_columns(3).spacing([16.0, 4.0]).show(ui, |ui| {
                    for (k, v) in kinds {
                        ui.label(widgets::mono(k.as_str()));
                        match kind_total(v) {
                            Some(n) => {
                                widgets::num(ui, format::signed(n));
                                ui.add(egui::ProgressBar::new(n as f32 / max).desired_width(160.0).fill(p.accent));
                            }
                            None => {
                                ui.label(widgets::mono(format::value(v)));
                                ui.label("");
                            }
                        }
                        ui.end_row();
                    }
                });
            });
            widgets::section(&mut cols[1], "Top users today", |ui| {
                if r.top_users_today.is_empty() {
                    widgets::muted(ui, "No requests today.");
                    return;
                }
                let mut open = None;
                TableBuilder::new(ui)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .id_salt("top-users")
                    .striped(true)
                    .vscroll(false)
                    .sense(egui::Sense::click())
                    .column(Column::auto().at_least(40.0))
                    .column(Column::remainder().at_least(120.0))
                    .column(Column::initial(90.0))
                    .header(widgets::ROW_H, |mut h| {
                        for t in ["ID", "Username", "Requests"] {
                            h.col(|ui| {
                                ui.strong(t);
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(widgets::ROW_H, r.top_users_today.len(), |mut row| {
                            let u = &r.top_users_today[row.index()];
                            row.col(|ui| {
                                ui.label(widgets::mono(u.id.to_string()));
                            });
                            row.col(|ui| {
                                ui.label(RichText::new(&u.username).color(p.accent)).on_hover_text("Open in Users");
                            });
                            row.col(|ui| widgets::num(ui, format::int(u.count)));
                            if row.response().clicked() {
                                open = Some(u.id);
                            }
                        });
                    });
                if open.is_some() {
                    env.signals.open_user = open;
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn by_kind_values() {
        assert_eq!(kind_total(&json!(12)), Some(12));
        assert_eq!(kind_total(&json!({"requests": 5, "cache_hits": 1})), Some(5));
        assert_eq!(kind_total(&json!({"count": 3})), Some(3));
        assert_eq!(kind_total(&json!({"other": 3})), None);
    }
}
