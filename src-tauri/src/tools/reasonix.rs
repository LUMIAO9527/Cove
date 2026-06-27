//! Reasonix adapter (DeepSeek-native terminal coding agent), v0.53.2+.
//!
//! Verified against the installed package source (dist/cli/chunk-P5SUHDUQ.js),
//! NOT the GitHub main-v2 branch (which differs significantly).
//!
//! Data layout:
//!   - sessions dir: `~/.reasonix/sessions/`  (flat — all sessions in one dir)
//!   - session file: `<name>.jsonl`           (name = stem, no UUID)
//!   - meta sidecar: `<name>.meta.json`       — { workspace, model, totalCostUsd, ... }
//!   - other sidecars: `.events.jsonl`, `.pending.json`, `.plan.json`, `.jsonl.bak`
//!
//! Session ↔ project (cwd) association lives in meta.json's `workspace` field.
//! Reasonix normalizes the path (Win: lowercase drive + forward slashes, e.g.
//! `D:\X` → `d:/x`), so we normalize the same way when filtering.
//!
//! jsonl line shape (one provider message per line):
//!   { v, role:"user|assistant|tool", content, reasoning_content, tool_calls,
//!     tool_call_id, model, ts }
//!
//! CLI:
//!   - new code session: `reasonix code [dir]`
//!   - resume latest in dir: `reasonix code [dir] -r`
//!   - (code mode has NO per-name resume; `--session` only exists on `chat`)

use crate::models::{Conversation, ContentBlock, SessionTranscript, TranscriptTurn};
use crate::paths::reasonix_dir;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// The `.meta.json` sidecar. Only fields we read; unknown keys ignored.
/// `workspace` is the normalized cwd Reasonix records when a session starts.
#[derive(Debug, Default, Deserialize)]
struct SessionMeta {
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default, rename = "totalCostUsd")]
    total_cost_usd: Option<f64>,
}

/// One jsonl line: a Reasonix (OpenAI-compatible) message. Fields are optional
/// because tool/system/assistant/user rows carry different subsets.
#[derive(Debug, Default, Deserialize)]
struct Message {
    #[serde(default)]
    role: Option<String>,
    /// `content` is a plain string for text rows (may be null on tool rows).
    #[serde(default)]
    content: Option<serde_json::Value>,
    #[serde(default, rename = "tool_calls")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(default, rename = "tool_call_id")]
    tool_call_id: Option<String>,
    #[serde(default, rename = "reasoning_content")]
    reasoning_content: Option<String>,
    #[serde(default)]
    model: Option<String>,
    /// ISO timestamp Reasonix stamps on each line.
    #[serde(default)]
    ts: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ToolCall {
    #[serde(default)]
    id: Option<String>,
    #[serde(default, rename = "type")]
    typ: Option<String>,
    #[serde(default)]
    function: Option<ToolFn>,
}

#[derive(Debug, Default, Deserialize)]
struct ToolFn {
    #[serde(default)]
    name: Option<String>,
    /// arguments is a raw JSON string in the OpenAI tool-call format.
    #[serde(default)]
    arguments: Option<String>,
}

/// Loose conversations: all sessions whose workspace is NOT a registered project.
pub fn scan_loose(registered_cwds: &[String]) -> Vec<Conversation> {
    scan_loose_in(&reasonix_dir(), registered_cwds)
}

pub(crate) fn scan_loose_in(home: &Path, registered_cwds: &[String]) -> Vec<Conversation> {
    let mut all = scan_all_in(home);
    all.retain(|c| !cwd_matches_registered(&c.cwd, registered_cwds));
    all
}

/// All sessions for one cwd (a registered project's path).
pub fn conversations_for_path(cwd: &str) -> Vec<Conversation> {
    conversations_for_path_in(&reasonix_dir(), cwd)
}

pub(crate) fn conversations_for_path_in(home: &Path, cwd: &str) -> Vec<Conversation> {
    let want = normalize_workspace(cwd);
    let mut all = scan_all_in(home);
    all.retain(|c| {
        // Sessions with empty workspace are "loose" (never claimed by a project).
        // Sessions with a workspace match only when it normalizes to `want`.
        !c.cwd.is_empty() && normalize_workspace(&c.cwd) == want
    });
    all.sort_by(|a, b| b.last_updated.cmp(&a.last_updated));
    all
}

/// Full transcript of a Reasonix session jsonl.
pub fn parse_transcript(session_path: &PathBuf, sid: &str) -> Option<SessionTranscript> {
    let content = fs::read_to_string(session_path).ok()?;
    let meta = read_meta(&meta_path(session_path));

    let mut turns: Vec<TranscriptTurn> = Vec::new();
    let mut model = meta.model.clone().unwrap_or_default();
    let mut last_ts = String::new();
    let mut last_user_text: Option<String> = None;

    for line in content.lines() {
        let msg: Message = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if let Some(ts) = &msg.ts {
            if !ts.is_empty() {
                last_ts = ts.clone();
            }
        }
        if model.is_empty() {
            if let Some(m) = &msg.model {
                if !m.is_empty() {
                    model = m.clone();
                }
            }
        }

        let role = msg.role.clone().unwrap_or_else(|| "user".to_string());
        let blocks = message_to_blocks(&msg);
        if blocks.is_empty() {
            continue;
        }

        if role == "user" {
            for b in &blocks {
                if let ContentBlock::Text { text } = b {
                    let t = text.trim();
                    if !t.is_empty() && !t.starts_with('<') {
                        last_user_text = Some(text.clone());
                        break;
                    }
                }
            }
        }

        turns.push(TranscriptTurn {
            // "tool" role rows are tool results — group under assistant.
            role: if role == "tool" {
                "assistant".to_string()
            } else {
                role.clone()
            },
            timestamp: msg.ts.clone().unwrap_or_default(),
            blocks,
        });
    }

    let title = last_user_text
        .as_deref()
        .map(|t| first_line_truncated(t, 60))
        .unwrap_or_else(|| format!("会话 {}", &sid[..sid.len().min(20)]));

    let cwd = meta.workspace.clone().unwrap_or_default();

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
        last_updated: last_ts,
    })
}

