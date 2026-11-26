use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Receiver,
        Arc,
    },
    time::{Duration, Instant},
};

use eframe::egui;
use eframe::egui_wgpu::RenderState;

use crate::config::BlockConfig;
use crate::layout;
use crate::mem;
use crate::model::Snapshot;
use crate::theme::{card, ERR, MUTED, ORANGE, ORANGE_DEEP};
use crate::tray::{make_tray, TraySlot};
use crate::ui::{block_title, chip, mcp_table, minimize_button, stats_card, CardAction};

pub struct App {
    rx: Receiver<Snapshot>,
    refresh: Arc<AtomicBool>,
    snapshot: Option<Snapshot>,
    #[cfg_attr(not(windows), allow(dead_code))]
    visible: bool,
    quitting: bool,
    #[cfg_attr(not(windows), allow(dead_code))]
    tray: TraySlot,
    zoom: f32,
    blocks: Vec<BlockConfig>,
    widths: Vec<f32>,
    render_state: Option<RenderState>,
    mem_log: bool,
    last_mem: Instant,
}

impl App {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rx: Receiver<Snapshot>,
        refresh: Arc<AtomicBool>,
        tray_enabled: bool,
        zoom: f32,
        blocks: Vec<BlockConfig>,
        render_state: Option<RenderState>,
        mem_log: bool,
    ) -> Self {
        let widths = vec![0.0; blocks.len()];
        Self {
            rx,
            refresh,
            snapshot: None,
            visible: true,
            quitting: false,
            tray: make_tray(tray_enabled),
            zoom,
            blocks,
            widths,
            render_state,
            mem_log,
            last_mem: Instant::now(),
        }
    }

    #[cfg(windows)]
    fn show_window(&mut self, ctx: &egui::Context) {
        self.visible = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    #[cfg(windows)]
    fn hide_window(&mut self, ctx: &egui::Context) {
        self.visible = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }

    #[cfg(windows)]
    fn toggle_visibility(&mut self, ctx: &egui::Context) {
        if self.visible {
            self.hide_window(ctx);
        } else {
            self.show_window(ctx);
        }
    }

    #[cfg(windows)]
    fn tray_click(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
        if self.visible && focused {
            self.hide_window(ctx);
        } else {
            self.show_window(ctx);
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
                self.tray_click(ctx);
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
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.place(ui);
                ui.add_space(8.0);
            });
    }

    fn place(&mut self, ui: &mut egui::Ui) {
        let mut cards = Vec::new();
        let mut chips = Vec::new();
        for (index, block) in self.blocks.iter().enumerate() {
            if !block.visible {
                continue;
            }
            if block.minimized {
                chips.push(index);
            } else {
                cards.push(index);
            }
        }

        let available = ui.available_width();
        let widths: Vec<f32> = cards.iter().map(|&index| self.widths[index]).collect();
        let rows = layout::rows(&widths, available, layout::GAP);
        let mut chips_placed = false;

        for (row_index, row) in rows.iter().enumerate() {
            let used: f32 = row.iter().map(|&position| self.widths[cards[position]]).sum::<f32>()
                + layout::GAP * row.len().saturating_sub(1) as f32;
            let remaining = available - used - layout::GAP;
            let last_row = row_index + 1 == rows.len();
            let place_chips = last_row && !chips.is_empty() && remaining >= self.chip_width(&chips);

            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = layout::GAP;
                for &position in row {
                    let index = cards[position];
                    let response = self.render_card(ui, index);
                    self.widths[index] = response.rect.width();
                }                if place_chips {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        for &index in &chips {
                            self.widths[index] = self.render_chip(ui, index);
                        }
                    });
                }
            });
            chips_placed |= place_chips;
            ui.add_space(layout::GAP);
        }

        if !chips.is_empty() && !chips_placed {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for &index in &chips {
                    self.widths[index] = self.render_chip(ui, index);
                }
            });
        }
    }

    fn chip_width(&self, chips: &[usize]) -> f32 {
        chips.iter().map(|&index| self.widths[index]).fold(0.0f32, f32::max).max(110.0)
    }

    fn render_chip(&mut self, ui: &mut egui::Ui, index: usize) -> f32 {
        let title = block_title(&self.blocks[index].id).to_string();
        let (clicked, width) = chip(ui, &title);
        if clicked {
            self.blocks[index].minimized = false;
        }
        width
    }

    fn render_card(&mut self, ui: &mut egui::Ui, index: usize) -> egui::Response {
        let title = block_title(&self.blocks[index].id).to_string();
        let mut action = CardAction::None;
        let response = card(ui, |ui| {
            let top = ui.cursor().top();
            ui.label(egui::RichText::new(&title).size(16.0).strong().color(crate::theme::TEXT));
            ui.add_space(4.0);
            self.render_block(ui, index);

            let content = ui.min_rect();
            let rect = egui::Rect::from_min_size(
                egui::pos2(content.right() - 18.0, top),
                egui::vec2(18.0, 22.0),
            );
            if ui
                .put(rect, minimize_button())
                .on_hover_text("Minimize")
                .clicked()
            {
                action = CardAction::Minimize;
            }
        });
        if action == CardAction::Minimize {
            self.blocks[index].minimized = true;
        }
        response
    }

    fn render_block(&mut self, ui: &mut egui::Ui, index: usize) {
        let id = self.blocks[index].id.as_str();
        match id {
            "mcp" => match self.snapshot.as_ref().map(|s| &s.gateway) {
                None => {
                    ui.label(egui::RichText::new("loading...").color(MUTED));
                }
                Some(Err(error)) => {
                    ui.label(egui::RichText::new(error).color(ERR));
                }
                Some(Ok(servers)) => mcp_table(ui, servers),
            },
            "usage" => match self.snapshot.as_ref().map(|s| &s.stats) {
                None => {
                    ui.label(egui::RichText::new("loading...").color(MUTED));
                }
                Some(Err(error)) => {
                    ui.label(egui::RichText::new(error).color(ERR));
                }
                Some(Ok(stats)) => stats_card(ui, stats),
            },
            other => {
                ui.label(egui::RichText::new(format!("no renderer for block '{other}'")).color(ERR));
            }
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(500));
        let zoom = ctx.zoom_factor();
        if (zoom - self.zoom).abs() > 0.005 {
            self.zoom = zoom;
            crate::config::Config::save_zoom(zoom);
        }
        if self.mem_log && self.last_mem.elapsed() >= Duration::from_secs(5) {
            self.last_mem = Instant::now();
            mem::write(self.render_state.as_ref());
        }
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
