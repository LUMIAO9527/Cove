use crate::models::ArchiveEntry;
use crate::models::Conversation;
use crate::related::find_related;
use crate::scan::parse_single_jsonl;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ArchiveIndex {
    pub entries: Vec<ArchiveEntry>,
}

impl ArchiveIndex {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = atomic_write(path, serde_json::to_string_pretty(self).unwrap_or_default());
    }

    pub fn add(&mut self, entry: ArchiveEntry) {
        self.entries.retain(|e| e.sid != entry.sid);
        self.entries.push(entry);
    }

    pub fn remove(&mut self, sid: &str) {
        self.entries.retain(|e| e.sid != sid);
    }
}

// ---------------------------------------------------------------------------
// 封装目录结构（v0.4.26 起）
//
// 旧结构（v0.4.25 及之前）有一个致命缺陷：archive_conversation 把
// project_subdir / tasks / file-history / session-env 四个目录的 file_name
// 全等于 <sid>，move 到同一个扁平 <archive>/<encoded>/ 下时，move_path 会
// 先 remove_dir_all(dest) 再 rename，导致后移的目录销毁先移的——除最后
// 一个幸存目录外全部数据丢失（详见 v0.4.26 评审 P0 #1）。
//
// 新结构：每个 SID 一个封装目录，内部按来源分子名，彻底消除碰撞。
//
//   <archive_root>/
//     index.json
//     <encoded>/
//       <sid>/                     ← 封装目录（每个会话一个）
//         transcript.jsonl         ← 原 <sid>.jsonl
//         project_subdir/          ← 原 projects/<encoded>/<sid>/
//         tasks/                   ← 原 tasks/<sid>/
//         file-history/            ← 原 file-history/<sid>/
//         session-env/             ← 原 session-env/<sid>/
//         session-meta.json        ← 原 sessions/<pid>.json（可能多个 → 用原名）
//         telemetry/               ← 原 telemetry/1p_failed_events.<sid>.*.json
//
// history.jsonl 行级过滤仍在原地（共享文件，不归档）。
//
// restore 按子名精确反路由到原始位置，不再用 name.contains(sid) 模糊匹配。
// ---------------------------------------------------------------------------

/// 封装目录内各来源的固定子名。
const SUB_TRANSCRIPT: &str = "transcript.jsonl";
const SUB_PROJECT_SUBDIR: &str = "project_subdir";
const SUB_TASKS: &str = "tasks";
const SUB_FILE_HISTORY: &str = "file-history";
const SUB_SESSION_ENV: &str = "session-env";
const SUB_TELEMETRY: &str = "telemetry";
const SUB_SESSION_META: &str = "session-meta";

/// 某会话的封装目录：<archive_root>/<encoded>/<sid>/
fn capsule_dir(archive_root: &Path, project_encoded: &str, sid: &str) -> PathBuf {
    archive_root.join(project_encoded).join(sid)
}

