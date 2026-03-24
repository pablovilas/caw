use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionStatus {
    Working,
    WaitingInput,
    Idle,
    Dead,
}

impl SessionStatus {
    pub fn symbol(&self) -> &str {
        match self {
            Self::Working => "●",
            Self::WaitingInput => "▲",
            Self::Idle => "◉",
            Self::Dead => "✕",
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Working => "working",
            Self::WaitingInput => "waiting",
            Self::Idle => "idle",
            Self::Dead => "dead",
        }
    }

    pub fn color_hex(&self) -> &str {
        match self {
            Self::Working => "#1D9E75",
            Self::WaitingInput => "#EF9F27",
            Self::Idle => "#888780",
            Self::Dead => "#E24B4A",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl TokenUsage {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }

    pub fn estimated_cost_usd(&self, model: &str) -> f64 {
        // Rough per-token pricing (USD per 1M tokens)
        let (input_price, output_price, cache_read_price, cache_write_price) = match model {
            m if m.contains("opus") => (15.0, 75.0, 1.5, 18.75),
            m if m.contains("sonnet") => (3.0, 15.0, 0.3, 3.75),
            m if m.contains("haiku") => (0.8, 4.0, 0.08, 1.0),
            m if m.contains("gpt-4o") => (2.5, 10.0, 1.25, 2.5),
            m if m.contains("gpt-4") => (10.0, 30.0, 5.0, 10.0),
            m if m.contains("o1") || m.contains("o3") || m.contains("o4") => (10.0, 40.0, 2.5, 10.0),
            _ => (3.0, 15.0, 0.3, 3.75), // default to sonnet-ish pricing
        };

        let per_m = 1_000_000.0;
        (self.input as f64 * input_price / per_m)
            + (self.output as f64 * output_price / per_m)
            + (self.cache_read as f64 * cache_read_price / per_m)
            + (self.cache_write as f64 * cache_write_price / per_m)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawInstance {
    pub id: String,
    pub pid: Option<u32>,
    pub working_dir: PathBuf,
    pub started_at: DateTime<Utc>,
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSession {
    pub instance_id: String,
    pub status: SessionStatus,
    pub last_message: Option<String>,
    pub git_branch: Option<String>,
    pub token_usage: Option<TokenUsage>,
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedSession {
    pub id: String,
    pub plugin: String,
    pub display_name: String,
    pub project_path: PathBuf,
    pub project_name: String,
    pub status: SessionStatus,
    pub last_message: Option<String>,
    pub git_branch: Option<String>,
    pub model: Option<String>,
    pub token_usage: TokenUsage,
    pub started_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub pid: Option<u32>,
}

impl NormalizedSession {
    pub fn from_raw(
        instance: &RawInstance,
        session: &RawSession,
        plugin: &str,
        display_name: &str,
    ) -> Self {
        let project_name = instance
            .working_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| instance.working_dir.to_string_lossy().to_string());

        // If the instance has no running process, it's dead
        let status = if instance.pid.is_none() {
            SessionStatus::Dead
        } else {
            session.status.clone()
        };

        Self {
            id: instance.id.clone(),
            plugin: plugin.to_string(),
            display_name: display_name.to_string(),
            project_path: instance.working_dir.clone(),
            project_name,
            status,
            last_message: session.last_message.clone(),
            git_branch: session.git_branch.clone()
                .or_else(|| instance.extra.get("git_branch").and_then(|v| v.as_str()).map(String::from)),
            model: session.extra.get("model").and_then(|v| v.as_str()).map(String::from),
            token_usage: session.token_usage.clone().unwrap_or_default(),
            started_at: instance.started_at,
            last_seen: Utc::now(),
            pid: instance.pid,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MonitorEvent {
    Added(NormalizedSession),
    Updated(NormalizedSession),
    Removed { id: String, plugin: String },
    Snapshot(Vec<NormalizedSession>),
}
