pub mod debug;
mod process;
mod session;

use async_trait::async_trait;
use caw_core::types::{RawInstance, RawSession};
use caw_core::{IPlugin, ProcessScanner};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct ClaudePlugin {
    scanner: Arc<Mutex<ProcessScanner>>,
}

impl ClaudePlugin {
    pub fn new(scanner: Arc<Mutex<ProcessScanner>>) -> Self {
        Self { scanner }
    }
}

#[async_trait]
impl IPlugin for ClaudePlugin {
    fn name(&self) -> &'static str {
        "claude-code"
    }

    fn display_name(&self) -> &'static str {
        "Claude Code"
    }

    async fn discover(&self) -> anyhow::Result<Vec<RawInstance>> {
        let processes = self.scanner.lock().unwrap().scan(&["claude"]);
        Ok(process::discover_claude_instances(processes))
    }

    async fn read_session(&self, id: &str) -> anyhow::Result<Option<RawSession>> {
        session::read_session(id)
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_secs(5)
    }
}
