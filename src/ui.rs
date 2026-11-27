use eframe::egui;

use crate::model::{McpServer, Stats, Task, TasksPage};
use crate::theme::{BORDER, CARD, ERR, MUTED, OK, ORANGE, ORANGE_DEEP, STRIPE, TEXT, WARN};

const AMBER: egui::Color32 = egui::Color32::from_rgb(217, 119, 6);
const ORANGE_800: egui::Color32 = egui::Color32::from_rgb(146, 64, 14);
const CACHE_BAR: egui::Color32 = egui::Color32::from_rgb(226, 214, 199);

pub fn block_title(id: &str) -> &str {
    match id {
        "mcp" => "MCP servers",
        "usage" => "Token usage",
        "tasks" => "Ongoing tasks",
        other => other,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CardAction {
    None,
    Minimize,
}

pub fn minimize_button() -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new("−").strong().color(MUTED)).frame(false)
}

pub fn chip(ui: &mut egui::Ui, title: &str) -> (bool, f32) {
    let mut clicked = false;
    let inner = egui::Frame::new()
        .fill(CARD)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let button =
                    egui::Button::new(egui::RichText::new(format!("+  {title}")).color(MUTED)).frame(false);
                clicked = ui.add(button).on_hover_text("Restore").clicked();
            });
        });
    (clicked, inner.response.rect.width())
}

pub fn mcp_table(ui: &mut egui::Ui, servers: &[McpServer]) {
    egui::ScrollArea::horizontal()
        .id_salt("mcp-table")
        .show(ui, |ui| {
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
                            egui::RichText::new(if server.tools > 0 {
                                server.tools.to_string()
                            } else {
                                "-".into()
                            })
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
        });
}

pub fn stats_card(ui: &mut egui::Ui, stats: &Stats) {
    let total = stats.tokens.input
        + stats.tokens.output
        + stats.tokens.reasoning
        + stats.tokens.cache.read
        + stats.tokens.cache.write;
    let parts = [
        Part { value: stats.tokens.input, bar: ORANGE_DEEP },
        Part { value: stats.tokens.output, bar: ORANGE },
        Part { value: stats.tokens.reasoning, bar: AMBER },
        Part { value: stats.tokens.cache.read, bar: CACHE_BAR },
        Part { value: stats.tokens.cache.write, bar: ORANGE_800 },
    ];

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Total").strong().color(TEXT));
        ui.label(egui::RichText::new(thousands(total)).monospace().strong().color(TEXT));
    });

    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for (position, (letter, part)) in ["I", "O", "R", "CR", "CW"].into_iter().zip(&parts).enumerate() {
            if position > 0 {
                ui.add_space(7.0);
            }
            // A glyph, not a painted rect: same font as the label, so it sits on the
            // text baseline exactly instead of being centered approximately.
            ui.label(egui::RichText::new("■").monospace().color(part.bar));
            ui.label(
                egui::RichText::new(format!("{letter} ({:.1}%)", share(part.value, total)))
                    .monospace()
                    .color(MUTED),
            );
        }
    });

    ui.add_space(6.0);
    let (bar, _) = ui.allocate_exact_size(egui::vec2(0.0, 8.0), egui::Sense::hover());

    ui.add_space(10.0);
    ui.label(
        egui::RichText::new(format!(
            "{} prompts  ·  {} steps  ·  {} sessions  ·  {} subagents  ·  ${:.2}",
            thousands(stats.prompts),
            thousands(stats.steps),
            thousands(stats.sessions),
            thousands(stats.subagents),
            stats.cost
        ))
        .color(MUTED)
        .small(),
    );

    // Painted after the last line so the bar matches the card's measured content width.
    usage_bar(ui, bar, ui.min_rect().width(), &parts, total);
}

/// One token category: `bar` paints both its segment and its legend swatch.
struct Part {
    value: i64,
    bar: egui::Color32,
}

