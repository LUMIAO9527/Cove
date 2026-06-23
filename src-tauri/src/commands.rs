use crate::archive::{
    archive_conversation as do_archive, list_archived, list_archived_conversations, purge_archived,
    restore_conversation as do_restore, ArchiveIndex,
};
use crate::cleanup::{delete_conversation, scan_orphans, DeleteResult, list_related, delete_paths};
use crate::models::{Conversation, ModelInfo, OrphanEntry, Project, RelatedItem, SessionTranscript};
use crate::paths::{archive_dir, claude_dir, encode_project_path};
use crate::projects_config::{self, ProjectEntry};
use crate::scan::conversations_for_path;
use crate::settings::{read_model_info, read_raw_model, is_tier_alias, set_default_tier, default_workspace, set_default_workspace};
use crate::transcript;
use tauri::Manager;

// ---------------------------------------------------------------------------
// Projects (user-managed list)
// ---------------------------------------------------------------------------

/// List all registered projects, each enriched with live conversation stats.
#[tauri::command]
pub fn get_projects() -> Vec<Project> {
    let cfg = projects_config::load();
    let mut out: Vec<Project> = cfg
        .projects
        .iter()
        .map(|e| entry_to_project(e))
        .collect();
    // most recently added first (config already keeps newest first, but sort defensively)
    out.sort_by(|a, b| b.added_at.cmp(&a.added_at));
    out
}

/// Register a new project by its real working directory.
#[tauri::command]
pub fn add_project(path: String, name: Option<String>) -> Result<Project, String> {
    let entry = projects_config::add(&path, name)?;
    Ok(entry_to_project(&entry))
}

/// Remove a project from the list (keeps all disk data).
#[tauri::command]
pub fn remove_project(path: String) -> bool {
    projects_config::remove(&path)
}

/// Rename a project's alias (the display name). An empty/whitespace name
/// resets it to the directory name. `added_at` is preserved.
#[tauri::command]
pub fn rename_project(path: String, name: String) -> Result<Project, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("名称不能为空".to_string());
    }
    if trimmed.chars().count() > 80 {
        return Err("名称过长（上限 80 字符）".to_string());
    }
    let entry = projects_config::rename(&path, Some(trimmed.to_string()))?;
    Ok(entry_to_project(&entry))
}

/// Detail view: conversations for a single project (precise, encode-based).
#[tauri::command]
pub fn get_project_detail(path: String) -> Vec<Conversation> {
    conversations_for_path(&path)
}

/// Loose conversations: ALL sessions minus those belonging to registered projects.
/// Used by the "对话" tab (scattered conversations like Codex).
#[tauri::command]
pub fn get_loose_conversations() -> Vec<Conversation> {
    use std::collections::HashSet;
    let root = claude_dir();
    let mut all = crate::scan::scan_all_conversations(&root);
    // collect encoded names of registered projects
    let registered: HashSet<String> = projects_config::load()
        .projects
        .iter()
        .map(|e| encode_project_path(&e.path))
        .collect();
    all.retain(|c| !registered.contains(&c.project_encoded));
    all
}

/// Build a Project (with live stats) from a config entry.
fn entry_to_project(e: &ProjectEntry) -> Project {
    let encoded = encode_project_path(&e.path);
    let convos = conversations_for_path(&e.path);
    let total_size: u64 = convos.iter().map(|c| c.size_bytes).sum();
    let last_updated = convos.iter().map(|c| c.last_updated).max().unwrap_or(0);

    Project {
        encoded_name: encoded.clone(),
        decoded_path: e.path.clone(),
        conversation_count: convos.len() as u32,
        total_size_bytes: total_size,
        last_updated,
        orphan_bytes: 0,
        conversations: convos,
        path: e.path.clone(),
        name: e
            .name
            .clone()
            .unwrap_or_else(|| dir_name_from(&e.path)),
        added_at: e.added_at,
    }
}

fn dir_name_from(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

// ---------------------------------------------------------------------------
// Existing operations (unchanged behavior)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_model_info() -> ModelInfo {
    read_model_info()
}

#[tauri::command]
pub fn delete_convo(sid: String, project_encoded: String) -> DeleteResult {
    delete_conversation(&sid, &project_encoded, &claude_dir())
}

/// 归档一个会话。返回 Ok(()) 全部关联数据归档成功；Err 含失败项描述（前端 toast）。
#[tauri::command]
pub fn archive_convo(sid: String, project_encoded: String) -> Result<(), String> {
    do_archive(&sid, &project_encoded, &claude_dir(), &archive_dir())
}

