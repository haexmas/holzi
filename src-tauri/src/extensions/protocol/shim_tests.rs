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

#[test]
fn the_title_observer_does_not_need_the_root_element() {
    assert!(SHIM.contains("new MutationObserver(reportTitle).observe(document, {"));
    assert!(!SHIM.contains("document.documentElement"));
}

#[test]
fn a_development_page_gets_the_same_shim_only_on_a_loopback_http_address() {
    let script = dev_init_script();
    let at = script.find(SHIM).expect("the shim itself, unchanged");
    let guard = &script[..at];
    assert!(guard.contains("if (at.protocol !== 'http:' || (at.hostname !== 'localhost' && at.hostname !== '127.0.0.1')) return;"));
    assert_eq!(script[at + SHIM.len()..].trim(), "})();");
}
