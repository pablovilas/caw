use std::collections::{HashMap, HashSet};
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
    pub app_name: Option<String>,
}

struct CachedProcess {
    pid: u32,
    name: String,
    cwd: Option<PathBuf>,
    cmd: Vec<String>,
}

pub struct ProcessScanner {
    system: System,
    last_refresh: Instant,
    cache_duration: Duration,
    cached: Vec<CachedProcess>,
    ppid_map: HashMap<u32, u32>,
    cmd_map: HashMap<u32, Vec<String>>,
    app_name_cache: HashMap<u32, Option<String>>,
}

impl ProcessScanner {
    pub fn new() -> Self {
        Self {
            system: System::new(),
            last_refresh: Instant::now() - Duration::from_secs(60),
            cache_duration: Duration::from_secs(5),
            cached: Vec::new(),
            ppid_map: HashMap::new(),
            cmd_map: HashMap::new(),
            app_name_cache: HashMap::new(),
        }
    }

    /// Refresh if stale, then return processes matching any of the given names.
    pub fn scan(&mut self, names: &[&str]) -> Vec<ProcessInfo> {
        self.refresh_if_needed();

        // On macOS, resolve cwd via lsof for matched processes missing it
        #[cfg(target_os = "macos")]
        {
            let missing: Vec<u32> = self.cached
                .iter()
                .filter(|p| p.cwd.is_none() && names.iter().any(|n| p.name == *n))
                .map(|p| p.pid)
                .collect();

            if !missing.is_empty() {
                let cwd_map = lsof_cwds(&missing);
                for info in &mut self.cached {
                    if info.cwd.is_none() {
                        if let Some(cwd) = cwd_map.get(&info.pid) {
                            info.cwd = Some(cwd.clone());
                        }
                    }
                }
            }
        }

        let matched: Vec<_> = self.cached
            .iter()
            .filter(|p| names.iter().any(|n| p.name == *n))
            .collect();

        // Resolve app_name only for new PIDs
        for p in &matched {
            if !self.app_name_cache.contains_key(&p.pid) {
                let name = resolve_app_name(p.pid, &self.ppid_map, &self.cmd_map);
                self.app_name_cache.insert(p.pid, name);
            }
        }

        matched.into_iter()
            .map(|p| {
                let app_name = self.app_name_cache.get(&p.pid).cloned().flatten();
                ProcessInfo {
                    pid: p.pid,
                    name: p.name.clone(),
                    cwd: p.cwd.clone(),
                    cmd: p.cmd.clone(),
                    app_name,
                }
            })
            .collect()
    }

    fn refresh_if_needed(&mut self) {
        if self.last_refresh.elapsed() < self.cache_duration {
            return;
        }

        // Only fetch process name + cwd. Skip CPU, memory, disk, tasks.
        let refresh_kind = sysinfo::ProcessRefreshKind::nothing()
            .with_cwd(sysinfo::UpdateKind::OnlyIfNotSet)
            .with_cmd(sysinfo::UpdateKind::OnlyIfNotSet);

        self.system
            .refresh_processes_specifics(sysinfo::ProcessesToUpdate::All, true, refresh_kind);

        let mut matched_infos: Vec<CachedProcess> = Vec::new();
        let mut ppid_map: HashMap<u32, u32> = HashMap::new();
        let mut cmd_map: HashMap<u32, Vec<String>> = HashMap::new();

        for (pid, process) in self.system.processes() {
            let pid_u32 = pid.as_u32();
            let name = process.name().to_string_lossy().to_string();
            let cwd = process.cwd().map(PathBuf::from);
            let cmd: Vec<String> = process
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().to_string())
                .collect();

            if let Some(ppid) = process.parent() {
                ppid_map.insert(pid_u32, ppid.as_u32());
            }
            cmd_map.insert(pid_u32, cmd.clone());

            matched_infos.push(CachedProcess {
                pid: pid_u32,
                name,
                cwd,
                cmd,
            });
        }

        // Prune app_name cache for dead PIDs
        let live_pids: HashSet<u32> = matched_infos.iter().map(|p| p.pid).collect();
        self.app_name_cache.retain(|pid, _| live_pids.contains(pid));

        self.cached = matched_infos;
        self.ppid_map = ppid_map;
        self.cmd_map = cmd_map;
        self.last_refresh = Instant::now();
    }
}

impl Default for ProcessScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Walk the process tree to find the host .app bundle.
fn resolve_app_name(
    pid: u32,
    ppid_map: &HashMap<u32, u32>,
    cmd_map: &HashMap<u32, Vec<String>>,
) -> Option<String> {
    if let Some(cmd) = cmd_map.get(&pid) {
        if let Some(first) = cmd.first() {
            if let Some(app) = extract_app_display_name(first) {
                return Some(app);
            }
        }
    }

    let mut current = pid;
    for _ in 0..20 {
        let ppid = match ppid_map.get(&current) {
            Some(&p) if p > 1 => p,
            _ => break,
        };

        if let Some(cmd) = cmd_map.get(&ppid) {
            if let Some(first) = cmd.first() {
                if let Some(app) = extract_app_display_name(first) {
                    return Some(app);
                }
            }
        }

        current = ppid;
    }

    #[cfg(target_os = "macos")]
    {
        return resolve_app_name_via_ps(pid);
    }

    #[cfg(not(target_os = "macos"))]
    None
}

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

        let (ppid_str, cmd) = line.split_once(char::is_whitespace)?;
        let ppid: u32 = ppid_str.trim().parse().ok()?;

        if let Some(app) = extract_app_display_name(cmd.trim()) {
            return Some(app);
        }

        if ppid <= 1 {
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

fn extract_app_display_name(cmd: &str) -> Option<String> {
    let idx = cmd.find(".app")?;
    let app_path = &cmd[..idx + 4];
    let bundle_name = app_path.rsplit('/').next().unwrap_or(app_path);
    let name = bundle_name.strip_suffix(".app").unwrap_or(bundle_name);

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
