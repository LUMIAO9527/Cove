use crate::models::{ModelInfo, TierSlot};
use crate::paths::claude_dir;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;

/// Read settings.json as a raw JSON value. Returns Null when the file is
/// missing or unparseable (first-run / corrupt-file safe).
fn read_settings_json() -> serde_json::Value {
    let path = claude_dir().join("settings.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null)
}

/// Canonical ordering for the well-known Claude Code tiers. New tiers fall
/// through to alphabetical (stable) order. Keeps the switcher menu stable
/// across opens instead of jumping as env var discovery order changes.
const KNOWN_TIER_ORDER: &[&str] = &["sonnet", "opus", "fable", "haiku"];

fn tier_sort_key(tier: &str) -> (usize, String) {
    let pos = KNOWN_TIER_ORDER
        .iter()
        .position(|t| *t == tier)
        .unwrap_or(usize::MAX);
    (pos, tier.to_lowercase())
}

/// Discover every model tier slot from a settings.json `env` object.
///
/// Pure function (no filesystem) so the discovery logic is unit-testable in
/// isolation — the previous version had a parsing bug that passed tests only
/// because tests built slots directly instead of parsing real env keys.
///
/// Two key shapes are recognized:
///   - `ANTHROPIC_DEFAULT_<TIER>_MODEL`      -> the tier's raw model id (source
///     of the slot; a tier only exists once this var is present and non-empty).
///   - `ANTHROPIC_DEFAULT_<TIER>_MODEL_NAME` -> optional clean display label.
///
/// The bare `ANTHROPIC_DEFAULT_MODEL` (no tier) is explicitly ignored — it is
/// not a per-tier slot. Anything else under the prefix is ignored too.
pub fn discover_tiers(env: Option<&serde_json::Map<String, serde_json::Value>>) -> ModelInfo {
    // lowercased tier -> (raw model id, optional clean name)
    let mut slots: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();
    if let Some(env) = env {
        for (key, val) in env {
            let raw_val = val.as_str().unwrap_or("").trim().to_string();
            let Some(rest) = key.strip_prefix("ANTHROPIC_DEFAULT_") else {
                continue;
            };
            // Display-name companion: ANTHROPIC_DEFAULT_<TIER>_MODEL_NAME.
            // NOTE: strip the _MODEL_NAME suffix, then ALSO strip _MODEL off
            // that to recover the tier (e.g. FABLE_MODEL_NAME -> FABLE).
            // The tier key is lowercased to match how _MODEL slots are stored.
            if let Some(after_name) = rest.strip_suffix("_NAME") {
                if let Some(tier_upper) = after_name.strip_suffix("_MODEL") {
                    if let Some(entry) = slots.get_mut(&tier_upper.to_lowercase()) {
                        if !raw_val.is_empty() {
                            entry.1 = Some(raw_val);
                        }
                    }
                }
                continue;
            }
            // Slot source: ANTHROPIC_DEFAULT_<TIER>_MODEL. The tier is the part
            // before the trailing _MODEL. A bare "MODEL" (the whole rest) has
            // no tier prefix and is skipped.
            if let Some(tier_upper) = rest.strip_suffix("_MODEL") {
                if !tier_upper.is_empty() && !raw_val.is_empty() {
                    slots
                        .entry(tier_upper.to_lowercase())
                        .or_insert((raw_val, None));
                }
            }
            // Anything else under the prefix is not a recognized shape; ignore.
        }
    }

    let mut tiers: Vec<TierSlot> = slots
        .into_iter()
        .map(|(tier, (model, name))| {
            let model_name = match name {
                Some(n) if !n.trim().is_empty() => n,
                _ => model.clone(),
            };
            TierSlot {
                tier,
                model,
                model_name,
            }
        })
        .collect();
    tiers.sort_by(|a, b| tier_sort_key(&a.tier).cmp(&tier_sort_key(&b.tier)));
    ModelInfo { tiers }
}

/// Discover every model tier slot from settings.json (see `discover_tiers`).
///
/// Dynamic discovery means new Claude Code tiers (fable today, anything later)
/// show up automatically — no hardcoded enum to keep in sync. Tiers with an
/// empty `_MODEL` value are skipped (a present-but-blank slot is not a real
/// config; showing "未配置" rows for them only clutters the menu).
pub fn read_model_info() -> ModelInfo {
    let v = read_settings_json();
    let env = v.get("env").and_then(|e| e.as_object());
    discover_tiers(env)
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
    let v = read_settings_json();
    v.get("model")
        .and_then(|m| m.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
}

/// Whether `raw` is a tier alias of a slot discovered in `info`. Used to
/// distinguish "tier alias" (the top-level `model` is one of the configured
/// tiers, e.g. "opus"/"fable") from a "direct model id" (cc-switch wrote a
/// concrete model name like "DeepSeek-V4-Pro" into the top-level `model`).
///
/// Dynamic: it matches against whatever tiers actually exist in settings.json,
/// so a newly added tier (e.g. fable) is recognized once its
/// ANTHROPIC_DEFAULT_FABLE_MODEL env var is present — no hardcoded list.
pub fn is_tier_alias(info: &ModelInfo, raw: &str) -> bool {
    info.tiers.iter().any(|s| s.tier == raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(tier: &str, model: &str, name: &str) -> crate::models::TierSlot {
        crate::models::TierSlot {
            tier: tier.to_string(),
            model: model.to_string(),
            model_name: name.to_string(),
        }
    }

    #[test]
    fn test_tier_alias_recognized_when_slot_exists() {
        // A tier alias is recognized only when a matching slot is configured.
        // This is the dynamic guarantee: fable counts once FABLE_MODEL exists.
        let info = ModelInfo {
            tiers: vec![
                slot("sonnet", "claude-sonnet-4-6", "claude-sonnet-4-6"),
                slot("opus", "claude-opus-4-8", "claude-opus-4-8"),
                slot("fable", "claude-fable-5", "claude-fable-5"),
                slot("haiku", "claude-haiku-4-5-20251001", "Claude Haiku 4.5"),
            ],
        };
        assert!(is_tier_alias(&info, "opus"));
        assert!(is_tier_alias(&info, "sonnet"));
        assert!(is_tier_alias(&info, "haiku"));
        assert!(is_tier_alias(&info, "fable"));
    }

    #[test]
    fn test_tier_alias_rejected_when_no_slot() {
        // "fable" is NOT a tier when settings.json has no FABLE_MODEL — a bare
        // top-level "model":"fable" would otherwise be mislabeled.
        let info = ModelInfo {
            tiers: vec![
                slot("sonnet", "s", "s"),
                slot("opus", "o", "o"),
                slot("haiku", "h", "h"),
            ],
        };
        assert!(!is_tier_alias(&info, "fable"));
    }

    #[test]
    fn test_direct_model_id_not_treated_as_tier() {
        // cc-switch writes the real model id into the top-level "model" field.
        // These must NOT match a tier alias (none of the tiers is named like
        // a model id). Values sampled from the user's real cc-switch.db.
        let info = ModelInfo {
            tiers: vec![
                slot("sonnet", "s", "s"),
                slot("opus", "o", "o"),
                slot("haiku", "h", "h"),
            ],
        };
        assert!(!is_tier_alias(&info, "DeepSeek-V4-Pro"));
        assert!(!is_tier_alias(&info, "gpt-5.5"));
        assert!(!is_tier_alias(&info, "gemini-3.5-flash"));
        assert!(!is_tier_alias(&info, "GLM-5.2"));
        assert!(!is_tier_alias(&info, ""));
    }

    #[test]
    fn test_discovery_orders_known_tiers_then_alpha() {
        // Well-known tiers come first in canonical order; unknown tiers follow
        // alphabetically. Keeps the switcher layout stable.
        let cases = [
            // (input tiers in env-discovery order, expected output order)
            (
                vec![("haiku", "h"), ("opus", "o"), ("sonnet", "s")],
                vec!["sonnet", "opus", "haiku"],
            ),
            (
                vec![
                    ("haiku", "h"),
                    ("fable", "f"),
                    ("opus", "o"),
                    ("sonnet", "s"),
                ],
                vec!["sonnet", "opus", "fable", "haiku"],
            ),
            (
                // unknown tier "zeta" lands after the known ones, alpha-sorted.
                vec![("zeta", "z"), ("opus", "o"), ("alpha2", "a")],
                vec!["opus", "alpha2", "zeta"],
            ),
        ];
        for (input, expected) in cases {
            let mut tiers: Vec<TierSlot> = input
                .into_iter()
                .map(|(t, m)| TierSlot {
                    tier: t.to_string(),
                    model: m.to_string(),
                    model_name: m.to_string(),
                })
                .collect();
            tiers.sort_by(|a, b| tier_sort_key(&a.tier).cmp(&tier_sort_key(&b.tier)));
            let got: Vec<&str> = tiers.iter().map(|t| t.tier.as_str()).collect();
            assert_eq!(got, expected);
        }
    }

    /// Build a serde_json env object from key/value pairs (for discover_tiers
    /// tests — exercises the REAL env-key parsing, not pre-built slots).
    fn env_obj(pairs: &[(&str, &str)]) -> serde_json::Map<String, serde_json::Value> {
        let mut m = serde_json::Map::new();
        for (k, v) in pairs {
            m.insert((*k).to_string(), serde_json::Value::String((*v).to_string()));
        }
        m
    }

    #[test]
    fn test_discover_real_four_tier_settings() {
        // Regression: the user's REAL settings.json (4 tiers incl. FABLE). An
        // earlier version of discover_tiers had a parsing bug that matched
        // "<TIER>_MODEL" against a wrong suffix check and dropped EVERY tier
        // (0 discovered), so the menu showed nothing usable. This test pins
        // the real env shape so that bug can't come back.
        let env = env_obj(&[
            ("ANTHROPIC_API_KEY", "sk-x"),
            ("ANTHROPIC_BASE_URL", "https://api.openai-next.com"),
            ("ANTHROPIC_DEFAULT_FABLE_MODEL", "claude-fable-5"),
            ("ANTHROPIC_DEFAULT_FABLE_MODEL_NAME", "claude-fable-5"),
            ("ANTHROPIC_DEFAULT_HAIKU_MODEL", "claude-haiku-4-5-thinking"),
            ("ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME", "claude-haiku-4-5-thinking"),
            ("ANTHROPIC_DEFAULT_OPUS_MODEL", "claude-opus-4-7-thinking"),
            ("ANTHROPIC_DEFAULT_OPUS_MODEL_NAME", "claude-opus-4-7-thinking"),
            ("ANTHROPIC_DEFAULT_SONNET_MODEL", "claude-sonnet-4-6-thinking"),
            ("ANTHROPIC_DEFAULT_SONNET_MODEL_NAME", "claude-sonnet-4-6-thinking"),
            ("CLAUDE_CODE_DISABLE_THINKING", "1"),
        ]);
        let info = discover_tiers(Some(&env));
        let names: Vec<&str> = info.tiers.iter().map(|t| t.tier.as_str()).collect();
        assert_eq!(names, vec!["sonnet", "opus", "fable", "haiku"]);

        // Each slot carries its real model id and clean name.
        let fable = info.tiers.iter().find(|t| t.tier == "fable").unwrap();
        assert_eq!(fable.model, "claude-fable-5");
        assert_eq!(fable.model_name, "claude-fable-5");
    }

    #[test]
    fn test_discover_uses_clean_name_when_present() {
        // The _MODEL_NAME companion overrides the raw id for display. Both
        // must be parsed off the SAME tier key (case-insensitive match), which
        // an earlier version got wrong by comparing mixed-case keys.
        let env = env_obj(&[
            ("ANTHROPIC_DEFAULT_OPUS_MODEL", "GLM-5.2[1M]"),
            ("ANTHROPIC_DEFAULT_OPUS_MODEL_NAME", "GLM-5.2"),
        ]);
        let info = discover_tiers(Some(&env));
        let opus = &info.tiers[0];
        assert_eq!(opus.tier, "opus");
        assert_eq!(opus.model, "GLM-5.2[1M]"); // raw keeps the [1M] suffix
        assert_eq!(opus.model_name, "GLM-5.2"); // clean label
    }

    #[test]
    fn test_discover_ignores_bare_default_model_and_noise() {
        // Bare ANTHROPIC_DEFAULT_MODEL (no tier) must NOT create a slot named
        // "" or "model". Unrelated vars are ignored. Empty values are skipped.
        let env = env_obj(&[
            ("ANTHROPIC_DEFAULT_MODEL", "should-be-ignored"),
            ("ANTHROPIC_DEFAULT_OPUS_MODEL", ""),
            ("ANTHROPIC_DEFAULT_SONNET_MODEL", "claude-sonnet-4-6"),
            ("ANTHROPIC_MODEL", "claude-opus-4-7"), // different prefix entirely
        ]);
        let info = discover_tiers(Some(&env));
        let names: Vec<&str> = info.tiers.iter().map(|t| t.tier.as_str()).collect();
        assert_eq!(names, vec!["sonnet"]); // opus skipped (empty), others ignored
    }

    #[test]
    fn test_discover_handles_missing_env() {
        // No env object at all (corrupt/empty settings.json) -> no tiers.
        let info = discover_tiers(None);
        assert!(info.tiers.is_empty());
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
