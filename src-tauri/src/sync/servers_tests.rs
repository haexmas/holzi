use iroh::{RelayMode, RelayUrl};

use super::*;
use crate::storage::preferences::{self, PrefScope};
use crate::sync::test_support::open_vault;

#[test]
fn unset_relays_fall_back_to_defaults() {
    let config = ServerConfig::default();
    assert_eq!(config.effective_nostr_relays(), default_nostr_relays());
    assert!(matches!(config.relay_mode(), RelayMode::Default));
}

#[test]
fn configured_nostr_relays_replace_the_defaults() {
    let config = ServerConfig {
        nostr_relays: vec!["wss://relay.example.org".to_string()],
        iroh_relays: Vec::new(),
    };
    assert_eq!(
        config.effective_nostr_relays(),
        vec!["wss://relay.example.org".to_string()]
    );
}

#[test]
fn configured_iroh_relays_build_a_custom_relay_mode() {
    let config = ServerConfig {
        nostr_relays: Vec::new(),
        iroh_relays: vec!["https://relay.example.org".to_string()],
    };
    let RelayMode::Custom(map) = config.relay_mode() else {
        panic!("expected a custom relay map");
    };
    assert_eq!(map.urls::<Vec<_>>().len(), 1);
}

#[test]
fn an_invalid_iroh_relay_url_falls_back_to_the_default_mode() {
    let config = ServerConfig {
        nostr_relays: Vec::new(),
        iroh_relays: vec!["not a url".to_string()],
    };
    assert!(matches!(config.relay_mode(), RelayMode::Default));
}

#[test]
fn read_reflects_the_stored_preferences() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = open_vault(dir.path());
    db.write(|tx| {
        preferences::insert_or_update(
            tx,
            PrefScope::Vault,
            PREF_NOSTR_RELAYS,
            r#"["wss://relay.example.org"]"#,
        )
    })
    .expect("write nostr relays");

    let config = crate::storage::query::read(&db, |r| read(r)).expect("read config");
    assert_eq!(
        config.nostr_relays,
        vec!["wss://relay.example.org".to_string()]
    );
    assert!(config.iroh_relays.is_empty());
}

#[test]
fn an_unparseable_stored_value_reads_as_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = open_vault(dir.path());
    db.write(|tx| {
        preferences::insert_or_update(tx, PrefScope::Vault, PREF_IROH_RELAYS, "not json")
    })
    .expect("write garbage");

    let config = crate::storage::query::read(&db, |r| read(r)).expect("read config");
    assert!(config.iroh_relays.is_empty());
}

#[test]
fn diff_relays_finds_what_to_insert_and_remove() {
    let a: RelayUrl = "https://a.example.org".parse().expect("url");
    let b: RelayUrl = "https://b.example.org".parse().expect("url");
    let c: RelayUrl = "https://c.example.org".parse().expect("url");
    let (insert, remove) = diff_relays(&[a.clone(), b.clone()], &[b.clone(), c.clone()]);
    assert_eq!(insert, vec![c]);
    assert_eq!(remove, vec![a]);
}

#[test]
fn parse_relay_urls_drops_invalid_entries() {
    let urls = parse_relay_urls(&[
        "https://ok.example.org".to_string(),
        "not a url".to_string(),
    ]);
    assert_eq!(urls.len(), 1);
}
