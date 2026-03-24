use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use sysinfo::System;

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cwd: Option<PathBuf>,
}

pub struct ProcessScanner {
    system: System,
    last_refresh: Instant,
    cache_duration: Duration,
    cached: Vec<ProcessInfo>,
}

impl ProcessScanner {
    pub fn new() -> Self {
        Self {
            system: System::new(),
            last_refresh: Instant::now() - Duration::from_secs(60), // force first refresh
            cache_duration: Duration::from_secs(2),
            cached: Vec::new(),
        }
    }

    /// Refresh if stale, then return processes matching any of the given names.
    pub fn scan(&mut self, names: &[&str]) -> Vec<ProcessInfo> {
        self.refresh_if_needed();
        self.cached
            .iter()
            .filter(|p| names.iter().any(|n| p.name == *n))
            .cloned()
            .collect()
    }

    fn refresh_if_needed(&mut self) {
        if self.last_refresh.elapsed() < self.cache_duration {
            return;
        }

        self.system
            .refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let mut infos: Vec<ProcessInfo> = Vec::new();

        for (pid, process) in self.system.processes() {
            let name = process.name().to_string_lossy().to_string();
            let cwd = process.cwd().map(PathBuf::from);

            infos.push(ProcessInfo {
                pid: pid.as_u32(),
                name,
                cwd,
            });
        }

        // On macOS, sysinfo often can't read cwd. Batch-resolve via lsof.
        #[cfg(target_os = "macos")]
        {
            let missing_cwd_pids: Vec<u32> = infos
                .iter()
                .filter(|p| p.cwd.is_none())
                .map(|p| p.pid)
                .collect();

            if !missing_cwd_pids.is_empty() {
                let cwd_map = lsof_cwds(&missing_cwd_pids);
                for info in &mut infos {
                    if info.cwd.is_none() {
                        info.cwd = cwd_map.get(&info.pid).cloned();
                    }
                }
            }
        }

        self.cached = infos;
        self.last_refresh = Instant::now();
    }
}

impl Default for ProcessScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Batch-resolve cwds via lsof (macOS fallback).
#[cfg(target_os = "macos")]
fn lsof_cwds(pids: &[u32]) -> HashMap<u32, PathBuf> {
    let mut map = HashMap::new();
    if pids.is_empty() {
        return map;
    }

    let pid_arg = pids
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let Ok(output) = std::process::Command::new("lsof")
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

/// Read the current git branch for a working directory.
pub fn read_git_branch(working_dir: &std::path::Path) -> Option<String> {
    let output = std::process::Command::new("git")
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
