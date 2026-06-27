//! Multi-tool support: Claude Code + Reasonix.
//!
//! Each tool has a completely different data layout and session schema, so
//! scan / transcript / launch are implemented per tool module. Archive /
//! cleanup / related remain Claude-only (see `tools::claude`).
//!
//! See HANDOFF.md §"多 CLI 工具管理" for the design and the per-tool data facts.

pub mod claude;
pub mod reasonix;

use crate::models::{Conversation, SessionTranscript};
use crate::paths::{claude_dir, reasonix_dir};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Which CLI tool a command targets. Serialized lowercase over IPC so the
/// frontend passes `"claude"` / `"reasonix"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolKind {
    Claude,
    Reasonix,
}

impl ToolKind {
    /// Parse a tool name from the frontend. Empty / unknown defaults to Claude
    /// (the legacy tool — old frontends that don't pass `tool` keep working).
    pub fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "reasonix" => ToolKind::Reasonix,
            _ => ToolKind::Claude,
        }
    }

    /// The tool's data root directory (~/.claude / %AppData%\reasonix).
    pub fn data_dir(&self) -> PathBuf {
        match self {
            ToolKind::Claude => claude_dir(),
            ToolKind::Reasonix => reasonix_dir(),
        }
    }

    /// The CLI command name as it lives on PATH after a standard install
    /// (`npm i -g ...`). NOT a hardcoded absolute path — different machines
    /// install to different npm prefixes, so we rely on PATH like `claude` did.
    pub fn cli_name(&self) -> &'static str {
        match self {
            ToolKind::Claude => "claude",
            ToolKind::Reasonix => "reasonix",
        }
    }

    /// The shell command to launch a session. `sid=None` starts a fresh session;
    /// `sid=Some` resumes.
    ///
    /// Claude uses `claude --resume <sid>`. Reasonix's `code` mode has no
    /// per-name resume (`--session` only exists on `chat`), so resume = run
    /// `reasonix code -r` in the target cwd, which resumes that workspace's
    /// latest session. The cwd is passed to spawn_terminal separately.
    pub fn launch_cmd(&self, sid: Option<&str>) -> String {
        let cli = self.cli_name();
        match (self, sid) {
            (ToolKind::Claude, Some(s)) if !s.is_empty() => {
                format!("{} --resume {}", cli, s)
            }
            (ToolKind::Reasonix, Some(s)) if !s.is_empty() => "reasonix code -r".to_string(),
            _ => cli.to_string(),
        }
    }

    /// Human-readable display name (for UI labels / error messages).
    pub fn display_name(&self) -> &'static str {
        match self {
            ToolKind::Claude => "Claude Code",
            ToolKind::Reasonix => "Reasonix",
        }
    }

    /// Whether the CLI is installed and reachable on PATH.
    ///
    /// Implementation: **scan PATH directories for an executable file** matching
    /// `cli_name`, resolving PATHEXT on Windows. This is a pure filesystem
    /// existence check — O(number of PATH dirs × PATHEXT extensions), typically
    /// <1ms — instead of spawning the CLI and waiting for it to print `--version`.
    ///
    /// **Why not spawn `--version` anymore (startup-latency fix):** the previous
    /// probe ran `cmd /C claude --version`. On Windows `claude --version` has to
    /// boot Node.js + load the package, costing 300–800ms per CLI. `get_installed_tools`
    /// probes two CLIs **serially**, so the popup's first paint blocked on
    /// ~0.6–1.6s of process spawning — that was the "启动卡顿". Switching to a
    /// PATH scan makes install detection instant.
    ///
    /// **PATHEXT correctness:** npm-installed CLIs ship as `.cmd` wrappers (not
    /// `.exe`). Rust's `Command::new("claude")` only tries `.exe` and misses
    /// `claude.cmd` — the original "installed but shows uninstalled" bug. The
    /// PATH scan here checks every PATHEXT extension (`.COM;.EXE;.BAT;.CMD;...`),
    /// matching exactly what `where`/`cmd` resolve, so `.cmd` CLIs are found.
    pub fn is_installed(&self) -> bool {
        which_on_path(self.cli_name()).is_some()
    }

    // ---- per-tool dispatch helpers ----
    // These wrap the per-tool modules so commands.rs can call one method
    // instead of a match at every call site.

    /// Loose conversations: all sessions minus those under registered projects.
    pub fn scan_loose(&self, registered_cwds: &[String]) -> Vec<Conversation> {
        match self {
            ToolKind::Claude => claude::scan_loose(registered_cwds),
            ToolKind::Reasonix => reasonix::scan_loose(registered_cwds),
        }
    }

    /// Conversations for one working directory (a registered project's path).
    pub fn conversations_for_path(&self, cwd: &str) -> Vec<Conversation> {
        match self {
            ToolKind::Claude => claude::conversations_for_path(cwd),
            ToolKind::Reasonix => reasonix::conversations_for_path(cwd),
        }
    }

    /// Full transcript of one session, for the history viewer.
    pub fn parse_transcript(&self, session_path: &PathBuf, sid: &str) -> Option<SessionTranscript> {
        match self {
            ToolKind::Claude => claude::parse_transcript(session_path, sid),
            ToolKind::Reasonix => reasonix::parse_transcript(session_path, sid),
        }
    }

    /// Resolve a session's transcript file path from (sid, project_key).
    /// `project_key` is the Claude encoded dir for Claude, and the cwd for the
    /// others (used to locate the right file among many).
    pub fn session_path(&self, sid: &str, project_key: &str) -> Option<PathBuf> {
        match self {
            ToolKind::Claude => claude::session_path(sid, project_key),
            ToolKind::Reasonix => reasonix::session_path(sid, project_key),
        }
    }
}

