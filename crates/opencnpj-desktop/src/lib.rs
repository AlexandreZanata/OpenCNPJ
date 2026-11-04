//! OpenCNPJ Admin: native desktop administration for an OpenCNPJ server (eframe/egui).
//!
//! The application talks to the admin listener over HTTP (`docs/ACCOUNTS.md`); every call
//! runs on a background thread ([`api::Task`]) and the UI repaints when it completes.

pub mod api;
pub mod app;
pub mod config;
pub mod format;
pub mod icon;
pub mod models;
pub mod screens;
pub mod state;
pub mod theme;
pub mod widgets;

use std::sync::Arc;

use eframe::egui;

pub const APP_NAME: &str = "OpenCNPJ Admin";
pub const APP_ID: &str = "opencnpj-admin";

/// Opens the main window and runs until it is closed.
pub fn run() -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_app_id(APP_ID)
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([960.0, 600.0])
        .with_icon(Arc::new(icon::icon_data()));
    let options = eframe::NativeOptions { viewport, centered: true, ..Default::default() };
    eframe::run_native(APP_NAME, options, Box::new(|cc| Ok(Box::new(app::AdminApp::new(cc)))))
}
