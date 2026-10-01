use iroh::{RelayMode, RelayUrl};

use super::*;
use crate::storage::preferences::{self, PrefScope};
use crate::sync::test_support::open_vault;

fn urls(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn unset_servers_mean_the_built_in_ones_are_in_use() {
    let config = ServerConfig::default();
    assert_eq!(config.effective_nostr_relays(), default_nostr_relays());
    assert_eq!(config.effective_iroh_relays(), default_iroh_relays());
    assert!(matches!(config.relay_mode(), RelayMode::Default));
}

#[test]
fn added_nostr_relays_are_used_besides_the_built_in_ones() {
    let config = ServerConfig {
        nostr_relays: urls(&["wss://relay.example.org"]),
        ..ServerConfig::default()
    };
    let mut expected = default_nostr_relays();
    expected.push("wss://relay.example.org".to_string());
    assert_eq!(config.effective_nostr_relays(), expected);
}

#[test]
fn a_switched_off_server_is_not_used_and_an_added_one_equal_to_a_built_in_one_counts_once() {
    let config = ServerConfig {
        nostr_relays: urls(&["wss://nos.lol", "wss://relay.example.org"]),
        disabled: urls(&["wss://relay.damus.io", "wss://relay.example.org"]),
        ..ServerConfig::default()
    };
    assert_eq!(
        config.effective_nostr_relays(),
        urls(&["wss://nos.lol", "wss://relay.primal.net"])
    );
}

#[test]
fn only_leaves_nothing_but_the_given_servers() {
    let config = ServerConfig::only(
        urls(&["wss://relay.example.org"]),
        urls(&["https://iroh.example.org"]),
    );
    assert_eq!(
        config.effective_nostr_relays(),
        urls(&["wss://relay.example.org"])
    );
    assert_eq!(
        config.effective_iroh_relays(),
        urls(&["https://iroh.example.org"])
    );
}

#[test]
fn added_iroh_relays_build_a_custom_relay_mode_with_the_built_in_ones() {
    let config = ServerConfig {
        iroh_relays: urls(&["https://relay.example.org"]),
        ..ServerConfig::default()
    };
    let RelayMode::Custom(map) = config.relay_mode() else {
        panic!("expected a custom relay map");
    };
    assert_eq!(map.urls::<Vec<_>>().len(), default_iroh_relays().len() + 1);
}

#[test]
fn switching_off_some_built_in_iroh_relays_keeps_the_others_as_they_are() {
    let defaults = default_iroh_relays();
    let config = ServerConfig {
        disabled: defaults[1..].to_vec(),
        ..ServerConfig::default()
    };
    let RelayMode::Custom(map) = config.relay_mode() else {
        panic!("expected a custom relay map");
    };
    let kept: Vec<RelayUrl> = map.urls();
    assert_eq!(kept.len(), 1);
    // Still the built-in configuration, not a re-parsed copy of the listed form.
    assert!(RelayMode::Default
        .relay_map()
        .urls::<Vec<RelayUrl>>()
        .contains(&kept[0]));
}

#[test]
fn switching_off_every_iroh_relay_uses_no_relay() {
    let config = ServerConfig::only(Vec::new(), Vec::new());
    assert!(config.effective_iroh_relays().is_empty());
    assert!(matches!(config.relay_mode(), RelayMode::Disabled));
}

#[test]
fn an_invalid_added_iroh_relay_url_is_left_out() {
    let config = ServerConfig {
        iroh_relays: urls(&["not a url"]),
        ..ServerConfig::default()
    };
    let RelayMode::Custom(map) = config.relay_mode() else {
        panic!("expected a custom relay map");
    };
    assert_eq!(map.urls::<Vec<_>>().len(), default_iroh_relays().len());
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
    assert!(config.disabled.is_empty());
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
fn legacy_preferences_keep_custom_servers_without_reenabling_built_ins() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = open_vault(dir.path());
    db.write(|tx| {
        preferences::insert_or_update(
            tx,
            PrefScope::Vault,
            PREF_NOSTR_RELAYS,
            r#"["wss://relay.example.org"]"#,
        )?;
        preferences::insert_or_update(
            tx,
            PrefScope::Vault,
            PREF_IROH_RELAYS,
            r#"["https://iroh.example.org"]"#,
        )
    })
    .expect("write legacy preferences");

    let config = crate::storage::query::read(&db, |r| read(r)).expect("read config");
    assert_eq!(
        config.effective_nostr_relays(),
        urls(&["wss://relay.example.org"])
    );
    assert_eq!(
        config.effective_iroh_relays(),
        urls(&["https://iroh.example.org"])
    );
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
