#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use eframe::egui;
use serde::Deserialize;

#[derive(Clone)]
struct Config {
    gateway_url: String,
    gateway_token: String,
    opencode_url: String,
    opencode_user: String,
    opencode_password: String,
    tls_insecure: bool,
    releases_url: String,
    always_on_top: bool,
    font: String,
    interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gateway_url: "http://192.168.101.91:8930/status".into(),
            gateway_token: String::new(),
            opencode_url: "https://192.168.101.91:10240/api/experimental/session/stats?tools=none&timezone=Asia/Shanghai".into(),
            opencode_user: "opencode".into(),
            opencode_password: String::new(),
            tls_insecure: false,
            releases_url: "https://api.github.com/repos/anomalyco/opencode/releases?per_page=40".into(),
            always_on_top: true,
            font: "Consolas".into(),
            interval: Duration::from_secs(30),
        }
    }
}

impl Config {
    fn load() -> Self {
        let mut config = Config::default();
        let candidates = [
            PathBuf::from("status-panel.conf"),
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("status-panel.conf")))
                .unwrap_or_default(),
        ];
        let Some(path) = candidates.iter().find(|p| p.is_file()) else {
            return config;
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return config;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim().to_string();
            match key.trim() {
                "gateway_url" => config.gateway_url = value,
                "gateway_token" => config.gateway_token = value,
                "opencode_url" => config.opencode_url = value,
                "opencode_user" => config.opencode_user = value,
                "opencode_password" => config.opencode_password = value,
                "tls_insecure" => config.tls_insecure = matches!(value.as_str(), "true" | "1" | "yes"),
                "releases_url" => config.releases_url = value,
                "always_on_top" => config.always_on_top = matches!(value.as_str(), "true" | "1" | "yes"),
                "font" => config.font = value,
                "interval_secs" => {
                    if let Ok(secs) = value.parse::<u64>() {
                        config.interval = Duration::from_secs(secs.max(5));
                    }
                }
                _ => {}
            }
        }
        config
    }
}

#[derive(Deserialize, Default, Clone)]
struct GatewayStatus {
    servers: Vec<McpServer>,
}

#[derive(Deserialize, Default, Clone)]
struct McpServer {
    name: String,
    transport: String,
    state: String,
    tools: i64,
    latency_ms: Option<f64>,
    last_error: Option<String>,
    retry_in_s: Option<i64>,
}

#[derive(Deserialize, Default, Clone)]
struct StatsEnvelope {
    data: Stats,
}

#[derive(Deserialize, Default, Clone)]
struct Stats {
    sessions: i64,
    subagents: i64,
    prompts: i64,
    steps: i64,
    cost: f64,
    tokens: Tokens,
}

#[derive(Deserialize, Default, Clone)]
struct Tokens {
    input: i64,
    output: i64,
    reasoning: i64,
    cache: Cache,
}

#[derive(Deserialize, Default, Clone)]
struct Cache {
    read: i64,
    write: i64,
}

#[derive(Deserialize, Default, Clone)]
struct Release {
    tag_name: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

struct Snapshot {
    gateway: Result<Vec<McpServer>, String>,
    stats: Result<Stats, String>,
    latest_v2: Result<Option<Release>, String>,
    at: Instant,
}

fn main() -> eframe::Result {
    let config = Config::load();
    if std::env::args().any(|arg| arg == "--check") {
        return check(config);
    }
    let (tx, rx) = mpsc::channel();
    let refresh = Arc::new(AtomicBool::new(false));
    let worker_refresh = refresh.clone();
    let always_on_top = config.always_on_top;
    let font = config.font.clone();
    thread::spawn(move || worker(config, tx, worker_refresh));

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([560.0, 430.0])
        .with_min_inner_size([360.0, 240.0]);
    if always_on_top {
        viewport = viewport.with_always_on_top();
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "Status Panel",
        options,
        Box::new(move |cc| {
            install_font(&cc.egui_ctx, &font);
            orange_theme(&cc.egui_ctx);
            Ok(Box::new(App { rx, refresh, snapshot: None }))
        }),
    )
}

fn http_client(config: &Config) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(config.tls_insecure)
        .build()
        .expect("http client")
}

const BG: egui::Color32 = egui::Color32::from_rgb(251, 247, 242);
const CARD: egui::Color32 = egui::Color32::WHITE;
const TEXT: egui::Color32 = egui::Color32::from_rgb(31, 35, 40);
const MUTED: egui::Color32 = egui::Color32::from_rgb(107, 114, 128);
const BORDER: egui::Color32 = egui::Color32::from_rgb(234, 223, 210);
const STRIPE: egui::Color32 = egui::Color32::from_rgb(246, 239, 230);
const ORANGE: egui::Color32 = egui::Color32::from_rgb(234, 88, 12);
const ORANGE_DEEP: egui::Color32 = egui::Color32::from_rgb(194, 65, 12);
const OK: egui::Color32 = egui::Color32::from_rgb(21, 128, 61);
const WARN: egui::Color32 = egui::Color32::from_rgb(180, 83, 9);
const ERR: egui::Color32 = egui::Color32::from_rgb(220, 38, 38);

