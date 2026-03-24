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

/// Discover Claude Code instances by matching running processes to session dirs.
/// Creates one instance per process — if two processes share a project, both appear.
pub fn discover_claude_instances(processes: Vec<ProcessInfo>) -> Vec<RawInstance> {
    let projects_dir = match dirs::home_dir() {
        Some(h) => h.join(".claude").join("projects"),
        None => return Vec::new(),
    };

    if !projects_dir.exists() {
        return Vec::new();
    }

    // Build map: encoded cwd → all processes with that cwd
    let mut encoded_to_processes: HashMap<String, Vec<&ProcessInfo>> = HashMap::new();
    for proc in &processes {
        if let Some(cwd) = &proc.cwd {
            let encoded = encode_path(cwd);
            encoded_to_processes.entry(encoded).or_default().push(proc);
        }
    }

    // Collect recent session files per project dir
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

        let Some(procs) = encoded_to_processes.get(dir_name.as_str()) else {
            continue;
        };

        let session_id = session_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Create one instance per process on this project
        for proc in procs {
            let working_dir = proc.cwd.clone().unwrap_or_default();
            let git_branch = read_git_branch(&working_dir);

            instances.push(RawInstance {
                id: format!("{}-{}", session_id, proc.pid),
                pid: Some(proc.pid),
                working_dir,
                started_at: Utc::now(),
                extra: serde_json::json!({
                    "session_file": session_path.to_string_lossy(),
                    "git_branch": git_branch,
                    "app_name": proc.app_name,
                }),
            });
        }
    }

    instances
}
