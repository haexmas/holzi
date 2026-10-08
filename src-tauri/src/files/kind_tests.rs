use super::*;

#[test]
fn the_extension_is_lower_case_and_hidden_names_have_none() {
    assert_eq!(extension("Brief.PDF"), "pdf");
    assert_eq!(extension("archiv.tar.gz"), "gz");
    assert_eq!(extension(".bashrc"), "");
    assert_eq!(extension("Makefile"), "");
}

#[test]
fn viewer_kinds_follow_the_media_type() {
    for (name, kind) in [
        ("notiz.txt", ViewerKind::Text),
        ("README.md", ViewerKind::Text),
        ("daten.json", ViewerKind::Text),
        ("brief.pdf", ViewerKind::Pdf),
        ("foto.JPG", ViewerKind::Image),
        ("grafik.svg", ViewerKind::Image),
        ("film.mp4", ViewerKind::Video),
        ("film.mkv", ViewerKind::Video),
        ("lied.mp3", ViewerKind::Audio),
        ("lied.flac", ViewerKind::Audio),
        ("handy.heic", ViewerKind::Info),
        ("bericht.docx", ViewerKind::Info),
        ("archiv.zip", ViewerKind::Info),
        ("ohne-endung", ViewerKind::Info),
    ] {
        assert_eq!(viewer_kind(name), kind, "{name}");
    }
}

#[test]
fn unknown_types_have_no_mime() {
    assert_eq!(mime_for("x.unknownext"), None);
    assert_eq!(mime_for("brief.pdf"), Some("application/pdf"));
}
