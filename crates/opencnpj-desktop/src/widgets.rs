//! Reusable widgets: cards, KPI tiles, pills, limit editors, modal dialogs, generic tables and
//! charts.

use std::collections::BTreeSet;

use eframe::egui::{self, Align, Color32, Layout, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use egui_plot::{Bar, BarChart, Legend, Line, Plot, PlotPoints};
use serde_json::Value;

use crate::format;
use crate::theme::Palette;

pub const ROW_H: f32 = 24.0;

pub fn mono(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).monospace()
}

/// Card frame with consistent padding and border.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = Palette::get(ui);
    egui::Frame::new()
        .fill(p.card)
        .stroke(egui::Stroke::new(1.0_f32, p.line))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, add)
        .inner
}

/// Card with a title line.
pub fn section<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).strong().size(15.0));
        ui.add_space(6.0);
        add(ui)
    })
}

const KPI_MIN_WIDTH: f32 = 168.0;
const KPI_GAP: f32 = 10.0;
/// Inner margin (2 × 12) plus stroke (2 × 1) of a [`card`].
const CARD_CHROME: f32 = 26.0;

/// Tiles per row for a KPI grid `available` points wide.
pub fn kpi_columns(available: f32) -> usize {
    (((available + KPI_GAP) / (KPI_MIN_WIDTH + KPI_GAP)).floor() as usize).max(1)
}

/// KPI tile `width` points wide: large value (truncated, full text on hover), small label.
pub fn kpi(ui: &mut Ui, label: &str, value: impl Into<String>, width: f32) {
    let p = Palette::get(ui);
    let value = value.into();
    card(ui, |ui| {
        ui.set_width((width - CARD_CHROME).max(40.0));
        ui.vertical(|ui| {
            ui.add(egui::Label::new(RichText::new(&value).monospace().size(18.0).strong()).truncate()).on_hover_text(&value);
            ui.add(egui::Label::new(RichText::new(label).small().color(p.muted)).truncate());
        });
    });
}

/// Grid of KPI tiles: as many columns as fit, tiles stretched to fill each row.
/// (Frames cannot be wrapped by `horizontal_wrapped`, which does not know their size.)
pub fn kpis(ui: &mut Ui, items: &[(&str, String)]) {
    let available = ui.available_width();
    let n = kpi_columns(available);
    let width = ((available - KPI_GAP * (n - 1) as f32) / n as f32).floor();
    for row in items.chunks(n) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = KPI_GAP;
            for (label, value) in row {
                kpi(ui, label, value.clone(), width);
            }
        });
        ui.add_space(KPI_GAP - ui.spacing().item_spacing.y);
    }
}

/// Small colored label with a tinted background.
pub fn pill(ui: &mut Ui, text: &str, color: Color32) -> egui::Response {
    ui.label(RichText::new(format!(" {text} ")).small().strong().color(color).background_color(color.gamma_multiply(0.16)))
}

/// Pill colored by account or job status.
pub fn status_pill(ui: &mut Ui, status: &str) -> egui::Response {
    let c = Palette::get(ui).status(status);
    pill(ui, if status.is_empty() { "–" } else { status }, c)
}

pub fn muted(ui: &mut Ui, text: impl Into<String>) -> egui::Response {
    let c = Palette::get(ui).muted;
    ui.label(RichText::new(text.into()).color(c))
}

/// Right-aligned monospace number for table cells.
pub fn num(ui: &mut Ui, text: impl Into<String>) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.label(mono(text));
    });
}

/// Right-aligned column header for numeric columns.
pub fn num_header(ui: &mut Ui, text: &str) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.strong(text);
    });
}

/// Screen title with an optional spinner and a refresh button; returns true on refresh.
pub fn header(ui: &mut Ui, title: &str, subtitle: &str, busy: bool) -> bool {
    let mut refresh = false;
    ui.horizontal(|ui| {
        ui.heading(title);
        if !subtitle.is_empty() {
            muted(ui, subtitle);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            refresh = ui.add_enabled(!busy, egui::Button::new("⟳ Refresh")).on_hover_text("Reload from the server").clicked();
            if busy {
                ui.spinner();
            }
        });
    });
    ui.add_space(4.0);
    refresh
}

/// Combo box over string options, with an optional "any" entry mapped to an empty string.
pub fn combo(ui: &mut Ui, id: &str, value: &mut String, options: &[String], any: Option<&str>) -> bool {
    let before = value.clone();
    let shown = if value.is_empty() { any.unwrap_or("–").to_string() } else { value.clone() };
    egui::ComboBox::from_id_salt(id).selected_text(shown).show_ui(ui, |ui| {
        if let Some(a) = any {
            ui.selectable_value(value, String::new(), a);
        }
        for o in options {
            ui.selectable_value(value, o.clone(), o);
        }
    });
    *value != before
}

