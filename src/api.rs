use std::time::Duration;

use crate::config::Config;
use crate::model::{GatewayStatus, Release, StatsEnvelope};

pub fn http_client(config: &Config) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(config.tls_insecure)
        .build()
        .expect("http client")
}

pub fn fetch_gateway(client: &reqwest::blocking::Client, config: &Config) -> Result<GatewayStatus, String> {
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

pub fn fetch_stats(client: &reqwest::blocking::Client, config: &Config) -> Result<StatsEnvelope, String> {
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

pub fn fetch_latest(client: &reqwest::blocking::Client, config: &Config) -> Result<Option<Release>, String> {
    let releases: Vec<Release> = client
        .get(&config.releases_url)
        .header(reqwest::header::USER_AGENT, "status-panel")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("releases: {e}"))?
        .json()
        .map_err(|e| format!("releases JSON: {e}"))?;
    pick_latest_v2(releases).map(Some).ok_or_else(|| "no stable v2 release found".to_string())
}

fn pick_latest_v2(releases: Vec<Release>) -> Option<Release> {
    releases
        .into_iter()
        .find(|release| !release.draft && !release.prerelease && release.tag_name.starts_with("v2."))
}

pub fn check(config: Config) -> eframe::Result {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_v2_skips_prerelease_draft_and_v1() {
        let release = |tag: &str, prerelease: bool, draft: bool| Release {
            tag_name: tag.to_string(),
            published_at: None,
            prerelease,
            draft,
        };
        let picked = pick_latest_v2(vec![
            release("v2.5.0", false, true),
            release("v2.4.2", true, false),
            release("v1.19.6", false, false),
            release("v2.4.1", false, false),
            release("v2.4.0", false, false),
        ])
        .expect("a stable v2 release");
        assert_eq!(picked.tag_name, "v2.4.1");

        assert!(pick_latest_v2(vec![release("v1.19.6", false, false)]).is_none());
    }
}
