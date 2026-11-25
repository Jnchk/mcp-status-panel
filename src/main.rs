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

struct Snapshot {
    gateway: Result<Vec<McpServer>, String>,
    stats: Result<Stats, String>,
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
    thread::spawn(move || worker(config, tx, worker_refresh));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([820.0, 620.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Status Panel",
        options,
        Box::new(|_cc| Ok(Box::new(App { rx, refresh, snapshot: None }))),
    )
}

fn http_client(config: &Config) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(config.tls_insecure)
        .build()
        .expect("http client")
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
    Ok(())
}

fn worker(config: Config, tx: mpsc::Sender<Snapshot>, refresh: Arc<AtomicBool>) {
    let client = http_client(&config);
    let mut last_run: Option<Instant> = None;
    loop {
        let due = refresh.swap(false, Ordering::SeqCst)
            || last_run.is_none_or(|t| t.elapsed() >= config.interval);
        if due {
            let gateway = fetch_gateway(&client, &config).map(|r| r.servers);
            let stats = fetch_stats(&client, &config).map(|r| r.data);
            let _ = tx.send(Snapshot { gateway, stats, at: Instant::now() });
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

struct App {
    rx: Receiver<Snapshot>,
    refresh: Arc<AtomicBool>,
    snapshot: Option<Snapshot>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        while let Ok(next) = self.rx.try_recv() {
            self.snapshot = Some(next);
        }
        ui.ctx().request_repaint_after(Duration::from_secs(1));

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading("Status Panel");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Refresh").clicked() {
                    self.refresh.store(true, Ordering::SeqCst);
                }
                if let Some(snapshot) = &self.snapshot {
                    let age = snapshot.at.elapsed().as_secs();
                    ui.weak(format!("updated {age}s ago"));
                } else {
                    ui.weak("loading...");
                }
            });
        });
        ui.separator();

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.heading("MCP servers");
                ui.add_space(2.0);
                match self.snapshot.as_ref().map(|s| &s.gateway) {
                    None => {
                        ui.weak("loading...");
                    }
                    Some(Err(error)) => {
                        ui.colored_label(egui::Color32::from_rgb(220, 80, 80), error);
                    }
                    Some(Ok(servers)) => mcp_table(ui, servers),
                }

                ui.add_space(20.0);
                ui.heading("Token usage");
                ui.add_space(2.0);
                match self.snapshot.as_ref().map(|s| &s.stats) {
                    None => {
                        ui.weak("loading...");
                    }
                    Some(Err(error)) => {
                        ui.colored_label(egui::Color32::from_rgb(220, 80, 80), error);
                    }
                    Some(Ok(stats)) => stats_card(ui, stats),
                }
                ui.add_space(8.0);
            });
    }
}

fn mcp_table(ui: &mut egui::Ui, servers: &[McpServer]) {
    egui::Grid::new("mcp")
        .striped(true)
        .num_columns(6)
        .spacing([18.0, 6.0])
        .show(ui, |ui| {
            for title in ["server", "transport", "state", "tools", "latency", "last error"] {
                ui.strong(title);
            }
            ui.end_row();

            for server in servers {
                ui.label(&server.name);
                ui.label(&server.transport);
                let color = match server.state.as_str() {
                    "idle" => egui::Color32::from_rgb(90, 170, 110),
                    "unknown" | "" => egui::Color32::GRAY,
                    "error" | "failed" => egui::Color32::from_rgb(220, 80, 80),
                    _ => egui::Color32::GRAY,
                };
                let suffix = server
                    .retry_in_s
                    .filter(|s| *s > 0)
                    .map(|s| format!(" (retry {s}s)"))
                    .unwrap_or_default();
                ui.colored_label(color, format!("{}{}", server.state, suffix));
                ui.label(if server.tools > 0 { server.tools.to_string() } else { "-".into() });
                ui.label(
                    server
                        .latency_ms
                        .map(|ms| format!("{ms:.0} ms"))
                        .unwrap_or_else(|| "-".into()),
                );
                match &server.last_error {
                    Some(error) => {
                        ui.colored_label(egui::Color32::from_rgb(220, 80, 80), error);
                    }
                    None => {
                        ui.weak("-");
                    }
                }
                ui.end_row();
            }
        });
}

fn stats_card(ui: &mut egui::Ui, stats: &Stats) {
    let total = stats.tokens.input
        + stats.tokens.output
        + stats.tokens.reasoning
        + stats.tokens.cache.read
        + stats.tokens.cache.write;

    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Total: {}", thousands(total)));
            ui.separator();
            ui.label(format!("Input: {}", thousands(stats.tokens.input)));
            ui.separator();
            ui.label(format!("Output: {}", thousands(stats.tokens.output)));
            ui.separator();
            ui.label(format!("Reasoning: {}", thousands(stats.tokens.reasoning)));
            ui.separator();
            ui.label(format!("Cache read: {}", thousands(stats.tokens.cache.read)));
            if stats.tokens.cache.write > 0 {
                ui.separator();
                ui.label(format!("Cache write: {}", thousands(stats.tokens.cache.write)));
            }
            ui.separator();
            ui.label(format!("Cost: ${:.2}", stats.cost));
        });
        ui.add_space(6.0);
        ui.weak(format!(
            "{} prompts · {} steps · {} sessions · {} subagents",
            thousands(stats.prompts),
            thousands(stats.steps),
            thousands(stats.sessions),
            thousands(stats.subagents)
        ));
    });
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