/// Editor state for a numeric limit where `null` means unlimited.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OptNum {
    pub set: bool,
    pub value: i64,
}

impl OptNum {
    pub fn from(v: Option<i64>, fallback: i64) -> Self {
        Self { set: v.is_some(), value: v.unwrap_or(fallback) }
    }

    pub fn get(&self) -> Option<i64> {
        self.set.then_some(self.value)
    }

    pub fn json(&self) -> Value {
        self.get().map_or(Value::Null, Value::from)
    }

    /// Checkbox plus value; `unset_label` names what `null` means here.
    pub fn ui(&mut self, ui: &mut Ui, set_label: &str, unset_label: &str) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.set, set_label);
            if self.set {
                ui.add(egui::DragValue::new(&mut self.value).range(0..=i64::MAX).speed(10.0));
            } else {
                muted(ui, unset_label);
            }
        });
    }
}

/// A key shown exactly once after create or rotate.
pub struct RevealedKey {
    pub title: String,
    pub key: String,
}

/// Modal that shows a freshly issued key with a copy button. Clears `slot` when closed.
pub fn key_modal(ctx: &egui::Context, slot: &mut Option<RevealedKey>) {
    let Some(k) = slot.as_ref() else { return };
    let mut close = false;
    egui::Modal::new(egui::Id::new("key-modal")).show(ctx, |ui| {
        let p = Palette::get(ui);
        ui.set_width(520.0);
        ui.heading(&k.title);
        ui.add_space(6.0);
        ui.label(
            RichText::new("This key is shown only once. Copy it now and give it to the account owner over a secure channel.").color(p.warn),
        );
        ui.add_space(8.0);
        let mut text = k.key.clone();
        ui.add(egui::TextEdit::singleline(&mut text).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("📋 Copy key").clicked() {
                ui.ctx().copy_text(k.key.clone());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Done").clicked() {
                    close = true;
                }
            });
        });
    });
    if close {
        *slot = None;
    }
}

/// Yes/no modal. Returns `Some(true)` on confirm, `Some(false)` on cancel, `None` while open.
pub fn confirm(ctx: &egui::Context, title: &str, body: &str, action: &str, danger: bool) -> Option<bool> {
    let mut out = None;
    let r = egui::Modal::new(egui::Id::new(("confirm", title))).show(ctx, |ui| {
        let p = Palette::get(ui);
        ui.set_width(420.0);
        ui.heading(title);
        ui.add_space(6.0);
        ui.label(body);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                out = Some(false);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let b = egui::Button::new(RichText::new(action).color(Color32::WHITE)).fill(if danger { p.bad } else { p.accent });
                if ui.add(b).clicked() {
                    out = Some(true);
                }
            });
        });
    });
    if out.is_none() && r.should_close() {
        out = Some(false);
    }
    out
}

/// Primary (accent filled) button.
pub fn primary(ui: &mut Ui, enabled: bool, text: &str) -> egui::Response {
    let p = Palette::get(ui);
    ui.add_enabled(enabled, egui::Button::new(RichText::new(text).color(Color32::WHITE)).fill(p.accent))
}

/// Danger (red filled) button.
pub fn danger(ui: &mut Ui, enabled: bool, text: &str) -> egui::Response {
    let p = Palette::get(ui);
    ui.add_enabled(enabled, egui::Button::new(RichText::new(text).color(Color32::WHITE)).fill(p.bad))
}

/// Table of loosely specified JSON objects: one column per key (union, first-seen order).
pub fn json_table(ui: &mut Ui, id: &str, rows: &[Value]) {
    if rows.is_empty() {
        muted(ui, "none");
        return;
    }
    let mut keys: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();
    for r in rows {
        if let Some(o) = r.as_object() {
            for k in o.keys() {
                if seen.insert(k.clone()) {
                    keys.push(k.clone());
                }
            }
        }
    }
    // Identifiers first, then the server's order.
    keys.sort_by_key(|k| match k.as_str() {
        "id" => 0,
        "prefix" | "key_prefix" | "kind" | "status" => 1,
        _ => 2,
    });
    if keys.is_empty() {
        for r in rows {
            ui.label(mono(format::value(r)));
        }
        return;
    }
    egui::ScrollArea::horizontal().id_salt(("json-table-scroll", id)).show(ui, |ui| {
        TableBuilder::new(ui)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .id_salt(("json-table", id))
            .striped(true)
            .vscroll(false)
            .columns(Column::auto().at_least(60.0).clip(true), keys.len())
            .header(ROW_H, |mut h| {
                for k in &keys {
                    h.col(|ui| {
                        ui.strong(k);
                    });
                }
            })
            .body(|body| {
                body.rows(ROW_H, rows.len(), |mut row| {
                    let r = &rows[row.index()];
                    for k in &keys {
                        row.col(|ui| {
                            ui.label(mono(format::field(k, r.get(k).unwrap_or(&Value::Null))));
                        });
                    }
                });
            });
    });
}

