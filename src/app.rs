use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Receiver,
        Arc,
    },
    time::Duration,
};

use eframe::egui;

use crate::model::Snapshot;
use crate::theme::{card, section_title, ERR, MUTED, ORANGE, ORANGE_DEEP};
use crate::tray::{make_tray, TraySlot};
use crate::ui::{mcp_table, stats_card};

pub struct App {
    rx: Receiver<Snapshot>,
    refresh: Arc<AtomicBool>,
    snapshot: Option<Snapshot>,
    #[cfg_attr(not(windows), allow(dead_code))]
    visible: bool,
    quitting: bool,
    #[cfg_attr(not(windows), allow(dead_code))]
    tray: TraySlot,
}

impl App {
    pub fn new(rx: Receiver<Snapshot>, refresh: Arc<AtomicBool>, tray_enabled: bool) -> Self {
        Self {
            rx,
            refresh,
            snapshot: None,
            visible: true,
            quitting: false,
            tray: make_tray(tray_enabled),
        }
    }

    #[cfg(windows)]
    fn toggle_visibility(&mut self, ctx: &egui::Context) {
        self.visible = !self.visible;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.visible));
        if self.visible {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    #[cfg(windows)]
    fn poll_tray(&mut self, ctx: &egui::Context) {
        let (show_id, quit_id) = match &self.tray {
            Some(tray) => (tray.show_id.clone(), tray.quit_id.clone()),
            None => return,
        };

        while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
            if let tray_icon::TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = event
            {
                self.toggle_visibility(ctx);
            }
        }

        while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            if event.id == show_id {
                self.toggle_visibility(ctx);
            } else if event.id == quit_id {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Status Panel").size(18.0).strong().color(ORANGE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let button = egui::Button::new(egui::RichText::new("Refresh").color(egui::Color32::WHITE).strong())
                    .fill(ORANGE_DEEP);
                if ui.add(button).clicked() {
                    self.refresh.store(true, Ordering::SeqCst);
                }
                if ui.small_button("+").on_hover_text("Zoom in (Ctrl +)").clicked() {
                    ui.ctx().set_zoom_factor((ui.ctx().zoom_factor() + 0.1).clamp(0.7, 2.0));
                }
                if ui.small_button("−").on_hover_text("Zoom out (Ctrl -)").clicked() {
                    ui.ctx().set_zoom_factor((ui.ctx().zoom_factor() - 0.1).clamp(0.7, 2.0));
                }
                let status = match &self.snapshot {
                    Some(snapshot) => format!("updated {}s ago", snapshot.at.elapsed().as_secs()),
                    None => "loading...".to_string(),
                };
                ui.label(egui::RichText::new(status).color(MUTED).small());
            });
        });
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("OpenCode").color(MUTED).small());
            match self.snapshot.as_ref().map(|s| &s.latest_v2) {
                Some(Ok(Some(release))) => {
                    ui.label(
                        egui::RichText::new(format!("latest v2 {}", release.tag_name))
                            .monospace()
                            .strong()
                            .color(ORANGE_DEEP)
                            .small(),
                    );
                    if let Some(date) = release.published_at.as_deref().and_then(|at| at.get(..10)) {
                        ui.label(egui::RichText::new(date).color(MUTED).small());
                    }
                }
                Some(Ok(None)) => {
                    ui.label(egui::RichText::new("latest v2 unknown").color(MUTED).small());
                }
                Some(Err(error)) => {
                    ui.label(egui::RichText::new(error).color(ERR).small());
                }
                None => {
                    ui.label(egui::RichText::new("checking version...").color(MUTED).small());
                }
            }
        });
        ui.add_space(8.0);
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                card(ui, |ui| {
                    section_title(ui, "MCP servers");
                    match self.snapshot.as_ref().map(|s| &s.gateway) {
                        None => {
                            ui.label(egui::RichText::new("loading...").color(MUTED));
                        }
                        Some(Err(error)) => {
                            ui.label(egui::RichText::new(error).color(ERR));
                        }
                        Some(Ok(servers)) => mcp_table(ui, servers),
                    }
                });

                ui.add_space(12.0);

                card(ui, |ui| {
                    section_title(ui, "Token usage");
                    match self.snapshot.as_ref().map(|s| &s.stats) {
                        None => {
                            ui.label(egui::RichText::new("loading...").color(MUTED));
                        }
                        Some(Err(error)) => {
                            ui.label(egui::RichText::new(error).color(ERR));
                        }
                        Some(Ok(stats)) => stats_card(ui, stats),
                    }
                });
                ui.add_space(8.0);
            });
    }
}

impl eframe::App for App {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(500));
        #[cfg(windows)]
        self.poll_tray(ctx);

        if ctx.input(|i| i.viewport().close_requested()) {
            if self.quitting {
                return;
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.visible = false;
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        while let Ok(next) = self.rx.try_recv() {
            self.snapshot = Some(next);
        }
        ui.ctx().request_repaint_after(Duration::from_secs(1));

        egui::Frame::new()
            .inner_margin(egui::Margin { left: 16, right: 16, top: 10, bottom: 10 })
            .show(ui, |ui| {
                self.header(ui);
                self.body(ui);
            });
    }
}
