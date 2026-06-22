use crate::models::ModelInfo;
use crate::paths::claude_dir;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Deserialize)]
struct SettingsFile {
    env: Option<SettingsEnv>,
}

#[derive(Deserialize)]
struct SettingsEnv {
    #[serde(rename = "ANTHROPIC_DEFAULT_OPUS_MODEL")]
    opus_model: Option<String>,
    #[serde(rename = "ANTHROPIC_DEFAULT_SONNET_MODEL")]
    sonnet_model: Option<String>,
    #[serde(rename = "ANTHROPIC_DEFAULT_HAIKU_MODEL")]
    haiku_model: Option<String>,
}

pub fn read_model_info() -> ModelInfo {
    let path = claude_dir().join("settings.json");
    let settings: SettingsFile = fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(SettingsFile { env: None });

    let env = settings.env.unwrap_or(SettingsEnv {
        opus_model: None,
        sonnet_model: None,
        haiku_model: None,
    });

    ModelInfo {
        opus_model: env.opus_model.unwrap_or_else(|| "未配置".to_string()),
        sonnet_model: env.sonnet_model.unwrap_or_else(|| "未配置".to_string()),
        haiku_model: env.haiku_model.unwrap_or_else(|| "未配置".to_string()),
    }
}

/// Read the top-level "model" field (the active default tier: opus/sonnet/haiku).
/// Returns "sonnet" when unset (Claude Code's own default).
pub fn read_default_tier() -> String {
    let path = claude_dir().join("settings.json");
    let v: serde_json::Value = fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null);
    v.get("model")
        .and_then(|m| m.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "sonnet".to_string())
}

/// Write the top-level "model" field (active default tier) to settings.json,
/// preserving every other field. `tier` should be "opus" | "sonnet" | "haiku".
/// 原子写：写 .tmp 再 rename。settings.json 是 Claude Code 共享配置，写入
/// 中途崩溃会破坏整个文件，原子替换保证要么全成功要么不变。
pub fn set_default_tier(tier: &str) -> Result<(), String> {
    let path = claude_dir().join("settings.json");
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取 settings.json 失败: {e}"))?;
    let mut v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("解析 settings.json 失败: {e}"))?;
    // Use an object as the root (guard against a malformed file).
    if !v.is_object() {
        return Err("settings.json 根不是对象".to_string());
    }
    v["model"] = serde_json::Value::String(tier.to_string());
    let out = serde_json::to_string_pretty(&v).map_err(|e| format!("序列化失败: {e}"))?;
    crate::archive::atomic_write(&path, out).map_err(|e| format!("写入 settings.json 失败: {e}"))?;
    Ok(())
}

// ===========================================================================
// Cove's own settings (~/.claude/cove-settings.json)
//
// Distinct from settings.json (Claude Code's own config) and cove-projects.json
// (the project list). Holds Cove-local preferences like the default workspace
// for the "新对话" (new chat) button on the loose-conversations tab.
// ===========================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CoveSettings {
    /// Absolute path of the default working directory for new chats.
    /// None / absent on first use (before the user has picked a folder).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_workspace: Option<String>,
}

/// Path of Cove's own settings file: `~/.claude/cove-settings.json`.
fn cove_settings_path() -> std::path::PathBuf {
    claude_dir().join("cove-settings.json")
}

/// Load Cove's settings. Returns an empty struct if the file is missing or
/// unreadable (first-run / corrupt-file safe).
pub fn cove_load() -> CoveSettings {
    let path = cove_settings_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// The configured default workspace, or None if unset.
pub fn default_workspace() -> Option<String> {
    cove_load().default_workspace.filter(|s| !s.trim().is_empty())
}

/// Set the default workspace, persisting cove-settings.json (creates the parent
/// dir if needed — ~/.claude/ always exists in practice, but be safe).
/// 原子写：写 .tmp 再 rename，避免崩溃截断配置文件。
pub fn set_default_workspace(path: &str) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("工作目录不能为空".to_string());
    }
    let mut cfg = cove_load();
    cfg.default_workspace = Some(trimmed.to_string());
    let file = cove_settings_path();
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    crate::archive::atomic_write(&file, json).map_err(|e| format!("写入 cove-settings.json 失败: {e}"))?;
    Ok(())
}