/// 恢复一个归档会话。返回 Ok(()) 全部数据还原成功；Err 含失败项。
#[tauri::command]
pub fn restore_convo(sid: String, project_encoded: String) -> Result<(), String> {
    do_restore(&sid, &project_encoded, &claude_dir(), &archive_dir())
}

#[tauri::command]
pub fn get_archive_index() -> ArchiveIndex {
    list_archived(&archive_dir())
}

/// 归档区所有会话，解析成完整 Conversation（真实标题/模型/消息数/大小/cwd），
/// 供归档页用和会话列表同一套卡片 + 信息浮层展示。
#[tauri::command]
pub fn get_archive_conversations() -> Vec<Conversation> {
    list_archived_conversations(&archive_dir())
}

#[tauri::command]
pub fn purge_archived_convo(sid: String, project_encoded: String) -> bool {
    purge_archived(&sid, &project_encoded, &archive_dir())
}

/// 一次性迁移：清空归档区（v0.4.26 前的旧结构被 P0 #1 bug 销毁过，数据
/// 不完整无法找回）。由 lib.rs setup 在 release 首启时调，靠 .archive-v2
/// marker 文件避免重复执行。返回 Ok(()) 表示已清空或本就已空。
#[tauri::command]
pub fn clear_archive_legacy() -> Result<(), String> {
    crate::archive::clear_all(&archive_dir())
}

#[tauri::command]
pub fn scan_orphan_data() -> Vec<OrphanEntry> {
    scan_orphans(&claude_dir())
}

/// 删除单个孤儿数据。校验路径必须在 claude_dir 之下（防止前端传入任意路径
/// 误删用户其他文件）。返回 true 表示删除成功。
#[tauri::command]
pub fn delete_orphan(location: String) -> bool {
    if !is_path_within_claude_dir(&location) {
        return false;
    }
    let path = std::path::PathBuf::from(&location);
    let r = if path.is_dir() {
        std::fs::remove_dir_all(&path)
    } else {
        std::fs::remove_file(&path)
    };
    r.is_ok()
}

/// 校验 `location` 是 claude_dir() 之下的路径（canonicalize 后 startswith 检查），
/// 且不等于 claude_dir 本身。用于 delete_orphan 的边界防御。
/// 路径无法 canonicalize（不存在）时返回 false。
fn is_path_within_claude_dir(location: &str) -> bool {
    let candidate = match std::fs::canonicalize(location) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let root = match std::fs::canonicalize(claude_dir()) {
        Ok(p) => p,
        Err(_) => return false,
    };
    // 必须严格在 root 之下，不能等于 root 本身（防止删整个 .claude）。
    candidate != root && candidate.starts_with(&root)
}