/// Resolve a Reasonix session path by id (= filename stem).
/// 回退查归档：活跃 sessions 目录找不到时，查 archive/reasonix/<sid>/<sid>.jsonl。
pub fn session_path(sid: &str, _project_key: &str) -> Option<PathBuf> {
    if let Some(p) = session_path_in(&reasonix_dir(), sid) {
        return Some(p);
    }
    // 回退：归档扁平布局 archive/reasonix/<sid>/<sid>.jsonl
    let stem = sid.trim_end_matches(".jsonl");
    let archived = crate::paths::archive_dir()
        .join("reasonix")
        .join(stem)
        .join(format!("{stem}.jsonl"));
    if archived.exists() { Some(archived) } else { None }
}

pub(crate) fn session_path_in(home: &Path, sid: &str) -> Option<PathBuf> {
    let dir = home.join("sessions");
    let stem = sid.trim_end_matches(".jsonl");
    let candidate = dir.join(format!("{stem}.jsonl"));
    if candidate.exists() {
        Some(candidate)
    } else {
        // Fall back: scan for a file whose stem contains the sid.
        fs::read_dir(&dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.extension().map_or(false, |e| e == "jsonl")
                    && p.file_stem()
                        .map(|s| s.to_string_lossy().contains(stem))
                        .unwrap_or(false)
            })
    }
}

/// All sidecar + main paths for a session, used by delete/archive/cleanup.
/// Reasonix writes (verified from chunk-P5SUHDUQ.js sidecar list + metaPath):
///   <stem>.jsonl            (main transcript)
///   <stem>.meta.json        (workspace/model/cost metadata)
///   <stem>.events.jsonl     (kernel event log)
///   <stem>.pending.json     (in-flight state)
///   <stem>.plan.json        (plan store)
///   <stem>.jsonl.bak        (transcript backup)
/// Returns whichever exist on disk.
pub fn session_data_paths(sid: &str) -> Vec<PathBuf> {
    session_data_paths_in(&reasonix_dir(), sid)
}

