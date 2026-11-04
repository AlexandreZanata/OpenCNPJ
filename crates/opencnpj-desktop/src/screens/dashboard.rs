//! Dashboard: server KPIs, live request rate and latency, per-route latency and caches.
//! Calls `GET /admin/api/overview`, `GET /admin/api/metrics?seconds=300` and
//! `GET /admin/api/response-cache`, every 2 seconds while visible.

use std::time::{Duration, Instant};

use eframe::egui::{self, RichText, Ui};
use egui_extras::{Column, TableBuilder};

use crate::api::Task;
use crate::format;
use crate::models::{MetricsResponse, Overview, ResponseCacheStats, SeriesPoint};
use crate::state::Env;
use crate::theme::Palette;
use crate::widgets::{self, Series};

const REFRESH: Duration = Duration::from_secs(2);

#[derive(Default)]
pub struct Dashboard {
    overview: Option<Overview>,
    metrics: Option<MetricsResponse>,
    response_cache: Option<ResponseCacheStats>,
    response_cache_missing: bool,
    t_overview: Task<Overview>,
    t_metrics: Task<MetricsResponse>,
    t_response_cache: Task<ResponseCacheStats>,
    last: Option<Instant>,
    route_filter: String,
}

/// Mean of the last complete `n` seconds (the newest second is still filling).
pub fn recent_rate(series: &[SeriesPoint], n: usize) -> (f64, f64) {
    let len = series.len();
    if len < 2 {
        return (0.0, 0.0);
    }
    let window = &series[len.saturating_sub(n + 1)..len - 1];
    let requests: f64 = window.iter().map(|p| p.requests).sum();
    let weighted: f64 = window.iter().map(|p| p.mean_ms * p.requests).sum();
    let rps = requests / window.len().max(1) as f64;
    let mean = if requests > 0.0 { weighted / requests } else { 0.0 };
    (rps, mean)
}

impl Dashboard {
    fn refresh(&mut self, env: &Env<'_>) {
        self.last = Some(Instant::now());
        if !self.t_overview.running() {
            self.t_overview = Task::call(&env.ctx, env.client(), |c| c.get("/admin/api/overview"));
        }
        if !self.t_metrics.running() {
            self.t_metrics = Task::call(&env.ctx, env.client(), |c| c.get("/admin/api/metrics?seconds=300"));
        }
        if !self.t_response_cache.running() && !self.response_cache_missing {
            self.t_response_cache = Task::call(&env.ctx, env.client(), |c| c.get("/admin/api/response-cache"));
        }
    }

    pub fn poll(&mut self, env: &mut Env<'_>) {
        if let Some(r) = self.t_overview.poll()
            && let Some(o) = env.check(r)
        {
            self.overview = Some(o);
        }
        if let Some(r) = self.t_metrics.poll()
            && let Some(m) = env.check(r)
        {
            self.metrics = Some(m);
        }
        match self.t_response_cache.poll() {
            Some(Ok(c)) => self.response_cache = Some(c),
            // Servers without the accounts feature have no response cache: hide the panel.
            Some(Err(e)) if e.is_not_found() => self.response_cache_missing = true,
            Some(Err(e)) => env.report(&e),
            None => {}
        }
    }

    fn busy(&self) -> bool {
        self.t_overview.running() || self.t_metrics.running()
    }

