//! Dark and light styles and the semantic colors used across screens.

use eframe::egui::{self, Color32, CornerRadius, FontId, Stroke, TextStyle, Theme, Visuals};

pub const ACCENT_DARK: Color32 = Color32::from_rgb(0x60, 0xa5, 0xfa);
pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(0x25, 0x63, 0xeb);

/// Semantic colors for the current theme.
#[derive(Clone, Copy)]
pub struct Palette {
    pub accent: Color32,
    pub accent2: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub bad: Color32,
    pub muted: Color32,
    pub card: Color32,
    pub line: Color32,
}

impl Palette {
    pub fn of(dark: bool) -> Self {
        if dark {
            Self {
                accent: ACCENT_DARK,
                accent2: Color32::from_rgb(0xc0, 0x84, 0xfc),
                ok: Color32::from_rgb(0x4a, 0xde, 0x80),
                warn: Color32::from_rgb(0xfb, 0xbf, 0x24),
                bad: Color32::from_rgb(0xf8, 0x71, 0x71),
                muted: Color32::from_rgb(0x98, 0xa2, 0xb3),
                card: Color32::from_rgb(0x1b, 0x20, 0x29),
                line: Color32::from_rgb(0x2e, 0x35, 0x41),
            }
        } else {
            Self {
                accent: ACCENT_LIGHT,
                accent2: Color32::from_rgb(0x93, 0x33, 0xea),
                ok: Color32::from_rgb(0x15, 0x80, 0x3d),
                warn: Color32::from_rgb(0xb4, 0x53, 0x09),
                bad: Color32::from_rgb(0xdc, 0x26, 0x26),
                muted: Color32::from_rgb(0x66, 0x70, 0x85),
                card: Color32::WHITE,
                line: Color32::from_rgb(0xe4, 0xe7, 0xec),
            }
        }
    }

    pub fn get(ui: &egui::Ui) -> Self {
        Self::of(ui.visuals().dark_mode)
    }

    /// Color for an account or job status.
    pub fn status(&self, status: &str) -> Color32 {
        match status {
            "active" | "ok" | "open" | "enabled" => self.ok,
            "pending" | "running" | "killing" => self.warn,
            "suspended" | "banned" | "failed" | "killed" | "closed" => self.bad,
            _ => self.muted,
        }
    }
}

fn tune(style: &mut egui::Style, dark: bool) {
    let p = Palette::of(dark);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.interact_size.y = 24.0;
    style.spacing.window_margin = egui::Margin::same(14);
    style.spacing.combo_width = 140.0;
    style.text_styles = [
        (TextStyle::Heading, FontId::proportional(20.0)),
        (TextStyle::Body, FontId::proportional(14.0)),
        (TextStyle::Button, FontId::proportional(14.0)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
        (TextStyle::Small, FontId::proportional(11.5)),
    ]
    .into();

    let v: &mut Visuals = &mut style.visuals;
    let radius = CornerRadius::same(6);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = radius;
    }
    v.selection.bg_fill = p.accent.gamma_multiply(if dark { 0.45 } else { 0.25 });
    v.selection.stroke = Stroke::new(1.0_f32, p.accent);
    v.hyperlink_color = p.accent;
    v.striped = true;
    if dark {
        v.panel_fill = Color32::from_rgb(0x13, 0x17, 0x1e);
        v.window_fill = Color32::from_rgb(0x1b, 0x20, 0x29);
        v.extreme_bg_color = Color32::from_rgb(0x0e, 0x11, 0x16);
        v.faint_bg_color = Color32::from_rgb(0x1a, 0x1f, 0x28);
    } else {
        v.panel_fill = Color32::from_rgb(0xf6, 0xf7, 0xf9);
        v.window_fill = Color32::WHITE;
        v.extreme_bg_color = Color32::WHITE;
        v.faint_bg_color = Color32::from_rgb(0xf0, 0xf2, 0xf5);
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.line);
}

/// Installs both styles; the active one follows [`egui::Context::set_theme`].
pub fn install(ctx: &egui::Context) {
    ctx.style_mut_of(Theme::Dark, |s| tune(s, true));
    ctx.style_mut_of(Theme::Light, |s| tune(s, false));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every non-ASCII character in the UI sources must exist in egui's default fonts
    /// (otherwise it renders as an empty box).
    #[test]
    fn ui_glyphs_exist_in_default_fonts() {
        let sources = [
            include_str!("app.rs"),
            include_str!("widgets.rs"),
            include_str!("format.rs"),
            include_str!("state.rs"),
            include_str!("screens/mod.rs"),
            include_str!("screens/login.rs"),
            include_str!("screens/dashboard.rs"),
            include_str!("screens/users.rs"),
            include_str!("screens/plans.rs"),
            include_str!("screens/usage.rs"),
            include_str!("screens/searches.rs"),
            include_str!("screens/audit.rs"),
            include_str!("screens/paging.rs"),
            include_str!("screens/operations.rs"),
        ];
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |_| {});
        let mut missing: Vec<char> = sources
            .iter()
            .flat_map(|s| s.chars())
            .filter(|c| !c.is_ascii())
            .filter(|c| !ctx.fonts_mut(|f| f.has_glyph(&FontId::proportional(14.0), *c)))
            .collect();
        missing.sort_unstable();
        missing.dedup();
        assert!(missing.is_empty(), "glyphs missing from the default fonts: {missing:?}");
    }

    #[test]
    fn status_colors() {
        let p = Palette::of(true);
        assert_eq!(p.status("active"), p.ok);
        assert_eq!(p.status("banned"), p.bad);
        assert_eq!(p.status("pending"), p.warn);
        assert_eq!(p.status("whatever"), p.muted);
    }
}
