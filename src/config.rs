use std::{path::{Path, PathBuf}, time::Duration};

#[derive(Clone)]
pub struct BlockConfig {
    pub id: String,
    pub visible: bool,
    pub minimized: bool,
}

impl BlockConfig {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            visible: true,
            minimized: false,
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub gateway_url: String,
    pub gateway_token: String,
    pub opencode_url: String,
    pub opencode_user: String,
    pub opencode_password: String,
    pub tls_insecure: bool,
    pub releases_url: String,
    pub always_on_top: bool,
    pub font: String,
    pub tray: bool,
    pub taskbar: bool,
    pub zoom: f32,
    pub blocks: Vec<BlockConfig>,
    pub interval: Duration,
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
            tray: true,
            taskbar: false,
            zoom: 1.0,
            blocks: vec![BlockConfig::new("mcp"), BlockConfig::new("usage")],
            interval: Duration::from_secs(30),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config = Config::default();
        let Some(path) = Self::path() else {
            return config;
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return config;
        };
        Config::from_text(&text)
    }

    pub fn path() -> Option<PathBuf> {
        [
            PathBuf::from("status-panel.conf"),
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("status-panel.conf")))
                .unwrap_or_default(),
        ]
        .into_iter()
        .find(|p| p.is_file())
    }

    pub fn save_zoom(zoom: f32) {
        if let Some(path) = Self::path() {
            let _ = save_zoom_at(&path, zoom);
        }
    }

    pub fn from_text(text: &str) -> Self {
        let mut config = Config::default();
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
                "tray" => config.tray = matches!(value.as_str(), "true" | "1" | "yes"),
                "taskbar" => config.taskbar = matches!(value.as_str(), "true" | "1" | "yes"),
                "zoom" => {
                    if let Ok(zoom) = value.parse::<f32>() {
                        config.zoom = zoom.clamp(0.7, 2.0);
                    }
                }
                "interval_secs" => {
                    if let Ok(secs) = value.parse::<u64>() {
                        config.interval = Duration::from_secs(secs.max(5));
                    }
                }
                other if other.starts_with("block.") => parse_block(&mut config, other, &value),
                _ => {}
            }
        }
        config
    }
}

fn parse_block(config: &mut Config, key: &str, value: &str) {
    let mut parts = key.split('.');
    if parts.next() != Some("block") {
        return;
    }
    let (Some(id), Some(field)) = (parts.next(), parts.next()) else {
        return;
    };
    let index = match config.blocks.iter().position(|block| block.id == id) {
        Some(index) => index,
        None => {
            config.blocks.push(BlockConfig::new(id));
            config.blocks.len() - 1
        }
    };
    let flag = matches!(value, "true" | "1" | "yes");
    match field {
        "visible" => config.blocks[index].visible = flag,
        "minimized" => config.blocks[index].minimized = flag,
        _ => {}
    }
}

fn save_zoom_at(path: &Path, zoom: f32) -> std::io::Result<()> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let entry = format!("zoom={zoom:.2}");
    let mut replaced = false;
    for line in &mut lines {
        if line.trim_start().starts_with("zoom=") {
            *line = entry.clone();
            replaced = true;
        }
    }
    if !replaced {
        lines.push(entry);
    }
    let mut output = lines.join("\n");
    output.push('\n');
    std::fs::write(path, output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_apply_without_file() {
        let config = Config::from_text("");
        assert_eq!(config.font, "Consolas");
        assert!(config.tray);
        assert!(!config.taskbar);
        assert!(config.always_on_top);
        assert_eq!(config.interval, Duration::from_secs(30));
    }

    #[test]
    fn parses_values_and_comments() {
        let config = Config::from_text(
            "# comment\n\
             gateway_url = http://host:8930/status\n\
             tls_insecure=yes\n\
             tray=false\n\
             taskbar=1\n\
             font=Consolas\n\
             interval_secs=45\n",
        );
        assert_eq!(config.gateway_url, "http://host:8930/status");
        assert!(config.tls_insecure);
        assert!(!config.tray);
        assert!(config.taskbar);
        assert_eq!(config.font, "Consolas");
        assert_eq!(config.interval, Duration::from_secs(45));
    }

    #[test]
    fn interval_has_a_floor_and_junk_is_ignored() {
        let config = Config::from_text("interval_secs=1\nnoise\nunknown=value\ninterval_secs=\n");
        assert_eq!(config.interval, Duration::from_secs(5));
    }

    #[test]
    fn zoom_is_parsed_and_clamped() {
        assert_eq!(Config::from_text("zoom=1.3").zoom, 1.3);
        assert_eq!(Config::from_text("zoom=0.2").zoom, 0.7);
        assert_eq!(Config::from_text("zoom=9").zoom, 2.0);
        assert_eq!(Config::from_text("zoom=abc").zoom, 1.0);
    }

    #[test]
    fn blocks_can_be_hidden_and_minimized() {
        let config = Config::from_text("block.mcp.minimized=true\nblock.usage.visible=no\n");
        let mcp = config.blocks.iter().find(|block| block.id == "mcp").unwrap();
        let usage = config.blocks.iter().find(|block| block.id == "usage").unwrap();
        assert!(mcp.visible);
        assert!(mcp.minimized);
        assert!(!usage.visible);
        assert!(!usage.minimized);
    }

    #[test]
    fn unknown_blocks_are_appended() {
        let config = Config::from_text("block.extra.visible=false\n");
        let extra = config.blocks.last().unwrap();
        assert_eq!(extra.id, "extra");
        assert!(!extra.visible);
        assert!(!extra.minimized);
    }

    #[test]
    fn save_zoom_rewrites_or_appends() {
        let path = std::env::temp_dir().join(format!("status-panel-zoom-{}.conf", std::process::id()));
        std::fs::write(&path, "font=Consolas\nzoom=1.00\n").unwrap();
        save_zoom_at(&path, 1.25).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("zoom=1.25"));
        assert!(text.contains("font=Consolas"));

        std::fs::write(&path, "font=Consolas\n").unwrap();
        save_zoom_at(&path, 0.9).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("zoom=0.90"));

        let _ = std::fs::remove_file(&path);
    }
}