/// 归档一个对话：把正文 + 全部关联数据 move 进该 SID 的封装目录。
///
/// 返回 Ok(()) 表示全部移动成功；Err 含失败项的中文描述（供前端 toast）。
/// history.jsonl 行级过滤仍在原地，不视作移动失败。
pub fn archive_conversation(
    sid: &str,
    project_encoded: &str,
    claude_root: &Path,
    archive_root: &Path,
) -> Result<(), String> {
    let set = find_related(sid, project_encoded, claude_root);

    let capsule = capsule_dir(archive_root, project_encoded, sid);
    // 先建好封装目录及其子目录，避免 move_path 时的目标父目录不存在。
    let _ = fs::create_dir_all(&capsule);
    for sub in [
        SUB_PROJECT_SUBDIR,
        SUB_TASKS,
        SUB_FILE_HISTORY,
        SUB_SESSION_ENV,
        SUB_TELEMETRY,
    ] {
        let _ = fs::create_dir_all(capsule.join(sub));
    }

    let mut errors: Vec<String> = Vec::new();
    // 第三轮评审 GLM M1 / Qwen P3-1 修复：用结构化标志代替字符串匹配。
    // 原实现 `errors.iter().any(|e| e.contains("对话正文"))` 依赖 label 文案
    // 不变，未来改 label 会静默走错分支。改成在 move 失败处直接置 bool 标志。
    let mut transcript_failed = false;

    // ① 正文 jsonl → transcript.jsonl
    if let Some(src) = &set.jsonl_file {
        let dest = capsule.join(SUB_TRANSCRIPT);
        if let Err(e) = move_path(Path::new(src), &dest) {
            transcript_failed = true;
            errors.push(format!("对话正文: {e}"));
        }
    }
    // ② 项目子目录 → project_subdir/
    if let Some(src) = &set.project_subdir {
        let dest = capsule.join(SUB_PROJECT_SUBDIR);
        if let Err(e) = move_path(Path::new(src), &dest) {
            errors.push(format!("项目子目录: {e}"));
        }
    }
    // ③ tasks → tasks/
    if let Some(src) = &set.tasks_dir {
        let dest = capsule.join(SUB_TASKS);
        if let Err(e) = move_path(Path::new(src), &dest) {
            errors.push(format!("tasks: {e}"));
        }
    }
    // ④ file-history → file-history/
    if let Some(src) = &set.file_history_dir {
        let dest = capsule.join(SUB_FILE_HISTORY);
        if let Err(e) = move_path(Path::new(src), &dest) {
            errors.push(format!("file-history: {e}"));
        }
    }
    // ⑤ telemetry 文件 → telemetry/<原文件名>
    for src in &set.telemetry_files {
        let name = Path::new(src)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let dest = capsule.join(SUB_TELEMETRY).join(&name);
        if let Err(e) = move_path(Path::new(src), &dest) {
            errors.push(format!("telemetry/{name}: {e}"));
        }
    }
    // ⑥ session-env → session-env/
    if let Some(src) = &set.session_env_dir {
        let dest = capsule.join(SUB_SESSION_ENV);
        if let Err(e) = move_path(Path::new(src), &dest) {
            errors.push(format!("session-env: {e}"));
        }
    }
    // ⑧ sessions/<pid>.json（session_meta_files）→ session-meta/<原文件名>
    //    （原文件名是 PID，restore 时按原文件名还原到 sessions/ 下）
    if !set.session_meta_files.is_empty() {
        let meta_dir = capsule.join(SUB_SESSION_META);
        let _ = fs::create_dir_all(&meta_dir);
        for src in &set.session_meta_files {
            let name = Path::new(src)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let dest = meta_dir.join(&name);
            if let Err(e) = move_path(Path::new(src), &dest) {
                errors.push(format!("sessions/{name}: {e}"));
            }
        }
    }
    // ⑦ history.jsonl：共享文件，不归档，原地按 SID 行级过滤（忽略错误，
    //    与原行为一致——history 行过滤失败不应阻塞归档）。
    if set.history_lines > 0 {
        let _ = crate::cleanup::remove_history_lines_public(claude_root, sid);
    }

    // 写入索引（无论部分移动成功与否都写——已移动的数据需要在索引里登记，
    // 否则 list_archived_conversations 扫不到）。title 留空（list 时实时解析）。
    let index_path = archive_root.join("index.json");
    let mut index = ArchiveIndex::load(&index_path);
    index.add(ArchiveEntry {
        sid: sid.to_string(),
        project_encoded: project_encoded.to_string(),
        title: String::new(),
        archived_at: now_millis(),
    });
    index.save(&index_path);

    // 第二轮评审 GLM N1 修复：archive 的部分失败处理。
    // 原实现部分失败返回 Err → 前端 animateRemoveCard 回滚卡片 → 同一会话
    // 同时出现在来源页和归档页（状态不一致）。
    // 新策略：区分"transcript（正文）失败"和"仅附属数据失败"：
    //   - transcript 失败：归档无效（list_archived 扫不到），返回 Err 让前端
    //     留卡 + toast，用户知道归档没成功可以重试。这种情况极罕见（同卷 move
    //     几乎不会失败），但必须让用户知道。
    //   - 仅附属（tasks/file-history/telemetry 等）失败：数据已部分归档，索引
    //     已登记，返回 Ok 让前端正常移卡。归档页能看到该会话（transcript 在），
    //     个别附属缺失影响有限。这是更常见的部分失败场景。
    // transcript_failed 已在 ① 处直接置位（结构化标志，不靠字符串匹配）。
    if transcript_failed {
        // 正文没归档成功——回滚已移动的附属数据（尽力），返回 Err。
        // 把已移到 capsule 的附属 move 回原位（best-effort，失败吞掉）。
        rollback_capsule_to_sources(&capsule, sid, project_encoded, claude_root);
        // capsule 可能还有残留，清掉（里面没有有效的 transcript）。
        let _ = fs::remove_dir_all(&capsule);
        // 从索引移除（这条没成功归档）。
        let mut index = ArchiveIndex::load(&index_path);
        index.remove(sid);
        index.save(&index_path);
        Err(format!("归档失败（对话正文未移动）：{}", errors.join("；")))
    } else {
        // 仅有附属失败或全部成功：返回 Ok，前端正常移卡。
        // 附属失败项已在 capsule 里（部分移动了），索引已登记。
        Ok(())
    }
}

