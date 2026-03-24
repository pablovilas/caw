mod session;

use async_trait::async_trait;
use caw_core::types::{RawInstance, RawSession};
use caw_core::IPlugin;
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use sysinfo::System;

pub struct CodexPlugin;

impl CodexPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodexPlugin {
    fn default() -> Self {
        Self::new()
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
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let mut pids = Vec::new();
        for (pid, process) in sys.processes() {
            let name = process.name().to_string_lossy().to_string();
            if name == "codex" || name.starts_with("codex") {
                pids.push(pid.as_u32());
            }
        }

        if pids.is_empty() {
            return Ok(Vec::new());
        }

        let cwd_map = get_cwds_via_lsof(&pids);

        let mut instances = Vec::new();
        for pid in pids {
            let cwd = match cwd_map.get(&pid) {
                Some(c) => c.clone(),
                None => continue,
            };

            let git_branch = read_git_branch(&cwd);

            instances.push(RawInstance {
                id: format!("codex-{}", pid),
                pid: Some(pid),
                working_dir: cwd,
                started_at: Utc::now(),
                extra: serde_json::json!({
                    "git_branch": git_branch,
                }),
            });
        }

        Ok(instances)
    }

    async fn read_session(&self, id: &str) -> anyhow::Result<Option<RawSession>> {
        session::read_session(id)
    }

    fn poll_interval(&self) -> Duration {
        Duration::from_secs(3)
    }
}

fn get_cwds_via_lsof(pids: &[u32]) -> HashMap<u32, PathBuf> {
    let mut map = HashMap::new();
    if pids.is_empty() {
        return map;
    }

    let pid_arg = pids.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");

    let Ok(output) = Command::new("lsof")
        .args(["-a", "-d", "cwd", "-p", &pid_arg, "-Fn"])
        .output()
    else {
        return map;
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut current_pid: Option<u32> = None;

    for line in stdout.lines() {
        if let Some(pid_str) = line.strip_prefix('p') {
            current_pid = pid_str.parse().ok();
        } else if let Some(name) = line.strip_prefix('n') {
            if let Some(pid) = current_pid {
                map.insert(pid, PathBuf::from(name));
            }
        }
    }

    map
}

fn read_git_branch(working_dir: &std::path::Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(working_dir)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() || branch == "HEAD" {
        None
    } else {
        Some(branch)
    }
}
