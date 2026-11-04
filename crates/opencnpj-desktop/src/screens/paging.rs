//! Paging shared by the append-only logs (Searches, Audit): newest first, older pages are
//! requested with `before` = the smallest id loaded so far.

use eframe::egui::{self, Ui};

use crate::api::Task;
use crate::format;
use crate::state::Env;
use crate::widgets;

const LIMITS: [u32; 4] = [50, 100, 200, 500];

/// Path for one page: newest first, older than `before` when set.
pub fn page_path(base: &str, filters: &[(&str, String)], limit: u32, before: Option<i64>) -> String {
    let mut params: Vec<(&str, String)> = filters.iter().map(|(k, v)| (*k, v.trim().to_string())).collect();
    params.push(("limit", limit.to_string()));
    if let Some(b) = before {
        params.push(("before", b.to_string()));
    }
    format::with_query(base, &params)
}

/// Accepts an empty filter or a positive integer id.
pub fn valid_id(s: &str) -> bool {
    let s = s.trim();
    s.is_empty() || s.parse::<i64>().is_ok_and(|v| v > 0)
}

/// Shared paging state for both logs.
pub struct Pager<T> {
    pub rows: Vec<T>,
    pub limit: u32,
    pub exhausted: bool,
    pub started: bool,
    pub task: Task<(bool, Vec<T>)>,
}

impl<T> Default for Pager<T> {
    fn default() -> Self {
        Self { rows: Vec::new(), limit: 100, exhausted: false, started: false, task: Task::default() }
    }
}

impl<T: Send + 'static + serde::de::DeserializeOwned> Pager<T> {
    pub fn fetch(&mut self, env: &Env<'_>, path: String, append: bool) {
        self.started = true;
        self.task = Task::call(&env.ctx, env.client(), move |c| c.get::<Vec<T>>(&path).map(|v| (append, v)));
    }

    pub fn poll(&mut self, env: &mut Env<'_>) {
        if let Some(r) = self.task.poll()
            && let Some((append, rows)) = env.check(r)
        {
            self.exhausted = rows.len() < self.limit as usize;
            if append {
                self.rows.extend(rows);
            } else {
                self.rows = rows;
            }
        }
    }

    pub fn footer(&mut self, ui: &mut Ui) -> bool {
        let mut more = false;
        ui.horizontal(|ui| {
            widgets::muted(ui, format!("{} rows loaded", format::int(self.rows.len() as u64)));
            if ui.add_enabled(!self.exhausted && !self.task.running() && !self.rows.is_empty(), egui::Button::new("Load more ⏷")).clicked()
            {
                more = true;
            }
            if self.exhausted && !self.rows.is_empty() {
                widgets::muted(ui, "end of log");
            }
            if self.task.running() {
                ui.spinner();
            }
        });
        more
    }

    pub fn limit_combo(&mut self, ui: &mut Ui, id: &str) -> bool {
        let mut changed = false;
        ui.label("Page");
        egui::ComboBox::from_id_salt(id).selected_text(self.limit.to_string()).width(70.0).show_ui(ui, |ui| {
            for l in LIMITS {
                changed |= ui.selectable_value(&mut self.limit, l, l.to_string()).changed();
            }
        });
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_paths() {
        assert_eq!(page_path("/admin/api/searches", &[("user_id", String::new())], 100, None), "/admin/api/searches?limit=100");
        assert_eq!(
            page_path("/admin/api/searches", &[("user_id", " 7 ".into())], 50, Some(1234)),
            "/admin/api/searches?user_id=7&limit=50&before=1234"
        );
        assert_eq!(
            page_path("/admin/api/audit", &[("actor_id", "1".into()), ("target_id", String::new())], 200, Some(9)),
            "/admin/api/audit?actor_id=1&limit=200&before=9"
        );
    }

    #[test]
    fn id_filters() {
        assert!(valid_id(""));
        assert!(valid_id(" 42 "));
        assert!(!valid_id("0"));
        assert!(!valid_id("-3"));
        assert!(!valid_id("ana"));
    }
}