fn usage_bar(ui: &egui::Ui, reserved: egui::Rect, width: f32, parts: &[Part], total: i64) {
    let rect = egui::Rect::from_min_size(reserved.min, egui::vec2(width, reserved.height()));
    let painter = ui.painter();
    painter.rect_filled(rect, 4, STRIPE);
    if total <= 0 {
        return;
    }

    let values: Vec<i64> = parts.iter().map(|part| part.value).collect();
    let edges = bar_edges(&values, total, width);
    let round = |on: bool| if on { 4 } else { 0 };
    let visible: Vec<usize> = (0..parts.len()).filter(|&index| parts[index].value > 0).collect();
    for (position, &index) in visible.iter().enumerate() {
        let last = position + 1 == visible.len();
        let radius = egui::CornerRadius {
            nw: round(position == 0),
            sw: round(position == 0),
            ne: round(last),
            se: round(last),
        };
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(rect.left() + edges[index], rect.top()),
                egui::pos2(rect.left() + edges[index + 1], rect.bottom()),
            ),
            radius,
            parts[index].bar,
        );
    }
}

/// Pixel offsets of every part boundary; the last part always ends exactly at `width`.
fn bar_edges(values: &[i64], total: i64, width: f32) -> Vec<f32> {
    let mut edges = Vec::with_capacity(values.len() + 1);
    let mut filled = 0i64;
    edges.push(0.0);
    for value in values {
        filled += value;
        edges.push(if filled >= total {
            width
        } else {
            filled as f32 / total as f32 * width
        });
    }
    edges
}

pub fn tasks_table(ui: &mut egui::Ui, page: &TasksPage) {
    if page.tasks.is_empty() {
        ui.label(egui::RichText::new("no ongoing tasks").color(MUTED));
        return;
    }

    let mut tasks: Vec<&Task> = page.tasks.iter().collect();
    tasks.sort_by(|a, b| {
        let rank = |task: &Task| u8::from(task.status != "in_progress");
        rank(a).cmp(&rank(b)).then_with(|| b.updated.cmp(&a.updated))
    });

    // As wide as the longest title, shrinking on narrow windows so UPDATED stays visible.
    let font = egui::TextStyle::Body.resolve(ui.style());
    let longest = tasks.iter().fold(0.0f32, |width, task| {
        let natural = ui
            .painter()
            .layout_no_wrap(task.title.clone(), font.clone(), egui::Color32::PLACEHOLDER)
            .size()
            .x;
        width.max(natural)
    });
    let title_width = longest.min(ui.available_width() - 330.0).max(160.0);

    egui::ScrollArea::horizontal()
        .id_salt("tasks-table")
        .show(ui, |ui| {
            egui::Grid::new("tasks")
                .striped(true)
                .num_columns(3)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    for title in ["PROJECT", "TASK", "UPDATED"] {
                        ui.label(egui::RichText::new(title).small().strong().color(MUTED));
                    }
                    ui.end_row();

                    for task in tasks {
                        ui.label(egui::RichText::new(short(&task.project, 18)).color(MUTED));
                        ui.scope(|ui| {
                            ui.set_max_width(title_width);
                            ui.add(
                                egui::Label::new(egui::RichText::new(&task.title).strong().color(TEXT))
                                    .wrap(),
                            )
                            .on_hover_text(task_hover(task));
                        });
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&task.updated).monospace().color(TEXT).small(),
                            );
                            ui.label(
                                egui::RichText::new(format!("[{}]", task.status.replace('_', " ")))
                                    .color(task_status_color(&task.status))
                                    .small(),
                            );
                        });
                        ui.end_row();
                    }
                });
        });

    if page.has_more {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(format!("showing {} of {}", page.tasks.len(), page.total))
                .color(MUTED)
                .small(),
        );
    }
}

fn task_hover(task: &Task) -> String {
    let mut text = task.title.clone();
    for extra in [&task.description, &task.latest_update] {
        let extra = extra.trim();
        if !extra.is_empty() {
            text.push_str("\n\n");
            text.push_str(extra);
        }
    }
    text
}

fn task_status_color(status: &str) -> egui::Color32 {
    match status {
        "in_progress" => ORANGE,
        "blocked" => ERR,
        _ => MUTED,
    }
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
    fn bar_edges_end_at_full_width() {
        assert_eq!(bar_edges(&[50, 50], 100, 200.0), vec![0.0, 100.0, 200.0]);
        assert_eq!(bar_edges(&[0, 100], 100, 200.0), vec![0.0, 0.0, 200.0]);
        assert_eq!(bar_edges(&[1, 2], 3, 300.0), vec![0.0, 100.0, 300.0]);
        assert_eq!(bar_edges(&[5], 5, 123.5), vec![0.0, 123.5]);
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
