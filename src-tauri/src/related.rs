use crate::models::RelatedSet;
use serde::Deserialize;
use std::fs;
use std::path::Path;

/// 为给定 SID 定位所有 8 处关联数据。
/// project_encoded: 该 SID 所属的编码项目名 (用于定位 ①②)
pub fn find_related(sid: &str, project_encoded: &str, claude_root: &Path) -> RelatedSet {
    let mut set = RelatedSet::default();

    // ① 正文 jsonl
    let jsonl = claude_root
        .join("projects")
        .join(project_encoded)
        .join(format!("{}.jsonl", sid));
    if jsonl.exists() {
        set.jsonl_file = Some(jsonl.to_string_lossy().to_string());
    }

    // ② 同名子目录 (subagents/ tool-results/)
    let subdir = claude_root
        .join("projects")
        .join(project_encoded)
        .join(sid);
    if subdir.is_dir() {
        set.project_subdir = Some(subdir.to_string_lossy().to_string());
    }

    // ③ tasks 目录
    let tasks = claude_root.join("tasks").join(sid);
    if tasks.is_dir() {
        set.tasks_dir = Some(tasks.to_string_lossy().to_string());
    }

    // ④ file-history 目录
    let fh = claude_root.join("file-history").join(sid);
    if fh.is_dir() {
        set.file_history_dir = Some(fh.to_string_lossy().to_string());
    }

    // ⑤ telemetry 文件 (文件名形如 1p_failed_events.<sid>.<ts>.json)
    // 用前缀 + 分隔符精确匹配，避免裸 contains(sid) 误命中子串碰巧相同的
    // 其他文件（评审 P2 #10）。sid 在文件名中紧跟 "1p_failed_events." 之后。
    let telemetry_dir = claude_root.join("telemetry");
    if telemetry_dir.is_dir() {
        let prefix = format!("1p_failed_events.{}.", sid);
        if let Ok(entries) = fs::read_dir(&telemetry_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&prefix) && name.ends_with(".json") {
                    set.telemetry_files
                        .push(entry.path().to_string_lossy().to_string());
                }
            }
        }
    }

    // ⑥ session-env 目录
    let se = claude_root.join("session-env").join(sid);
    if se.is_dir() {
        set.session_env_dir = Some(se.to_string_lossy().to_string());
    }

    // ⑦ history.jsonl 命中行数
    let history = claude_root.join("history.jsonl");
    if history.exists() {
        set.history_lines = count_history_lines_for_sid(&history, sid);
    }

    // ⑧ sessions PID 文件 (文件内 sessionId 匹配, 尽力而为)
    let sessions_dir = claude_root.join("sessions");
    if sessions_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&sessions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(record) = serde_json::from_str::<HistoryLine>(&content) {
                            if record.session_id.as_deref() == Some(sid) {
                                set.session_meta_files
                                    .push(path.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    set
}

#[derive(Deserialize)]
struct HistoryLine {
    #[serde(default, rename = "sessionId")]
    session_id: Option<String>,
}

fn count_history_lines_for_sid(history_path: &Path, sid: &str) -> u32 {
    let content = match fs::read_to_string(history_path) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let mut count = 0;
    for line in content.lines() {
        if let Ok(record) = serde_json::from_str::<HistoryLine>(line) {
            if record.session_id.as_deref() == Some(sid) {
                count += 1;
            }
        }
    }
    count
}

/// 列出 set 中所有实际存在的路径（用于删除/归档遍历）
impl RelatedSet {
    pub fn all_paths(&self) -> Vec<String> {
        let mut v = Vec::new();
        if let Some(p) = &self.jsonl_file {
            v.push(p.clone());
        }
        if let Some(p) = &self.project_subdir {
            v.push(p.clone());
        }
        if let Some(p) = &self.tasks_dir {
            v.push(p.clone());
        }
        if let Some(p) = &self.file_history_dir {
            v.push(p.clone());
        }
        v.extend(self.telemetry_files.iter().cloned());
        if let Some(p) = &self.session_env_dir {
            v.push(p.clone());
        }
        v.extend(self.session_meta_files.iter().cloned());
        v
    }
}