    pub fn ui(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
        if self.last.is_none_or(|t| t.elapsed() >= REFRESH) {
            self.refresh(env);
        }
        ui.ctx().request_repaint_after(REFRESH);
        let subtitle = self.overview.as_ref().map(|o| format!("server v{} · auto-refresh 2 s", o.version)).unwrap_or_default();
        if widgets::header(ui, "Dashboard", &subtitle, self.busy()) {
            self.refresh(env);
        }
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            self.kpis(ui);
            ui.add_space(10.0);
            self.charts(ui);
            ui.add_space(10.0);
            self.routes(ui);
            ui.add_space(10.0);
            self.caches(ui);
        });
    }

    fn kpis(&self, ui: &mut Ui) {
        let Some(o) = &self.overview else {
            widgets::muted(ui, "Loading overview…");
            return;
        };
        let series = self.metrics.as_ref().map_or(&[][..], |m| &m.series[..]);
        let (rps, mean) = recent_rate(series, 10);
        let errors: f64 = series.iter().map(|p| p.errors).sum();
        let s = &o.snapshot;
        let version = s.format_version.iter().map(u32::to_string).collect::<Vec<_>>().join(".");
        widgets::kpis(
            ui,
            &[
                ("Uptime", format::duration(o.uptime_s)),
                ("Requests (total)", format::int(o.requests_total)),
                ("Requests/s (10 s)", format!("{rps:.1}")),
                ("Mean latency ms (10 s)", format::ms(mean)),
                ("5xx (5 min)", format::int(errors as u64)),
                ("Timeouts / overload", format!("{} / {}", format::int(o.timeouts_total), format::int(o.overload_total))),
                ("RSS anon", format::bytes(o.process.rss_anon_bytes)),
                ("RSS file (mmap)", format::bytes(o.process.rss_file_bytes)),
                ("CPU seconds / threads", format!("{:.0} / {}", o.process.cpu_seconds, o.process.threads)),
                ("Postings cache hit rate", format::percent(o.cache.hit_rate())),
                ("Postings cache used", format!("{} / {}", format::bytes(o.cache.bytes), format::bytes(o.cache.budget))),
                ("Snapshot competence", if s.competence.is_empty() { "–".into() } else { s.competence.clone() }),
                ("Documents", format::int(s.counts.documents)),
                ("Companies", format::int(s.counts.companies)),
                ("Snapshot size", format::bytes(s.file_bytes)),
                ("Format / profile", format!("{version} / {}", s.profile)),
            ],
        );
    }

    fn charts(&self, ui: &mut Ui) {
        let p = Palette::get(ui);
        let Some(m) = &self.metrics else { return };
        let now = m.series.last().map_or(0, |s| s.second);
        let xs: Vec<f64> = m.series.iter().map(|s| (s.second - now) as f64).collect();
        let fmt = |x: f64| if x == 0.0 { "now".to_string() } else { format!("{x:.0}s") };
        ui.columns(2, |cols| {
            widgets::section(&mut cols[0], "Requests per second (last 5 min)", |ui| {
                widgets::line_chart(
                    ui,
                    "rps",
                    &xs,
                    &[
                        Series { name: "requests", values: m.series.iter().map(|s| s.requests).collect(), color: p.accent },
                        Series { name: "5xx", values: m.series.iter().map(|s| s.errors).collect(), color: p.bad },
                    ],
                    &fmt,
                    200.0,
                );
            });
            widgets::section(&mut cols[1], "Mean latency, ms (last 5 min)", |ui| {
                widgets::line_chart(
                    ui,
                    "latency",
                    &xs,
                    &[Series { name: "mean ms", values: m.series.iter().map(|s| s.mean_ms).collect(), color: p.accent2 }],
                    &fmt,
                    200.0,
                );
            });
        });
    }

    fn routes(&mut self, ui: &mut Ui) {
        let Some(m) = &self.metrics else { return };
        let filter = &mut self.route_filter;
        widgets::section(ui, "Routes (since start)", |ui| {
            ui.horizontal(|ui| {
                ui.label("Filter");
                ui.add(egui::TextEdit::singleline(filter).hint_text("route contains…").desired_width(220.0));
            });
            let f = filter.to_lowercase();
            let rows: Vec<_> = m.routes.iter().filter(|r| f.is_empty() || r.route.to_lowercase().contains(&f)).collect();
            if rows.is_empty() {
                widgets::muted(ui, "No requests recorded yet.");
                return;
            }
            let p = Palette::get(ui);
            TableBuilder::new(ui)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .id_salt("routes")
                .striped(true)
                .vscroll(false)
                .column(Column::remainder().at_least(180.0).clip(true))
                .columns(Column::initial(76.0).at_least(56.0), 9)
                .header(widgets::ROW_H, |mut h| {
                    h.col(|ui| {
                        ui.strong("Route");
                    });
                    for t in ["Count", "Mean ms", "p50", "p95", "p99", "Max", "4xx", "5xx", "Errors"] {
                        h.col(|ui| widgets::num_header(ui, t));
                    }
                })
                .body(|body| {
                    body.rows(widgets::ROW_H, rows.len(), |mut row| {
                        let r = rows[row.index()];
                        row.col(|ui| {
                            ui.label(widgets::mono(&r.route));
                        });
                        for v in [
                            format::int(r.count),
                            format::ms(r.mean_ms),
                            format::ms(r.p50_ms),
                            format::ms(r.p95_ms),
                            format::ms(r.p99_ms),
                            format::ms(r.max_ms),
                        ] {
                            row.col(|ui| widgets::num(ui, v));
                        }
                        let (c4, c5) = (r.class("4xx"), r.class("5xx"));
                        row.col(|ui| widgets::num(ui, format::int(c4)));
                        row.col(|ui| widgets::num(ui, format::int(c5)));
                        row.col(|ui| {
                            let rate = if r.count > 0 { (c4 + c5) as f64 / r.count as f64 } else { 0.0 };
                            let color = if c5 > 0 {
                                p.bad
                            } else if rate > 0.05 {
                                p.warn
                            } else {
                                p.muted
                            };
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(format::percent(Some(rate))).monospace().color(color));
                            });
                        });
                    });
                });
        });
    }

    fn caches(&self, ui: &mut Ui) {
        let Some(o) = &self.overview else { return };
        ui.columns(2, |cols| {
            widgets::section(&mut cols[0], "Postings cache", |ui| {
                let c = &o.cache;
                stats_grid(
                    ui,
                    "postings-cache",
                    &[
                        ("Entries", format::int(c.entries)),
                        ("Used", format::bytes(c.bytes)),
                        ("Budget", format::bytes(c.budget)),
                        ("Hits / misses", format!("{} / {}", format::int(c.hits), format::int(c.misses))),
                        ("Hit rate", format::percent(c.hit_rate())),
                    ],
                );
            });
            widgets::section(&mut cols[1], "Response cache", |ui| match &self.response_cache {
                Some(c) if c.enabled == Some(false) => {
                    widgets::muted(ui, "Disabled on this server.");
                }
                Some(c) => stats_grid(
                    ui,
                    "response-cache",
                    &[
                        ("Entries", format::int(c.entries)),
                        ("Used", format::bytes(c.bytes)),
                        ("Budget", format::bytes(c.budget)),
                        ("Hits / misses", format!("{} / {}", format::int(c.hits), format::int(c.misses))),
                        ("Hit rate", format::percent(c.hit_rate())),
                        ("Coalesced", format::int(c.coalesced)),
                    ],
                ),
                None if self.response_cache_missing => {
                    widgets::muted(ui, "Not available on this server.");
                }
                None => {
                    widgets::muted(ui, "Loading…");
                }
            });
        });
    }
}

/// Two-column label/value grid.
pub fn stats_grid(ui: &mut Ui, id: &str, rows: &[(&str, String)]) {
    let muted = Palette::get(ui).muted;
    egui::Grid::new(id).num_columns(2).spacing([24.0, 4.0]).show(ui, |ui| {
        for (k, v) in rows {
            ui.label(RichText::new(*k).color(muted));
            ui.label(widgets::mono(v.clone()));
            ui.end_row();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(second: i64, requests: f64, mean_ms: f64) -> SeriesPoint {
        SeriesPoint { second, requests, errors: 0.0, mean_ms }
    }

    #[test]
    fn recent_rate_skips_the_current_second() {
        assert_eq!(recent_rate(&[], 10), (0.0, 0.0));
        assert_eq!(recent_rate(&[p(1, 5.0, 1.0)], 10), (0.0, 0.0));
        let s = [p(1, 10.0, 2.0), p(2, 30.0, 4.0), p(3, 1000.0, 99.0)];
        let (rps, mean) = recent_rate(&s, 10);
        assert_eq!(rps, 20.0);
        assert_eq!(mean, 3.5);
        let (rps, _) = recent_rate(&s, 1);
        assert_eq!(rps, 30.0);
    }
}
