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

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TasksPage {
    #[serde(default)]
    pub has_more: bool,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub tasks: Vec<Task>,
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub title: String,
    pub project: String,
    pub status: String,
    pub updated: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub latest_update: String,
}

#[derive(Default)]
pub struct Snapshot {
    pub gateway: Option<Result<Vec<McpServer>, String>>,
    pub stats: Option<Result<Stats, String>>,
    pub tasks: Option<Result<TasksPage, String>>,
    pub latest_v2: Option<Result<Option<Release>, String>>,
    pub at: Option<Instant>,
}

/// One source finishing; fields left `None` keep their previous value.
#[derive(Default)]
pub struct Update {
    pub gateway: Option<Result<Vec<McpServer>, String>>,
    pub stats: Option<Result<Stats, String>>,
    pub tasks: Option<Result<TasksPage, String>>,
    pub latest_v2: Option<Result<Option<Release>, String>>,
}

impl Snapshot {
    pub fn apply(&mut self, update: Update) {
        if let Some(value) = update.gateway {
            self.gateway = Some(value);
        }
        if let Some(value) = update.stats {
            self.stats = Some(value);
        }
        if let Some(value) = update.tasks {
            self.tasks = Some(value);
        }
        if let Some(value) = update.latest_v2 {
            self.latest_v2 = Some(value);
        }
        self.at = Some(Instant::now());
    }
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
    fn tasks_payload_deserializes() {
        let payload = r#"{
            "hasMore": false, "total": 2,
            "tasks": [
                {"created":"2026-11-09","description":"来源：Redmine #14331","detailPath":"Obsitrace/Tasks/x.md",
                 "id":"task-1","latestUpdate":"- 2026-11-11T18:12:03+08:00 [in_progress] 已讨论",
                 "project":"SD-WAN管理平台","status":"in_progress","title":"梳理功能清单","updated":"2026-11-11"},
                {"created":"2026-11-25","id":"task-2","project":"SD-WAN管理平台",
                 "status":"todo","title":"部署与自测","updated":"2026-11-25"}
            ]
        }"#;
        let page: TasksPage = serde_json::from_str(payload).expect("tasks json");
        assert_eq!(page.total, 2);
        assert_eq!(page.tasks.len(), 2);
        assert!(!page.has_more);
        assert_eq!(page.tasks[0].status, "in_progress");
        assert_eq!(page.tasks[0].latest_update, "- 2026-11-11T18:12:03+08:00 [in_progress] 已讨论");
        assert_eq!(page.tasks[1].description, "");
        assert_eq!(page.tasks[1].latest_update, "");
    }

    #[test]
    fn updates_merge_without_dropping_other_sources() {
        let mut snapshot = Snapshot::default();
        assert!(snapshot.at.is_none());
        snapshot.apply(Update {
            stats: Some(Ok(Stats::default())),
            ..Default::default()
        });
        assert!(snapshot.stats.is_some());
        assert!(snapshot.gateway.is_none());
        assert!(snapshot.at.is_some());

        snapshot.apply(Update {
            gateway: Some(Err("down".into())),
            ..Default::default()
        });
        assert!(snapshot.stats.is_some(), "a gateway update must keep the stats");
        assert!(matches!(snapshot.gateway, Some(Err(_))));
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