pub(crate) fn session_data_paths_in(home: &Path, sid: &str) -> Vec<PathBuf> {
    let dir = home.join("sessions");
    let stem = sid.trim_end_matches(".jsonl");
    let candidates = [
        format!("{stem}.jsonl"),
        format!("{stem}.meta.json"),
        format!("{stem}.events.jsonl"),
        format!("{stem}.pending.json"),
        format!("{stem}.plan.json"),
        format!("{stem}.jsonl.bak"),
    ];
    candidates
        .iter()
        .map(|n| dir.join(n))
        .filter(|p| p.exists())
        .collect()
}

// ---- helpers ----

fn scan_all_in(home: &Path) -> Vec<Conversation> {
    let dir = home.join("sessions");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut convos: Vec<Conversation> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            // main session files end with .jsonl but NOT .events.jsonl (that's a sidecar)
            p.extension().map_or(false, |e| e == "jsonl")
                && !p.to_string_lossy().ends_with(".events.jsonl")
        })
        .filter_map(|p| parse_conversation(&p))
        .collect();
    convos.sort_by(|a, b| b.last_updated.cmp(&a.last_updated));
    convos
}

fn parse_conversation(path: &Path) -> Option<Conversation> {
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let meta = read_meta(&meta_path(path));

    let metadata = fs::metadata(path).ok()?;
    let size_bytes = metadata.len();
    let last_updated = metadata
        .modified()
        .map(to_unix_millis)
        .unwrap_or(0);

    // cwd = workspace from meta. Sessions with no workspace are "loose".
    let cwd = meta.workspace.clone().unwrap_or_default();
    let model = meta
        .model
        .clone()
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "未知".to_string());

    // Title + preview: first user text from the jsonl (meta has no title field).
    let first_user = first_user_text_from_jsonl(path).unwrap_or_default();
    let title = if first_user.trim().is_empty() {
        format!("会话 {}", &stem[..stem.len().min(20)])
    } else {
        first_line_truncated(&first_user, 60)
    };
    let first_user_preview = truncate_chars(&first_user, 120);

    let message_count = count_message_lines(path);

    Some(Conversation {
        id: stem,
        title,
        project_encoded: cwd.clone(),
        model,
        message_count,
        size_bytes,
        first_user_preview,
        last_updated,
        is_archived: false,
        cwd,
    })
}

/// Path of a session's `.meta.json` sidecar. Reasonix names it
/// `<stem>.meta.json` (NOT `<stem>.jsonl.meta.json`).
fn meta_path(session_path: &Path) -> PathBuf {
    let stem = session_path.file_stem().map(|s| s.to_os_string()).unwrap_or_default();
    // session_path = .../<stem>.jsonl ; we want .../<stem>.meta.json
    let parent = session_path.parent().unwrap_or_else(|| Path::new(""));
    let mut name = stem;
    name.push(".meta.json");
    parent.join(name)
}

fn read_meta(meta_path: &Path) -> SessionMeta {
    fs::read_to_string(meta_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn message_to_blocks(msg: &Message) -> Vec<ContentBlock> {
    let mut blocks = Vec::new();

    // reasoning_content (DeepSeek reasoning) -> Thinking block, first.
    if let Some(r) = &msg.reasoning_content {
        if !r.trim().is_empty() {
            blocks.push(ContentBlock::Thinking {
                thinking: r.clone(),
            });
        }
    }

    // tool_call_id present => tool RESULT row (role: "tool").
    if let Some(id) = &msg.tool_call_id {
        let text = content_to_string(&msg.content);
        if !text.is_empty() {
            blocks.push(ContentBlock::ToolResult {
                tool_use_id: id.clone(),
                text,
            });
        }
        return blocks;
    }

    // tool_calls present => assistant issued tool calls.
    if let Some(calls) = &msg.tool_calls {
        for call in calls {
            let name = call
                .function
                .as_ref()
                .and_then(|f| f.name.clone())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let id = call.id.clone().unwrap_or_default();
            let input = call
                .function
                .as_ref()
                .and_then(|f| f.arguments.as_deref())
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::Value::Null);
            blocks.push(ContentBlock::ToolUse { id, name, input });
        }
    }

    // content text (user prompt / assistant reply).
    let text = content_to_string(&msg.content);
    if !text.trim().is_empty() {
        blocks.push(ContentBlock::Text { text });
    }

    blocks
}

fn content_to_string(content: &Option<serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) if !other.is_null() => other.to_string(),
        _ => String::new(),
    }
}

