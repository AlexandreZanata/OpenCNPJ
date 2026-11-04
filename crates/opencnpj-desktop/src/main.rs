// Release builds on Windows are GUI applications: no console window.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    opencnpj_desktop::run()
}
