use std::path::PathBuf;

/// 返回 .claude 根目录 (C:\Users\<user>\.claude)
pub fn claude_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .expect("USERPROFILE or HOME must be set");
    PathBuf::from(home).join(".claude")
}

/// 返回 Reasonix 数据根目录。优先 $REASONIX_HOME（Reasonix 官方支持的环境变量覆盖），
/// 否则按平台：Windows %AppData%\reasonix\，其他 ~/.reasonix（见 Reasonix CONFIG_PATHS.md）。
pub fn reasonix_dir() -> PathBuf {
    if let Ok(home) = std::env::var("REASONIX_HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home);
        }
    }
    #[cfg(target_os = "windows")]
    {
        // %AppData% = Roaming（Reasonix 官方 Windows 路径用 Roaming）
        if let Ok(appdata) = std::env::var("APPDATA") {
            if !appdata.trim().is_empty() {
                return PathBuf::from(appdata).join("reasonix");
            }
        }
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .expect("USERPROFILE or HOME must be set");
    PathBuf::from(home).join(".reasonix")
}

/// 将工作目录路径编码为 projects 下的目录名。
/// 规则: 将 ':' '\' '/' 各自替换为 '-'。
///   "D:\\Programs\\ClaudeCode" -> "D--Programs-ClaudeCode"
///   (盘符冒号 + 第一个反斜杠 => "D--")
///
/// 会先剥离 Windows 长路径前缀 `\\?\`（`fs::canonicalize` 在 Windows 上会加上它，
/// 例如 `\\?\D:\Programs\X`）。如果不剥离，前缀里的 `\` 会全变成 `-`，
/// 编码出 `--------D-Programs-X`，和 Claude Code 自己存的 `D--Programs-X`
/// 对不上 —— 这会导致注册项目的会话全部被误判为"散落对话"。这是 v0.4.9 修复
/// "项目里新开的会话跑到散落对话页"的根因。
pub fn encode_project_path(path: &str) -> String {
    let trimmed = strip_verbatim_prefix(path);
    trimmed
        .replace(':', "-")
        .replace('\\', "-")
        .replace('/', "-")
}

/// Public wrapper for use by projects_config (storing clean paths).
pub fn strip_verbatim_prefix_pub(path: &str) -> &str {
    strip_verbatim_prefix(path)
}

/// 去掉 Windows `\\?\` / `\\.\` verbatim 前缀（canonicalize 会加）。
/// 大小写不敏感。注意 `\\?\unc\` 也以 `\\?\` 开头，要先判 unc 再判普通 verbatim。
fn strip_verbatim_prefix(path: &str) -> &str {
    let p = path.trim_start();
    // 字节级匹配，避免非 ASCII 开头时的 char boundary 问题（路径几乎都是 ASCII 前缀）。
    let bytes = p.as_bytes();
    // "\\?\unc\\" (8 bytes) -> "\\" + rest; 即保留 2 个反斜杠作 UNC 前缀
    if bytes.len() >= 8
        && &bytes[..8] == b"\\\\?\\unc"
        && (bytes[7] == b'\\' || bytes[7] == b'/')
    {
        // 取 "unc" 之后的内容，前面补回 "\\"
        // p[8..] = "server\share..."，拼成 "\\server\share..."
        // 但我们要返回 &str，无法拼接；改为返回去掉 "\\?\unc" (6字节) 后的部分，
        // 这样剩下 "\\server\share" —— 正好是标准 UNC 形式。
        return &p[6..];
    }
    // "\\?\" (4 bytes) 或 "\\.\" (4 bytes) -> 去掉
    if bytes.len() >= 4 && (bytes[0..4] == [b'\\', b'\\', b'?', b'\\'] || bytes[0..4] == [b'\\', b'\\', b'.', b'\\']) {
        return &p[4..];
    }
    p
}

/// 反向解码: 将编码名还原为 Windows 路径 (尽力而为)。
/// 编码后形如 "D--Programs-ClaudeCode":
///   第一个 "--" => ":\\", 后续每个 "-" => "\\"
pub fn decode_project_path(encoded: &str) -> String {
    if let Some(pos) = encoded.find("--") {
        let (drive, rest) = encoded.split_at(pos);
        let drive_letter = drive.chars().next().unwrap_or('C');
        let rest = &rest[2..]; // 跳过 --
        return format!("{}:\\{}", drive_letter, rest.replace('-', "\\"));
    }
    encoded.to_string()
}

/// 归档存放目录 (与 .claude 同级的 .claude-managed\archive)
pub fn archive_dir() -> PathBuf {
    claude_dir()
        .parent()
        .expect(".claude should have a parent directory")
        .join(".claude-managed")
        .join("archive")
}
