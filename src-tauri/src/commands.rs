use crate::archive::{
    archive_conversation as do_archive, list_archived, list_archived_conversations, purge_archived,
    restore_conversation as do_restore, ArchiveIndex,
};
use crate::cleanup::{delete_conversation, scan_orphans, DeleteResult, list_related, delete_paths};
use crate::models::{Conversation, ModelInfo, OrphanEntry, Project, RelatedItem, SessionTranscript};
use crate::paths::{archive_dir, claude_dir, encode_project_path};
use crate::projects_config::{self, ProjectEntry};
use crate::settings::{read_model_info, read_raw_model, is_tier_alias, set_default_tier, default_workspace, set_default_workspace};
use crate::tools::ToolKind;
use tauri::Manager;

// ---------------------------------------------------------------------------
// Projects (user-managed list)
// ---------------------------------------------------------------------------

/// List all registered projects for `tool`, each enriched with live conversation stats.
///
/// 顺序：直接按配置文件 Vec 的存储顺序返回（= 用户拖拽后的顺序）。不再按
/// added_at 排序——那样会覆盖用户拖拽得到的自定义顺序。配置 Vec 由 add 的
/// insert(0) 维持"最新在前"作为默认，用户拖拽后 Vec 顺序即显示顺序。
#[tauri::command]
pub fn get_projects(tool: String) -> Vec<Project> {
    let tool = ToolKind::from_name(&tool);
    let cfg = projects_config::load(tool);
    cfg.projects
        .iter()
        .map(|e| entry_to_project(tool, e))
        .collect()
}

/// Register a new project by its real working directory, under `tool`.
#[tauri::command]
pub fn add_project(tool: String, path: String, name: Option<String>) -> Result<Project, String> {
    let tool = ToolKind::from_name(&tool);
    let entry = projects_config::add(tool, &path, name)?;
    Ok(entry_to_project(tool, &entry))
}

/// Remove a project from the list (keeps all disk data).
#[tauri::command]
pub fn remove_project(tool: String, path: String) -> bool {
    let tool = ToolKind::from_name(&tool);
    projects_config::remove(tool, &path)
}

/// Rename a project's alias (the display name). An empty/whitespace name
/// resets it to the directory name. `added_at` is preserved.
#[tauri::command]
pub fn rename_project(tool: String, path: String, name: String) -> Result<Project, String> {
    let tool = ToolKind::from_name(&tool);
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("名称不能为空".to_string());
    }
    if trimmed.chars().count() > 80 {
        return Err("名称过长（上限 80 字符）".to_string());
    }
    let entry = projects_config::rename(tool, &path, Some(trimmed.to_string()))?;
    Ok(entry_to_project(tool, &entry))
}

/// Reorder the project list to match `ordered_paths` (the full new order after
/// a drag-drop). Persists to config; the new order is reflected on next
/// get_projects. 前端拖拽完成后调用，传入整列新顺序的路径数组。
#[tauri::command]
pub fn reorder_projects(tool: String, ordered_paths: Vec<String>) -> Result<(), String> {
    let tool = ToolKind::from_name(&tool);
    projects_config::reorder(tool, &ordered_paths)
}

/// Detail view: conversations for a single project (precise, encode-based for
/// Claude; cwd-based for Reasonix).
#[tauri::command]
pub fn get_project_detail(tool: String, path: String) -> Vec<Conversation> {
    let tool = ToolKind::from_name(&tool);
    tool.conversations_for_path(&path)
}

/// Loose conversations: ALL sessions for `tool` minus those belonging to
/// registered projects. Used by the "对话" tab (scattered conversations).
#[tauri::command]
pub fn get_loose_conversations(tool: String) -> Vec<Conversation> {
    let tool = ToolKind::from_name(&tool);
    // Registered project cwds for this tool — used to filter them out.
    let registered: Vec<String> = projects_config::load(tool)
        .projects
        .iter()
        .map(|e| e.path.clone())
        .collect();
    tool.scan_loose(&registered)
}

