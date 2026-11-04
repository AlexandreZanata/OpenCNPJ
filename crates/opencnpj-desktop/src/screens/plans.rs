//! Plans: table and create/edit/delete form for every plan field. Calls
//! `GET/POST /admin/api/plans` and `PATCH/DELETE /admin/api/plans/{id}`.

use eframe::egui::{self, Align, Layout, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use serde_json::{Map, Value, json};

use crate::api::Task;
use crate::format;
use crate::models::Plan;
use crate::state::{Env, perm};
use crate::theme::Palette;
use crate::widgets::{self, OptNum};

/// Plan being created (`id == None`) or edited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanForm {
    pub id: Option<i64>,
    pub name: String,
    pub daily_limit: OptNum,
    pub monthly_limit: OptNum,
    pub per_minute: OptNum,
    pub burst: OptNum,
    pub max_page_size: OptNum,
    pub export: bool,
    pub max_export_rows: OptNum,
    pub downloads: bool,
}

impl PlanForm {
    pub fn new() -> Self {
        Self::from_plan(&Plan {
            daily_limit: Some(1000),
            monthly_limit: Some(20_000),
            per_minute: Some(60),
            burst: Some(30),
            max_page_size: Some(100),
            max_export_rows: Some(0),
            ..Plan::default()
        })
        .with_id(None)
    }

    fn with_id(mut self, id: Option<i64>) -> Self {
        self.id = id;
        self
    }

    pub fn from_plan(p: &Plan) -> Self {
        Self {
            id: Some(p.id),
            name: p.name.clone(),
            daily_limit: OptNum::from(p.daily_limit, 1000),
            monthly_limit: OptNum::from(p.monthly_limit, 20_000),
            per_minute: OptNum::from(p.per_minute, 60),
            burst: OptNum::from(p.burst, 30),
            max_page_size: OptNum::from(p.max_page_size, 100),
            export: p.export,
            max_export_rows: OptNum::from(p.max_export_rows, 0),
            downloads: p.downloads,
        }
    }

    /// Every plan field (`null` = unlimited), as POST and PATCH accept.
    pub fn body(&self) -> Value {
        let mut m = Map::new();
        m.insert("name".into(), json!(self.name.trim()));
        for (k, v) in [
            ("daily_limit", &self.daily_limit),
            ("monthly_limit", &self.monthly_limit),
            ("per_minute", &self.per_minute),
            ("burst", &self.burst),
            ("max_page_size", &self.max_page_size),
            ("max_export_rows", &self.max_export_rows),
        ] {
            m.insert(k.into(), v.json());
        }
        m.insert("export".into(), json!(self.export));
        m.insert("downloads".into(), json!(self.downloads));
        Value::Object(m)
    }
}

impl Default for PlanForm {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct Plans {
    plans: Vec<Plan>,
    loaded: bool,
    t_list: Task<Vec<Plan>>,
    form: Option<PlanForm>,
    t_save: Task<Plan>,
    confirm_delete: Option<(i64, String)>,
    t_delete: Task<Value>,
}

impl Plans {
    fn load(&mut self, env: &Env<'_>) {
        self.loaded = true;
        self.t_list = Task::call(&env.ctx, env.client(), |c| c.get("/admin/api/plans"));
    }

