use crate::models::Conversation;
use crate::paths::{claude_dir, encode_project_path};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// jsonl record (partial deserialization — only fields we care about).
#[derive(Debug, Deserialize)]
struct JsonlRecord {
    #[serde(rename = "type")]
    typ: Option<String>,
    message: Option<PartialMessage>,
    summary: Option<String>,
    /// Claude Code's own session name: the last prompt text. `/resume` shows a
    /// truncated first line of this. Present on interactive CLI sessions.
    #[serde(rename = "lastPrompt", default)]
    last_prompt: Option<String>,
    /// AI-generated session title (Claude Code writes this to label the session).
    #[serde(rename = "aiTitle", default)]
    ai_title: Option<String>,
    /// User-set session title (via Claude Code's `/rename` or similar).
    #[serde(rename = "customTitle", default)]
    custom_title: Option<String>,
    /// Real working directory embedded in each record.
    #[serde(default)]
    cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PartialMessage {
    #[allow(dead_code)]
    role: Option<String>,
    model: Option<String>,
    content: Option<serde_json::Value>,
}

/// Resolve the encoded project directory for a real working path.
/// e.g. `D:\Programs\ClaudeCode` -> `~/.claude/projects/D--Programs-ClaudeCode`.
/// Returns None if that directory does not exist on disk.
pub fn project_dir_for_path(real_path: &str) -> Option<PathBuf> {
    let encoded = encode_project_path(real_path);
    let dir = claude_dir().join("projects").join(&encoded);
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// Read all conversations for a given real working path (precise, encode-based).
pub fn conversations_for_path(real_path: &str) -> Vec<Conversation> {
    match project_dir_for_path(real_path) {
        Some(dir) => parse_conversations_in_project(&dir),
        None => Vec::new(),
    }
}

/// Scan ALL conversations under `<claude_root>/projects/` (every encoded subdir).
/// Used by the "loose conversations" feature (all minus registered projects).
pub fn scan_all_conversations(claude_root: &Path) -> Vec<Conversation> {
    let projects_dir = claude_root.join("projects");
    let mut all = Vec::new();
    if let Ok(entries) = fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                all.extend(parse_conversations_in_project(&path));
            }
        }
    }
    all.sort_by(|a, b| b.last_updated.cmp(&a.last_updated));
    all
}

/// Parse all *.jsonl files inside one project directory.
pub fn parse_conversations_in_project(proj_dir: &Path) -> Vec<Conversation> {
    let mut convos = Vec::new();
    let encoded_name = proj_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    if let Ok(entries) = fs::read_dir(proj_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "jsonl") {
                let sid = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if let Some(mut convo) = parse_single_jsonl(&path, &sid, &encoded_name) {
                    convo.size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    convo.last_updated = entry
                        .metadata()
                        .and_then(|m| m.modified())
                        .map(to_unix_millis)
                        .unwrap_or(0);
                    convos.push(convo);
                }
            }
        }
    }

    convos.sort_by(|a, b| b.last_updated.cmp(&a.last_updated));
    convos
}

pub fn parse_single_jsonl(path: &Path, sid: &str, encoded: &str) -> Option<Conversation> {
    let content = fs::read_to_string(path).ok()?;

    let mut message_count = 0u32;
    let mut model = String::new();
    let mut summary_title: Option<String> = None;
    let mut ai_title: Option<String> = None;
    let mut custom_title: Option<String> = None;
    let mut first_user_text: Option<String> = None;
    let mut last_user_text: Option<String> = None;
    let mut last_prompt_text: Option<String> = None;
    let mut cwd: String = String::new();

    for line in content.lines() {
        let record: JsonlRecord = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // capture the first non-empty cwd embedded in the records
        if cwd.is_empty() {
            if let Some(c) = &record.cwd {
                if !c.is_empty() {
                    cwd = c.clone();
                }
            }
        }

        // capture Claude Code's own session name (last prompt). Only keep the
        // last one if multiple exist.
        if let Some(lp) = &record.last_prompt {
            if !lp.is_empty() {
                last_prompt_text = Some(lp.clone());
            }
        }
        // AI-generated title (Claude Code writes ai-title to label sessions).
        if let Some(t) = &record.ai_title {
            if !t.is_empty() {
                ai_title = Some(t.clone());
            }
        }
        // User-set title (Claude Code /rename). Beats everything else.
        if let Some(t) = &record.custom_title {
            if !t.is_empty() {
                custom_title = Some(t.clone());
            }
        }

        match record.typ.as_deref() {
            Some("summary") => {
                if let Some(s) = record.summary {
                    summary_title = Some(s);
                }
            }
            Some("assistant") => {
                message_count += 1;
                if model.is_empty() {
                    if let Some(msg) = record.message {
                        if let Some(m) = msg.model {
                            model = m;
                        }
                    }
                }
            }
            Some("user") => {
                message_count += 1;
                if let Some(msg) = record.message {
                    if let Some(content) = msg.content {
                        let text = extract_text(&content);
                        // skip command/system-prefixed messages
                        if !text.trim_start().starts_with('<') && text.chars().count() > 2 {
                            if first_user_text.is_none() {
                                first_user_text = Some(text.clone());
                            }
                            last_user_text = Some(text);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Title resolution (highest priority first):
    //   1. custom-title — user explicitly renamed via Claude Code /rename.
    //   2. ai-title     — Claude Code's AI-generated session label.
    //   3. summary      — older summary record (some versions write it).
    //   4. lastPrompt   — first line of the last prompt (what `/resume` shows).
    //   5. last user msg — fallback for agent/SDK sessions without lastPrompt.
    //   6. session id   — never show "(无标题)"; use a short id prefix instead.
    let title = custom_title
        .or(ai_title)
        .or(summary_title)
        .or_else(|| last_prompt_text.as_ref().map(|t| first_line_truncated(t, 60)))
        .or_else(|| last_user_text.as_ref().map(|t| first_line_truncated(t, 60)))
        .unwrap_or_else(|| format!("会话 {}", &sid[..sid.len().min(8)]));

    let first_user_preview = first_user_text
        .map(|t| {
            let chars: Vec<char> = t.chars().collect();
            if chars.len() > 120 {
                format!("{}...", chars[..120].iter().collect::<String>())
            } else {
                t
            }
        })
        .unwrap_or_default();

    Some(Conversation {
        id: sid.to_string(),
        title,
        project_encoded: encoded.to_string(),
        model: if model.is_empty() {
            "未知".to_string()
        } else {
            model
        },
        message_count,
        size_bytes: 0,
        first_user_preview,
        last_updated: 0,
        is_archived: false,
        cwd,
    })
}

/// Extract plain text from message.content (string or array).
fn extract_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|item| {
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    item.get("text").and_then(|t| t.as_str()).map(String::from)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

/// Take the first non-empty line of `s` (splitting on \r or \n), strip a
/// leading markdown heading marker, and truncate to `max_chars` characters
/// (UTF-8 safe). Mirrors how Claude Code's `/resume` derives a session title
/// from the last prompt.
fn first_line_truncated(s: &str, max_chars: usize) -> String {
    // Split on any line break; pick the first non-empty trimmed line.
    let first_line = s
        .split(|c| c == '\r' || c == '\n')
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("");
    // Strip a leading markdown heading marker ("#", "##", ...).
    let stripped = first_line.trim_start_matches('#').trim();
    let chars: Vec<char> = stripped.chars().collect();
    if chars.len() > max_chars {
        format!("{}...", chars[..max_chars].iter().collect::<String>())
    } else {
        stripped.to_string()
    }
}

fn to_unix_millis(time: SystemTime) -> i64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
