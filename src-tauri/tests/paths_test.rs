use cove_lib::paths::{claude_dir, decode_project_path, encode_project_path};

#[test]
fn test_encode_drive_path() {
    assert_eq!(
        encode_project_path("D:\\Programs\\ClaudeCode"),
        "D--Programs-ClaudeCode"
    );
}

#[test]
fn test_encode_unix_style() {
    assert_eq!(
        encode_project_path("D:/Programs/ClaudeCode"),
        "D--Programs-ClaudeCode"
    );
}

#[test]
fn test_decode_roundtrip() {
    let encoded = encode_project_path("C:\\Users\\test\\app");
    assert_eq!(encoded, "C--Users-test-app");
    let decoded = decode_project_path(&encoded);
    assert_eq!(decoded, "C:\\Users\\test\\app");
}

#[test]
fn test_decode_c_drive() {
    assert_eq!(decode_project_path("C--Users-test"), "C:\\Users\\test");
}

/// Regression: fs::canonicalize on Windows adds a `\\?\` verbatim prefix
/// (e.g. `\\?\D:\Programs\Brains`). Storing that verbatim and then encoding it
/// produced `--------D-Programs-Brains`, which never matched Claude Code's own
/// `D--Programs-Brains` dir — so a project's sessions all leaked to the
/// "loose conversations" tab. encode_project_path must strip the prefix first.
#[test]
fn test_encode_strips_verbatim_prefix() {
    // The exact value canonicalize produces on Windows:
    assert_eq!(
        encode_project_path(r"\\?\D:\Programs\Projects\Brains"),
        "D--Programs-Projects-Brains"
    );
    // Forward-slash verbatim variant too:
    assert_eq!(
        encode_project_path(r"\\?\D:/Programs/Projects/Brains"),
        "D--Programs-Projects-Brains"
    );
    // No prefix — unchanged behavior:
    assert_eq!(
        encode_project_path(r"D:\Programs\Projects\Brains"),
        "D--Programs-Projects-Brains"
    );
    // `\\.\` device prefix also stripped:
    assert_eq!(
        encode_project_path(r"\\.\D:\Programs\X"),
        "D--Programs-X"
    );
}

#[test]
fn test_claude_dir_exists() {
    let dir = claude_dir();
    assert!(
        dir.join("projects").exists(),
        "projects dir should exist at {}",
        dir.display()
    );
}
