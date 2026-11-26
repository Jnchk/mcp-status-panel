#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod api;
mod app;
mod config;
mod icon;
mod icon_art;
mod layout;
mod mem;
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
use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use eframe::wgpu;

use crate::app::App;
use crate::config::Config;

fn wgpu_configuration() -> WgpuConfiguration {
    let WgpuSetupCreateNew {
        instance_descriptor,
        display_handle,
        power_preference,
        native_adapter_selector,
        device_descriptor,
    } = WgpuSetupCreateNew::without_display_handle();

    let instance_descriptor = wgpu::InstanceDescriptor {
        backends: if cfg!(windows) {
            wgpu::Backends::DX12
        } else {
            instance_descriptor.backends
        },
        ..instance_descriptor
    };

    let setup = WgpuSetupCreateNew {
        instance_descriptor,
        display_handle,
        power_preference,
        native_adapter_selector,
        device_descriptor: std::sync::Arc::new(move |adapter| {
            let mut descriptor = device_descriptor(adapter);
            descriptor.memory_hints = wgpu::MemoryHints::MemoryUsage;
            descriptor
        }),
    };

    WgpuConfiguration {
        wgpu_setup: WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}

fn main() -> eframe::Result {
    let config = Config::load();
    if std::env::args().any(|arg| arg == "--check") {
        return api::check(config);
    }
    let mem_log = std::env::args().any(|arg| arg == "--mem");

    let (tx, rx) = mpsc::channel();
    let refresh = std::sync::Arc::new(AtomicBool::new(false));
    let worker_refresh = refresh.clone();
    let always_on_top = config.always_on_top;
    let font = config.font.clone();
    let tray_enabled = config.tray;
    let zoom = config.zoom;
    let blocks = config.blocks.clone();
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
    let options = eframe::NativeOptions {
        viewport,
        wgpu_options: wgpu_configuration(),
        ..Default::default()
    };
    eframe::run_native(
        "Status Panel",
        options,
        Box::new(move |cc| {
            theme::install_font(&cc.egui_ctx, &font);
            theme::orange_theme(&cc.egui_ctx);
            cc.egui_ctx.set_zoom_factor(zoom);
            let render_state = cc.wgpu_render_state.clone();
            Ok(Box::new(App::new(
                rx,
                refresh,
                tray_enabled,
                zoom,
                blocks,
                render_state,
                mem_log,
            )))
        }),
    )
}
