use std::time::Duration;

use serde::Deserialize;

use crate::config::Config;
use crate::model::{GatewayStatus, Release, StatsEnvelope, TasksPage};

#[derive(Deserialize)]
struct RpcResponse {
    #[serde(default)]
    result: Option<RpcResult>,
    #[serde(default)]
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcResult {
    #[serde(default)]
    content: Vec<RpcContent>,
}

#[derive(Deserialize)]
struct RpcContent {
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct RpcError {
    #[serde(default)]
    message: String,
}

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

pub fn fetch_tasks(client: &reqwest::blocking::Client, config: &Config) -> Result<TasksPage, String> {
    if config.mcp_token.is_empty() {
        return Err("mcp_token is not set in status-panel.conf".into());
    }
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "call_tool",
            "arguments": {
                "server": "obsitrace",
                "tool": "query_tasks",
                "arguments": {"statuses": ["in_progress", "todo"], "limit": 50}
            }
        }
    });
    let text = client
        .post(config.mcp_url())
        .bearer_auth(&config.mcp_token)
        .json(&body)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("obsitrace: {e}"))?
        .text()
        .map_err(|e| format!("obsitrace body: {e}"))?;
    parse_tasks_response(&text)
}

fn parse_tasks_response(body: &str) -> Result<TasksPage, String> {
    let response: RpcResponse =
        serde_json::from_str(body).map_err(|e| format!("obsitrace JSON: {e}"))?;
    if let Some(error) = response.error {
        return Err(format!("obsitrace: {}", error.message));
    }
    let text = response
        .result
        .and_then(|result| result.content.into_iter().next())
        .map(|content| content.text)
        .ok_or_else(|| "obsitrace: empty result".to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("obsitrace payload: {e}"))
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
    match fetch_tasks(&client, &config) {
        Ok(page) => {
            println!("tasks: {} ongoing", page.total);
            for task in page.tasks {
                println!("  {:<12} {:<16} {}", task.status, task.project, task.title);
            }
            if page.has_more {
                println!("  (more than the 50 returned)");
            }
        }
        Err(error) => eprintln!("tasks error: {error}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tasks_response_parses_nested_content() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"{\"hasMore\":false,\"total\":1,\"tasks\":[{\"id\":\"t1\",\"title\":\"a\",\"project\":\"p\",\"status\":\"todo\",\"updated\":\"2026-11-25\"}]}"}]}}"#;
        let page = parse_tasks_response(body).expect("tasks");
        assert_eq!(page.total, 1);
        assert_eq!(page.tasks[0].title, "a");
    }

    #[test]
    fn tasks_response_surfaces_rpc_errors() {
        let body = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"unknown tool \"query_tasks\""}}"#;
        assert_eq!(
            parse_tasks_response(body).unwrap_err(),
            "obsitrace: unknown tool \"query_tasks\""
        );
        assert_eq!(
            parse_tasks_response(r#"{"jsonrpc":"2.0","id":1,"result":{"content":[]}}"#).unwrap_err(),
            "obsitrace: empty result"
        );
        assert!(parse_tasks_response("nope").unwrap_err().starts_with("obsitrace JSON:"));
    }

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
