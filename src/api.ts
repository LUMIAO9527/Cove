import { invoke } from "@tauri-apps/api/core";

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
    // --- Projects (user-managed) ---
    getProjects: () => invoke<Project[]>("get_projects"),
    addProject: (path: string, name?: string) =>
        invoke<Project>("add_project", { path, name: name ?? null }),
    removeProject: (path: string) => invoke<boolean>("remove_project", { path }),
    renameProject: (path: string, name: string) =>
        invoke<Project>("rename_project", { path, name }),
    getProjectDetail: (path: string) => invoke<Conversation[]>("get_project_detail", { path }),
    getLooseConversations: () => invoke<Conversation[]>("get_loose_conversations"),

    getModelInfo: () => invoke<ModelInfo>("get_model_info"),
    getModelState: () => invoke<ModelState>("get_model_state"),
    setDefaultTier: (tier: string) => invoke<string>("set_default_tier_cmd", { tier }),

    // --- Default workspace for new chats (loose-tab "新对话" button) ---
    getDefaultWorkspace: () => invoke<string | null>("get_default_workspace"),
    setDefaultWorkspace: (path: string) =>
        invoke<void>("set_default_workspace_cmd", { path }),

    // --- Conversations ---
    deleteConvo: (sid: string, projectEncoded: string) =>
        invoke<DeleteResult>("delete_convo", { sid, projectEncoded }),
    archiveConvo: (sid: string, projectEncoded: string) =>
        invoke<void>("archive_convo", { sid, projectEncoded }),

    // --- Archive ---
    restoreConvo: (sid: string, projectEncoded: string) =>
        invoke<void>("restore_convo", { sid, projectEncoded }),
    getArchiveIndex: () => invoke<{ entries: ArchiveEntry[] }>("get_archive_index"),
    getArchiveConversations: () => invoke<Conversation[]>("get_archive_conversations"),
    purgeArchivedConvo: (sid: string, projectEncoded: string) =>
        invoke<boolean>("purge_archived_convo", { sid, projectEncoded }),

    // --- Cleanup ---
    scanOrphans: () => invoke<OrphanEntry[]>("scan_orphan_data"),
    deleteOrphan: (location: string) => invoke<boolean>("delete_orphan", { location }),
    deleteAllOrphans: () => invoke<number>("delete_all_orphans"),
    listRelatedFiles: (sid: string, projectEncoded: string) =>
        invoke<RelatedItem[]>("list_related_files", { sid, projectEncoded }),
    deleteRelatedFiles: (sid: string, paths: string[]) =>
        invoke<number>("delete_related_files", { sid, paths }),
    renameSession: (sid: string, projectEncoded: string, name: string) =>
        invoke<string>("rename_session", { sid, projectEncoded, name }),
    getTranscript: (sid: string, projectEncoded: string) =>
        invoke<SessionTranscript>("get_session_transcript", { sid, projectEncoded }),

    // Tell the backend a native dialog (folder picker) is open so it doesn't
    // treat the OS-induced focus loss as "click outside" and collapse the popup.
    setDialogOpen: (open: boolean) =>
        invoke<void>("set_dialog_open", { open }),

    // --- Launch Claude Code (real working path + optional sid) ---
    openClaudeSession: (path: string, sid?: string) =>
        invoke<void>("open_claude_session", { path, sid: sid ?? null }),

    // Open a folder in the system file explorer (Windows Explorer).
    openInExplorer: (path: string) =>
        invoke<void>("open_in_explorer", { path }),
};