/// 把封装目录里的附属子项 move 回原始来源位置（best-effort）。
/// 仅在 transcript 移动失败、需要回滚已移动的附属数据时调用。
/// 失败项吞掉（不影响主流程的错误返回）。
fn rollback_capsule_to_sources(
    capsule: &Path,
    sid: &str,
    project_encoded: &str,
    claude_root: &Path,
) {
    // 复用 restore 的路由逻辑：把 capsule 各子项 move 回原位。
    // 这里不关心 errors，全部 best-effort。
    let restore_back = |sub: &str, dest: &Path| {
        let src = capsule.join(sub);
        if !src.exists() {
            return;
        }
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = move_path(&src, dest);
    };
    restore_back(
        SUB_PROJECT_SUBDIR,
        &claude_root.join("projects").join(project_encoded).join(sid),
    );
    restore_back(SUB_TASKS, &claude_root.join("tasks").join(sid));
    restore_back(
        SUB_FILE_HISTORY,
        &claude_root.join("file-history").join(sid),
    );
    restore_back(
        SUB_SESSION_ENV,
        &claude_root.join("session-env").join(sid),
    );
    // telemetry 和 session-meta 是多文件，单独处理。
    let telemetry_src = capsule.join(SUB_TELEMETRY);
    if telemetry_src.is_dir() {
        if let Ok(entries) = fs::read_dir(&telemetry_src) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let dest = claude_root.join("telemetry").join(&name);
                let _ = move_path(&entry.path(), &dest);
            }
        }
    }
    let meta_src = capsule.join(SUB_SESSION_META);
    if meta_src.is_dir() {
        if let Ok(entries) = fs::read_dir(&meta_src) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let dest = claude_root.join("sessions").join(&name);
                let _ = move_path(&entry.path(), &dest);
            }
        }
    }
}

