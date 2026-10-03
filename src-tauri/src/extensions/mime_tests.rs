use super::*;

#[test]
fn known_extensions_and_a_safe_default() {
    assert_eq!(for_path("index.html"), "text/html; charset=utf-8");
    assert_eq!(for_path("_nuxt/A.B.JS"), "text/javascript; charset=utf-8");
    assert_eq!(for_path("icon.svg"), "image/svg+xml");
    assert_eq!(for_path("README"), "application/octet-stream");
    assert_eq!(for_path("archive.tar.gz"), "application/octet-stream");
    assert!(is_image("a/b.png") && !is_image("a.js"));
}
