use crate::paths::claude_dir;
use crate::tools::ToolKind;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// A user-registered project entry, persisted to config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub path: String,        // real working directory (absolute)
    pub name: Option<String>, // optional alias; None => use dir name
    pub added_at: i64,       // unix millis
}

/// Top-level config: the list of registered projects.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectsConfig {
    pub projects: Vec<ProjectEntry>,
}

/// Filename for each tool's project list. All live under ~/.claude/ so they
/// share one home with Cove's own settings (cove-settings.json etc.). The
/// legacy `cove-projects.json` (pre-multi-tool) is migrated into the Claude
/// file on first load — see `load`.
fn config_filename(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::Claude => "cove-projects-claude.json",
        ToolKind::Reasonix => "cove-projects-reasonix.json",
    }
}

/// Path of the config file for `tool`: `~/.claude/cove-projects-<tool>.json`.
pub fn config_path_for(tool: ToolKind) -> PathBuf {
    claude_dir().join(config_filename(tool))
}

/// Legacy pre-multi-tool config path. Kept for the one-time migration in `load`.
fn legacy_config_path() -> PathBuf {
    claude_dir().join("cove-projects.json")
}

/// Load the config for `tool`. Returns an empty config if the file is missing.
///
/// One-time migration: when loading Claude's config and the tool-specific file
/// doesn't exist yet but the legacy `cove-projects.json` does, the legacy file
/// is adopted as Claude's config (renamed on disk) so existing users keep their
/// projects. The migration is idempotent: after the first run the tool-specific
/// file exists, so the legacy path is never touched again.
///
/// Also strips the Windows `\\?\` verbatim prefix from any stored path (older
/// versions stored canonicalize() output verbatim, which broke
/// encode_project_path matching). If anything changed, persists the cleaned
/// config back.
pub fn load(tool: ToolKind) -> ProjectsConfig {
    let path = config_path_for(tool);

    // Legacy migration: Claude-only, first run after upgrade.
    if tool == ToolKind::Claude && !path.exists() {
        let legacy = legacy_config_path();
        if legacy.exists() {
            // Adopt the legacy file in place by renaming it to the new name.
            let _ = fs::rename(&legacy, &path);
        }
    }

    let mut cfg = match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => ProjectsConfig::default(),
    };
    // Migrate: strip `\\?\` prefixes from stored paths so encoding matches
    // Claude Code's own project directory names.
    let mut changed = false;
    for entry in cfg.projects.iter_mut() {
        let cleaned = crate::paths::strip_verbatim_prefix_pub(&entry.path).to_string();
        if cleaned != entry.path {
            entry.path = cleaned;
            changed = true;
        }
    }
    if changed {
        let _ = save(tool, &cfg);
    }
    cfg
}

/// Persist the config for `tool` (creates the parent dir if needed).
/// 原子写：写 .tmp 再 rename，避免写入中途崩溃导致配置文件截断
/// （截断会被 load() 的 unwrap_or_default() 静默清空 → 用户丢全部项目）。
pub fn save(tool: ToolKind, cfg: &ProjectsConfig) -> Result<(), String> {
    let path = config_path_for(tool);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    crate::archive::atomic_write(&path, json).map_err(|e| e.to_string())
}

/// Returns true if `path` is already registered under `tool`.
pub fn exists(tool: ToolKind, real_path: &str) -> bool {
    let cfg = load(tool);
    cfg.projects.iter().any(|p| same_path(&p.path, real_path))
}

/// Add a project under `tool`. Validates the directory exists and not already
/// registered. Returns the added entry, or an error message.
pub fn add(tool: ToolKind, real_path: &str, name: Option<String>) -> Result<ProjectEntry, String> {
    let p = PathBuf::from(real_path);
    if !p.is_dir() {
        return Err(format!("目录不存在: {}", real_path));
    }
    // Canonicalize to a stable absolute form (resolves . / ..).
    let canon = fs::canonicalize(&p)
        .map(|c| c.to_string_lossy().to_string())
        .unwrap_or_else(|_| real_path.to_string());
    // Strip the Windows verbatim prefix `\\?\` that canonicalize adds. We store
    // the CLEAN path so encode_project_path matches Claude Code's own directory
    // naming (e.g. `D:\X` -> `D--X`, not `\\?\D:\X` -> `--------D-X`). Storing
    // the prefixed form was the root cause of sessions leaking to the "loose"
    // tab instead of showing under their project.
    let canon = crate::paths::strip_verbatim_prefix_pub(&canon).to_string();

    if exists(tool, &canon) {
        return Err("该项目已添加".to_string());
    }

    let entry = ProjectEntry {
        path: canon,
        name: name.filter(|s| !s.trim().is_empty()),
        added_at: now_millis(),
    };

    let mut cfg = load(tool);
    cfg.projects.insert(0, entry.clone()); // newest first
    save(tool, &cfg)?;
    Ok(entry)
}

