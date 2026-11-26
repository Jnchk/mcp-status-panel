#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod api;
mod app;
mod config;
mod icon;
mod model;
mod theme;
mod tray;
mod ui;
mod worker;

use std::{
    sync::{atomic::AtomicBool, mpsc},
    thread,
};

use eframe::egui;

use crate::app::App;
use crate::config::Config;

fn main() -> eframe::Result {
    let config = Config::load();
    if std::env::args().any(|arg| arg == "--check") {
        return api::check(config);
    }

    let (tx, rx) = mpsc::channel();
    let refresh = std::sync::Arc::new(AtomicBool::new(false));
    let worker_refresh = refresh.clone();
    let always_on_top = config.always_on_top;
    let font = config.font.clone();
    let tray_enabled = config.tray;
    let taskbar = if cfg!(windows) { config.taskbar } else { true };
    thread::spawn(move || worker::worker(config, tx, worker_refresh));

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([560.0, 430.0])
        .with_min_inner_size([360.0, 240.0])
        .with_taskbar(taskbar)
        .with_icon(icon::window_icon());
    if always_on_top {
        viewport = viewport.with_always_on_top();
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "Status Panel",
        options,
        Box::new(move |cc| {
            theme::install_font(&cc.egui_ctx, &font);
            theme::orange_theme(&cc.egui_ctx);
            Ok(Box::new(App::new(rx, refresh, tray_enabled)))
        }),
    )
}
