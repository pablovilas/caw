use caw_core::types::{RawSession, SessionStatus, TokenUsage};
use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(serde::Deserialize)]
struct JournalEntry {
    role: Option<String>,
    message: Option<MessageContent>,
    #[serde(rename = "type")]
    entry_type: Option<String>,
}

#[derive(serde::Deserialize)]
struct MessageContent {
    role: Option<String>,
    content: Option<serde_json::Value>,
    model: Option<String>,
    usage: Option<UsageData>,
    stop_reason: Option<String>,
}

#[derive(serde::Deserialize)]
struct UsageData {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
}

/// Read session by ID. The id is the JSONL filename stem.
/// We find the matching session file in ~/.claude/projects/
pub fn read_session(session_id: &str) -> anyhow::Result<Option<RawSession>> {
    let projects_dir = match dirs::home_dir() {
        Some(h) => h.join(".claude").join("projects"),
        None => return Ok(None),
    };

    if !projects_dir.exists() {
        return Ok(None);
    }

    let session_file = find_session_file(&projects_dir, session_id);
    let Some(path) = session_file else {
        return Ok(None);
    };

    parse_session_file(&path, session_id)
}

fn find_session_file(projects_dir: &Path, session_id: &str) -> Option<PathBuf> {
    let target = format!("{}.jsonl", session_id);

    if let Ok(project_entries) = fs::read_dir(projects_dir) {
        for project_entry in project_entries.flatten() {
            let candidate = project_entry.path().join(&target);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn parse_session_file(path: &Path, instance_id: &str) -> anyhow::Result<Option<RawSession>> {
    let file = fs::File::open(path)?;
    let file_size = file.metadata()?.len();

    // For large files, only parse the last ~100KB for status + last message,
    // but we need full file for token totals. Use a two-pass approach:
    // Pass 1: tail for status. Pass 2: full scan for tokens (skip if file is small).
    let mut total_usage = TokenUsage::default();
    let mut state = ParseState::default();

    if file_size > 200_000 {
        // Large file: read last 100KB for recent state
        let mut tail_file = fs::File::open(path)?;
        let offset = file_size.saturating_sub(100_000);
        tail_file.seek(SeekFrom::Start(offset))?;
        let reader = BufReader::new(tail_file);
        let mut first_line = true;

        for line in reader.lines() {
            let line = match line {
                Ok(l) if !l.trim().is_empty() => l,
                _ => continue,
            };

            if first_line && offset > 0 {
                first_line = false;
                continue;
            }
            first_line = false;

            process_line(&line, &mut state, &mut total_usage);
        }

        // Full scan for token totals
        let full_file = fs::File::open(path)?;
        let reader = BufReader::new(full_file);
        let mut full_usage = TokenUsage::default();
        for line in reader.lines() {
            let line = match line {
                Ok(l) if !l.trim().is_empty() => l,
                _ => continue,
            };
            if line.contains("\"usage\"") {
                if let Ok(entry) = serde_json::from_str::<JournalEntry>(&line) {
                    if let Some(msg) = &entry.message {
                        if let Some(usage) = &msg.usage {
                            full_usage.input += usage.input_tokens.unwrap_or(0);
                            full_usage.output += usage.output_tokens.unwrap_or(0);
                            full_usage.cache_read += usage.cache_read_input_tokens.unwrap_or(0);
                            full_usage.cache_write += usage.cache_creation_input_tokens.unwrap_or(0);
                        }
                    }
                }
            }
        }
        total_usage = full_usage;
    } else {
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = match line {
                Ok(l) if !l.trim().is_empty() => l,
                _ => continue,
            };
            process_line(&line, &mut state, &mut total_usage);
        }
    }

    // Determine status based on file recency and last entry state
    let modified = fs::metadata(path)?
        .modified()
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    let age = std::time::SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default();

    let recent = age.as_secs() < 30;

    let status = if recent {
        // File was written to recently — assistant is working or about to
        SessionStatus::Working
    } else if state.last_stop_reason.as_deref() == Some("end_turn") {
        // Assistant finished its turn cleanly
        SessionStatus::Idle
    } else if state.last_role.as_deref() == Some("user") {
        // Last entry was user — waiting for assistant to respond
        SessionStatus::WaitingInput
    } else if state.last_role.as_deref() == Some("assistant") {
        // Assistant stopped without end_turn (tool_use, permission prompt, etc.)
        // → needs user action
        SessionStatus::WaitingInput
    } else {
        SessionStatus::Idle
    };

    Ok(Some(RawSession {
        instance_id: instance_id.to_string(),
        status,
        last_message: state.last_message,
        git_branch: None,
        token_usage: Some(total_usage),
        extra: serde_json::json!({ "model": state.model }),
    }))
}

#[derive(Default)]
struct ParseState {
    last_role: Option<String>,
    last_stop_reason: Option<String>,
    last_message: Option<String>,
    model: Option<String>,
}

fn process_line(line: &str, state: &mut ParseState, total_usage: &mut TokenUsage) {
    let entry: JournalEntry = match serde_json::from_str(line) {
        Ok(e) => e,
        Err(_) => return,
    };

    if let Some(msg) = &entry.message {
        if let Some(role) = &msg.role {
            state.last_role = Some(role.clone());
            // Reset stop_reason when role changes
            state.last_stop_reason = None;
        }

        if let Some(m) = &msg.model {
            state.model = Some(m.clone());
        }

        if let Some(sr) = &msg.stop_reason {
            state.last_stop_reason = Some(sr.clone());
        }

        if let Some(usage) = &msg.usage {
            total_usage.input += usage.input_tokens.unwrap_or(0);
            total_usage.output += usage.output_tokens.unwrap_or(0);
            total_usage.cache_read += usage.cache_read_input_tokens.unwrap_or(0);
            total_usage.cache_write += usage.cache_creation_input_tokens.unwrap_or(0);
        }

        if msg.role.as_deref() == Some("assistant") {
            if let Some(content) = &msg.content {
                if let Some(text) = extract_text_from_content(content) {
                    state.last_message = Some(text);
                }
            }
        }
    }

    if let Some(role) = &entry.role {
        state.last_role = Some(role.clone());
    }

    if let Some(entry_type) = &entry.entry_type {
        if entry_type == "human" || entry_type == "user" {
            state.last_role = Some("user".to_string());
        } else if entry_type == "assistant" {
            state.last_role = Some("assistant".to_string());
        }
    }
}

fn extract_text_from_content(content: &serde_json::Value) -> Option<String> {
    match content {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(arr) => {
            // Get the last text block for the most recent content
            let mut last_text = None;
            for item in arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    last_text = Some(text.to_string());
                }
            }
            last_text
        }
        _ => None,
    }
}