/// 恢复归档的对话：把封装目录里的各子项按来源反路由回原位。
///
/// 路由表（与 archive_conversation 对称）：
///   transcript.jsonl      → claude_root/projects/<encoded>/<sid>.jsonl
///   project_subdir/       → claude_root/projects/<encoded>/<sid>/
///   tasks/                → claude_root/tasks/<sid>/
///   file-history/         → claude_root/file-history/<sid>/
///   session-env/          → claude_root/session-env/<sid>/
///   telemetry/*           → claude_root/telemetry/<原文件名>
///   session-meta/<pid>.json → claude_root/sessions/<pid>.json
///
/// 返回 Ok(()) 全部成功；Err 含失败项（前端 toast）。即使部分失败也继续
/// 尝试其余项，最大化恢复。最后从索引移除该 SID。
pub fn restore_conversation(
    sid: &str,
    project_encoded: &str,
    claude_root: &Path,
    archive_root: &Path,
) -> Result<(), String> {
    let capsule = capsule_dir(archive_root, project_encoded, sid);
    if !capsule.is_dir() {
        // 封装目录不存在——可能是旧结构残留或已被手动删除。
        // 从索引清掉（保持索引与磁盘一致），但**返回 Err**：前端收到 Err 会
        // toast 报错，用户知道"没有数据被恢复"；如果返回 Ok，前端会 toast
        // "已恢复到原项目"，误导用户以为数据回来了（第二轮评审 Qwen 发现）。
        let index_path = archive_root.join("index.json");
        let mut index = ArchiveIndex::load(&index_path);
        index.remove(sid);
        index.save(&index_path);
        return Err("归档数据不存在，可能已被清除".to_string());
    }

    let mut errors: Vec<String> = Vec::new();
    // 单次反路由辅助：把 capsule 下的 <sub> 还原到 <dest>。
    let mut restore_one = |sub: &str, dest: &Path, label: &str| {
        let src = capsule.join(sub);
        if !src.exists() {
            return; // 该来源原本就没有，跳过
        }
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = move_path(&src, dest) {
            errors.push(format!("{label}: {e}"));
        }
    };

    // transcript.jsonl → projects/<encoded>/<sid>.jsonl
    restore_one(
        SUB_TRANSCRIPT,
        &claude_root
            .join("projects")
            .join(project_encoded)
            .join(format!("{sid}.jsonl")),
        "对话正文",
    );
    // project_subdir → projects/<encoded>/<sid>/
    restore_one(
        SUB_PROJECT_SUBDIR,
        &claude_root
            .join("projects")
            .join(project_encoded)
            .join(sid),
        "项目子目录",
    );
    // tasks → tasks/<sid>/
    restore_one(
        SUB_TASKS,
        &claude_root.join("tasks").join(sid),
        "tasks",
    );
    // file-history → file-history/<sid>/
    restore_one(
        SUB_FILE_HISTORY,
        &claude_root.join("file-history").join(sid),
        "file-history",
    );
    // session-env → session-env/<sid>/
    restore_one(
        SUB_SESSION_ENV,
        &claude_root.join("session-env").join(sid),
        "session-env",
    );
    // telemetry/* → telemetry/<原文件名>
    let telemetry_src = capsule.join(SUB_TELEMETRY);
    if telemetry_src.is_dir() {
        if let Ok(entries) = fs::read_dir(&telemetry_src) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let dest = claude_root.join("telemetry").join(&name);
                let _ = fs::create_dir_all(dest.parent().unwrap_or(Path::new(".")));
                if let Err(e) = move_path(&entry.path(), &dest) {
                    errors.push(format!("telemetry/{name}: {e}"));
                }
            }
        }
    }
    // session-meta/<pid>.json → sessions/<pid>.json
    let meta_src = capsule.join(SUB_SESSION_META);
    if meta_src.is_dir() {
        if let Ok(entries) = fs::read_dir(&meta_src) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let dest = claude_root.join("sessions").join(&name);
                let _ = fs::create_dir_all(dest.parent().unwrap_or(Path::new(".")));
                if let Err(e) = move_path(&entry.path(), &dest) {
                    errors.push(format!("sessions/{name}: {e}"));
                }
            }
        }
    }

    // 关键安全保证（第二轮评审 DeepSeek 严重发现）：
    // **只有全部成功移出才删封装目录 + 移索引**。部分失败时保留 capsule 和
    // 索引条目——未恢复的子项仍在 capsule 里，用户可以重试 restore 或手动
    // 从 capsule 取回。原实现无条件 remove_dir_all 会销毁未恢复的数据。
    let index_path = archive_root.join("index.json");
    if errors.is_empty() {
        // 全部成功：清理封装目录 + 从索引移除。
        // capsule 里此时应已空（所有子项都 move 走了），remove_dir_all 清掉
        // 空壳。即使删 capsule 失败也不致命（数据已恢复到原位），吞错即可。
        let _ = fs::remove_dir_all(&capsule);
        let mut index = ArchiveIndex::load(&index_path);
        index.remove(sid);
        index.save(&index_path);
        Ok(())
    } else {
        // 部分失败：保留 capsule 和索引，让用户能重试或手动取回。
        // 不删 capsule、不移索引——归档页该会话仍显示，可再次点恢复。
        Err(format!(
            "部分数据恢复失败（归档保留，可重试）：{}",
            errors.join("；")
        ))
    }
}

/// 列出所有归档条目（来自 index.json）。
pub fn list_archived(archive_root: &Path) -> ArchiveIndex {
    ArchiveIndex::load(&archive_root.join("index.json"))
}

