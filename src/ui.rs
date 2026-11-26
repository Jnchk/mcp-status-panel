use eframe::egui;

use crate::model::{McpServer, Stats};
use crate::theme::{ERR, MUTED, OK, TEXT, WARN};

pub fn mcp_table(ui: &mut egui::Ui, servers: &[McpServer]) {
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

pub fn stats_card(ui: &mut egui::Ui, stats: &Stats) {
    let cache_write = stats.tokens.cache.write;
    let total = stats.tokens.input
        + stats.tokens.output
        + stats.tokens.reasoning
        + stats.tokens.cache.read
        + cache_write;
    egui::Grid::new("stats")
        .num_columns(3)
        .spacing([28.0, 6.0])
        .show(ui, |ui| {
            stat_row(ui, "Total", &thousands(total), Some(100.0));
            stat_row(ui, "Input", &thousands(stats.tokens.input), Some(share(stats.tokens.input, total)));
            stat_row(ui, "Output", &thousands(stats.tokens.output), Some(share(stats.tokens.output, total)));
            stat_row(
                ui,
                "Reasoning",
                &thousands(stats.tokens.reasoning),
                Some(share(stats.tokens.reasoning, total)),
            );
            stat_row(
                ui,
                "Cache read",
                &thousands(stats.tokens.cache.read),
                Some(share(stats.tokens.cache.read, total)),
            );
            if cache_write > 0 {
                stat_row(ui, "Cache write", &thousands(cache_write), Some(share(cache_write, total)));
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

fn share(value: i64, total: i64) -> f64 {
    if total > 0 {
        value as f64 / total as f64 * 100.0
    } else {
        0.0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_234), "1,234");
        assert_eq!(thousands(232_590_348), "232,590,348");
        assert_eq!(thousands(-1_234), "-1,234");
    }

    #[test]
    fn short_truncates_on_char_boundaries() {
        assert_eq!(short("hello", 10), "hello");
        assert_eq!(short("hello world", 5), "hello…");
        assert_eq!(short("中文测试", 2), "中文…");
    }

    #[test]
    fn share_is_percentage_of_total() {
        assert_eq!(share(50, 200), 25.0);
        assert_eq!(share(200, 200), 100.0);
        assert_eq!(share(1, 0), 0.0);
    }

    #[test]
    fn state_colors_follow_semantics() {
        assert_eq!(state_color("idle"), OK);
        assert_eq!(state_color("unknown"), WARN);
        assert_eq!(state_color(""), WARN);
        assert_eq!(state_color("failed"), ERR);
        assert_eq!(state_color("error"), ERR);
        assert_eq!(state_color("weird"), MUTED);
    }
}
