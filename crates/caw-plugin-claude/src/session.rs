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

    // ID format is "{uuid}-{pid}" — strip the pid suffix to get the JSONL filename
    // UUIDs are 36 chars (8-4-4-4-12).
    let file_stem = if session_id.len() > 36 {
        &session_id[..36]
    } else {
        session_id
    };
    let session_file = find_session_file(&projects_dir, file_stem);
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
    let mut last_role: Option<String> = None;
    let mut last_message: Option<String> = None;
    let mut model: Option<String> = None;

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

            // Skip the first (potentially partial) line when seeking
            if first_line && offset > 0 {
                first_line = false;
                continue;
            }
            first_line = false;

            process_line(&line, &mut last_role, &mut last_message, &mut model, &mut total_usage);
        }

        // For token totals in large files, scan the full file just for usage
        let full_file = fs::File::open(path)?;
        let reader = BufReader::new(full_file);
        let mut full_usage = TokenUsage::default();
        for line in reader.lines() {
            let line = match line {
                Ok(l) if !l.trim().is_empty() => l,
                _ => continue,
            };
            // Quick check before full parse
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
        // Small file: parse everything
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = match line {
                Ok(l) if !l.trim().is_empty() => l,
                _ => continue,
            };
            process_line(&line, &mut last_role, &mut last_message, &mut model, &mut total_usage);
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
        git_branch: None, // populated from instance.extra in from_raw
        token_usage: Some(total_usage),
        extra: serde_json::json!({ "model": model }),
    }))
}

fn process_line(
    line: &str,
    last_role: &mut Option<String>,
    last_message: &mut Option<String>,
    model: &mut Option<String>,
    total_usage: &mut TokenUsage,
) {
    let entry: JournalEntry = match serde_json::from_str(line) {
        Ok(e) => e,
        Err(_) => return,
    };

    if let Some(msg) = &entry.message {
        if let Some(role) = &msg.role {
            *last_role = Some(role.clone());
        }

        if let Some(m) = &msg.model {
            *model = Some(m.clone());
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
                    *last_message = Some(text);
                }
            }
        }
    }

    // Handle entries with role at top level (e.g., type: "user")
    if let Some(role) = &entry.role {
        *last_role = Some(role.clone());
    }

    // Some entries have type field indicating the role
    if let Some(entry_type) = &entry.entry_type {
        if entry_type == "human" || entry_type == "user" {
            *last_role = Some("user".to_string());
        } else if entry_type == "assistant" {
            *last_role = Some("assistant".to_string());
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