    pub fn poll(&mut self, env: &mut Env<'_>) {
        if let Some(r) = self.t_list.poll()
            && let Some(p) = env.check(r)
        {
            self.plans = p;
        }
        if let Some(r) = self.t_save.poll()
            && let Some(p) = env.check(r)
        {
            env.toasts.success(format!("Saved plan {}", p.name));
            self.form = Some(PlanForm::from_plan(&p));
            env.signals.plans_changed = true;
            self.load(env);
        }
        if let Some(r) = self.t_delete.poll()
            && env.check(r).is_some()
        {
            env.toasts.success("Plan deleted");
            self.form = None;
            env.signals.plans_changed = true;
            self.load(env);
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, env: &mut Env<'_>) {
        if !self.loaded {
            self.load(env);
        }
        let can = env.can(perm::PLANS_MANAGE);
        if widgets::header(ui, "Plans", "quotas, rate limits and features; empty limit = unlimited", self.t_list.running()) {
            self.load(env);
        }
        ui.horizontal(|ui| {
            if widgets::primary(ui, can, "➕ New plan").on_disabled_hover_text("Requires plans.manage").clicked() {
                self.form = Some(PlanForm::new());
            }
            if !can {
                widgets::muted(ui, "Read-only: editing plans requires plans.manage.");
            }
        });
        ui.add_space(6.0);
        if self.form.is_some() {
            egui::SidePanel::right("plan-form")
                .resizable(true)
                .default_width(380.0)
                .min_width(340.0)
                .show_inside(ui, |ui| self.form_ui(ui, env, can));
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(ui, |ui| self.table(ui));

        if let Some((id, name)) = self.confirm_delete.clone() {
            match widgets::confirm(
                ui.ctx(),
                &format!("Delete plan {name}"),
                "Plans with users cannot be deleted (409); move their users first.",
                "Delete",
                true,
            ) {
                Some(true) => {
                    self.confirm_delete = None;
                    self.t_delete = Task::call(&env.ctx, env.client(), move |c| c.delete(&format!("/admin/api/plans/{id}")));
                }
                Some(false) => self.confirm_delete = None,
                None => {}
            }
        }
    }

    fn table(&mut self, ui: &mut Ui) {
        if self.plans.is_empty() {
            widgets::muted(ui, if self.t_list.running() { "Loading…" } else { "No plans." });
            return;
        }
        let p = Palette::get(ui);
        let selected = self.form.as_ref().and_then(|f| f.id);
        let mut clicked = None;
        let yes_no = |b: bool| if b { "yes" } else { "no" };
        TableBuilder::new(ui)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .id_salt("plans-table")
            .striped(true)
            .sense(egui::Sense::click())
            .auto_shrink(false)
            .column(Column::auto().at_least(36.0))
            .column(Column::auto().at_least(110.0))
            .columns(Column::initial(92.0), 9)
            .header(widgets::ROW_H, |mut h| {
                for t in ["ID", "Name", "Daily", "Monthly", "Per min", "Burst", "Page size", "Export", "Export rows", "Downloads", "Users"]
                {
                    h.col(|ui| {
                        ui.strong(t);
                    });
                }
            })
            .body(|body| {
                body.rows(widgets::ROW_H, self.plans.len(), |mut row| {
                    let pl = &self.plans[row.index()];
                    row.set_selected(selected == Some(pl.id));
                    row.col(|ui| {
                        ui.label(widgets::mono(pl.id.to_string()));
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(&pl.name).strong());
                    });
                    for v in [pl.daily_limit, pl.monthly_limit, pl.per_minute, pl.burst, pl.max_page_size] {
                        row.col(|ui| widgets::num(ui, format::limit(v)));
                    }
                    row.col(|ui| {
                        widgets::pill(ui, yes_no(pl.export), if pl.export { p.ok } else { p.muted });
                    });
                    row.col(|ui| widgets::num(ui, format::limit(pl.max_export_rows)));
                    row.col(|ui| {
                        widgets::pill(ui, yes_no(pl.downloads), if pl.downloads { p.ok } else { p.muted });
                    });
                    row.col(|ui| widgets::num(ui, pl.users.map_or("–".into(), format::signed)));
                    if row.response().clicked() {
                        clicked = Some(pl.clone());
                    }
                });
            });
        if let Some(pl) = clicked {
            self.form = Some(PlanForm::from_plan(&pl));
        }
    }

    fn form_ui(&mut self, ui: &mut Ui, env: &mut Env<'_>, can: bool) {
        let busy = self.t_save.running() || self.t_delete.running();
        let mut close = false;
        let mut save = false;
        let mut delete = None;
        let Some(f) = &mut self.form else { return };
        let users_on_plan = f.id.and_then(|id| self.plans.iter().find(|p| p.id == id)).and_then(|p| p.users).unwrap_or(0);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading(if f.id.is_some() { "Edit plan" } else { "New plan" });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("✖").on_hover_text("Close").clicked() {
                    close = true;
                }
                if busy {
                    ui.spinner();
                }
            });
        });
        ui.add_space(6.0);
        ui.add_enabled_ui(can, |ui| {
            egui::Grid::new("plan-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(200.0));
                ui.end_row();
                ui.label("Daily requests");
                f.daily_limit.ui(ui, "limit", "unlimited");
                ui.end_row();
                ui.label("Monthly requests");
                f.monthly_limit.ui(ui, "limit", "unlimited");
                ui.end_row();
                ui.label("Per minute");
                f.per_minute.ui(ui, "limit", "unlimited");
                ui.end_row();
                ui.label("Burst");
                f.burst.ui(ui, "limit", "unlimited");
                ui.end_row();
                ui.label("Max page size");
                f.max_page_size.ui(ui, "limit", "server maximum");
                ui.end_row();
                ui.label("Export");
                ui.checkbox(&mut f.export, "CSV/NDJSON export allowed");
                ui.end_row();
                ui.label("Max export rows");
                f.max_export_rows.ui(ui, "limit", "unlimited");
                ui.end_row();
                ui.label("Downloads");
                ui.checkbox(&mut f.downloads, "snapshot downloads allowed");
                ui.end_row();
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if widgets::primary(ui, !busy && !f.name.trim().is_empty(), if f.id.is_some() { "Save plan" } else { "Create plan" })
                    .clicked()
                {
                    save = true;
                }
                if let Some(id) = f.id {
                    let r = widgets::danger(ui, !busy && users_on_plan == 0, "Delete")
                        .on_disabled_hover_text(format!("{users_on_plan} user(s) are on this plan"));
                    if r.clicked() {
                        delete = Some((id, f.name.clone()));
                    }
                }
            });
        });
        if save {
            let body = f.body();
            self.t_save = match f.id {
                Some(id) => Task::call(&env.ctx, env.client(), move |c| c.patch(&format!("/admin/api/plans/{id}"), &body)),
                None => Task::call(&env.ctx, env.client(), move |c| c.post("/admin/api/plans", &body)),
            };
        }
        if delete.is_some() {
            self.confirm_delete = delete;
        }
        if close {
            self.form = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_round_trip_and_body() {
        let p = Plan {
            id: 2,
            name: "pro".into(),
            daily_limit: Some(100_000),
            monthly_limit: Some(2_000_000),
            per_minute: Some(600),
            burst: Some(120),
            max_page_size: Some(1000),
            export: true,
            max_export_rows: Some(100_000),
            downloads: false,
            users: Some(3),
        };
        let f = PlanForm::from_plan(&p);
        assert_eq!(f.id, Some(2));
        assert_eq!(
            f.body(),
            json!({"name":"pro","daily_limit":100000,"monthly_limit":2000000,"per_minute":600,"burst":120,
                   "max_page_size":1000,"export":true,"max_export_rows":100000,"downloads":false})
        );
        let unlimited = PlanForm::from_plan(&Plan { id: 4, name: "unlimited".into(), ..Plan::default() });
        assert_eq!(unlimited.body()["daily_limit"], Value::Null);
        assert_eq!(unlimited.body()["per_minute"], Value::Null);
        assert_eq!(PlanForm::new().id, None);
    }
}
