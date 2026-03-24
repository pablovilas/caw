use caw_core::process::read_git_branch;
use caw_core::types::RawInstance;
use caw_core::ProcessInfo;
use chrono::Utc;
use std::path::PathBuf;

/// Encode a filesystem path the same way Claude does for project dir names.
fn encode_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('/', "-")
}

/// Find the most recently modified JSONL for a given project directory.
fn find_session_file(projects_dir: &std::path::Path, encoded_cwd: &str) -> Option<PathBuf> {
    let project_dir = projects_dir.join(encoded_cwd);
    if !project_dir.exists() {
        return None;
    }

    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;
    if let Ok(files) = std::fs::read_dir(&project_dir) {
        for file in files.flatten() {
            let path = file.path();
            if path.extension().is_some_and(|e| e == "jsonl") {
                if let Ok(meta) = path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if best.as_ref().is_none_or(|(_, t)| modified > *t) {
                            best = Some((path, modified));
                        }
                    }
                }
            }
        }
    }

    best.map(|(p, _)| p)
}

/// Discover Claude Code sessions. One session per running process.
/// JSONL files provide metadata only — processes are the source of truth.
pub fn discover_claude_instances(processes: Vec<ProcessInfo>) -> Vec<RawInstance> {
    let projects_dir = dirs::home_dir().map(|h| h.join(".claude").join("projects"));

    let mut instances = Vec::new();

    for proc in &processes {
        let cwd = match &proc.cwd {
            Some(c) => c.clone(),
            None => continue,
        };

        let git_branch = read_git_branch(&cwd);

        // Find the JSONL session file for this project (for metadata)
        let session_file = projects_dir
            .as_ref()
            .and_then(|pd| find_session_file(pd, &encode_path(&cwd)));

        let session_id = session_file
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("claude-{}", proc.pid));

        instances.push(RawInstance {
            id: session_id,
            pid: Some(proc.pid),
            working_dir: cwd,
            started_at: Utc::now(),
            extra: serde_json::json!({
                "session_file": session_file.as_ref().map(|p| p.to_string_lossy().to_string()),
                "git_branch": git_branch,
                "app_name": proc.app_name,
            }),
        });
    }

    instances
}