fn install_font(ctx: &egui::Context, name: &str) {
    let Some(path) = font_file(name) else {
        return;
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("custom".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().insert(0, "custom".to_owned());
    }
    ctx.set_fonts(fonts);
}

fn font_file(name: &str) -> Option<PathBuf> {
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

fn orange_theme(ctx: &egui::Context) {
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

fn check(config: Config) -> eframe::Result {
    let client = http_client(&config);
    match fetch_gateway(&client, &config) {
        Ok(status) => {
            println!("gateway: {} servers", status.servers.len());
            for server in status.servers {
                println!(
                    "  {:<14} {:<6} {:<8} tools={:<3} latency={:?}",
                    server.name, server.transport, server.state, server.tools, server.latency_ms
                );
            }
        }
        Err(error) => eprintln!("gateway error: {error}"),
    }
    match fetch_stats(&client, &config) {
        Ok(envelope) => {
            let stats = envelope.data;
            println!(
                "tokens: in={} out={} reasoning={} cache_read={} cost={:.2}",
                stats.tokens.input, stats.tokens.output, stats.tokens.reasoning, stats.tokens.cache.read, stats.cost
            );
        }
        Err(error) => eprintln!("stats error: {error}"),
    }
    match fetch_latest(&client, &config) {
        Ok(Some(release)) => println!(
            "latest v2: {} ({})",
            release.tag_name,
            release.published_at.as_deref().unwrap_or("?")
        ),
        Ok(None) => println!("latest v2: none"),
        Err(error) => eprintln!("releases error: {error}"),
    }
    Ok(())
}

fn worker(config: Config, tx: mpsc::Sender<Snapshot>, refresh: Arc<AtomicBool>) {
    let client = http_client(&config);
    let mut last_run: Option<Instant> = None;
    let mut latest_v2: Result<Option<Release>, String> = Ok(None);
    let mut last_version_check: Option<Instant> = None;
    loop {
        if last_version_check.is_none_or(|t| t.elapsed() >= Duration::from_secs(3600)) {
            latest_v2 = fetch_latest(&client, &config);
            last_version_check = Some(Instant::now());
        }
        let due = refresh.swap(false, Ordering::SeqCst)
            || last_run.is_none_or(|t| t.elapsed() >= config.interval);
        if due {
            let gateway = fetch_gateway(&client, &config).map(|r| r.servers);
            let stats = fetch_stats(&client, &config).map(|r| r.data);
            let _ = tx.send(Snapshot {
                gateway,
                stats,
                latest_v2: latest_v2.clone(),
                at: Instant::now(),
            });
            last_run = Some(Instant::now());
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn fetch_gateway(client: &reqwest::blocking::Client, config: &Config) -> Result<GatewayStatus, String> {
    if config.gateway_token.is_empty() {
        return Err("gateway_token is not set in status-panel.conf".into());
    }
    client
        .get(&config.gateway_url)
        .bearer_auth(&config.gateway_token)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("gateway: {e}"))?
        .json()
        .map_err(|e| format!("gateway JSON: {e}"))
}

fn fetch_stats(client: &reqwest::blocking::Client, config: &Config) -> Result<StatsEnvelope, String> {
    if config.opencode_password.is_empty() {
        return Err("opencode_password is not set in status-panel.conf".into());
    }
    client
        .get(&config.opencode_url)
        .basic_auth(&config.opencode_user, Some(&config.opencode_password))
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("opencode: {e}"))?
        .json()
        .map_err(|e| format!("opencode JSON: {e}"))
}

fn fetch_latest(client: &reqwest::blocking::Client, config: &Config) -> Result<Option<Release>, String> {
    let releases: Vec<Release> = client
        .get(&config.releases_url)
        .header(reqwest::header::USER_AGENT, "status-panel")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("releases: {e}"))?
        .json()
        .map_err(|e| format!("releases JSON: {e}"))?;
    releases
        .into_iter()
        .find(|release| !release.draft && !release.prerelease && release.tag_name.starts_with("v2."))
        .map(Some)
        .ok_or_else(|| "no stable v2 release found".to_string())
}

struct App {
    rx: Receiver<Snapshot>,
    refresh: Arc<AtomicBool>,
    snapshot: Option<Snapshot>,
}

impl eframe::App for App {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        while let Ok(next) = self.rx.try_recv() {
            self.snapshot = Some(next);
        }
        ui.ctx().request_repaint_after(Duration::from_secs(1));

        ui.add_space(10.0);
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

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(CARD)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(14))
        .show(ui, add);
}

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(egui::RichText::new(title).size(16.0).strong().color(TEXT));
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(8.0);
}

fn mcp_table(ui: &mut egui::Ui, servers: &[McpServer]) {
    egui::Grid::new("mcp")
        .striped(true)
        .num_columns(6)
        .spacing([18.0, 8.0])
        .show(ui, |ui| {
            for title in ["SERVER", "TRANSPORT", "STATE", "TOOLS", "LATENCY", "LAST ERROR"] {
                ui.label(egui::RichText::new(title).small().strong().color(MUTED));
            }
            ui.end_row();

            for server in servers {
                ui.label(egui::RichText::new(&server.name).strong().color(TEXT));
                ui.label(egui::RichText::new(&server.transport).color(MUTED));
                let suffix = server
                    .retry_in_s
                    .filter(|s| *s > 0)
                    .map(|s| format!(" (retry {s}s)"))
                    .unwrap_or_default();
                ui.label(
                    egui::RichText::new(format!("{}{}", server.state, suffix))
                        .color(state_color(&server.state))
                        .strong(),
                );
                ui.label(
                    egui::RichText::new(if server.tools > 0 { server.tools.to_string() } else { "-".into() })
                        .monospace()
                        .color(TEXT),
                );
                ui.label(
                    egui::RichText::new(
                        server
                            .latency_ms
                            .map(|ms| format!("{ms:.0} ms"))
                            .unwrap_or_else(|| "-".into()),
                    )
                    .monospace()
                    .color(TEXT),
                );
                match &server.last_error {
                    Some(error) => {
                        ui.label(egui::RichText::new(short(error, 48)).color(ERR))
                            .on_hover_text(error);
                    }
                    None => {
                        ui.label(egui::RichText::new("-").color(MUTED));
                    }
                }
                ui.end_row();
            }
        });
}

fn stats_card(ui: &mut egui::Ui, stats: &Stats) {
    let cache_write = stats.tokens.cache.write;
    let total = stats.tokens.input
        + stats.tokens.output
        + stats.tokens.reasoning
        + stats.tokens.cache.read
        + cache_write;
    let share = |value: i64| {
        if total > 0 {
            value as f64 / total as f64 * 100.0
        } else {
            0.0
        }
    };

    egui::Grid::new("stats")
        .num_columns(3)
        .spacing([28.0, 6.0])
        .show(ui, |ui| {
            stat_row(ui, "Total", &thousands(total), Some(100.0));
            stat_row(ui, "Input", &thousands(stats.tokens.input), Some(share(stats.tokens.input)));
            stat_row(ui, "Output", &thousands(stats.tokens.output), Some(share(stats.tokens.output)));
            stat_row(
                ui,
                "Reasoning",
                &thousands(stats.tokens.reasoning),
                Some(share(stats.tokens.reasoning)),
            );
            stat_row(
                ui,
                "Cache read",
                &thousands(stats.tokens.cache.read),
                Some(share(stats.tokens.cache.read)),
            );
            if cache_write > 0 {
                stat_row(ui, "Cache write", &thousands(cache_write), Some(share(cache_write)));
            }
            stat_row(ui, "Cost", &format!("${:.2}", stats.cost), None);
        });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(format!(
            "{} prompts  ·  {} steps  ·  {} sessions  ·  {} subagents",
            thousands(stats.prompts),
            thousands(stats.steps),
            thousands(stats.sessions),
            thousands(stats.subagents)
        ))
        .color(MUTED)
        .small(),
    );
}

fn stat_row(ui: &mut egui::Ui, label: &str, value: &str, percent: Option<f64>) {
    ui.label(egui::RichText::new(label).strong().color(TEXT));
    ui.label(egui::RichText::new(value).monospace().color(TEXT));
    match percent {
        Some(percent) => {
            ui.label(egui::RichText::new(format!("{percent:>5.1}%")).monospace().color(MUTED));
        }
        None => {
            ui.label(egui::RichText::new("     -").monospace().color(MUTED));
        }
    }
    ui.end_row();
}

fn state_color(state: &str) -> egui::Color32 {
    match state {
        "idle" => OK,
        "unknown" | "" => WARN,
        "error" | "failed" => ERR,
        _ => MUTED,
    }
}

fn short(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut cut: String = text.chars().take(max).collect();
        cut.push('…');
        cut
    }
}

fn thousands(value: i64) -> String {
    let digits = value.abs().to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if value < 0 { format!("-{out}") } else { out }
}
