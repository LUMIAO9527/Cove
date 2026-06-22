use crate::archive::path_size;
use crate::models::{OrphanEntry, RelatedItem};
use crate::related::find_related;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteResult {
    pub success: bool,
    pub freed_bytes: u64,
    pub removed_paths: Vec<String>,
}

/// Turn a RelatedSet into a flat, user-readable list of items (each with a
/// Chinese label + size), for the delete selector UI. Only includes items that
/// actually exist on disk.
pub fn list_related(sid: &str, project_encoded: &str, claude_root: &Path) -> Vec<RelatedItem> {
    let set = find_related(sid, project_encoded, claude_root);
    let mut out: Vec<RelatedItem> = Vec::new();
    let push = |out: &mut Vec<RelatedItem>, kind: &str, label: &str, path_str: String| {
        let size = path_size(Path::new(&path_str));
        if size > 0 || Path::new(&path_str).exists() {
            out.push(RelatedItem {
                kind: kind.to_string(),
                label: label.to_string(),
                path: path_str,
                size_bytes: size,
            });
        }
    };
    if let Some(p) = &set.jsonl_file {
        push(&mut out, "jsonl", "对话正文 (jsonl)", p.clone());
    }
    if let Some(p) = &set.project_subdir {
        push(&mut out, "subdir", "项目子目录 (subagents/results)", p.clone());
    }
    if let Some(p) = &set.tasks_dir {
        push(&mut out, "tasks", "任务数据 (tasks)", p.clone());
    }
    if let Some(p) = &set.file_history_dir {
        push(&mut out, "file-history", "文件历史 (file-history)", p.clone());
    }
    for p in &set.telemetry_files {
        push(&mut out, "telemetry", "遥测数据 (telemetry)", p.clone());
    }
    if let Some(p) = &set.session_env_dir {
        push(&mut out, "session-env", "会话环境 (session-env)", p.clone());
    }
    for p in &set.session_meta_files {
        push(&mut out, "session-meta", "会话元数据 (sessions)", p.clone());
    }
    // history.jsonl lines aren't a deletable path — surface as info only when > 0.
    if set.history_lines > 0 {
        out.push(RelatedItem {
            kind: "history".to_string(),
            label: format!("历史记录行 (history.jsonl, {} 行)", set.history_lines),
            path: claude_root.join("history.jsonl").to_string_lossy().to_string(),
            size_bytes: 0,
        });
    }
    out
}

/// Delete the given list of absolute paths. Returns total bytes freed.
/// For "history" kind (a jsonl file shared across sessions) this only removes
/// lines matching the SID, not the whole file — caller must pass the SID.
pub fn delete_paths(paths: &[String], sid: &str, claude_root: &Path) -> u64 {
    let mut freed = 0u64;
    for p_str in paths {
        let p = Path::new(p_str);
        if !p.exists() {
            continue;
        }
        // history.jsonl is shared: do line-level filtering, not whole-file delete.
        let is_history = p.file_name().map_or(false, |n| n == "history.jsonl");
        if is_history {
            // Measure the lines we're about to drop, then rewrite.
            let before = path_size(p);
            let _ = remove_history_lines(claude_root, sid);
            let after = path_size(p);
            freed += before.saturating_sub(after);
        } else if p.is_dir() {
            freed += path_size(p);
            let _ = fs::remove_dir_all(p);
        } else {
            freed += path_size(p);
            let _ = fs::remove_file(p);
        }
    }
    freed
}

/// 删除一个对话及其全部 8 处关联数据
pub fn delete_conversation(sid: &str, project_encoded: &str, claude_root: &Path) -> DeleteResult {
    let set = find_related(sid, project_encoded, claude_root);

    let mut removed = Vec::new();
    let mut freed: u64 = 0;
    let mut success = true;

    // 删除文件/目录 (①②③④⑤⑥⑧)
    for path_str in set.all_paths() {
        let path = std::path::PathBuf::from(&path_str);
        let size = path_size(&path);
        let r = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        if r.is_ok() {
            removed.push(path_str);
            freed += size;
        } else {
            success = false;
        }
    }

    // ⑦ history.jsonl 行级过滤重写
    if set.history_lines > 0 {
        if remove_history_lines(claude_root, sid).is_ok() {
            // history 行的字节数无法精确回收, 粗估
            freed += set.history_lines as u64 * 80;
        }
    }

    DeleteResult {
        success,
        freed_bytes: freed,
        removed_paths: removed,
    }
}

/// 从 history.jsonl 中移除指定 SID 的行，保留其余。
///
/// Public 包装供 archive.rs 复用（归档时同步清 history 行）。原子写：
/// 写 history.tmp 再 rename，避免写入中途崩溃损坏共享文件（v0.4.26 #20 修复）。
pub fn remove_history_lines_public(claude_root: &Path, sid: &str) -> Result<(), std::io::Error> {
    remove_history_lines(claude_root, sid)
}