/// One series of a chart.
pub struct Series<'a> {
    pub name: &'a str,
    pub values: Vec<f64>,
    pub color: Color32,
}

fn label_at(labels: &[String], x: f64) -> String {
    let i = x.round();
    if (x - i).abs() > 0.01 || i < 0.0 {
        return String::new();
    }
    labels.get(i as usize).cloned().unwrap_or_default()
}

/// Bar chart over categorical labels; with `stacked`, series are stacked in order.
pub fn bar_chart(ui: &mut Ui, id: &str, labels: &[String], series: &[Series<'_>], stacked: bool, height: f32) {
    let n = series.len().max(1) as f64;
    let width = if stacked { 0.75 } else { 0.8 / n };
    let charts: Vec<BarChart> = series
        .iter()
        .enumerate()
        .map(|(k, s)| {
            let offset = if stacked { 0.0 } else { (k as f64 - (n - 1.0) / 2.0) * width };
            let bars = s
                .values
                .iter()
                .enumerate()
                .map(|(i, v)| Bar::new(i as f64 + offset, *v).width(width).fill(s.color).name(label_at(labels, i as f64)))
                .collect();
            BarChart::new(s.name, bars).color(s.color)
        })
        .collect();
    let mut stackedc: Vec<BarChart> = Vec::new();
    for c in charts {
        let c = if stacked {
            let refs: Vec<&BarChart> = stackedc.iter().collect();
            c.stack_on(&refs)
        } else {
            c
        };
        stackedc.push(c);
    }
    Plot::new(("bar", id))
        .height(height)
        .legend(Legend::default())
        .allow_zoom(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .include_y(0.0)
        .y_axis_min_width(40.0)
        .x_axis_formatter(|m, _| label_at(labels, m.value))
        .show(ui, |p| {
            for c in stackedc {
                p.bar_chart(c);
            }
        });
}

/// Line chart; `x_fmt` labels the x axis and the hover tooltip.
pub fn line_chart(ui: &mut Ui, id: &str, xs: &[f64], series: &[Series<'_>], x_fmt: &dyn Fn(f64) -> String, height: f32) {
    Plot::new(("line", id))
        .height(height)
        .legend(Legend::default())
        .allow_zoom(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .include_y(0.0)
        .y_axis_min_width(40.0)
        .x_axis_formatter(|m, _| x_fmt(m.value))
        .label_formatter(|name, v| {
            let n = if name.is_empty() { String::new() } else { format!("{name}\n") };
            format!("{n}{}\n{:.2}", x_fmt(v.x), v.y)
        })
        .show(ui, |p| {
            for s in series {
                let pts: PlotPoints = xs.iter().zip(&s.values).map(|(x, y)| [*x, *y]).collect();
                p.line(Line::new(s.name, pts).color(s.color).width(1.8_f32).fill(0.0_f32).fill_alpha(0.12_f32));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opt_num_maps_to_json_null() {
        let mut n = OptNum::from(None, 1000);
        assert_eq!(n.json(), Value::Null);
        assert_eq!(n.value, 1000);
        n.set = true;
        n.value = 60;
        assert_eq!(n.json(), Value::from(60));
        assert_eq!(OptNum::from(Some(5), 0).get(), Some(5));
    }

    #[test]
    fn kpi_grid_columns() {
        assert_eq!(kpi_columns(100.0), 1);
        assert_eq!(kpi_columns(168.0), 1);
        assert_eq!(kpi_columns(346.0), 2);
        assert_eq!(kpi_columns(1030.0), 5);
    }

    #[test]
    fn category_labels() {
        let labels = vec!["a".to_string(), "b".to_string()];
        assert_eq!(label_at(&labels, 1.0), "b");
        assert_eq!(label_at(&labels, 0.5), "");
        assert_eq!(label_at(&labels, 7.0), "");
        assert_eq!(label_at(&labels, -1.0), "");
    }
}
