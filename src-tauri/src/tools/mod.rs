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
    /// We probe by spawning the CLI's `--version` and checking it exits ok.
    ///
    /// **Windows quirk (root cause of "installed but shows uninstalled")**:
    /// npm-installed CLIs ship as `*.cmd` wrapper scripts (e.g. `claude.cmd`),
    /// NOT `.exe`. `std::process::Command::new("claude")` on Windows resolves
    /// the program by appending `.exe` only — it does NOT consult PATHEXT, so
    /// it never finds `claude.cmd` and the probe silently fails. Verified: bare
    /// `Command::new("claude")` → fail; `cmd /c claude` → ok.
    ///
    /// Fix: on Windows, run the probe through `cmd /c` so the command processor
    /// does PATHEXT resolution (`.CMD`/`.BAT` included). The CLI's own `--version`
    /// is fast and exits immediately, so wrapping it in cmd adds no perceptible
    /// delay. On non-Windows, PATH resolution already works without a shell.
    pub fn is_installed(&self) -> bool {
        let cli = self.cli_name();
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", cli, "--version"]);
            c
        } else {
            let mut c = std::process::Command::new(cli);
            c.arg("--version");
            c
        };
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .stdin(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
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