fn first_user_text_from_jsonl(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let msg: Message = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if msg.role.as_deref() != Some("user") {
            continue;
        }
        let text = content_to_string(&msg.content);
        let t = text.trim();
        if !t.is_empty() && !t.starts_with('<') {
            return Some(text);
        }
    }
    None
}

/// Count non-empty jsonl lines (≈ message count, matching `reasonix sessions`).
fn count_message_lines(path: &Path) -> u32 {
    fs::read_to_string(path)
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count() as u32)
        .unwrap_or(0)
}

fn cwd_matches_registered(cwd: &str, registered: &[String]) -> bool {
    if cwd.is_empty() {
        return false; // loose sessions are never "registered"
    }
    let want = normalize_workspace(cwd);
    registered
        .iter()
        .any(|r| normalize_workspace(r) == want)
}

/// Normalize a path the way Reasonix does (chunk-P5SUHDUQ.js:169):
/// Win: resolve + backslashes→forward + lowercase drive letter. Other: posix resolve.
fn normalize_workspace(p: &str) -> String {
    if p.is_empty() {
        return String::new();
    }
    #[cfg(target_os = "windows")]
    {
        // reasonix uses node's path.resolve on win32. We approximate: normalize
        // separators, strip verbatim prefix, lowercase the drive letter.
        let mut s = p.replace('\\', "/");
        // strip \\?\ verbatim prefix
        if let Some(stripped) = s.strip_prefix(r"//?/") {
            s = stripped.to_string();
        }
        // lowercase drive letter: "D:/" -> "d:/"
        let bytes = s.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
            let mut arr = s.into_bytes();
            arr[0] = arr[0].to_ascii_lowercase();
            return String::from_utf8(arr).unwrap_or_default();
        }
        s
    }
    #[cfg(not(target_os = "windows"))]
    {
        p.to_string()
    }
}

fn first_line_truncated(s: &str, max_chars: usize) -> String {
    let first_line = s
        .split(|c| c == '\r' || c == '\n')
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let stripped = first_line.trim_start_matches('#').trim();
    truncate_chars(stripped, max_chars)
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > max_chars {
        format!("{}...", chars[..max_chars].iter().collect::<String>())
    } else {
        s.to_string()
    }
}

