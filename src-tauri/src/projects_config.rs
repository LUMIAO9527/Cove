use crate::paths::claude_dir;
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

/// Path of the config file: `~/.claude/cove-projects.json`.
pub fn config_path() -> PathBuf {
    claude_dir().join("cove-projects.json")
}

/// Load the config. Returns an empty config if the file is missing or unreadable.
///
/// As a one-time migration, strips the Windows `\\?\` verbatim prefix from any
/// stored path (older versions stored canonicalize() output verbatim, which
/// broke encode_project_path matching). If anything changed, the cleaned
/// config is persisted back.
pub fn load() -> ProjectsConfig {
    let path = config_path();
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
        let _ = save(&cfg);
    }
    cfg
}

/// Persist the config (creates the parent dir if needed).
/// 原子写：写 .tmp 再 rename，避免写入中途崩溃导致 cove-projects.json 截断
/// （截断会被 load() 的 unwrap_or_default() 静默清空 → 用户丢全部项目）。
pub fn save(cfg: &ProjectsConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    crate::archive::atomic_write(&path, json).map_err(|e| e.to_string())
}

/// Returns true if `path` is already registered.
pub fn exists(real_path: &str) -> bool {
    let cfg = load();
    cfg.projects.iter().any(|p| same_path(&p.path, real_path))
}

/// Add a project. Validates the directory exists and not already registered.
/// Returns the added entry, or an error message.
pub fn add(real_path: &str, name: Option<String>) -> Result<ProjectEntry, String> {
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

    if exists(&canon) {
        return Err("该项目已添加".to_string());
    }

    let entry = ProjectEntry {
        path: canon,
        name: name.filter(|s| !s.trim().is_empty()),
        added_at: now_millis(),
    };

    let mut cfg = load();
    cfg.projects.insert(0, entry.clone()); // newest first
    save(&cfg)?;
    Ok(entry)
}

/// Remove a project by its real path. Does NOT touch disk data.
/// Returns true if removed.
pub fn remove(real_path: &str) -> bool {
    let mut cfg = load();
    let before = cfg.projects.len();
    cfg.projects.retain(|p| !same_path(&p.path, real_path));
    let changed = cfg.projects.len() != before;
    if changed {
        let _ = save(&cfg);
    }
    changed
}

/// Rename a project's alias by its real path. An empty/whitespace name clears
/// the alias so it falls back to the directory name. `added_at` is preserved.
/// Returns the updated entry, or an error if the project is not found.
pub fn rename(real_path: &str, name: Option<String>) -> Result<ProjectEntry, String> {
    let mut cfg = load();
    let entry = cfg
        .projects
        .iter_mut()
        .find(|p| same_path(&p.path, real_path))
        .ok_or_else(|| format!("项目未找到: {}", real_path))?;
    entry.name = name.filter(|s| !s.trim().is_empty());
    let updated = entry.clone();
    save(&cfg)?;
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

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
