use std::path::PathBuf;

use eframe::egui;

pub const BG: egui::Color32 = egui::Color32::from_rgb(251, 247, 242);
pub const CARD: egui::Color32 = egui::Color32::WHITE;
pub const TEXT: egui::Color32 = egui::Color32::from_rgb(31, 35, 40);
pub const MUTED: egui::Color32 = egui::Color32::from_rgb(107, 114, 128);
pub const BORDER: egui::Color32 = egui::Color32::from_rgb(234, 223, 210);
pub const STRIPE: egui::Color32 = egui::Color32::from_rgb(246, 239, 230);
pub const ORANGE: egui::Color32 = egui::Color32::from_rgb(234, 88, 12);
pub const ORANGE_DEEP: egui::Color32 = egui::Color32::from_rgb(194, 65, 12);
pub const OK: egui::Color32 = egui::Color32::from_rgb(21, 128, 61);
pub const WARN: egui::Color32 = egui::Color32::from_rgb(180, 83, 9);
pub const ERR: egui::Color32 = egui::Color32::from_rgb(220, 38, 38);

const BUNDLED: &[u8] = include_bytes!("../assets/MapleMono-CN-Regular.subset.ttf");

/// Default font: covers Latin, Simplified Chinese and kana with matching metrics.
pub const DEFAULT_FONT: &str = "Maple Mono CN";

/// Loads the configured font. Returns a warning when the name was set but not found.
pub fn install_font(ctx: &egui::Context, name: &str) -> Option<String> {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "bundled".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(BUNDLED)),
    );

    // Anything but the bundled family is a deliberate override and goes first.
    let mut warning = None;
    let custom = if is_bundled(name) {
        None
    } else {
        match font_data(name) {
            Some(data) => Some(data),
            None => {
                warning = Some(format!("font \"{name}\" not found, using {DEFAULT_FONT}"));
                None
            }
        }
    };
    let has_custom = custom.is_some();
    if let Some(data) = custom {
        fonts
            .font_data
            .insert("custom".to_owned(), std::sync::Arc::new(data));
    }

    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let list = fonts.families.entry(family).or_default();
        list.insert(0, "bundled".to_owned());
        if has_custom {
            list.insert(0, "custom".to_owned());
        }
    }
    ctx.set_fonts(fonts);
    warning
}

fn is_bundled(name: &str) -> bool {
    let name = name.trim().to_lowercase();
    // "Noto..." was the bundled name before Maple; treat it as the bundled font too.
    name.is_empty() || name.starts_with("maple") || name.starts_with("noto")
}

/// Windows ships the UI faces inside a collection, so a name maps to file + face index.
fn windows_face(name: &str) -> Option<(&'static str, u32)> {
    match name.trim().to_ascii_lowercase().as_str() {
        "microsoft yahei ui" | "microsoft yahei ui regular" => Some(("msyh.ttc", 1)),
        "microsoft yahei" | "microsoft yahei regular" => Some(("msyh.ttc", 0)),
        _ => None,
    }
}

fn font_data(name: &str) -> Option<egui::FontData> {
    if let Some((file, index)) = windows_face(name) {
        let bytes = std::fs::read(PathBuf::from(r"C:\Windows\Fonts").join(file)).ok()?;
        return Some(egui::FontData::from_blob(std::sync::Arc::new(bytes), index));
    }
    font_file(name)
        .and_then(|path| std::fs::read(path).ok())
        .map(egui::FontData::from_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_font_is_a_font() {
        assert!(BUNDLED.len() > 500_000, "subset should ship the CJK glyphs");
        assert!(BUNDLED.starts_with(b"OTTO") || BUNDLED.starts_with(&[0, 1, 0, 0]));
    }

    #[test]
    fn bundled_names_bypass_the_system_lookup() {
        assert!(is_bundled(""));
        assert!(is_bundled("Maple Mono CN"));
        assert!(is_bundled("Noto Sans Mono CJK SC"));
        assert!(!is_bundled("Microsoft YaHei UI"));
    }

    #[test]
    fn yahei_names_pick_the_right_face() {
        assert_eq!(windows_face("Microsoft YaHei UI"), Some(("msyh.ttc", 1)));
        assert_eq!(windows_face("microsoft yahei"), Some(("msyh.ttc", 0)));
        assert_eq!(windows_face("Consolas"), None);
    }
}

fn font_file(name: &str) -> Option<PathBuf> {
    let direct = PathBuf::from(name);
    if direct.is_file() {
        return Some(direct);
    }
    let file = match name.to_ascii_lowercase().as_str() {
        "consolas" => "consola.ttf".to_string(),
        "dejavu sans mono" | "dejavusansmono" => "DejaVuSansMono.ttf".to_string(),
        other => format!("{other}.ttf"),
    };
    let dirs: &[&str] = if cfg!(windows) {
        &[r"C:\Windows\Fonts"]
    } else {
        &["/usr/share/fonts/truetype/dejavu", "/usr/share/fonts"]
    };
    dirs.iter()
        .map(|dir| PathBuf::from(dir).join(&file))
        .find(|path| path.is_file())
}

pub fn orange_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = BG;
    visuals.window_fill = BG;
    visuals.extreme_bg_color = CARD;
    visuals.faint_bg_color = STRIPE;
    visuals.override_text_color = Some(TEXT);
    visuals.weak_text_color = Some(MUTED);
    visuals.hyperlink_color = ORANGE_DEEP;
    visuals.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(234, 88, 12, 48);
    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.inactive.weak_bg_fill = CARD;
    visuals.widgets.inactive.fg_stroke.color = TEXT;
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(250, 235, 220);
    visuals.widgets.hovered.fg_stroke.color = TEXT;
    visuals.widgets.active.bg_fill = ORANGE_DEEP;
    visuals.widgets.active.fg_stroke.color = egui::Color32::WHITE;

    let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 28.0;
    style.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
    style.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(14.0));
    style.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
    style.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
    style.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(12.5));

    ctx.set_theme(egui::ThemePreference::Light);
    ctx.set_visuals_of(egui::Theme::Light, visuals);
    ctx.set_style_of(egui::Theme::Light, style);
}

pub fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    egui::Frame::new()
        .fill(CARD)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.vertical(add);
        })
        .response
}
