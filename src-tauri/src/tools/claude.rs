//! Claude Code adapter — wraps the existing scan.rs / transcript.rs into the
//! unified per-tool signatures.
//!
//! Claude is the legacy tool: its project_key is the *encoded* project dir name
//! (e.g. `D--Programs-AppLab`), NOT the cwd. This is why the other tools take a
//! cwd as project_key while Claude takes an encoded name — keep them distinct.

use crate::models::{Conversation, SessionTranscript};
use crate::paths::{claude_dir, encode_project_path};
use crate::scan::{conversations_for_path as scan_for_path, scan_all_conversations};
use crate::transcript;
use std::path::PathBuf;

/// Loose conversations for Claude: all sessions under ~/.claude/projects/ minus
/// those whose encoded dir matches a registered project path.
pub fn scan_loose(registered_cwds: &[String]) -> Vec<Conversation> {
    use std::collections::HashSet;
    let root = claude_dir();
    let mut all = scan_all_conversations(&root);
    let registered: HashSet<String> = registered_cwds
        .iter()
        .map(|p| encode_project_path(p))
        .collect();
    all.retain(|c| !registered.contains(&c.project_encoded));
    all
}

/// Conversations for one registered project (by its real cwd).
pub fn conversations_for_path(cwd: &str) -> Vec<Conversation> {
    scan_for_path(cwd)
}

/// Full transcript of a Claude session jsonl.
pub fn parse_transcript(session_path: &PathBuf, sid: &str) -> Option<SessionTranscript> {
    if !session_path.exists() {
        return None;
    }
    transcript::parse(session_path, sid)
}

/// Resolve a Claude session jsonl path: ~/.claude/projects/<encoded>/<sid>.jsonl.
/// `project_key` is the encoded dir name (NOT cwd — use encode_project_path on
/// the real cwd before calling if you only have that).
pub fn session_path(sid: &str, project_key: &str) -> Option<PathBuf> {
    let p = claude_dir()
        .join("projects")
        .join(project_key)
        .join(format!("{sid}.jsonl"));
    if p.exists() {
        Some(p)
    } else {
        None
    }
}
