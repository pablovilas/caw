use async_trait::async_trait;
use caw_core::types::{RawInstance, RawSession, SessionStatus};
use caw_core::IPlugin;
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

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
        let pids = pgrep("opencode");
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
                id: format!("opencode-{}", pid),
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

fn pgrep(name: &str) -> Vec<u32> {
    let Ok(output) = Command::new("pgrep").args(["-x", name]).output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect()
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
