//! Full-conversation transcript parsing for the session-history viewer.
//!
//! Unlike `scan.rs` (which only extracts a title/preview/count per session),
//! this module reads every `user` / `assistant` record of a single jsonl and
//! preserves the content-block structure so the frontend can render text,
//! thinking, tool calls, and tool results separately.

use crate::models::{ContentBlock, SessionTranscript, TranscriptTurn};
use serde::Deserialize;
use std::fs;

/// Partial deserialization of a jsonl line — only the fields we need.
#[derive(Debug, Deserialize)]
struct Record {
    #[serde(rename = "type")]
    typ: Option<String>,
    message: Option<Message>,
    /// ISO timestamp present on user/assistant records.
    #[serde(default)]
    timestamp: Option<String>,
    // Title-bearing records (used to resolve the transcript title).
    #[serde(rename = "customTitle", default)]
    custom_title: Option<String>,
    #[serde(rename = "aiTitle", default)]
    ai_title: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(rename = "lastPrompt", default)]
    last_prompt: Option<String>,
    /// Real working directory embedded in user/assistant records.
    #[serde(default)]
    cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Message {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    model: Option<String>,
    /// Content is a plain string (user) or an array of blocks (assistant).
    #[serde(default)]
    content: Option<serde_json::Value>,
}

/// Parse one session's jsonl into a transcript. Returns None if the file
/// can't be read; returns a transcript with empty turns if it parses but has
/// no user/assistant records.
pub fn parse(path: &std::path::Path, sid: &str) -> Option<SessionTranscript> {
    let content = fs::read_to_string(path).ok()?;

    let mut turns: Vec<TranscriptTurn> = Vec::new();
    let mut model = String::new();
    let mut custom_title: Option<String> = None;
    let mut ai_title: Option<String> = None;
    let mut summary_title: Option<String> = None;
    let mut last_prompt_text: Option<String> = None;
    let mut cwd = String::new();
    let mut last_updated = String::new();

    for line in content.lines() {
        let record: Record = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if let Some(t) = &record.custom_title {
            if !t.is_empty() {
                custom_title = Some(t.clone());
            }
        }
        if let Some(t) = &record.ai_title {
            if !t.is_empty() {
                ai_title = Some(t.clone());
            }
        }
        if let Some(s) = &record.summary {
            if !s.is_empty() {
                summary_title = Some(s.clone());
            }
        }
        if let Some(lp) = &record.last_prompt {
            if !lp.is_empty() {
                last_prompt_text = Some(lp.clone());
            }
        }

        // Capture the first non-empty cwd (scan.rs does the same — a session
        // doesn't change directories mid-conversation).
        if cwd.is_empty() {
            if let Some(c) = &record.cwd {
                if !c.is_empty() {
                    cwd = c.clone();
                }
            }
        }

        let typ = record.typ.as_deref();
        if typ != Some("user") && typ != Some("assistant") {
            continue;
        }

        let msg = match record.message {
            Some(m) => m,
            None => continue,
        };

        // Capture the model from the first assistant record that carries one.
        if model.is_empty() {
            if let Some(m) = msg.model {
                if !m.is_empty() {
                    model = m;
                }
            }
        }

        let role = msg
            .role
            .clone()
            .unwrap_or_else(|| typ.unwrap_or("").to_string());
        let blocks = match msg.content {
            None => Vec::new(),
            Some(serde_json::Value::String(s)) => {
                // User content can be a plain string. Skip tool-result-shaped
                // noise: a bare string is always a real user prompt.
                if s.trim().is_empty() {
                    Vec::new()
                } else {
                    vec![ContentBlock::Text { text: s }]
                }
            }
            Some(serde_json::Value::Array(arr)) => parse_blocks(&arr),
            Some(_) => Vec::new(),
        };

        if blocks.is_empty() {
            continue;
        }

        // Track the most recent turn timestamp for the detail-view meta line.
        // Records are appended in file order, which is chronological for
        // Claude Code jsonl, so the last non-empty one wins.
        if let Some(ts) = &record.timestamp {
            if !ts.is_empty() {
                last_updated = ts.clone();
            }
        }

        turns.push(TranscriptTurn {
            role,
            timestamp: record.timestamp.clone().unwrap_or_default(),
            blocks,
        });
    }

    let title = custom_title
        .or(ai_title)
        .or(summary_title)
        .or_else(|| last_prompt_text.as_ref().map(|t| first_line_truncated(t, 60)))
        .unwrap_or_else(|| format!("会话 {}", &sid[..sid.len().min(8)]));

    Some(SessionTranscript {
        session_id: sid.to_string(),
        title,
        model: if model.is_empty() {
            "未知".to_string()
        } else {
            model
        },
        turns,
        cwd,
        last_updated,
    })
}

/// Convert an array of content blocks into typed `ContentBlock`s. Unknown
/// block types are dropped (defensive — only text/thinking/tool_use/tool_result
/// are expected, but new types may appear in future Claude Code versions).
fn parse_blocks(arr: &[serde_json::Value]) -> Vec<ContentBlock> {
    arr.iter()
        .filter_map(|item| {
            let kind = item.get("type").and_then(|t| t.as_str())?;
            match kind {
                "text" => {
                    let text = item
                        .get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    if text.is_empty() {
                        None
                    } else {
                        Some(ContentBlock::Text { text })
                    }
                }
                "thinking" => {
                    let thinking = item
                        .get("thinking")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    if thinking.is_empty() {
                        None
                    } else {
                        Some(ContentBlock::Thinking { thinking })
                    }
                }
                "tool_use" => {
                    let id = item
                        .get("id")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = item
                        .get("name")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input = item
                        .get("input")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    if name.is_empty() {
                        None
                    } else {
                        Some(ContentBlock::ToolUse { id, name, input })
                    }
                }
                "tool_result" => {
                    let tool_use_id = item
                        .get("tool_use_id")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    // tool_result.content is a string OR an array of text blocks.
                    let text = match item.get("content") {
                        Some(serde_json::Value::String(s)) => s.clone(),
                        Some(serde_json::Value::Array(blocks)) => blocks
                            .iter()
                            .filter_map(|b| {
                                if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                                    b.get("text").and_then(|t| t.as_str()).map(String::from)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                        _ => String::new(),
                    };
                    if text.is_empty() {
                        None
                    } else {
                        Some(ContentBlock::ToolResult { tool_use_id, text })
                    }
                }
                _ => None, // unknown block type — skip defensively
            }
        })
        .collect()
}

/// First non-empty line, leading markdown `#` stripped, truncated to max_chars
/// (UTF-8 safe). Mirrors scan.rs's helper for consistency.
fn first_line_truncated(s: &str, max_chars: usize) -> String {
    let first_line = s
        .split(|c| c == '\r' || c == '\n')
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let stripped = first_line.trim_start_matches('#').trim();
    let chars: Vec<char> = stripped.chars().collect();
    if chars.len() > max_chars {
        format!("{}...", chars[..max_chars].iter().collect::<String>())
    } else {
        stripped.to_string()
    }
}
