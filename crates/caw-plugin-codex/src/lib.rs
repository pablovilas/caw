mod session;

use async_trait::async_trait;
use caw_core::process::read_git_branch;
use caw_core::types::{RawInstance, RawSession};
use caw_core::{IPlugin, ProcessScanner};
use chrono::Utc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct CodexPlugin {
    scanner: Arc<Mutex<ProcessScanner>>,
}

impl CodexPlugin {
    pub fn new(scanner: Arc<Mutex<ProcessScanner>>) -> Self {
        Self { scanner }
    }
}

#[async_trait]
impl IPlugin for CodexPlugin {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn display_name(&self) -> &'static str {
        "OpenAI Codex CLI"
    }

    async fn discover(&self) -> anyhow::Result<Vec<RawInstance>> {
        let processes = self.scanner.lock().unwrap().scan(&["codex"]);

        let mut instances = Vec::new();
        for proc in processes {
            let cwd = match &proc.cwd {
                Some(c) => c.clone(),
                None => continue,
            };

            let git_branch = read_git_branch(&cwd);

            instances.push(RawInstance {
                id: format!("codex-{}", proc.pid),
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
        session::read_session(id)
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_secs(5)
    }
}
