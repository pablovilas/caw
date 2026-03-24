use caw_core::process::read_git_branch;
use caw_core::types::RawInstance;
use caw_core::ProcessInfo;
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;

/// Encode a filesystem path the same way Claude does for project dir names.
fn encode_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('/', "-")
}

/// Discover Claude Code sessions.
///
/// Each active JSONL file = one session row. Processes provide liveness info
/// (PID, app name) but the JSONL file is the source of truth for sessions.
/// A project may have multiple active sessions (different JSONL files) and
/// multiple processes — we can't reliably map PID → JSONL, so we pick a
/// process to associate with each session for the focus feature.
pub fn discover_claude_instances(processes: Vec<ProcessInfo>) -> Vec<RawInstance> {
    let projects_dir = match dirs::home_dir() {
        Some(h) => h.join(".claude").join("projects"),
        None => return Vec::new(),
    };

    if !projects_dir.exists() {
        return Vec::new();
    }

    // Build map: encoded cwd → processes
    let mut encoded_to_processes: HashMap<String, Vec<&ProcessInfo>> = HashMap::new();
    for proc in &processes {
        if let Some(cwd) = &proc.cwd {
            let encoded = encode_path(cwd);
            encoded_to_processes.entry(encoded).or_default().push(proc);
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

        let dir_name = project_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Get processes for this project (if any)
        let procs = encoded_to_processes.get(dir_name.as_str());

        // Collect ALL active JSONL files (modified in last hour)
        let Ok(files) = std::fs::read_dir(&project_path) else {
            continue;
        };

        let mut active_sessions: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

        for file in files.flatten() {
            let path = file.path();
            if path.extension().is_some_and(|e| e == "jsonl") {
                if let Ok(meta) = path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        let age = std::time::SystemTime::now()
                            .duration_since(modified)
                            .unwrap_or_default();
                        if age.as_secs() < 3600 {
                            active_sessions.push((path, modified));
                        }
                    }
                }
            }
        }

        // Sort by most recent first
        active_sessions.sort_by(|a, b| b.1.cmp(&a.1));

        let num_procs = procs.map(|p| p.len()).unwrap_or(0);

        // Only show as many sessions as there are running processes.
        // If no processes, skip entirely (dead project).
        if num_procs == 0 {
            continue;
        }
        active_sessions.truncate(num_procs);

        // Get working dir and git branch from a process
        let (working_dir, git_branch) = {
            let proc = &procs.unwrap()[0];
            let wd = proc.cwd.clone().unwrap_or_default();
            let branch = read_git_branch(&wd);
            (wd, branch)
        };

        // One instance per session, assign processes 1:1
        for (i, (session_path, _modified)) in active_sessions.iter().enumerate() {
            let session_id = session_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let proc = procs.unwrap()[i];
            let pid = Some(proc.pid);
            let app_name = proc.app_name.clone();

            instances.push(RawInstance {
                id: session_id,
                pid,
                working_dir: working_dir.clone(),
                started_at: Utc::now(),
                extra: serde_json::json!({
                    "session_file": session_path.to_string_lossy(),
                    "git_branch": git_branch,
                    "app_name": app_name,
                }),
            });
        }
    }

    instances
}