/// 从 history.jsonl 中移除指定 SID 的行，保留其余
fn remove_history_lines(claude_root: &Path, sid: &str) -> Result<(), std::io::Error> {
    let history = claude_root.join("history.jsonl");
    if !history.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&history)?;
    let kept: Vec<&str> = content
        .lines()
        .filter(|line| {
            // 用 serde 精确匹配, 避免误删
            match serde_json::from_str::<HistoryLine>(line) {
                Ok(record) => record.session_id.as_deref() != Some(sid),
                Err(_) => true, // 无法解析的行保留
            }
        })
        .collect();
    let mut output = kept.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    // 原子写：history.jsonl 是跨会话共享文件，非原子覆写有数据损坏风险。
    crate::archive::atomic_write(&history, output)
}

#[derive(Deserialize)]
struct HistoryLine {
    #[serde(default, rename = "sessionId")]
    session_id: Option<String>,
}

// path_size 已抽到 archive.rs 作为 pub fn，本模块通过 crate::archive::path_size 复用
// （第二轮评审 DeepSeek 中等项：消除重复定义）。

// ===== 全局孤儿扫描 (Task 10 用) =====

/// Decode a Claude-Code project-encoded directory name back to a readable path.
/// e.g. "C--Users-user--AppLab" -> "C:\Users\user\AppLab"
fn decode_project_dir(encoded: &str) -> String {
    let mut path = String::new();
    let mut i = 0;
    let bytes = encoded.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'-' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'-' {
                path.push('-');
                i += 2;
            } else {
                // single dash = path separator
                if path.ends_with(':') {
                    // "C:" + "-" => "C:\"
                    path.push('\\');
                } else {
                    path.push('\\');
                }
                i += 1;
            }
        } else {
            path.push(bytes[i] as char);
            i += 1;
        }
    }
    path
}

/// 扫描所有孤儿数据：SID-named 条目在 projects 下无对应正文
pub fn scan_orphans(claude_root: &Path) -> Vec<OrphanEntry> {
    let live_sids = collect_live_sids(claude_root);
    let mut orphans = Vec::new();

    // tasks / file-history / session-env 下的孤立目录
    // 这些是全局目录，无法从路径反推所属项目。
    let scan_dirs = ["tasks", "file-history", "session-env"];
    for dir_name in &scan_dirs {
        let dir = claude_root.join(dir_name);
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if looks_like_sid(&name) && !live_sids.contains(&name) {
                    orphans.push(OrphanEntry {
                        sid: name.clone(),
                        location: entry.path().to_string_lossy().to_string(),
                        kind: dir_name.to_string(),
                        size_bytes: path_size(&entry.path()),
                        belongs_to: "全局数据（无项目关联）".to_string(),
                    });
                }
            }
        }
    }

    // telemetry: 文件名第一个 UUID 段
    let telemetry_dir = claude_root.join("telemetry");
    if let Ok(entries) = fs::read_dir(&telemetry_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(sid) = name
                .strip_prefix("1p_failed_events.")
                .and_then(|s| s.split('.').next())
            {
                if looks_like_sid(sid) && !live_sids.contains(sid) {
                    orphans.push(OrphanEntry {
                        sid: sid.to_string(),
                        location: entry.path().to_string_lossy().to_string(),
                        kind: "telemetry".to_string(),
                        size_bytes: path_size(&entry.path()),
                        belongs_to: "全局数据（无项目关联）".to_string(),
                    });
                }
            }
        }
    }

    // projects 下的孤立子目录 (SID 名但无同名 jsonl)
    // 这类孤儿的父目录就是 encoded 项目名，可解码出所属项目路径。
    let projects_dir = claude_root.join("projects");
    if let Ok(proj_entries) = fs::read_dir(&projects_dir) {
        for proj in proj_entries.flatten() {
            if proj.path().is_dir() {
                let proj_encoded = proj.file_name().to_string_lossy().to_string();
                let proj_decoded = decode_project_dir(&proj_encoded);
                if let Ok(conv_entries) = fs::read_dir(proj.path()) {
                    for conv in conv_entries.flatten() {
                        let name = conv.file_name().to_string_lossy().to_string();
                        if conv.path().is_dir()
                            && looks_like_sid(&name)
                            && !live_sids.contains(&name)
                        {
                            orphans.push(OrphanEntry {
                                sid: name,
                                location: conv.path().to_string_lossy().to_string(),
                                kind: "project-subdir".to_string(),
                                size_bytes: path_size(&conv.path()),
                                belongs_to: format!("原属项目：{}", proj_decoded),
                            });
                        }
                    }
                }
            }
        }
    }

    orphans
}

fn collect_live_sids(claude_root: &Path) -> HashSet<String> {
    let mut sids = HashSet::new();
    let projects_dir = claude_root.join("projects");
    if let Ok(proj_entries) = fs::read_dir(&projects_dir) {
        for proj in proj_entries.flatten() {
            if proj.path().is_dir() {
                if let Ok(conv_entries) = fs::read_dir(proj.path()) {
                    for conv in conv_entries.flatten() {
                        let name = conv.file_name().to_string_lossy().to_string();
                        if let Some(sid) = name.strip_suffix(".jsonl") {
                            if looks_like_sid(sid) {
                                sids.insert(sid.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    sids
}

fn looks_like_sid(s: &str) -> bool {
    // UUID 格式: 8-4-4-4-12 (共 36 字符, 4 个连字符)
    s.len() == 36 && s.matches('-').count() == 4
}