/// 列出归档区所有会话，解析成完整 Conversation（带真实标题/模型/消息数/大小/cwd）。
///
/// 新结构（v0.4.26+）：扫每个封装目录 `<archive>/<encoded>/<sid>/transcript.jsonl`。
/// 归档时间来自 index.json（jsonl 本身的 mtime 不代表归档时刻）。
///
/// title 解析优先级（复用 scan.rs 的逻辑）：custom-title > ai-title > summary >
/// lastPrompt > 最后 user 消息 > sid 前缀，绝不会出现"无标题"或裸 sid。
pub fn list_archived_conversations(archive_root: &Path) -> Vec<Conversation> {
    let mut convos: Vec<Conversation> = Vec::new();
    let index = list_archived(archive_root);
    let archived_at_of = |sid: &str| -> i64 {
        index
            .entries
            .iter()
            .find(|e| e.sid == sid)
            .map(|e| e.archived_at)
            .unwrap_or(0)
    };

    // 遍历归档区下的每个项目子目录（<archive>/<encoded>/）。
    if let Ok(proj_dirs) = fs::read_dir(archive_root) {
        for proj_entry in proj_dirs.flatten() {
            let proj_path = proj_entry.path();
            if !proj_path.is_dir() {
                continue;
            }
            let encoded = proj_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            // 每个项目目录下，每个 SID 一个封装目录。
            if let Ok(sid_dirs) = fs::read_dir(&proj_path) {
                for sid_entry in sid_dirs.flatten() {
                    let sid_path = sid_entry.path();
                    if !sid_path.is_dir() {
                        continue;
                    }
                    let sid = sid_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let transcript = sid_path.join(SUB_TRANSCRIPT);
                    if !transcript.is_file() {
                        continue; // 不是会话封装目录（或损坏）
                    }
                    // 整个封装目录的大小（含 transcript + 各子目录）。
                    let capsule_size = path_size(&sid_path);
                    if let Some(mut convo) = parse_single_jsonl(&transcript, &sid, &encoded) {
                        convo.size_bytes = capsule_size;
                        let archived_at = archived_at_of(&sid);
                        convo.last_updated = if archived_at > 0 {
                            archived_at
                        } else {
                            sid_entry
                                .metadata()
                                .and_then(|m| m.modified())
                                .map(to_unix_millis)
                                .unwrap_or(0)
                        };
                        // 标记为归档态（scan.rs 的 parse_single_jsonl 硬编码 false，
                        // 这里覆盖为 true，让前端查看器能据此隐藏"继续会话"按钮）。
                        convo.is_archived = true;
                        convos.push(convo);
                    }
                }
            }
        }
    }

    // 最近归档的排前面。
    convos.sort_by(|a, b| b.last_updated.cmp(&a.last_updated));
    convos
}

/// 物理删除归档条目：直接整个封装目录删掉。
/// 封装目录是按 SID 命名的，无歧义，不会误删其他会话的数据。
pub fn purge_archived(sid: &str, project_encoded: &str, archive_root: &Path) -> bool {
    let capsule = capsule_dir(archive_root, project_encoded, sid);
    let ok = if capsule.is_dir() {
        fs::remove_dir_all(&capsule).is_ok()
    } else {
        true // 不存在视为已删
    };
    // 若该项目目录已空，顺手清掉（保持归档区整洁）。
    if let Some(proj_dir) = capsule.parent() {
        if proj_dir.is_dir() {
            if let Ok(mut entries) = fs::read_dir(proj_dir) {
                if entries.next().is_none() {
                    let _ = fs::remove_dir_all(proj_dir);
                }
            }
        }
    }
    let mut index = ArchiveIndex::load(&archive_root.join("index.json"));
    index.remove(sid);
    index.save(&archive_root.join("index.json"));
    ok
}

/// 清空整个归档区（用于首启迁移：v0.4.26 前的旧结构已被 #1 bug 销毁过，
/// 数据不完整无法找回，直接清掉重头开始）。保留 archive_root 本身。
///
/// **跳过 `.archive-v2` marker 文件**——这是迁移完成标记，必须跨 clear_all
/// 保留，否则每次启动都会重跑迁移，清掉用户第一次迁移后新归档的数据
/// （第二轮评审 Qwen 发现的真实数据丢失 bug）。其他所有内容（index.json、
/// 各项目子目录、封装目录）一律清掉。
pub fn clear_all(archive_root: &Path) -> Result<(), String> {
    if !archive_root.exists() {
        return Ok(());
    }
    if let Ok(entries) = fs::read_dir(archive_root) {
        for entry in entries.flatten() {
            let p = entry.path();
            // 保留迁移 marker，防重复清空。
            if p.file_name().and_then(|n| n.to_str()) == Some(".archive-v2") {
                continue;
            }
            let r = if p.is_dir() {
                fs::remove_dir_all(&p)
            } else {
                fs::remove_file(&p)
            };
            if let Err(e) = r {
                return Err(format!("清空归档失败 ({p:?}): {e}"));
            }
        }
    }
    Ok(())
}

