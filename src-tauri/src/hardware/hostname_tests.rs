use super::*;

fn some(name: &str) -> Option<String> {
    Some(name.to_string())
}

#[test]
fn the_platform_name_wins_over_the_host_name() {
    assert_eq!(choose(some("Pixel 8"), some("laptop")), some("Pixel 8"));
}

#[test]
fn without_a_platform_name_the_host_name_is_used() {
    assert_eq!(choose(None, some(" laptop ")), some("laptop"));
    assert_eq!(choose(some("  "), some("laptop")), some("laptop"));
}

#[test]
fn localhost_is_never_an_alias() {
    assert_eq!(choose(None, some("localhost")), None);
    assert_eq!(choose(some("LOCALHOST"), some("laptop")), some("laptop"));
    assert_eq!(choose(None, None), None);
}
