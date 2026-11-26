use std::{path::PathBuf, time::Duration};

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
            interval: Duration::from_secs(30),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config = Config::default();
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
        Config::from_text(&text)
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
}
