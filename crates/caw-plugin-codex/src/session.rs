use caw_core::types::{RawSession, SessionStatus, TokenUsage};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(serde::Deserialize)]
struct CodexEntry {
    #[serde(rename = "type")]
    entry_type: Option<String>,
    payload: Option<serde_json::Value>,
}

/// Find the most recent codex session file.
pub fn find_recent_session() -> Option<PathBuf> {
    let codex_dir = dirs::home_dir()?.join(".codex").join("sessions");
    if !codex_dir.exists() {
        return None;
    }

    // Sessions are organized as sessions/YYYY/MM/DD/*.jsonl
    // Find the most recently modified one
    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;

    for year in read_sorted_dirs(&codex_dir).into_iter().rev().take(1) {
        for month in read_sorted_dirs(&year).into_iter().rev().take(1) {
            for day in read_sorted_dirs(&month).into_iter().rev().take(1) {
                if let Ok(files) = fs::read_dir(&day) {
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
            }
        }
    }

    best.map(|(p, _)| p)
}

fn read_sorted_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs
}

pub fn read_session(instance_id: &str) -> anyhow::Result<Option<RawSession>> {
    let session_path = match find_recent_session() {
        Some(p) => p,
        None => return Ok(None),
    };

    parse_session_file(&session_path, instance_id)
}

fn parse_session_file(path: &Path, instance_id: &str) -> anyhow::Result<Option<RawSession>> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);

    let mut last_role: Option<String> = None;
    let mut last_message: Option<String> = None;
    let mut model: Option<String> = None;
    let mut total_usage = TokenUsage::default();

    for line in reader.lines() {
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            _ => continue,
        };

        let entry: CodexEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let entry_type = entry.entry_type.as_deref().unwrap_or("");

        match entry_type {
            "response_item" => {
                if let Some(payload) = &entry.payload {
                    let role = payload.get("role").and_then(|v| v.as_str());
                    if let Some(r) = role {
                        last_role = Some(r.to_string());
                    }

                    // Extract assistant text
                    if role == Some("assistant") {
                        if let Some(content) = payload.get("content").and_then(|v| v.as_array()) {
                            for item in content {
                                if item.get("type").and_then(|v| v.as_str()) == Some("output_text")
                                {
                                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                                        if !text.is_empty() {
                                            last_message = Some(text.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "turn_context" => {
                if let Some(payload) = &entry.payload {
                    if let Some(m) = payload.get("model").and_then(|v| v.as_str()) {
                        model = Some(m.to_string());
                    }
                }
            }
            "event_msg" => {
                if let Some(payload) = &entry.payload {
                    let evt_type = payload.get("type").and_then(|v| v.as_str());
                    if evt_type == Some("token_count") {
                        if let Some(info) = payload.get("info") {
                            if let Some(usage) = info.get("total_token_usage") {
                                total_usage.input =
                                    usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                                total_usage.output = usage
                                    .get("output_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                total_usage.cache_read = usage
                                    .get("cached_input_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Determine status
    let modified = fs::metadata(path)?
        .modified()
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    let age = std::time::SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default();

    let status = match last_role.as_deref() {
        Some("user") => SessionStatus::WaitingInput,
        Some("assistant") => {
            if age.as_secs() < 5 {
                SessionStatus::Working
            } else {
                SessionStatus::Idle
            }
        }
        _ => SessionStatus::Idle,
    };

    Ok(Some(RawSession {
        instance_id: instance_id.to_string(),
        status,
        last_message,
        git_branch: None, // populated from instance.extra
        token_usage: Some(total_usage),
        extra: serde_json::json!({ "model": model }),
    }))
}