/// Build a Project (with live stats) from a config entry, using `tool` to pick
/// the right scan adapter.
fn entry_to_project(tool: ToolKind, e: &ProjectEntry) -> Project {
    let convos = tool.conversations_for_path(&e.path);
    let total_size: u64 = convos.iter().map(|c| c.size_bytes).sum();
    let last_updated = convos.iter().map(|c| c.last_updated).max().unwrap_or(0);

    // For Claude the encoded dir is a real concept (Claude Code's project dir
    // name); for other tools there is no such encoding, so leave it empty.
    let encoded = match tool {
        ToolKind::Claude => encode_project_path(&e.path),
        ToolKind::Reasonix => String::new(),
    };

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
pub fn delete_convo(tool: String, sid: String, project_encoded: String) -> DeleteResult {
    let tool = ToolKind::from_name(&tool);
    match tool {
        ToolKind::Claude => delete_conversation(&sid, &project_encoded, &claude_dir()),
        ToolKind::Reasonix => {
            // Reasonix: a session is a flat <name>.jsonl + sidecars. Delete them all.
            let paths = crate::tools::reasonix::session_data_paths(&sid);
            let mut removed = Vec::new();
            let mut freed: u64 = 0;
            let mut success = true;
            for p in &paths {
                let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                let r = if p.is_dir() {
                    std::fs::remove_dir_all(p)
                } else {
                    std::fs::remove_file(p)
                };
                if r.is_ok() {
                    removed.push(p.to_string_lossy().to_string());
                    freed += size;
                } else {
                    success = false;
                }
            }
            DeleteResult { success, freed_bytes: freed, removed_paths: removed }
        }
    }
}

/// 归档一个会话。Claude 走 8 处关联数据归档；Reasonix 走扁平 sidecar 归档。
/// 返回 Ok(()) 全部关联数据归档成功；Err 含失败项描述（前端 toast）。
#[tauri::command]
pub fn archive_convo(tool: String, sid: String, project_encoded: String) -> Result<(), String> {
    let tool = ToolKind::from_name(&tool);
    match tool {
        ToolKind::Claude => do_archive(&sid, &project_encoded, &claude_dir(), &archive_dir()),
        ToolKind::Reasonix => crate::archive::archive_reasonix_session(&sid, &archive_dir()),
    }
}

/// 恢复一个归档会话。返回 Ok(()) 全部数据还原成功；Err 含失败项。
#[tauri::command]
pub fn restore_convo(tool: String, sid: String, project_encoded: String) -> Result<(), String> {
    let tool = ToolKind::from_name(&tool);
    match tool {
        ToolKind::Claude => do_restore(&sid, &project_encoded, &claude_dir(), &archive_dir()),
        ToolKind::Reasonix => crate::archive::restore_reasonix_session(&sid, &archive_dir()),
    }
}

#[tauri::command]
pub fn get_archive_index() -> ArchiveIndex {
    list_archived(&archive_dir())
}

/// 归档区所有会话，解析成完整 Conversation（真实标题/模型/消息数/大小/cwd），
/// 供归档页用和会话列表同一套卡片 + 信息浮层展示。按工具取各自的归档区。
#[tauri::command]
pub fn get_archive_conversations(tool: String) -> Vec<Conversation> {
    let tool = ToolKind::from_name(&tool);
    match tool {
        ToolKind::Claude => list_archived_conversations(&archive_dir()),
        ToolKind::Reasonix => crate::archive::list_archived_reasonix(&archive_dir()),
    }
}

#[tauri::command]
pub fn purge_archived_convo(tool: String, sid: String, project_encoded: String) -> bool {
    let tool = ToolKind::from_name(&tool);
    match tool {
        ToolKind::Claude => purge_archived(&sid, &project_encoded, &archive_dir()),
        ToolKind::Reasonix => crate::archive::purge_archived_reasonix(&sid, &archive_dir()),
    }
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

/// Launch a coding-agent session in `path` for `tool`.
/// - `sid` None / empty => new session (bare CLI command)
/// - `sid` Some        => resume the given session
///
/// The exact resume form is tool-specific (`claude --resume X` vs
/// `reasonix --resume X`); see `ToolKind::launch_cmd`.
///
/// Spawns an independent terminal window (Windows Terminal, fallback cmd),
/// detached via `.spawn()` so the Tauri main process never blocks.
#[tauri::command]
pub fn open_session(
    tool: String,
    path: String,
    sid: Option<String>,
) -> Result<(), String> {
    let tool = ToolKind::from_name(&tool);
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
    let sid = sid.and_then(|s| {
        let s = s.trim();
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    });
    // Validate the sid shape against shell injection. Each tool's id format
    // differs (Claude = UUID, Reasonix = filename stem), so dispatch by tool.
    if let Some(ref s) = sid {
        if !is_valid_session_id_for(tool, s) {
            return Err(format!("非法会话 ID: {}", s));
        }
    }
    let cmd = tool.launch_cmd(sid.as_deref());
    spawn_terminal(&dir, &cmd)
}

/// Whether `s` is a safe (shell-injection-free) session id for `tool`.
fn is_valid_session_id_for(tool: ToolKind, s: &str) -> bool {
    match tool {
        ToolKind::Claude => is_valid_uuid(s),
        // Reasonix ids are filename stems like "20260603-090200.000-deepseek-v4-flash".
        // Allow alphanumerics, dash, dot, underscore — no shell metacharacters.
        ToolKind::Reasonix => {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_')
        }
    }
}

/// 校验字符串是合法的 Claude Code 会话 ID（UUID v4 格式：8-4-4-4-12
/// 十六进制，全小写或全大写）。用于 `is_valid_session_id_for` 防 shell 注入。
fn is_valid_uuid(s: &str) -> bool {
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
    use super::{is_valid_session_id_for, is_valid_uuid, ToolKind};

    #[test]
    fn test_valid_uuid_accepted() {
        assert!(is_valid_uuid("aaaa1111-2222-3333-4444-555555555555"));
        assert!(is_valid_uuid("ABCDEF12-3456-7890-ABCD-EF1234567890"));
        assert!(is_valid_uuid("00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn test_shell_injection_rejected() {
        // 评审 P2 #8：含 shell 元字符的伪造 sid 必须被拒绝。
        assert!(!is_valid_uuid("foo & echo pwned"));
        assert!(!is_valid_uuid("a;rm -rf /"));
        assert!(!is_valid_uuid("x | calc"));
        assert!(!is_valid_uuid("a\"b"));
        assert!(!is_valid_uuid("$(whoami)"));
        assert!(!is_valid_uuid(""));
    }

    #[test]
    fn test_malformed_uuid_rejected() {
        assert!(!is_valid_uuid("aaaa1111-2222-3333-4444")); // 太短
        assert!(!is_valid_uuid("aaaa1111-2222-3333-4444-555555555555-extra")); // 太长
        assert!(!is_valid_uuid("aaaa1111z2222-3333-4444-555555555555")); // dash 位置错
        assert!(!is_valid_uuid("aaaa1111-2222-3333-4444-55555555555z")); // 非 hex
    }

    #[test]
    fn test_valid_session_id_for_per_tool() {
        // Claude requires UUID shape.
        assert!(is_valid_session_id_for(
            ToolKind::Claude,
            "aaaa1111-2222-3333-4444-555555555555"
        ));
        // Reasonix ids are filename stems — alnum/dash/dot/underscore only.
        assert!(is_valid_session_id_for(
            ToolKind::Reasonix,
            "20260603-090200.000-deepseek-v4-flash"
        ));
        // Shell metacharacters rejected for both tools.
        assert!(!is_valid_session_id_for(ToolKind::Reasonix, "a & calc"));
        assert!(!is_valid_session_id_for(ToolKind::Reasonix, "a|b"));
        assert!(!is_valid_session_id_for(ToolKind::Reasonix, ""));
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

/// Open a blank terminal window at the user's home directory. Used by the
/// "未安装" page's "打开" button: the user copies the install command with the
/// adjacent copy button, then pastes+runs it here. We do NOT pre-fill or
/// auto-execute the install command — two decoupled buttons keep the flow
/// predictable and let the user review the command before running it.
///
/// The home dir is the natural place (npm global installs live under the
/// user profile), and it always exists. Falls back to the system temp if the
/// home env vars are somehow missing.
#[tauri::command]
pub fn open_install_terminal() -> Result<(), String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    // Empty command => spawn_terminal opens an interactive shell at `home`
    // (wt -d <home> cmd /k "" would run an empty line; instead pass a harmless
    // echo-free form by using a no-op). The simplest reliable shape is to not
    // pass a command at all: spawn a plain cmd via wt so the user lands on a
    // fresh prompt. We inline a minimal spawn here rather than reuse
    // spawn_terminal (which expects a command string for its /k form).
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x00000010;
        let dir_str = home.to_string_lossy().to_string();
        // Preferred: Windows Terminal at <home>, plain cmd (no command => the
        // user lands on a fresh prompt and pastes the install command).
        let wt_ok = std::process::Command::new("wt")
            .args(["-d", &dir_str, "cmd"])
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
            .is_ok();
        if wt_ok {
            return Ok(());
        }
        // Fallback: classic console. 用 current_dir 设工作目录而非手动拼
        // `cd /d "<home>"`——后者在 home 含特殊字符时引号拼接脆弱，current_dir
        // 走 OS 原生进程属性，无注入面。start 第一个 token 会被当窗口标题，
        // 故给个空串占位避开这个 cmd 经典坑。
        std::process::Command::new("cmd")
            .current_dir(&home)
            .args(["/C", "start", "", "cmd"])
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
            .map_err(|e| format!("无法打开终端: {e}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let term = std::env::var("TERMINAL")
            .unwrap_or_else(|_| "x-terminal-emulator".to_string());
        std::process::Command::new(&term)
            .current_dir(&home)
            .spawn()
            .map_err(|e| format!("无法打开终端 {}: {}", term, e))?;
        Ok(())
    }
}

/// Open one of Cove's data directories in the system file explorer. `which`:
///  - "claude"   → ~/.claude（磁盘清理页/项目页的"打开数据目录"）
///  - "projects" → ~/.claude/projects（项目页的"打开数据目录"，定位到项目根）
///  - "archive"  → ~/.claude-managed/archive（归档页的"打开归档目录"）
/// 统一一个命令覆盖三个页面的"打开目录"需求，避免每页加一个命令。
#[tauri::command]
pub fn open_app_data_dir(which: String) -> Result<(), String> {
    let dir = match which.as_str() {
        "claude" => crate::paths::claude_dir(),
        "projects" => crate::paths::claude_dir().join("projects"),
        "archive" => crate::paths::archive_dir(),
        other => return Err(format!("未知目录类型: {}", other)),
    };
    if !dir.exists() {
        // 归档目录可能还没创建（从未归档过），projects/claude 一定存在。
        // 对不存在的目录，尝试创建后再打开（避免"目录不存在"报错）。
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    open_in_explorer(dir.to_string_lossy().to_string())
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

/// Which CLI tools are installed on PATH. The frontend uses this to disable
/// the tool switcher for tools that aren't installed (can't manage sessions
/// for a CLI that doesn't exist). Returns a map of tool name → bool.
#[tauri::command]
pub fn get_installed_tools() -> std::collections::HashMap<String, bool> {
    let mut out = std::collections::HashMap::new();
    out.insert("claude".to_string(), ToolKind::Claude.is_installed());
    out.insert("reasonix".to_string(), ToolKind::Reasonix.is_installed());
    out
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

/// Rename a session. Claude Code only — appends a `custom-title` line to its
/// jsonl transcript, the SAME mechanism the `/rename` slash command uses
/// (`/resume` reads the most recent `{"type":"custom-title","customTitle":...}`
/// entry as the display name). Appending is idempotent.
///
/// Other tools (Reasonix) don't expose this mechanism via their data files, so
/// renaming returns an error (a future version may add Cove-local aliases).
///
/// Returns the new title on success.
#[tauri::command]
pub fn rename_session(
    tool: String,
    sid: String,
    project_encoded: String,
    name: String,
) -> Result<String, String> {
    let tool_kind = ToolKind::from_name(&tool);
    if tool_kind != ToolKind::Claude {
        return Err(format!("{} 暂不支持会话重命名", tool_kind.display_name()));
    }
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

    // 用 serde_json::json! 构造整行，sid 和 title 都自动正确转义
    // （之前 sid 裸插值，若含引号/反斜杠会破坏 JSON 行）。
    let line = serde_json::json!({
        "type": "custom-title",
        "customTitle": trimmed,
        "sessionId": sid,
    }).to_string()
        + "\n";

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
/// for the session-history viewer. `project_key` is the Claude encoded dir name
/// (Claude) or the cwd (Reasonix) — used to locate the right file.
#[tauri::command]
pub fn get_session_transcript(
    tool: String,
    sid: String,
    project_key: String,
) -> Result<SessionTranscript, String> {
    let tool = ToolKind::from_name(&tool);
    let jsonl = tool
        .session_path(&sid, &project_key)
        .ok_or_else(|| format!("找不到会话文件: {sid}"))?;
    tool.parse_transcript(&jsonl, &sid)
        .ok_or_else(|| format!("读取会话失败: {}", jsonl.display()))
}

/// Open the directory containing a session's jsonl in the system file explorer.
/// Used by the session-detail page's ▾ menu ("在文件夹打开"). Locates the jsonl
/// the same way get_session_transcript does, then opens its parent dir.
#[tauri::command]
pub fn open_session_location(
    tool: String,
    sid: String,
    project_key: String,
) -> Result<(), String> {
    let tool = ToolKind::from_name(&tool);
    let jsonl = tool
        .session_path(&sid, &project_key)
        .ok_or_else(|| format!("找不到会话文件: {sid}"))?;
    // 打开 jsonl 所在目录（父目录），而非文件本身——explorer 打开文件会
    // 用默认程序打开它，不是定位到资源管理器。
    let dir = jsonl
        .parent()
        .ok_or_else(|| "无法解析会话文件所在目录".to_string())?;
    if !dir.exists() {
        return Err(format!("目录不存在: {}", dir.display()));
    }
    open_in_explorer(dir.to_string_lossy().to_string())
}

/// Save text content to a file path (from a save dialog). Used by the
/// session-detail page's ▾ menu ("导出为 .md"). Plain atomic write — the
/// frontend supplies both the path (user-chosen via save dialog) and content.
///
/// 路径安全：正常流程路径来自原生 save 对话框（用户自选），但 IPC 层无强制保证，
/// 故做深度防御——只允许写到用户目录下、且后缀必须是 .md/.txt，避免被污染前端
/// 用来覆写系统文件。
#[tauri::command]
pub fn save_text_file(path: String, content: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    // 后缀白名单：只允许文本导出格式。
    let allowed = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_lowercase().as_str(), "md" | "txt"))
        .unwrap_or(false);
    if !allowed {
        return Err("只允许保存为 .md 或 .txt 文件".to_string());
    }
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::archive::atomic_write(&p, content).map_err(|e| e.to_string())
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
