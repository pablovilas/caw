use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};
use sysinfo::System;

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cwd: Option<PathBuf>,
    pub cmd: Vec<String>,
    /// The host application (e.g. "iTerm", "VS Code", "Terminal")
    pub app_name: Option<String>,
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
            last_refresh: Instant::now() - Duration::from_secs(60),
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

        // Collect parent PID mapping for app_name resolution
        let mut ppid_map: HashMap<u32, u32> = HashMap::new();
        let mut cmd_map: HashMap<u32, Vec<String>> = HashMap::new();

        for (pid, process) in self.system.processes() {
            let pid_u32 = pid.as_u32();
            if let Some(ppid) = process.parent() {
                ppid_map.insert(pid_u32, ppid.as_u32());
            }
            let cmd: Vec<String> = process
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().to_string())
                .collect();
            cmd_map.insert(pid_u32, cmd);
        }

        for (pid, process) in self.system.processes() {
            let name = process.name().to_string_lossy().to_string();
            let cwd = process.cwd().map(PathBuf::from);
            let cmd = cmd_map.get(&pid.as_u32()).cloned().unwrap_or_default();
            let app_name = resolve_app_name(pid.as_u32(), &ppid_map, &cmd_map);

            infos.push(ProcessInfo {
                pid: pid.as_u32(),
                name,
                cwd,
                cmd,
                app_name,
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

/// Walk the process tree via sysinfo's parent PID data to find the host .app bundle.
fn resolve_app_name(
    pid: u32,
    ppid_map: &HashMap<u32, u32>,
    cmd_map: &HashMap<u32, Vec<String>>,
) -> Option<String> {
    // First check the process's own cmd for an .app path
    if let Some(cmd) = cmd_map.get(&pid) {
        let cmd_str = cmd.first().unwrap_or(&String::new()).clone();
        if let Some(app) = extract_app_display_name(&cmd_str) {
            return Some(app);
        }
    }

    // Walk up parent chain
    let mut current = pid;
    for _ in 0..20 {
        let ppid = match ppid_map.get(&current) {
            Some(&p) if p > 1 => p,
            _ => break,
        };

        if let Some(cmd) = cmd_map.get(&ppid) {
            let cmd_str = cmd.first().unwrap_or(&String::new()).clone();
            if let Some(app) = extract_app_display_name(&cmd_str) {
                return Some(app);
            }
        }

        current = ppid;
    }

    // On macOS, sysinfo cmd() is often empty. Fall back to ps.
    #[cfg(target_os = "macos")]
    {
        return resolve_app_name_via_ps(pid);
    }

    #[cfg(not(target_os = "macos"))]
    None
}

/// macOS fallback: walk process tree via ps command.
#[cfg(target_os = "macos")]
fn resolve_app_name_via_ps(pid: u32) -> Option<String> {
    let mut current = pid;
    for _ in 0..20 {
        let output = Command::new("ps")
            .args(["-o", "ppid=,command=", "-p", &current.to_string()])
            .output()
            .ok()?;

        let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if line.is_empty() {
            break;
        }

        // Parse "  ppid command..."
        let line = line.trim();
        let (ppid_str, cmd) = line.split_once(char::is_whitespace)?;
        let ppid: u32 = ppid_str.trim().parse().ok()?;

        if let Some(app) = extract_app_display_name(cmd.trim()) {
            return Some(app);
        }

        if ppid <= 1 {
            // Check PID 1's parent too
            let output = Command::new("ps")
                .args(["-o", "command=", "-p", &ppid.to_string()])
                .output()
                .ok()?;
            let cmd = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return extract_app_display_name(&cmd);
        }
        current = ppid;
    }
    None
}

/// Extract a human-readable app name from a command path containing ".app".
/// "/Applications/iTerm.app/Contents/MacOS/iTerm2" → "iTerm"
/// "/Applications/Visual Studio Code.app/..." → "VS Code"
fn extract_app_display_name(cmd: &str) -> Option<String> {
    let idx = cmd.find(".app")?;
    let app_path = &cmd[..idx + 4];

    // Get the filename of the .app bundle
    let bundle_name = app_path.rsplit('/').next().unwrap_or(app_path);
    let name = bundle_name.strip_suffix(".app").unwrap_or(bundle_name);

    // Friendly display names
    let display = match name {
        "iTerm" | "iTerm2" => "iTerm",
        "Visual Studio Code" | "Code" => "VS Code",
        "Terminal" => "Terminal",
        "Warp" => "Warp",
        "Alacritty" => "Alacritty",
        "kitty" => "Kitty",
        "Hyper" => "Hyper",
        "WezTerm" => "WezTerm",
        other => other,
    };

    Some(display.to_string())
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

/// Read the current git branch for a working directory.
pub fn read_git_branch(working_dir: &std::path::Path) -> Option<String> {
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