fn to_unix_millis(time: std::time::SystemTime) -> i64 {
    time.duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp_reasonix_home() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cove-reasonix-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_session(home: &Path, name: &str, meta: Option<&str>, lines: &[&str]) -> PathBuf {
        let sess_dir = home.join("sessions");
        fs::create_dir_all(&sess_dir).unwrap();
        let jsonl = sess_dir.join(format!("{name}.jsonl"));
        {
            let mut f = fs::File::create(&jsonl).unwrap();
            for l in lines {
                writeln!(f, "{l}").unwrap();
            }
        }
        if let Some(meta_content) = meta {
            let mut f = fs::File::create(sess_dir.join(format!("{name}.meta.json"))).unwrap();
            write!(f, "{meta_content}").unwrap();
        }
        jsonl
    }

    #[test]
    fn test_conversations_for_path_filters_by_workspace() {
        let home = tmp_reasonix_home();
        let meta_a = r#"{"workspace":"d:/proj/applab/cove","model":"deepseek-v4-flash"}"#;
        write_session(
            &home,
            "code-Cove-20260625160000",
            Some(meta_a),
            &[r#"{"v":1,"role":"user","content":"hi","ts":"2026-06-25T16:00:00Z"}"#],
        );
        // different workspace
        let meta_b = r#"{"workspace":"d:/other","model":"deepseek-v4-flash"}"#;
        write_session(
            &home,
            "code-Other-20260625170000",
            Some(meta_b),
            &[r#"{"v":1,"role":"user","content":"x","ts":"2026-06-25T17:00:00Z"}"#],
        );

        // Win path "D:\proj\applab\cove" normalizes to "d:/proj/applab/cove"
        let convos = conversations_for_path_in(&home, "D:\\proj\\applab\\cove");
        assert_eq!(convos.len(), 1);
        assert_eq!(convos[0].cwd, "d:/proj/applab/cove");
        assert_eq!(convos[0].id, "code-Cove-20260625160000");
    }

    #[test]
    fn test_scan_loose_excludes_registered_and_includes_no_workspace() {
        let home = tmp_reasonix_home();
        // registered workspace
        let meta_reg = r#"{"workspace":"d:/reg/proj"}"#;
        write_session(
            &home,
            "code-Reg-2026062501",
            Some(meta_reg),
            &[r#"{"v":1,"role":"user","content":"do"}"#],
        );
        // loose: no workspace at all
        write_session(
            &home,
            "default-2026062502",
            None,
            &[r#"{"v":1,"role":"user","content":"loose one"}"#],
        );

        let loose = scan_loose_in(&home, &["D:\\reg\\proj".to_string()]);
        assert_eq!(loose.len(), 1, "registered excluded, no-workspace included");
        assert_eq!(loose[0].id, "default-2026062502");
    }

    #[test]
    fn test_parse_transcript_maps_blocks() {
        let home = tmp_reasonix_home();
        let name = "code-Cove-2026062503";
        let lines = [
            r#"{"v":1,"role":"user","content":"fix the bug","ts":"2026-06-25T09:02:03Z"}"#,
            r#"{"v":1,"role":"assistant","reasoning_content":"thinking","content":"","ts":"2026-06-25T09:02:04Z"}"#,
            r#"{"v":1,"role":"assistant","content":"","tool_calls":[{"id":"call_1","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"a.ts\"}"}}],"ts":"2026-06-25T09:02:05Z"}"#,
            r#"{"v":1,"role":"tool","tool_call_id":"call_1","content":"file text","ts":"2026-06-25T09:02:06Z"}"#,
            r#"{"v":1,"role":"assistant","model":"deepseek-v4-flash","content":"done","ts":"2026-06-25T09:02:07Z"}"#,
        ];
        let path = write_session(&home, name, None, &lines);

        let t = parse_transcript(&path, name).expect("parse ok");
        assert_eq!(t.model, "deepseek-v4-flash");
        assert_eq!(t.title, "fix the bug");
        assert_eq!(t.last_updated, "2026-06-25T09:02:07Z");
        assert_eq!(t.turns.len(), 5);
        assert!(matches!(t.turns[0].blocks[0], ContentBlock::Text { .. }));
        assert!(t.turns[1].blocks.iter().any(|b| matches!(b, ContentBlock::Thinking { .. })));
        assert!(matches!(t.turns[2].blocks[0], ContentBlock::ToolUse { .. }));
        assert!(matches!(t.turns[3].blocks[0], ContentBlock::ToolResult { .. }));
        assert!(t.turns[4].blocks.iter().any(|b| matches!(b, ContentBlock::Text { .. })));
    }

    #[test]
    fn test_session_data_paths_finds_sidecars() {
        let home = tmp_reasonix_home();
        let sess_dir = home.join("sessions");
        fs::create_dir_all(&sess_dir).unwrap();
        fs::write(sess_dir.join("x.jsonl"), "data").unwrap();
        fs::write(sess_dir.join("x.meta.json"), "{}").unwrap();
        fs::write(sess_dir.join("x.events.jsonl"), "[]").unwrap();

        let paths = session_data_paths_in(&home, "x");
        assert_eq!(paths.len(), 3);
        assert!(paths.iter().any(|p| p.ends_with("x.jsonl")));
        assert!(paths.iter().any(|p| p.ends_with("x.meta.json")));
        assert!(paths.iter().any(|p| p.ends_with("x.events.jsonl")));
    }

    #[test]
    fn test_normalize_workspace_windows() {
        assert_eq!(normalize_workspace("D:\\proj\\Cove"), "d:/proj/Cove");
        assert_eq!(normalize_workspace(r"\\?\D:\x"), "d:/x");
        assert_eq!(normalize_workspace("C:/users/a"), "c:/users/a");
        assert_eq!(normalize_workspace(""), "");
    }
}