fn move_path(src: &Path, dest: &Path) -> Result<(), std::io::Error> {
    if !src.exists() {
        return Ok(()); // 源不存在，视为已移动
    }
    if dest.exists() {
        if dest.is_dir() {
            fs::remove_dir_all(dest)?;
        } else {
            fs::remove_file(dest)?;
        }
    }
    match fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(_) => {
            // 跨卷 rename 会失败，回退 copy + remove。
            if src.is_dir() {
                copy_dir_recursive(src, dest)?;
                fs::remove_dir_all(src)
            } else {
                fs::copy(src, dest)?;
                fs::remove_file(src)
            }
        }
    }
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), std::io::Error> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn to_unix_millis(time: SystemTime) -> i64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn path_size(path: &Path) -> u64 {
    if path.is_file() {
        fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    } else if path.is_dir() {
        let mut total = 0;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                total += path_size(&entry.path());
            }
        }
        total
    } else {
        0
    }
}

/// 原子写：写 `<path>.tmp` 再 rename 成 `<path>`。同一卷上 rename 是原子的，
/// 写入中途崩溃（断电/BSOD）只会留下 .tmp 残留，不会截断正式文件。
/// 失败时尝试清掉 .tmp（不影响正式文件）。
pub fn atomic_write(path: &Path, content: String) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &content)?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Windows 上若目标已存在 rename 会失败；删掉目标再试一次。
            let _ = fs::remove_file(path);
            fs::rename(&tmp, path).map_err(|e2| {
                // 二次失败：清掉 .tmp 残留，把后一个错误透出。
                let _ = fs::remove_file(&tmp);
                e2
            })?;
            // 第一次的错误 e 不再用；保留变量避免警告。
            drop(e);
            Ok(())
        }
    }
}

// ===========================================================================
// Reasonix 归档（扁平 sidecar 模型，与 Claude 的 8 处关联数据完全不同）
// ===========================================================================
// 布局：<archive_root>/reasonix/<sid>/  存该会话的全部 sidecar（扁平，不分子目录）。
// reasonix 的会话 = <name>.jsonl + <name>.meta.json + <name>.events.jsonl 等，
// 归档时整体移进 capsule，恢复时整体移回原 sessions 目录。

/// Reasonix 归档区根：<archive_root>/reasonix/
fn reasonix_archive_root(archive_root: &Path) -> PathBuf {
    archive_root.join("reasonix")
}

/// 单个 reasonix 会话的 capsule 目录。
fn reasonix_capsule(archive_root: &Path, sid: &str) -> PathBuf {
    reasonix_archive_root(archive_root).join(sid)
}

/// Reasonix 会话的 meta.json 路径（无论在 sessions 还是归档区，都按 stem 找）。
fn reasonix_meta_of(dir: &Path, stem: &str) -> PathBuf {
    dir.join(format!("{stem}.meta.json"))
}

