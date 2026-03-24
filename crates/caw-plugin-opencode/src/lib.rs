use async_trait::async_trait;
use caw_core::types::{RawInstance, RawSession, SessionStatus};
use caw_core::IPlugin;
use chrono::Utc;
use std::time::Duration;
use sysinfo::System;

pub struct OpenCodePlugin;

impl OpenCodePlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenCodePlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IPlugin for OpenCodePlugin {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn display_name(&self) -> &'static str {
        "OpenCode"
    }

    async fn discover(&self) -> anyhow::Result<Vec<RawInstance>> {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let mut instances = Vec::new();

        for (pid, process) in sys.processes() {
            let name = process.name().to_string_lossy().to_string();
            if name != "opencode" && !name.starts_with("opencode") {
                continue;
            }

            let cwd = process.cwd().map(|p| p.to_path_buf()).unwrap_or_default();
            if cwd.as_os_str().is_empty() {
                continue;
            }

            instances.push(RawInstance {
                id: format!("opencode-{}", pid.as_u32()),
                pid: Some(pid.as_u32()),
                working_dir: cwd,
                started_at: Utc::now(),
                extra: serde_json::json!({}),
            });
        }

        Ok(instances)
    }

    async fn read_session(&self, id: &str) -> anyhow::Result<Option<RawSession>> {
        Ok(Some(RawSession {
            instance_id: id.to_string(),
            status: SessionStatus::Idle,
            last_message: None,
            git_branch: None,
            token_usage: None,
            extra: serde_json::json!({}),
        }))
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_secs(3)
    }
}
