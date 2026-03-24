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
/// Takes pre-scanned process list from the shared ProcessScanner.
pub fn discover_claude_instances(processes: Vec<ProcessInfo>) -> Vec<RawInstance> {
    let projects_dir = match dirs::home_dir() {
        Some(h) => h.join(".claude").join("projects"),
        None => return Vec::new(),
    };

    if !projects_dir.exists() {
        return Vec::new();
    }

    // Build map: encoded cwd → first process (one instance per project)
    let mut encoded_to_process: HashMap<String, &ProcessInfo> = HashMap::new();
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

        let Some(proc) = encoded_to_process.get(dir_name.as_str()) else {
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