/// Remove a project by its real path under `tool`. Does NOT touch disk data.
/// Returns true if removed.
pub fn remove(tool: ToolKind, real_path: &str) -> bool {
    let mut cfg = load(tool);
    let before = cfg.projects.len();
    cfg.projects.retain(|p| !same_path(&p.path, real_path));
    let changed = cfg.projects.len() != before;
    if changed {
        let _ = save(tool, &cfg);
    }
    changed
}

/// Rename a project's alias by its real path under `tool`. An empty/whitespace
/// name clears the alias so it falls back to the directory name. `added_at` is
/// preserved. Returns the updated entry, or an error if the project is not found.
pub fn rename(tool: ToolKind, real_path: &str, name: Option<String>) -> Result<ProjectEntry, String> {
    let mut cfg = load(tool);
    let entry = cfg
        .projects
        .iter_mut()
        .find(|p| same_path(&p.path, real_path))
        .ok_or_else(|| format!("项目未找到: {}", real_path))?;
    entry.name = name.filter(|s| !s.trim().is_empty());
    let updated = entry.clone();
    save(tool, &cfg)?;
    Ok(updated)
}

/// Case-insensitive on Windows, exact elsewhere.
fn same_path(a: &str, b: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        a.eq_ignore_ascii_case(b)
    }
    #[cfg(not(target_os = "windows"))]
    {
        a == b
    }
}

/// Reorder the project list to match `ordered_paths`. The Vec order IS the
/// display order (前端 get_projects 直接返回它，不再额外排序), so persisting
/// a new Vec order = persisting a new display order.
///
/// 语义：`ordered_paths` 是用户拖拽后的完整路径顺序（前端在 drop 时算好整列新
/// 顺序传入）。后端按这个顺序重排 cfg.projects；不在列表里的项目（理论不会
/// 发生，但防御性）保持原相对顺序追加到末尾。长度不匹配 / 全部缺失时拒绝写入
/// 并报错，避免误清空。
pub fn reorder(tool: ToolKind, ordered_paths: &[String]) -> Result<(), String> {
    let mut cfg = load(tool);
    if ordered_paths.is_empty() {
        return Err("排序路径列表为空".to_string());
    }
    // 校验：ordered_paths 必须是当前 projects 集合的一个排列（元素相同，可缺可多都算错误）。
    let current_count = cfg.projects.len();
    if ordered_paths.len() != current_count {
        return Err(format!(
            "排序路径数量（{}）与当前项目数（{}）不符",
            ordered_paths.len(),
            current_count
        ));
    }
    // 逐个按 ordered_paths 取出，重组 cfg.projects。
    let old: Vec<ProjectEntry> = std::mem::take(&mut cfg.projects);
    let mut remaining: Vec<ProjectEntry> = old;
    let mut reordered: Vec<ProjectEntry> = Vec::with_capacity(current_count);
    for want in ordered_paths {
        let pos = remaining.iter().position(|p| same_path(&p.path, want));
        match pos {
            Some(i) => reordered.push(remaining.remove(i)),
            None => return Err(format!("排序中包含未知项目: {}", want)),
        }
    }
    // remaining 此时必为空（长度已校验相等，且每个 want 都命中）。
    // 理论上 reordered.len() == current_count。
    cfg.projects = reordered;
    save(tool, &cfg)
}

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_filename_per_tool() {
        assert_eq!(config_filename(ToolKind::Claude), "cove-projects-claude.json");
        assert_eq!(config_filename(ToolKind::Reasonix), "cove-projects-reasonix.json");
    }
}
