use cove_lib::archive::{
    archive_conversation, list_archived, purge_archived, restore_conversation, ArchiveIndex,
};
use std::fs;

/// 构建带关联数据的假 .claude 目录
fn make_fake_claude() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();
    let proj = base.join("projects").join("D--Test-App");
    fs::create_dir_all(&proj).unwrap();

    let sid1 = "aaaa1111-2222-3333-4444-555555555555";

    // ① 正文
    fs::write(
        proj.join(format!("{}.jsonl", sid1)),
        r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
    )
    .unwrap();

    // ② 同名子目录
    let sub = proj.join(sid1).join("tool-results");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("r1.txt"), "dummy").unwrap();

    // ③ tasks
    fs::create_dir_all(base.join("tasks").join(sid1)).unwrap();
    // ④ file-history
    fs::create_dir_all(base.join("file-history").join(sid1)).unwrap();

    dir
}

fn make_archive_root() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

const SID1: &str = "aaaa1111-2222-3333-4444-555555555555";

#[test]
fn test_archive_moves_jsonl_out() {
    let claude = make_fake_claude();
    let archive = make_archive_root();

    archive_conversation(SID1, "D--Test-App", claude.path(), archive.path())
        .expect("archive should succeed");

    // 原位置不应再有 jsonl
    assert!(!claude
        .path()
        .join("projects")
        .join("D--Test-App")
        .join(format!("{}.jsonl", SID1))
        .exists());
    // 新结构：jsonl 在封装目录下叫 transcript.jsonl
    let archived = archive
        .path()
        .join("D--Test-App")
        .join(SID1)
        .join("transcript.jsonl");
    assert!(archived.exists(), "archived transcript.jsonl should exist");
}

#[test]
fn test_archive_moves_subdir() {
    let claude = make_fake_claude();
    let archive = make_archive_root();

    archive_conversation(SID1, "D--Test-App", claude.path(), archive.path())
        .expect("archive should succeed");

    // 同名子目录也应被移走（到封装目录的 project_subdir/）
    assert!(!claude
        .path()
        .join("projects")
        .join("D--Test-App")
        .join(SID1)
        .exists());
    let archived_subdir = archive
        .path()
        .join("D--Test-App")
        .join(SID1)
        .join("project_subdir");
    assert!(
        archived_subdir.is_dir(),
        "project_subdir should be archived"
    );
}

/// 回归测试：v0.4.26 P0 #1 修复。
/// 旧 bug：project_subdir/tasks/file-history/session-env 四个目录 basename 全等于
/// <sid>，扁平 move 到同一目标，后者销毁前者。新结构用封装目录隔离，必须全部保留。
#[test]
fn test_archive_preserves_all_same_name_dirs() {
    let claude = make_fake_claude();
    let archive = make_archive_root();
    let base = claude.path();

    // make_fake_claude 已建了 project_subdir、tasks、file-history；补一个 session-env。
    fs::create_dir_all(base.join("session-env").join(SID1)).unwrap();
    // 给每个目录放一个能识别的标记文件，确认归档后内容都在。
    fs::write(
        base.join("projects")
            .join("D--Test-App")
            .join(SID1)
            .join("marker-subdir"),
        "x",
    )
    .unwrap();
    fs::write(base.join("tasks").join(SID1).join("marker-tasks"), "x").unwrap();
    fs::write(
        base.join("file-history").join(SID1).join("marker-fh"),
        "x",
    )
    .unwrap();
    fs::write(
        base.join("session-env").join(SID1).join("marker-se"),
        "x",
    )
    .unwrap();

    archive_conversation(SID1, "D--Test-App", claude.path(), archive.path())
        .expect("archive should succeed");

    let capsule = archive.path().join("D--Test-App").join(SID1);
    // 四个同名 SID 目录必须各自归档到封装目录下的子目录，互不销毁。
    assert!(
        capsule.join("project_subdir").join("marker-subdir").exists(),
        "project_subdir content must survive"
    );
    assert!(
        capsule.join("tasks").join("marker-tasks").exists(),
        "tasks content must survive"
    );
    assert!(
        capsule.join("file-history").join("marker-fh").exists(),
        "file-history content must survive"
    );
    assert!(
        capsule.join("session-env").join("marker-se").exists(),
        "session-env content must survive"
    );
}

#[test]
fn test_restore_moves_back() {
    let claude = make_fake_claude();
    let archive = make_archive_root();

    archive_conversation(SID1, "D--Test-App", claude.path(), archive.path())
        .expect("archive should succeed");
    restore_conversation(SID1, "D--Test-App", claude.path(), archive.path())
        .expect("restore should succeed");

    // 原位置应恢复
    assert!(claude
        .path()
        .join("projects")
        .join("D--Test-App")
        .join(format!("{}.jsonl", SID1))
        .exists());
    // 封装目录应整体清掉
    assert!(!archive
        .path()
        .join("D--Test-App")
        .join(SID1)
        .exists());
}

