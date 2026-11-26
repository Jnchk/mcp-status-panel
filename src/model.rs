use std::time::Instant;

use serde::Deserialize;

#[derive(Deserialize, Default, Clone)]
pub struct GatewayStatus {
    pub servers: Vec<McpServer>,
}

#[derive(Deserialize, Default, Clone)]
pub struct McpServer {
    pub name: String,
    pub transport: String,
    pub state: String,
    pub tools: i64,
    pub latency_ms: Option<f64>,
    pub last_error: Option<String>,
    pub retry_in_s: Option<i64>,
}

#[derive(Deserialize, Default, Clone)]
pub struct StatsEnvelope {
    pub data: Stats,
}

#[derive(Deserialize, Default, Clone)]
pub struct Stats {
    pub sessions: i64,
    pub subagents: i64,
    pub prompts: i64,
    pub steps: i64,
    pub cost: f64,
    pub tokens: Tokens,
}

#[derive(Deserialize, Default, Clone)]
pub struct Tokens {
    pub input: i64,
    pub output: i64,
    pub reasoning: i64,
    pub cache: Cache,
}

#[derive(Deserialize, Default, Clone)]
pub struct Cache {
    pub read: i64,
    pub write: i64,
}

#[derive(Deserialize, Default, Clone)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub draft: bool,
}

pub struct Snapshot {
    pub gateway: Result<Vec<McpServer>, String>,
    pub stats: Result<Stats, String>,
    pub latest_v2: Result<Option<Release>, String>,
    pub at: Instant,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_payload_deserializes() {
        let payload = r#"{
            "generated_at": "2026-11-25T02:56:12Z",
            "servers": [
                {"name":"outline","transport":"http","state":"idle","tools":8,"latency_ms":94,
                 "last_ok":"2026-11-23T05:55:47Z","last_error":null,"retry_in_s":0},
                {"name":"mail","transport":"stdio","state":"unknown","tools":0,"latency_ms":null,
                 "last_ok":null,"last_error":"spawn failed","retry_in_s":30}
            ]
        }"#;
        let status: GatewayStatus = serde_json::from_str(payload).expect("gateway json");
        assert_eq!(status.servers.len(), 2);
        assert_eq!(status.servers[0].name, "outline");
        assert_eq!(status.servers[0].latency_ms, Some(94.0));
        assert_eq!(status.servers[1].state, "unknown");
        assert_eq!(status.servers[1].last_error.as_deref(), Some("spawn failed"));
        assert_eq!(status.servers[1].retry_in_s, Some(30));
    }

    #[test]
    fn stats_payload_deserializes() {
        let payload = r#"{"data":{
            "sessions":11,"subagents":1,"prompts":204,"steps":1325,
            "tokens":{"input":6800000,"output":410000,"reasoning":590000,
                      "cache":{"read":210000000,"write":0}},
            "cost":0.0,"activeDays":7
        }}"#;
        let envelope: StatsEnvelope = serde_json::from_str(payload).expect("stats json");
        let stats = envelope.data;
        assert_eq!(stats.sessions, 11);
        assert_eq!(stats.prompts, 204);
        assert_eq!(stats.tokens.cache.read, 210_000_000);
        assert_eq!(stats.cost, 0.0);
    }
}
