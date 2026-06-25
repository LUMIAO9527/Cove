use serde::{Deserialize, Serialize};

/// A single conversation (session).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String, // session UUID
    pub title: String,
    pub project_encoded: String,
    pub model: String,
    pub message_count: u32,
    pub size_bytes: u64,
    pub first_user_preview: String,
    pub last_updated: i64, // unix millis
    pub is_archived: bool,
    /// Real working directory, read from the jsonl record (precise, no decode ambiguity).
    #[serde(default)]
    pub cwd: String,
}

/// A project (working directory) registered by the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub encoded_name: String,
    pub decoded_path: String,
    pub conversation_count: u32,
    pub total_size_bytes: u64,
    pub last_updated: i64,
    pub orphan_bytes: u64,
    pub conversations: Vec<Conversation>,
    // --- v0.2: manual project registration ---
    pub path: String,        // real working directory (absolute)
    pub name: String,        // user-facing name (alias or dir name)
    pub added_at: i64,       // unix millis when added
}

/// Global model configuration.
///
/// Dynamically discovered from `settings.json`: every `ANTHROPIC_DEFAULT_<X>_MODEL`
/// env var becomes one tier slot. New Claude Code tiers (fable, and anything
/// future) are picked up with zero code changes — no more hardcoded
/// opus/sonnet/haiku.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Discovered tier slots, ordered sonnet → opus → fable → haiku, then
    /// unknown tiers alphabetically. Empty when settings.json has no
    /// ANTHROPIC_DEFAULT_*_MODEL at all (stock Claude Code / fresh install).
    pub tiers: Vec<TierSlot>,
}

/// One model tier slot discovered from settings.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierSlot {
    /// Tier alias key, lowercased as written in the env var name (e.g.
    /// "opus", "sonnet", "fable", "haiku"). The frontend matches this against
    /// the top-level "model" value to decide which row is active.
    pub tier: String,
    /// Raw model id (may carry technical suffixes like `[1M]`).
    pub model: String,
    /// Clean display label (cc-switch's `_MODEL_NAME`), or the raw id when no
    /// clean label is configured. The frontend prefers this for the tray label.
    pub model_name: String,
}

/// The 8 related-data locations for a single SID.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelatedSet {
    pub jsonl_file: Option<String>,
    pub project_subdir: Option<String>,
    pub tasks_dir: Option<String>,
    pub file_history_dir: Option<String>,
    pub telemetry_files: Vec<String>,
    pub session_env_dir: Option<String>,
    pub history_lines: u32,
    pub session_meta_files: Vec<String>,
}

/// A single related-data item presented to the user in the delete selector.
/// label = Chinese description; path = absolute path; size_bytes = on-disk size
/// (0 if unknown/file-level); kind = stable key for grouping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedItem {
    pub kind: String,
    pub label: String,
    pub path: String,
    pub size_bytes: u64,
}

/// Orphan data entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrphanEntry {
    pub sid: String,
    pub location: String,
    pub kind: String,
    pub size_bytes: u64,
    /// Human-readable owner if this orphan can be linked to a known conversation
    /// (e.g. "项目名 · 会话标题"), else empty string.
    #[serde(default)]
    pub belongs_to: String,
}

/// Archive index entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub sid: String,
    pub project_encoded: String,
    pub title: String,
    pub archived_at: i64,
}

// ============================ 会话历史查看（transcript） ============================

/// Full transcript of one session, returned by `get_session_transcript`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTranscript {
    pub session_id: String,
    pub title: String,
    pub model: String,
    pub turns: Vec<TranscriptTurn>,
    /// Real working directory of this session (first non-empty cwd in the
    /// jsonl). Empty string if absent. Used for the detail-view meta line.
    #[serde(default)]
    pub cwd: String,
    /// ISO timestamp of the LAST user/assistant turn ("" if none). Frontend
    /// formats it as a relative time. Cheaper than carrying unix millis and
    /// reusing the string already parsed per-turn.
    #[serde(default)]
    pub last_updated: String,
}

/// One user or assistant turn in a transcript. Skips non-message records
/// (mode/system/summary/etc.) — those are metadata, not conversation turns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptTurn {
    /// "user" | "assistant"
    pub role: String,
    /// ISO timestamp from the record, "" if absent.
    #[serde(default)]
    pub timestamp: String,
    /// Ordered content blocks of this turn.
    pub blocks: Vec<ContentBlock>,
}

/// A single content block within a turn. Preserves block type so the frontend
/// can render text / thinking / tool calls / tool results separately.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ContentBlock {
    /// Plain text (user prompt, or assistant prose).
    Text {
        text: String,
    },
    /// Assistant reasoning (extended thinking).
    Thinking {
        thinking: String,
    },
    /// Assistant invoking a tool. `input` kept as raw JSON for the frontend
    /// to format per-tool (truncated on the client if huge).
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Tool output fed back as a user-side block.
    ToolResult {
        tool_use_id: String,
        text: String,
    },
}