/// 回归测试：restore 必须把各关联目录还原到原始位置，而非全塞回 projects/<encoded>/。
/// 这是 P0 #2 的修复——旧 restore 把 tasks/file-history/session-env 错放成项目子目录。
#[test]
fn test_restore_routes_to_original_locations() {
    let claude = make_fake_claude();
    let archive = make_archive_root();
    let base = claude.path();

    fs::create_dir_all(base.join("session-env").join(SID1)).unwrap();
    fs::write(base.join("tasks").join(SID1).join("marker-tasks"), "x").unwrap();
    fs::write(
        base.join("file-history").join(SID1).join("marker-fh"),
        "x",
    )
    .unwrap();
    fs::write(
        base.join("session-env").join(SID1).join("marker-se"),
        "x",
    )
    .unwrap();

    archive_conversation(SID1, "D--Test-App", base, archive.path())
        .expect("archive should succeed");
    restore_conversation(SID1, "D--Test-App", base, archive.path())
        .expect("restore should succeed");

    // 各目录必须回到原始全局位置（不是 projects/<encoded>/<sid>）。
    assert!(
        base.join("tasks").join(SID1).join("marker-tasks").exists(),
        "tasks must restore to tasks/<sid>/"
    );
    assert!(
        base.join("file-history")
            .join(SID1)
            .join("marker-fh")
            .exists(),
        "file-history must restore to file-history/<sid>/"
    );
    assert!(
        base.join("session-env").join(SID1).join("marker-se").exists(),
        "session-env must restore to session-env/<sid>/"
    );
}

#[test]
fn test_index_records_archive() {
    let claude = make_fake_claude();
    let archive = make_archive_root();

    archive_conversation(SID1, "D--Test-App", claude.path(), archive.path())
        .expect("archive should succeed");

    let index = list_archived(archive.path());
    assert!(
        index.entries.iter().any(|e| e.sid == SID1),
        "index should contain archived SID"
    );
}

#[test]
fn test_index_removed_on_restore() {
    let claude = make_fake_claude();
    let archive = make_archive_root();

    archive_conversation(SID1, "D--Test-App", claude.path(), archive.path())
        .expect("archive should succeed");
    restore_conversation(SID1, "D--Test-App", claude.path(), archive.path())
        .expect("restore should succeed");

    let index = list_archived(archive.path());
    assert!(
        !index.entries.iter().any(|e| e.sid == SID1),
        "index should NOT contain restored SID"
    );
}

#[test]
fn test_purge_deletes_archived() {
    let claude = make_fake_claude();
    let archive = make_archive_root();

    archive_conversation(SID1, "D--Test-App", claude.path(), archive.path())
        .expect("archive should succeed");
    purge_archived(SID1, "D--Test-App", archive.path());

    // 封装目录应被整体删除
    assert!(!archive
        .path()
        .join("D--Test-App")
        .join(SID1)
        .exists());
    // 索引应移除
    let index = list_archived(archive.path());
    assert!(!index.entries.iter().any(|e| e.sid == SID1));
}

/// 回归测试：restore 封装目录不存在时返回 Err（而非 Ok 误导用户）。
/// 第二轮评审 Qwen 发现：原实现返回 Ok，前端 toast "已恢复到原项目"，
/// 但实际没有数据被恢复。v0.4.28 改为返回 Err。
#[test]
fn test_restore_missing_capsule_returns_err() {
    let claude = make_fake_claude();
    let archive = make_archive_root();

    // capsule 不存在（模拟归档后被外部删除），restore 应返回 Err。
    let result = restore_conversation(SID1, "D--Test-App", claude.path(), archive.path());
    assert!(result.is_err(), "restore of missing capsule must return Err");
}

/// 回归测试：clear_all 跳过 .archive-v2 marker。
/// 第二轮评审 Qwen 发现：原 clear_all 清掉所有内容包括 marker，导致每次
/// release 启动都重跑迁移，清掉用户第一次迁移后新归档的数据。v0.4.28 修复。
#[test]
fn test_clear_all_preserves_marker() {
    let archive = make_archive_root();
    let root = archive.path();
    std::fs::create_dir_all(root.join("D--Proj").join("some-sid")).unwrap();
    std::fs::write(
        root.join("D--Proj").join("some-sid").join("transcript.jsonl"),
        "{}",
    )
    .unwrap();
    std::fs::write(root.join("index.json"), "{}").unwrap();
    // 写 marker
    std::fs::write(root.join(".archive-v2"), "v0.4.28").unwrap();

    cove_lib::archive::clear_all(root).expect("clear_all should succeed");

    // marker 必须保留，其他内容清掉。
    assert!(
        root.join(".archive-v2").exists(),
        "marker must survive clear_all"
    );
    assert!(!root.join("index.json").exists(), "index.json should be cleared");
    assert!(
        !root.join("D--Proj").exists(),
        "project dir should be cleared"
    );
}

#[test]
fn test_archive_index_load_save_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.json");
    let mut index = ArchiveIndex::load(&path);
    assert!(index.entries.is_empty());

    index.add(cove_lib::models::ArchiveEntry {
        sid: "test-sid".to_string(),
        project_encoded: "D--Test".to_string(),
        title: "标题".to_string(),
        archived_at: 12345,
    });
    index.save(&path);

    let reloaded = ArchiveIndex::load(&path);
    assert_eq!(reloaded.entries.len(), 1);
    assert_eq!(reloaded.entries[0].sid, "test-sid");
}
