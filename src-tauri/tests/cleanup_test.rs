use cove_lib::cleanup::{delete_conversation, scan_orphans};
use cove_lib::related::find_related;
use std::fs;

/// 构建带全部关联数据的假 .claude 目录 (与 related_test 相同结构)
fn make_fake_claude() -> tempfile::TempDir {
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
    fs::write(
        proj.join(format!("{}.jsonl", sid2)),
        r#"{"type":"user","message":{"role":"user","content":"hi2"}}"#,
    )
    .unwrap();

    // ② 同名子目录
    let sub = proj.join(sid1).join("tool-results");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("r1.txt"), "dummy").unwrap();

    // ③ tasks
    let tasks = base.join("tasks").join(sid1);
    fs::create_dir_all(&tasks).unwrap();
    fs::write(tasks.join("todos.md"), "todo").unwrap();

    // ④ file-history
    let fh = base.join("file-history").join(sid1);
    fs::create_dir_all(&fh).unwrap();
    fs::write(fh.join("main.rs"), "snapshot").unwrap();

    // ⑤ telemetry
    let tele = base.join("telemetry");
    fs::create_dir_all(&tele).unwrap();
    fs::write(
        tele.join(format!("1p_failed_events.{}.event-uuid-1234.json", sid1)),
        "{}",
    )
    .unwrap();

    // ⑥ session-env
    fs::create_dir_all(base.join("session-env").join(sid1)).unwrap();

    // ⑦ history.jsonl
    let h1 = format!(
        r#"{{"display":"/resume","sessionId":"{}"}}"#,
        sid1
    );
    let h2 = format!(
        r#"{{"display":"/help","sessionId":"{}"}}"#,
        sid2
    );
    fs::write(base.join("history.jsonl"), format!("{}\n{}", h1, h2)).unwrap();

    dir
}

const SID1: &str = "aaaa1111-2222-3333-4444-555555555555";
const SID2: &str = "bbbb2222-3333-4444-5555-666666666666";

#[test]
fn test_delete_removes_jsonl_body() {
    let dir = make_fake_claude();
    let result = delete_conversation(SID1, "D--Test-App", dir.path());
    assert!(result.success);

    let r = find_related(SID1, "D--Test-App", dir.path());
    assert!(r.jsonl_file.is_none(), "jsonl should be deleted");
}

#[test]
fn test_delete_removes_all_related() {
    let dir = make_fake_claude();
    delete_conversation(SID1, "D--Test-App", dir.path());

    let r = find_related(SID1, "D--Test-App", dir.path());
    assert!(r.project_subdir.is_none());
    assert!(r.tasks_dir.is_none());
    assert!(r.file_history_dir.is_none());
    assert!(r.telemetry_files.is_empty());
    assert!(r.session_env_dir.is_none());
    assert_eq!(r.history_lines, 0, "history lines should be removed");
}

#[test]
fn test_delete_preserves_other_sids() {
    let dir = make_fake_claude();
    delete_conversation(SID1, "D--Test-App", dir.path());

    // SID2 的数据应该完整保留
    let r2 = find_related(SID2, "D--Test-App", dir.path());
    assert!(r2.jsonl_file.is_some(), "SID2 jsonl should remain");

    // history 里 SID2 的行应保留
    let history = fs::read_to_string(dir.path().join("history.jsonl")).unwrap();
    assert!(history.contains(SID2), "SID2 history line should remain");
    assert!(!history.contains(SID1), "SID1 history line should be gone");
}

#[test]
fn test_delete_reports_freed_bytes() {
    let dir = make_fake_claude();
    let result = delete_conversation(SID1, "D--Test-App", dir.path());
    assert!(result.freed_bytes > 0, "should report some freed bytes");
    assert!(!result.removed_paths.is_empty(), "should report removed paths");
}

#[test]
fn test_scan_orphans_finds_orphaned_data() {
    let dir = make_fake_claude();
    // 删除 SID1 的正文, 制造孤儿
    fs::remove_file(
        dir.path()
            .join("projects")
            .join("D--Test-App")
            .join(format!("{}.jsonl", SID1)),
    )
    .unwrap();

    let orphans = scan_orphans(dir.path());
    // SID1 的 tasks, file-history, session-env, telemetry 都应成为孤儿
    assert!(orphans.iter().any(|o| o.sid == SID1), "SID1 should be orphaned");
    // SID2 仍有正文, 不应是孤儿
    assert!(!orphans.iter().any(|o| o.sid == SID2), "SID2 should NOT be orphaned");
}

#[test]
fn test_scan_orphans_clean_when_no_orphans() {
    let dir = make_fake_claude();
    let orphans = scan_orphans(dir.path());
    // 所有 SID 都有正文, 无孤儿
    assert!(
        orphans.is_empty(),
        "no orphans expected, got: {:?}",
        orphans
    );
}
