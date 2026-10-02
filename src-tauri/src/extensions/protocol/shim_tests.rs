use super::*;

#[test]
fn the_shim_goes_right_after_the_head_start_tag() {
    let html = r#"<!doctype html><HTML lang="de"><Head data-x=">"><title>x</title></head></html>"#;
    let out = inject(html);
    let at = out.find("<script>").unwrap();
    assert_eq!(
        &out[..at],
        r#"<!doctype html><HTML lang="de"><Head data-x=">">"#
    );
    assert!(out[at..].starts_with(&format!("<script>{SHIM}</script><title>")));
}

#[test]
fn without_head_the_shim_follows_html_or_starts_the_document() {
    assert!(inject("<html><body></body></html>").starts_with("<html><script>"));
    assert!(inject("<p>bare</p>").starts_with("<script>"));
    // `<header>` is not `<head>`.
    assert!(inject("<header>x</header>").starts_with("<script>"));
}

#[test]
fn the_shim_only_accepts_its_init_from_the_parent_and_never_from_the_top_window() {
    assert!(SHIM.contains("if (event.source !== host) return;"));
    assert!(SHIM.contains("if (window.top === window) return;"));
    assert!(!SHIM.contains("eval("));
}
