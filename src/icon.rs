use eframe::egui;

pub use crate::icon_art::rgba;

pub fn window_icon() -> egui::IconData {
    let size = 64;
    egui::IconData {
        rgba: rgba(size),
        width: size,
        height: size,
    }
}