/// Resolve `name` to an existing executable on PATH (like the `which`/`where`
/// command). Returns the first match as an absolute PathBuf, or None.
///
/// On Windows, tries every extension in PATHEXT (`.COM;.EXE;.BAT;.CMD;...`) in
/// each PATH dir — exactly mirroring how `cmd` resolves commands, so `.cmd`
/// npm wrappers (e.g. `claude.cmd`) are found. This is a pure file-existence
/// scan; no process is spawned, so it's effectively instant (<1ms).
///
/// On Unix, tries `name` bare then `name` + no extension in each PATH dir.
fn which_on_path(name: &str) -> Option<std::path::PathBuf> {
    let path_env = std::env::var_os("PATH")?;
    let dirs = std::env::split_paths(&path_env);

    // PATHEXT extensions to try on Windows. Default mirrors the system default
    // if the env var is missing. Lowercased for case-insensitive compare.
    #[cfg(target_os = "windows")]
    let exts: Vec<String> = std::env::var("PATHEXT")
        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC".to_string())
        .split(';')
        .map(|s| s.to_lowercase())
        .collect();
    #[cfg(not(target_os = "windows"))]
    let exts: Vec<String> = vec![String::new()]; // no extension appending on Unix

    for dir in dirs {
        #[cfg(target_os = "windows")]
        {
            // On Windows, try the bare name first (e.g. `claude` with no ext
            // is rare, but cheap), then each PATHEXT ext (`claude.cmd`, etc.).
            let candidates: Vec<std::path::PathBuf> = std::iter::once(dir.join(name))
                .chain(exts.iter().map(|e| dir.join(format!("{}{}", name, e))))
                .collect();
            for c in candidates {
                if c.is_file() {
                    return Some(c);
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let c = dir.join(name);
            if c.is_file() {
                return Some(c);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_name() {
        assert_eq!(ToolKind::from_name("claude"), ToolKind::Claude);
        assert_eq!(ToolKind::from_name("REASONIX"), ToolKind::Reasonix);
        assert_eq!(ToolKind::from_name("reasonix"), ToolKind::Reasonix);
        // empty / unknown -> Claude (legacy default)
        assert_eq!(ToolKind::from_name(""), ToolKind::Claude);
        assert_eq!(ToolKind::from_name("garbage"), ToolKind::Claude);
    }

    #[test]
    fn test_launch_cmd_fresh() {
        assert_eq!(ToolKind::Claude.launch_cmd(None), "claude");
        assert_eq!(ToolKind::Reasonix.launch_cmd(None), "reasonix");
    }

    #[test]
    fn test_launch_cmd_resume() {
        let sid = "abc123";
        assert_eq!(ToolKind::Claude.launch_cmd(Some(sid)), "claude --resume abc123");
        // Reasonix code mode has no per-name resume — it resumes the cwd's latest.
        assert_eq!(ToolKind::Reasonix.launch_cmd(Some(sid)), "reasonix code -r");
    }

    #[test]
    fn test_launch_cmd_empty_sid_is_fresh() {
        // Some("") should behave like a fresh session (no resume of nothing).
        assert_eq!(ToolKind::Claude.launch_cmd(Some("")), "claude");
        assert_eq!(ToolKind::Reasonix.launch_cmd(Some("")), "reasonix");
    }

    #[test]
    fn test_serde_lowercase() {
        assert_eq!(
            serde_json::to_string(&ToolKind::Reasonix).unwrap(),
            "\"reasonix\""
        );
        let t: ToolKind = serde_json::from_str("\"reasonix\"").unwrap();
        assert_eq!(t, ToolKind::Reasonix);
    }
}