/// 归档一个 reasonix 会话：把 <sid>.jsonl + 全部 sidecar 移进 capsule。
/// 返回 Ok(()) 全部成功；Err 含失败项。
pub fn archive_reasonix_session(sid: &str, archive_root: &Path) -> Result<(), String> {
    let paths = crate::tools::reasonix::session_data_paths(sid);
    if paths.is_empty() {
        return Err(format!("找不到会话文件: {sid}"));
    }
    let capsule = reasonix_capsule(archive_root, sid);
    let _ = fs::create_dir_all(&capsule);

    let mut errors: Vec<String> = Vec::new();
    for src in &paths {
        // 文件名保留原样（含 .meta.json / .events.jsonl 等）。
        let fname = src.file_name().unwrap_or_default();
        let dest = capsule.join(fname);
        if let Err(e) = move_path(src, &dest) {
            errors.push(format!("{}: {e}", src.display()));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// 恢复一个归档的 reasonix 会话：把 capsule 里的文件移回 sessions 目录。
pub fn restore_reasonix_session(sid: &str, archive_root: &Path) -> Result<(), String> {
    let capsule = reasonix_capsule(archive_root, sid);
    if !capsule.is_dir() {
        return Err(format!("归档中找不到会话: {sid}"));
    }
    let sessions_dir = crate::paths::reasonix_dir().join("sessions");
    let _ = fs::create_dir_all(&sessions_dir);

    let mut errors: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(&capsule) {
        for entry in entries.flatten() {
            let src = entry.path();
            let fname = src.file_name().unwrap_or_default();
            let dest = sessions_dir.join(fname);
            if let Err(e) = move_path(&src, &dest) {
                errors.push(format!("{}: {e}", src.display()));
            }
        }
    }
    // capsule 已空则删掉，保持归档区整洁。
    let _ = fs::remove_dir_all(&capsule);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// 物理删除归档的 reasonix 会话：整个 capsule 删掉。
pub fn purge_archived_reasonix(sid: &str, archive_root: &Path) -> bool {
    let capsule = reasonix_capsule(archive_root, sid);
    if capsule.is_dir() {
        fs::remove_dir_all(&capsule).is_ok()
    } else {
        true
    }
}

/// 列出归档区的所有 reasonix 会话（解析成 Conversation，供归档页展示）。
pub fn list_archived_reasonix(archive_root: &Path) -> Vec<Conversation> {
    let root = reasonix_archive_root(archive_root);
    let mut convos: Vec<Conversation> = Vec::new();
    let Ok(entries) = fs::read_dir(&root) else {
        return convos;
    };
    for entry in entries.flatten() {
        let capsule = entry.path();
        if !capsule.is_dir() {
            continue;
        }
        let sid = capsule.file_name().unwrap_or_default().to_string_lossy().to_string();
        // capsule 里找 <sid>.jsonl 作为正文。
        let jsonl = capsule.join(format!("{sid}.jsonl"));
        if !jsonl.exists() {
            continue;
        }
        let meta_path = reasonix_meta_of(&capsule, &sid);
        let meta_text = fs::read_to_string(&meta_path).ok();
        let meta: serde_json::Value = meta_text
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::Value::Null);
        let cwd = meta.get("workspace").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let model = meta.get("model").and_then(|v| v.as_str()).unwrap_or("未知").to_string();

        let metadata = fs::metadata(&jsonl).ok();
        let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let last_updated = metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0))
            .unwrap_or(0);

        // 标题：从 jsonl 取首条 user 文本，否则用 sid。
        let title = first_user_text(&jsonl)
            .map(|t| truncate(&t, 60))
            .unwrap_or_else(|| format!("会话 {}", &sid[..sid.len().min(20)]));
        let preview = first_user_text(&jsonl)
            .map(|t| truncate(&t, 120))
            .unwrap_or_default();

        let message_count = count_lines(&jsonl);

        convos.push(Conversation {
            id: sid.clone(),
            title,
            project_encoded: cwd.clone(),
            model,
            message_count,
            size_bytes,
            first_user_preview: preview,
            last_updated,
            is_archived: true,
            cwd,
        });
    }
    convos.sort_by(|a, b| b.last_updated.cmp(&a.last_updated));
    convos
}

fn first_user_text(jsonl: &Path) -> Option<String> {
    let content = fs::read_to_string(jsonl).ok()?;
    for line in content.lines() {
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("role").and_then(|r| r.as_str()) != Some("user") {
            continue;
        }
        if let Some(s) = v.get("content").and_then(|c| c.as_str()) {
            let t = s.trim();
            if !t.is_empty() && !t.starts_with('<') {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn truncate(s: &str, max_chars: usize) -> String {
    let first_line = s.split(|c| c == '\r' || c == '\n').find(|l| !l.trim().is_empty()).unwrap_or("");
    let stripped = first_line.trim_start_matches('#').trim();
    let chars: Vec<char> = stripped.chars().collect();
    if chars.len() > max_chars {
        format!("{}...", chars[..max_chars].iter().collect::<String>())
    } else {
        stripped.to_string()
    }
}

fn count_lines(path: &Path) -> u32 {
    fs::read_to_string(path)
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count() as u32)
        .unwrap_or(0)
}
