use async_trait::async_trait;
use caw_core::process::read_git_branch;
use caw_core::types::{RawInstance, RawSession, SessionStatus};
use caw_core::{IPlugin, ProcessScanner};
use chrono::Utc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct OpenCodePlugin {
    scanner: Arc<Mutex<ProcessScanner>>,
}

impl OpenCodePlugin {
    pub fn new(scanner: Arc<Mutex<ProcessScanner>>) -> Self {
        Self { scanner }
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
        let scanner = self.scanner.clone();
        let processes = tokio::task::spawn_blocking(move || {
            scanner.lock().unwrap().scan(&["opencode"])
        }).await?;

        let mut instances = Vec::new();
        for proc in processes {
            let cwd = match &proc.cwd {
                Some(c) => c.clone(),
                None => continue,
            };

            let git_branch = read_git_branch(&cwd);

            instances.push(RawInstance {
                id: format!("opencode-{}", proc.pid),
                pid: Some(proc.pid),
                working_dir: cwd,
                started_at: Utc::now(),
                extra: serde_json::json!({
                    "git_branch": git_branch,
                    "app_name": proc.app_name,
                }),
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
        Duration::from_secs(5)
    }
}
