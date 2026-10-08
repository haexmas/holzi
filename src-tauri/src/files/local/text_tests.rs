use super::*;

fn file_with(content: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("datei");
    std::fs::write(&path, content).unwrap();
    (dir, path)
}

#[test]
fn a_short_text_comes_back_whole() {
    let (_dir, path) = file_with("Grüße\nzweite Zeile".as_bytes());
    assert_eq!(
        read_text(&path, TEXT_LIMIT).unwrap(),
        TextContent {
            text: "Grüße\nzweite Zeile".to_owned(),
            truncated: false
        }
    );
}

#[test]
fn a_longer_text_is_cut_at_the_limit() {
    let (_dir, path) = file_with(&b"a".repeat(20));
    let content = read_text(&path, 8).unwrap();
    assert_eq!(content.text, "aaaaaaaa");
    assert!(content.truncated);
}

#[test]
fn a_text_exactly_at_the_limit_is_not_truncated() {
    let (_dir, path) = file_with(&b"a".repeat(8));
    assert!(!read_text(&path, 8).unwrap().truncated);
}

#[test]
fn a_nul_byte_near_the_start_marks_a_binary_file() {
    let (_dir, path) = file_with(b"PK\x03\x04\x00\x00binary");
    assert_eq!(
        read_text(&path, TEXT_LIMIT).unwrap_err().code,
        FilesErrorCode::Binary
    );
}

#[test]
fn invalid_utf8_is_replaced() {
    let (_dir, path) = file_with(b"ok \xff ok");
    assert_eq!(read_text(&path, TEXT_LIMIT).unwrap().text, "ok \u{fffd} ok");
}

#[test]
fn a_missing_file_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        read_text(&dir.path().join("fehlt"), TEXT_LIMIT)
            .unwrap_err()
            .code,
        FilesErrorCode::NotFound
    );
}
