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
    /// Display-name variants written by cc-switch (e.g. OPUS_MODEL=GLM-5.2[1M],
    /// OPUS_MODEL_NAME=GLM-5.2). The plain _MODEL keeps technical suffixes like
    /// [1M] (context window) that look ugly in the UI; _NAME is the clean label.
    /// Absent on stock Claude Code installs — falls back to _MODEL.
    #[serde(rename = "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME", default)]
    opus_model_name: Option<String>,
    #[serde(rename = "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME", default)]
    sonnet_model_name: Option<String>,
    #[serde(rename = "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME", default)]
    haiku_model_name: Option<String>,
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
        opus_model_name: None,
        sonnet_model_name: None,
        haiku_model_name: None,
    });

    // For each tier, produce (raw_id, clean_name) reading each env field once.
    // The clean name prefers the _NAME label (cc-switch writes it without the
    // technical [1M] suffix); falls back to the raw _MODEL id; finally to "未配置".
    // Done per-field instead of via a closure to avoid moving env fields twice.
    let resolve = |model: Option<String>, name: Option<String>| -> (String, String) {
        let raw = model.unwrap_or_else(|| "未配置".to_string());
        let clean = match name {
            Some(n) if !n.trim().is_empty() => n,
            _ => raw.clone(),
        };
        (raw, clean)
    };

    let (opus_model, opus_model_name) = resolve(env.opus_model, env.opus_model_name);
    let (sonnet_model, sonnet_model_name) = resolve(env.sonnet_model, env.sonnet_model_name);
    let (haiku_model, haiku_model_name) = resolve(env.haiku_model, env.haiku_model_name);

    ModelInfo {
        opus_model,
        sonnet_model,
        haiku_model,
        opus_model_name,
        sonnet_model_name,
        haiku_model_name,
    }
}

/// Read the RAW top-level "model" field exactly as written in settings.json.
///
/// Two shapes occur in the wild:
///   1. A tier alias: "opus" | "sonnet" | "haiku" (stock Claude Code). The active
///      model is then the one mapped by the matching ANTHROPIC_DEFAULT_*_MODEL.
///   2. A direct model id: e.g. "DeepSeek-V4-Pro", "gpt-5.5" — written by cc-switch
///      when a provider sets an explicit model. In this case there is NO tier
///      alias; the three tier slots don't apply and the UI must show this id as
///      the single active default rather than forcing it into a tier.
///
/// Returns "" when the field is absent (treated as the Claude Code default sonnet
/// by the caller). We deliberately do NOT collapse case (2) to "sonnet" here —
/// that would mislabel the active model. The frontend decides how to render.
pub fn read_raw_model() -> String {
    let path = claude_dir().join("settings.json");
    let v: serde_json::Value = fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null);
    v.get("model")
        .and_then(|m| m.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
}

/// Whether `tier` is one of the three standard aliases. Used by the frontend to
/// distinguish "tier alias" (case 1) from "direct model id" (case 2).
pub fn is_tier_alias(tier: &str) -> bool {
    matches!(tier, "opus" | "sonnet" | "haiku")
}

#[cfg(test)]
mod tests {
    use super::is_tier_alias;

    #[test]
    fn test_tier_aliases_recognized() {
        // The three standard Claude Code tier aliases must be detected so the
        // switcher can highlight the matching row.
        assert!(is_tier_alias("opus"));
        assert!(is_tier_alias("sonnet"));
        assert!(is_tier_alias("haiku"));
    }

    #[test]
    fn test_direct_model_id_not_treated_as_tier() {
        // cc-switch writes the real model id into the top-level "model" field
        // (e.g. "DeepSeek-V4-Pro", "gpt-5.5", "gemini-3.5-flash"). These must
        // NOT be misclassified as a tier alias — otherwise the switcher would
        // wrongly highlight a row and the tray would show the wrong slot.
        // Values sampled from the user's real ~/.cc-switch/cc-switch.db.
        assert!(!is_tier_alias("DeepSeek-V4-Pro"));
        assert!(!is_tier_alias("gpt-5.5"));
        assert!(!is_tier_alias("gemini-3.5-flash"));
        assert!(!is_tier_alias("GLM-5.2"));
        assert!(!is_tier_alias(""));
    }
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
