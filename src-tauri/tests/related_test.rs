use cove_lib::related::find_related;
use std::fs;

/// 构建一个假的 .claude 目录, 为 SID1 创建全部 8 处关联数据
fn make_fake_claude_with_related() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();
    let proj = base.join("projects").join("D--Test-App");
    fs::create_dir_all(&proj).unwrap();

    let sid1 = "aaaa1111-2222-3333-4444-555555555555";
    let sid2 = "bbbb2222-3333-4444-5555-666666666666";

    // ① 正文
    fs::write(
        proj.join(format!("{}.jsonl", sid1)),
        r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
    )
    .unwrap();
    // 另一个 SID 的正文(用于验证不误删)
    fs::write(
        proj.join(format!("{}.jsonl", sid2)),
        r#"{"type":"user","message":{"role":"user","content":"hi2"}}"#,
    )
    .unwrap();

    // ② 同名子目录 (tool-results)
    let sub = proj.join(sid1).join("tool-results");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("r1.txt"), "dummy").unwrap();

    // ③ tasks 目录
    let tasks = base.join("tasks").join(sid1);
    fs::create_dir_all(&tasks).unwrap();
    fs::write(tasks.join("todos.md"), "todo").unwrap();

    // ④ file-history 目录
    let fh = base.join("file-history").join(sid1);
    fs::create_dir_all(&fh).unwrap();
    fs::write(fh.join("main.rs"), "snapshot").unwrap();

    // ⑤ telemetry 文件 (文件名含 SID 作为第一个 UUID)
    let tele = base.join("telemetry");
    fs::create_dir_all(&tele).unwrap();
    fs::write(
        tele.join(format!("1p_failed_events.{}.event-uuid-1234.json", sid1)),
        "{}",
    )
    .unwrap();
    // 另一个 SID 的 telemetry
    fs::write(
        tele.join(format!("1p_failed_events.{}.event-uuid-5678.json", sid2)),
        "{}",
    )
    .unwrap();

    // ⑥ session-env 目录
    fs::create_dir_all(base.join("session-env").join(sid1)).unwrap();

    // ⑦ history.jsonl (两行: sid1 和 sid2)
    let h1 = format!(
        r#"{{"display":"/resume","timestamp":1781024794475,"project":"D:\\Test\\App","sessionId":"{}"}}"#,
        sid1
    );
    let h2 = format!(
        r#"{{"display":"/help","timestamp":1781024794476,"project":"D:\\Other","sessionId":"{}"}}"#,
        sid2
    );
    fs::write(base.join("history.jsonl"), format!("{}\n{}", h1, h2)).unwrap();

    // ⑧ sessions PID 文件 (文件内含 sessionId)
    let sess = base.join("sessions");
    fs::create_dir_all(&sess).unwrap();
    fs::write(
        sess.join("12345.json"),
        format!(r#"{{"sessionId":"{}","pid":12345}}"#, sid1),
    )
    .unwrap();

    dir
}

const SID1: &str = "aaaa1111-2222-3333-4444-555555555555";

#[test]
fn test_finds_jsonl_body() {
    let dir = make_fake_claude_with_related();
    let r = find_related(SID1, "D--Test-App", dir.path());
    assert!(r.jsonl_file.is_some(), "should find jsonl body");
}

#[test]
fn test_finds_project_subdir() {
    let dir = make_fake_claude_with_related();
    let r = find_related(SID1, "D--Test-App", dir.path());
    assert!(r.project_subdir.is_some(), "should find project subdir");
}

#[test]
fn test_finds_tasks_and_file_history() {
    let dir = make_fake_claude_with_related();
    let r = find_related(SID1, "D--Test-App", dir.path());
    assert!(r.tasks_dir.is_some(), "should find tasks dir");
    assert!(r.file_history_dir.is_some(), "should find file-history dir");
}

#[test]
fn test_finds_telemetry_files() {
    let dir = make_fake_claude_with_related();
    let r = find_related(SID1, "D--Test-App", dir.path());
    assert_eq!(r.telemetry_files.len(), 1, "should find 1 telemetry file");
}

#[test]
fn test_finds_session_env() {
    let dir = make_fake_claude_with_related();
    let r = find_related(SID1, "D--Test-App", dir.path());
    assert!(r.session_env_dir.is_some(), "should find session-env dir");
}

#[test]
fn test_counts_history_lines() {
    let dir = make_fake_claude_with_related();
    let r = find_related(SID1, "D--Test-App", dir.path());
    assert_eq!(r.history_lines, 1, "should count 1 history line for SID1");
}

#[test]
fn test_finds_session_meta() {
    let dir = make_fake_claude_with_related();
    let r = find_related(SID1, "D--Test-App", dir.path());
    assert_eq!(
        r.session_meta_files.len(),
        1,
        "should find 1 session meta file"
    );
}

#[test]
fn test_nonexistent_sid_finds_nothing() {
    let dir = make_fake_claude_with_related();
    let r = find_related("nonexistent-sid-xxxx", "D--Test-App", dir.path());
    assert!(r.jsonl_file.is_none());
    assert_eq!(r.history_lines, 0);
    assert!(r.telemetry_files.is_empty());
}

#[test]
fn test_all_paths_collects_everything() {
    let dir = make_fake_claude_with_related();
    let r = find_related(SID1, "D--Test-App", dir.path());
    let paths = r.all_paths();
    // jsonl + subdir + tasks + file-history + 1 telemetry + session-env + 1 session-meta = 7
    assert!(paths.len() >= 7, "should collect at least 7 paths, got {}: {:?}", paths.len(), paths);
}
