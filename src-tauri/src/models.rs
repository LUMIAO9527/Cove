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
/// Each tier has two names:
///   - `*_model`:      the raw id (may carry technical suffixes like `[1M]`).
///   - `*_model_name`: the clean display label (cc-switch's `_MODEL_NAME`), or
///                     the raw id when no clean label is configured.
/// The frontend prefers `*_model_name` for the tray label.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub opus_model: String,
    pub sonnet_model: String,
    pub haiku_model: String,
    #[serde(default)]
    pub opus_model_name: String,
    #[serde(default)]
    pub sonnet_model_name: String,
    #[serde(default)]
    pub haiku_model_name: String,
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