#[tauri::command]
pub fn delete_all_orphans() -> u32 {
    let orphans = scan_orphans(&claude_dir());
    let mut count = 0u32;
    for o in &orphans {
        if !is_path_within_claude_dir(&o.location) {
            continue; // 防御：跳过任何越界路径
        }
        let path = std::path::PathBuf::from(&o.location);
        let r = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        if r.is_ok() {
            count += 1;
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Launch Claude Code
// ---------------------------------------------------------------------------

/// Launch a Claude Code session in `path`.
/// - `sid` None / empty => new session (`claude`)
/// - `sid` Some        => resume (`claude --resume <sid>`)
///
/// Spawns an independent terminal window (Windows Terminal, fallback cmd),
/// detached via `.spawn()` so the Tauri main process never blocks.
#[tauri::command]
pub fn open_claude_session(path: String, sid: Option<String>) -> Result<(), String> {
    let dir = std::path::PathBuf::from(&path);
    // Validate directory exists; fallback to home if somehow missing.
    let dir = if dir.is_dir() {
        dir
    } else {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .map_err(|_| "无法定位用户主目录".to_string())?;
        std::path::PathBuf::from(home)
    };
    let claude_cmd = match &sid {
        Some(s) if !s.is_empty() => {
            // sid 直接拼进 shell 命令（claude --resume <sid>），必须校验是合法
            // 会话 ID（UUID 8-4-4-4-12 十六进制），否则含 shell 元字符（& | > 等）
            // 的伪造 sid 会触发命令注入。正常 sid 来自 jsonl 文件名，必为 UUID；
            // 这里挡的是 IPC 被直接构造调用的情况。评审 P2 #8 修复。
            if !is_valid_session_id(s) {
                return Err(format!("非法会话 ID: {}", s));
            }
            format!("claude --resume {}", s)
        }
        _ => "claude".to_string(),
    };
    spawn_terminal(&dir, &claude_cmd)
}

/// 校验字符串是合法的 Claude Code 会话 ID（UUID v4 格式：8-4-4-4-12
/// 十六进制，全小写或全大写）。用于 open_claude_session 防 shell 注入。
fn is_valid_session_id(s: &str) -> bool {
    // 长度 36（32 hex + 4 dash），dash 位置固定在 8/13/18/23。
    if s.len() != 36 {
        return false;
    }
    let bytes = s.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        match i {
            8 | 13 | 18 | 23 => {
                if *b != b'-' {
                    return false;
                }
            }
            _ => {
                if !b.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::is_valid_session_id;

    #[test]
    fn test_valid_session_id_accepted() {
        assert!(is_valid_session_id("aaaa1111-2222-3333-4444-555555555555"));
        assert!(is_valid_session_id("ABCDEF12-3456-7890-ABCD-EF1234567890"));
        assert!(is_valid_session_id("00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn test_shell_injection_rejected() {
        // 评审 P2 #8：含 shell 元字符的伪造 sid 必须被拒绝。
        assert!(!is_valid_session_id("foo & echo pwned"));
        assert!(!is_valid_session_id("a;rm -rf /"));
        assert!(!is_valid_session_id("x | calc"));
        assert!(!is_valid_session_id("a\"b"));
        assert!(!is_valid_session_id("$(whoami)"));
        assert!(!is_valid_session_id(""));
    }

    #[test]
    fn test_malformed_uuid_rejected() {
        assert!(!is_valid_session_id("aaaa1111-2222-3333-4444")); // 太短
        assert!(!is_valid_session_id("aaaa1111-2222-3333-4444-555555555555-extra")); // 太长
        assert!(!is_valid_session_id("aaaa1111z2222-3333-4444-555555555555")); // dash 位置错
        assert!(!is_valid_session_id("aaaa1111-2222-3333-4444-55555555555z")); // 非 hex
    }
}

/// Open a folder in the system file explorer (Windows Explorer). Used by the
/// project detail page's "打开文件夹" button. Spawns explorer.exe detached so
/// it never blocks the Tauri main process. No-op (returns Err) if the path
/// doesn't exist, so the frontend can surface a toast.
#[tauri::command]
pub fn open_in_explorer(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("路径不存在: {}", path));
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("无法打开资源管理器: {e}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        // Non-Windows fallback: xdg-open / open. Kept for completeness; Cove is
        // Windows-only in practice.
        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        std::process::Command::new(opener)
            .arg(&path)
            .spawn()
            .map_err(|e| format!("无法打开文件管理器: {e}"))?;
        Ok(())
    }
}

/// Hide the main window. Called by the frontend AFTER its close animation
/// finishes — this is the real "hide" that follows the slide-out animation.
#[tauri::command]
pub fn hide_window(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
}

/// Toggle the "native dialog open" flag. While true, the main window's
/// focus-loss handler skips collapsing the popup — needed because opening a
/// system dialog (folder picker) transfers focus to that dialog, which would
/// otherwise be read as "user clicked outside" and wrongly hide Cove.
#[tauri::command]
pub fn set_dialog_open(open: bool) {
    crate::set_dialog_open_internal(open);
}

/// Active default model state for the model-switcher UI.
///
/// Returns three fields so the frontend can handle both shapes of the top-level
/// `"model"` field in settings.json:
///   - `model`: the raw value exactly as written ("sonnet", "opus", OR a direct
///     model id like "DeepSeek-V4-Pro" set by cc-switch).
///   - `tier`: a tier alias ("opus"/"sonnet"/"fable"/"haiku"/...) when `model`
///     matches a discovered tier slot; "" when it's a direct model id (no tier
///     applies). This is what the switcher highlights.
///   - `info`: all discovered tier slots (each with raw id + clean display name).
///
/// The empty-`tier` case is the cc-switch "direct model" path: the tray label
/// shows the raw model and the switcher shows no tier highlighted (with a note
/// that the active model is set directly, not via a tier alias).
#[tauri::command]
pub fn get_model_state() -> serde_json::Value {
    let info = read_model_info();
    let raw = read_raw_model();
    let tier = if is_tier_alias(&info, &raw) {
        raw.clone()
    } else {
        String::new()
    };
    serde_json::json!({
        "model": raw,
        "tier": tier,
        "info": info,
    })
}

/// Set the active default tier (writes top-level "model" in settings.json,
/// preserving everything else). `tier` must be one of the slots currently
/// discovered in settings.json — dynamic, so newly configured tiers (e.g.
/// fable) are accepted without a code change, while arbitrary strings (and
/// shell-injection attempts via IPC) are rejected.
#[tauri::command]
pub fn set_default_tier_cmd(tier: String) -> Result<String, String> {
    let t = tier.trim();
    let info = read_model_info();
    if t.is_empty() || !is_tier_alias(&info, t) {
        return Err(format!("未知档位: {tier}"));
    }
    set_default_tier(t)?;
    Ok(format!("已切换默认模型为 {}", t))
}

/// The configured default workspace for new chats (the "新对话" button on the
/// loose-conversations tab). None until the user picks one.
#[tauri::command]
pub fn get_default_workspace() -> Option<String> {
    default_workspace()
}

/// Set the default workspace for new chats (persists cove-settings.json).
#[tauri::command]
pub fn set_default_workspace_cmd(path: String) -> Result<(), String> {
    set_default_workspace(&path)
}

/// List a conversation's related data items (with Chinese labels + sizes) so the
/// frontend can show a delete-selector before purging.
#[tauri::command]
pub fn list_related_files(sid: String, project_encoded: String) -> Vec<RelatedItem> {
    let root = claude_dir();
    list_related(&sid, &project_encoded, &root)
}

/// Delete the user-selected related paths for a conversation. `paths` is the
/// subset the user checked. history.jsonl entries are filtered line-by-line.
#[tauri::command]
pub fn delete_related_files(sid: String, paths: Vec<String>) -> Result<u64, String> {
    let root = claude_dir();
    Ok(delete_paths(&paths, &sid, &root))
}

/// Rename a Claude Code session by appending a `custom-title` line to its jsonl
/// transcript. This is the SAME mechanism the `/rename` slash command uses:
/// `/resume` reads the most recent `{"type":"custom-title","customTitle":"..."}`
/// entry as the session's display name. Appending is idempotent — calling
/// rename again just means the newer title wins; no rewrite of history.
///
/// Returns the new title on success.
#[tauri::command]
pub fn rename_session(sid: String, project_encoded: String, name: String) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("名称不能为空".to_string());
    }
    if trimmed.chars().count() > 80 {
        return Err("名称过长（上限 80 字符）".to_string());
    }
    let jsonl = claude_dir()
        .join("projects")
        .join(&project_encoded)
        .join(format!("{sid}.jsonl"));
    if !jsonl.exists() {
        return Err(format!("找不到会话文件: {}", jsonl.display()));
    }

    // Escape the title for JSON (handles quotes, backslashes, newlines, etc).
    let escaped = serde_json::Value::String(trimmed.to_string())
        .to_string();
    // escaped includes surrounding quotes, e.g. "\"模型能力排名\""
    let line = format!(
        "{{\"type\":\"custom-title\",\"customTitle\":{},\"sessionId\":\"{}\"}}\n",
        escaped, sid
    );

    // Append (Open + Append). Using append mode so we don't load the whole file.
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&jsonl)
        .map_err(|e| format!("打开会话文件失败: {e}"))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("写入失败: {e}"))?;
    Ok(trimmed.to_string())
}

/// Read a full session transcript (every user/assistant turn, block-preserving)
/// for the session-history viewer.
#[tauri::command]
pub fn get_session_transcript(
    sid: String,
    project_encoded: String,
) -> Result<SessionTranscript, String> {
    let jsonl = claude_dir()
        .join("projects")
        .join(&project_encoded)
        .join(format!("{sid}.jsonl"));
    if !jsonl.exists() {
        return Err(format!("找不到会话文件: {}", jsonl.display()));
    }
    transcript::parse(&jsonl, &sid)
        .ok_or_else(|| format!("读取会话失败: {}", jsonl.display()))
}

#[cfg(target_os = "windows")]
fn spawn_terminal(dir: &std::path::Path, claude_cmd: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x00000010;

    let dir_str = dir.to_string_lossy().to_string();

    // Preferred: Windows Terminal — wt -d <dir> cmd /k "<claude cmd>"
    let wt_ok = std::process::Command::new("wt")
        .args(["-d", &dir_str, "cmd", "/k", claude_cmd])
        .creation_flags(CREATE_NEW_CONSOLE)
        .spawn()
        .is_ok();

    if wt_ok {
        return Ok(());
    }

    // Fallback: classic console via cmd /C start.
    let inner = format!("cd /d \"{}\" && {}", dir_str, claude_cmd);
    std::process::Command::new("cmd")
        .args(["/C", "start", "cmd", "/K", &inner])
        .creation_flags(CREATE_NEW_CONSOLE)
        .spawn()
        .map_err(|e| format!("无法启动终端: {}", e))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn spawn_terminal(dir: &std::path::Path, claude_cmd: &str) -> Result<(), String> {
    let term = std::env::var("TERMINAL").unwrap_or_else(|_| "x-terminal-emulator".to_string());
    std::process::Command::new(&term)
        .arg("-e")
        .arg(claude_cmd)
        .current_dir(dir)
        .spawn()
        .map_err(|e| format!("无法启动终端 {}: {}", term, e))?;
    Ok(())
}
