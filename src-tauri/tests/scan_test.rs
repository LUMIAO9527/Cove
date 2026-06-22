use cove_lib::scan::{parse_conversations_in_project, scan_all_conversations};
use std::fs;

/// 在临时目录里构建一个假的 .claude/projects/<encoded>/ 结构
fn make_fake_claude() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    // D:\Test\App 编码后为 D--Test-App
    let proj = dir.path().join("projects").join("D--Test-App");
    fs::create_dir_all(&proj).unwrap();

    // 对话 1: 完整对话 (有 assistant, 有 model, 中文首条用户消息)
    let sid1 = "aaaa1111-2222-3333-4444-555555555555";
    let lines = vec![
        r#"{"type":"user","message":{"role":"user","content":"如何用 Rust 写一个 HTTP 服务器"}}"#,
        r#"{"type":"assistant","message":{"role":"assistant","model":"glm-5.2","content":[{"type":"text","text":"可以使用 actix-web"}],"stop_reason":"end_turn"}}"#,
        r#"{"type":"user","message":{"role":"user","content":"谢谢，我试试"}}"#,
    ];
    fs::write(proj.join(format!("{}.jsonl", sid1)), lines.join("\n")).unwrap();

    // 对话 2: 仅一条用户消息, 无 assistant, 无 summary
    let sid2 = "bbbb2222-3333-4444-5555-666666666666";
    fs::write(
        proj.join(format!("{}.jsonl", sid2)),
        r#"{"type":"user","message":{"role":"user","content":"今天天气"}}"#,
    )
    .unwrap();

    // 对话 3: 带 summary 记录的对话
    let sid3 = "cccc3333-4444-5555-6666-777777777777";
    let lines3 = vec![
        r#"{"type":"summary","summary":"这是生成的标题"}"#,
        r#"{"type":"user","message":{"role":"user","content":"随便问"}}"#,
        r#"{"type":"assistant","message":{"role":"assistant","model":"glm-5.1","content":[{"type":"text","text":"好的"}]}}"#,
    ];
    fs::write(proj.join(format!("{}.jsonl", sid3)), lines3.join("\n")).unwrap();

    dir
}

#[test]
fn test_scan_all_finds_conversations() {
    let dir = make_fake_claude();
    let convos = scan_all_conversations(dir.path());
    assert_eq!(convos.len(), 3, "should find all 3 conversations");
    assert!(
        convos
            .iter()
            .all(|c| c.project_encoded == "D--Test-App"),
        "all conversations belong to D--Test-App"
    );
}

#[test]
fn test_parse_extracts_model_and_count() {
    let dir = make_fake_claude();
    let proj_dir = dir.path().join("projects").join("D--Test-App");
    let convos = parse_conversations_in_project(&proj_dir);
    let c = convos
        .iter()
        .find(|c| c.id == "aaaa1111-2222-3333-4444-555555555555")
        .unwrap();
    assert_eq!(c.model, "glm-5.2");
    assert_eq!(c.message_count, 3); // 2 user + 1 assistant
}

#[test]
fn test_title_falls_back_to_first_user_message() {
    let dir = make_fake_claude();
    let proj_dir = dir.path().join("projects").join("D--Test-App");
    let convos = parse_conversations_in_project(&proj_dir);
    let c = convos
        .iter()
        .find(|c| c.id == "bbbb2222-3333-4444-5555-666666666666")
        .unwrap();
    assert_eq!(c.title, "今天天气");
}

#[test]
fn test_title_uses_summary_when_present() {
    let dir = make_fake_claude();
    let proj_dir = dir.path().join("projects").join("D--Test-App");
    let convos = parse_conversations_in_project(&proj_dir);
    let c = convos
        .iter()
        .find(|c| c.id == "cccc3333-4444-5555-6666-777777777777")
        .unwrap();
    assert_eq!(c.title, "这是生成的标题");
    assert_eq!(c.model, "glm-5.1");
}

#[test]
fn test_no_model_shows_unknown() {
    let dir = make_fake_claude();
    let proj_dir = dir.path().join("projects").join("D--Test-App");
    let convos = parse_conversations_in_project(&proj_dir);
    let c = convos
        .iter()
        .find(|c| c.id == "bbbb2222-3333-4444-5555-666666666666")
        .unwrap();
    assert_eq!(c.model, "未知");
}

#[test]
fn test_scan_empty_dir_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let convos = scan_all_conversations(dir.path());
    assert!(convos.is_empty());
}
