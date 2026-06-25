import { invoke } from "@tauri-apps/api/core";

/** Which CLI tool a command targets. Mirrors the Rust `ToolKind` enum
 *  (serde `rename_all = "lowercase"`). */
export type ToolName = "claude" | "reasonix";

/** Default tool (legacy behavior — old frontends that never set one). */
export const DEFAULT_TOOL: ToolName = "claude";

export interface Conversation {
    id: string;
    title: string;
    project_encoded: string;
    model: string;
    message_count: number;
    size_bytes: number;
    first_user_preview: string;
    last_updated: number;
    is_archived: boolean;
    /** Real working directory read from the jsonl record. */
    cwd: string;
}

export interface Project {
    encoded_name: string;
    decoded_path: string;
    conversation_count: number;
    total_size_bytes: number;
    last_updated: number;
    orphan_bytes: number;
    conversations: Conversation[];
    // v0.2: manual registration
    path: string;        // real working directory (absolute)
    name: string;        // user-facing name (alias or dir name)
    added_at: number;    // unix millis when added
}

export interface TierSlot {
    /** Tier alias key, lowercased as in the env var name (opus/sonnet/fable/haiku). */
    tier: string;
    /** Raw model id (may carry technical suffixes like `[1M]`). */
    model: string;
    /** Clean display label (cc-switch _MODEL_NAME), falls back to `model`. */
    model_name: string;
}

export interface ModelInfo {
    /** Discovered tier slots (sonnet→opus→fable→haiku, then alpha). */
    tiers: TierSlot[];
}

export interface ModelState {
    /** Raw top-level "model" value: a tier alias OR a direct model id (cc-switch). */
    model: string;
    /** "opus"/"sonnet"/"haiku" when model is a tier alias; "" when it's a direct id. */
    tier: string;
    info: ModelInfo;
}

export interface DeleteResult {
    success: boolean;
    freed_bytes: number;
    removed_paths: string[];
}

export interface ArchiveEntry {
    sid: string;
    project_encoded: string;
    title: string;
    archived_at: number;
}

export interface OrphanEntry {
    sid: string;
    location: string;
    kind: string;
    size_bytes: number;
    belongs_to: string;
}

export interface RelatedItem {
    kind: string;
    label: string;
    path: string;
    size_bytes: number;
}

// --- Session transcript (history viewer) ---
export type ContentBlock =
    | { kind: "Text"; text: string }
    | { kind: "Thinking"; thinking: string }
    | { kind: "ToolUse"; id: string; name: string; input: unknown }
    | { kind: "ToolResult"; tool_use_id: string; text: string };

export interface TranscriptTurn {
    role: string; // "user" | "assistant"
    timestamp: string;
    blocks: ContentBlock[];
}

export interface SessionTranscript {
    session_id: string;
    title: string;
    model: string;
    turns: TranscriptTurn[];
    /** Real working directory (first non-empty cwd in the jsonl), "" if absent. */
    cwd: string;
    /** ISO timestamp of the last turn, "" if absent. */
    last_updated: string;
}

export const api = {
    // --- Projects (user-managed) — per-tool ---
    getProjects: (tool: ToolName) => invoke<Project[]>("get_projects", { tool }),
    addProject: (tool: ToolName, path: string, name?: string) =>
        invoke<Project>("add_project", { tool, path, name: name ?? null }),
    removeProject: (tool: ToolName, path: string) =>
        invoke<boolean>("remove_project", { tool, path }),
    renameProject: (tool: ToolName, path: string, name: string) =>
        invoke<Project>("rename_project", { tool, path, name }),
    getProjectDetail: (tool: ToolName, path: string) =>
        invoke<Conversation[]>("get_project_detail", { tool, path }),
    getLooseConversations: (tool: ToolName) =>
        invoke<Conversation[]>("get_loose_conversations", { tool }),

    // --- Model (Claude-only: tier switcher reads ~/.claude/settings.json) ---
    getModelInfo: () => invoke<ModelInfo>("get_model_info"),
    getModelState: () => invoke<ModelState>("get_model_state"),
    setDefaultTier: (tier: string) => invoke<string>("set_default_tier_cmd", { tier }),

    // --- Default workspace for new chats (loose-tab "新对话" button) ---
    getDefaultWorkspace: () => invoke<string | null>("get_default_workspace"),
    setDefaultWorkspace: (path: string) =>
        invoke<void>("set_default_workspace_cmd", { path }),

    // --- Conversations (delete per-tool; archive Claude-only) ---
    deleteConvo: (tool: ToolName, sid: string, projectEncoded: string) =>
        invoke<DeleteResult>("delete_convo", { tool, sid, projectEncoded }),
    archiveConvo: (tool: ToolName, sid: string, projectEncoded: string) =>
        invoke<void>("archive_convo", { tool, sid, projectEncoded }),

    // --- Archive (per-tool: Claude 8-关联数据归档 / Reasonix sidecar 归档) ---
    restoreConvo: (tool: ToolName, sid: string, projectEncoded: string) =>
        invoke<void>("restore_convo", { tool, sid, projectEncoded }),
    getArchiveIndex: () => invoke<{ entries: ArchiveEntry[] }>("get_archive_index"),
    getArchiveConversations: (tool: ToolName) =>
        invoke<Conversation[]>("get_archive_conversations", { tool }),
    purgeArchivedConvo: (tool: ToolName, sid: string, projectEncoded: string) =>
        invoke<boolean>("purge_archived_convo", { tool, sid, projectEncoded }),

    // --- Cleanup (Claude-only) ---
    scanOrphans: () => invoke<OrphanEntry[]>("scan_orphan_data"),
    deleteOrphan: (location: string) => invoke<boolean>("delete_orphan", { location }),
    deleteAllOrphans: () => invoke<number>("delete_all_orphans"),
    listRelatedFiles: (sid: string, projectEncoded: string) =>
        invoke<RelatedItem[]>("list_related_files", { sid, projectEncoded }),
    deleteRelatedFiles: (sid: string, paths: string[]) =>
        invoke<number>("delete_related_files", { sid, paths }),
    // rename: Claude-only on the backend (appends custom-title to jsonl).
    // Other tools return an error from the backend.
    renameSession: (tool: ToolName, sid: string, projectKey: string, name: string) =>
        invoke<string>("rename_session", { tool, sid, projectEncoded: projectKey, name }),
    getTranscript: (tool: ToolName, sid: string, projectKey: string) =>
        invoke<SessionTranscript>("get_session_transcript", { tool, sid, projectKey }),

    // Tell the backend a native dialog (folder picker) is open so it doesn't
    // treat the OS-induced focus loss as "click outside" and collapse the popup.
    setDialogOpen: (open: boolean) =>
        invoke<void>("set_dialog_open", { open }),

    // --- Tool installation status (for the tool switcher: disable uninstalled tools) ---
    getInstalledTools: () =>
        invoke<Record<string, boolean>>("get_installed_tools"),

    // --- Launch a coding-agent session (per-tool). `projectKey` is the Claude
    //     encoded dir name (Claude) or cwd (Reasonix). sid=None => new session. ---
    openSession: (tool: ToolName, path: string, sid?: string) =>
        invoke<void>("open_session", { tool, path, sid: sid ?? null }),

    // Open a folder in the system file explorer (Windows Explorer).
    openInExplorer: (path: string) =>
        invoke<void>("open_in_explorer", { path }),

    // Open a blank terminal at the user's home dir (for pasting+running an
    // install command copied from the "未安装" page). Decoupled from the copy
    // button on purpose: the user reviews the command before pressing Enter.
    openInstallTerminal: () => invoke<void>("open_install_terminal"),
};
