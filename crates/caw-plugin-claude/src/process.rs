use caw_core::types::RawInstance;
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use sysinfo::System;

#[derive(Debug, Clone)]
pub struct ClaudeProcess {
    pub pid: u32,
    pub cwd: Option<PathBuf>,
}

/// Get all running claude CLI processes with their cwd.
pub fn get_claude_processes() -> Vec<ClaudeProcess> {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut pids = Vec::new();
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_string();
        if name == "claude" {
            pids.push(pid.as_u32());
        }
    }

    if pids.is_empty() {
        return Vec::new();
    }

    let cwd_map = get_cwds_via_lsof(&pids);

    pids.iter()
        .map(|&pid| ClaudeProcess {
            pid,
            cwd: cwd_map.get(&pid).cloned(),
        })
        .collect()
}

fn get_cwds_via_lsof(pids: &[u32]) -> HashMap<u32, PathBuf> {
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

/// Encode a filesystem path the same way Claude does for project dir names.
/// "/Users/pablo/Projects/caw" → "-Users-pablo-Projects-caw"
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

fn encode_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('/', "-")
}

/// Discover Claude Code instances by matching running processes to session dirs.
pub fn discover_claude_instances() -> Vec<RawInstance> {
    let projects_dir = match dirs::home_dir() {
        Some(h) => h.join(".claude").join("projects"),
        None => return Vec::new(),
    };

    if !projects_dir.exists() {
        return Vec::new();
    }

    let processes = get_claude_processes();

    // Build map: encoded cwd → first process found (one instance per project)
    let mut encoded_to_process: HashMap<String, &ClaudeProcess> = HashMap::new();
    for proc in &processes {
        if let Some(cwd) = &proc.cwd {
            let encoded = encode_path(cwd);
            encoded_to_process.entry(encoded).or_insert(proc);
        }
    }

    let mut instances = Vec::new();

    let Ok(project_entries) = std::fs::read_dir(&projects_dir) else {
        return Vec::new();
    };

    for project_entry in project_entries.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }

        // Find the most recently modified .jsonl file
        let mut best_session: Option<(PathBuf, std::time::SystemTime)> = None;

        let Ok(files) = std::fs::read_dir(&project_path) else {
            continue;
        };

        for file in files.flatten() {
            let path = file.path();
            if path.extension().is_some_and(|e| e == "jsonl") {
                if let Ok(meta) = path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if best_session.as_ref().is_none_or(|(_, t)| modified > *t) {
                            best_session = Some((path, modified));
                        }
                    }
                }
            }
        }

        let Some((session_path, modified)) = best_session else {
            continue;
        };

        // Only include sessions modified in the last hour
        let age = std::time::SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default();
        if age.as_secs() > 3600 {
            continue;
        }

        let dir_name = project_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let matched = encoded_to_process.get(dir_name.as_str());

        let Some(proc) = matched else {
            continue;
        };

        let session_id = session_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let working_dir = proc.cwd.clone().unwrap_or_default();
        let git_branch = read_git_branch(&working_dir);

        instances.push(RawInstance {
            id: session_id,
            pid: Some(proc.pid),
            working_dir,
            started_at: Utc::now(),
            extra: serde_json::json!({
                "session_file": session_path.to_string_lossy(),
                "git_branch": git_branch,
            }),
        });
    }

    instances
}
