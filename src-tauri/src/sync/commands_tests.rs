use super::*;
use crate::storage::query;
use crate::sync::device_list::RemovedDevice;
use crate::sync::test_support::Member;

fn status_of(member: &Member) -> ThisDevice {
    query::read(member.device.db(), |r| {
        this_device(r, &member.keys.device_pubkey)
    })
    .expect("read status")
}

#[test]
fn a_device_holding_the_main_role_is_main_and_a_listed_other_is_linked() {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    linked.device.pull_from(&main.device);

    assert_eq!(status_of(&main), ThisDevice::Main);
    assert_eq!(status_of(&linked), ThisDevice::Linked);
}

#[test]
fn a_device_the_list_does_not_name_awaits_admission() {
    let main = Member::genesis();
    let stranger = Member::join(&main);

    assert_eq!(status_of(&stranger), ThisDevice::AwaitingAdmission);
}

#[test]
fn a_device_the_list_removes_is_removed() {
    let main = Member::genesis();
    let linked = Member::join(&main);
    main.add(&linked);
    let removed = RemovedDevice {
        device_pubkey: linked.keys.device_pubkey,
        vault_device_uuid: linked.device.db().device_id(),
        limit_hlc: "0/0".to_string(),
        removed_at: 3,
    };
    main.issue_list(|mut list| {
        list.devices
            .retain(|d| d.device_pubkey != removed.device_pubkey);
        list.removed.push(removed);
        list
    });
    linked.device.pull_from(&main.device);

    assert_eq!(status_of(&linked), ThisDevice::Removed);
}

#[test]
fn the_identity_is_shown_as_npub_and_hex_of_the_same_key() {
    use nostr::nips::nip19::FromBech32;
    let main = Member::genesis();

    let identity = query::read(main.device.db(), |r| public_identity(r))
        .expect("read")
        .expect("an identity after genesis");

    assert!(identity.npub.starts_with("npub1"));
    assert_eq!(identity.hex, keys::hex(&main.vault));
    let decoded = nostr::key::PublicKey::from_bech32(&identity.npub).expect("a valid npub");
    assert_eq!(decoded.to_bytes(), main.vault);
}

#[test]
fn no_private_key_ever_shows_in_the_identity() {
    let main = Member::genesis();
    let secret = query::read(main.device.db(), |r| keys::vault_secret(r))
        .expect("read")
        .expect("main holds the secret");

    let identity = query::read(main.device.db(), |r| public_identity(r))
        .expect("read")
        .expect("an identity");
    let json = serde_json::to_string(&identity).expect("json");

    assert!(!json.contains(&keys::hex(secret.as_slice())));
}

#[test]
fn servers_are_checked_by_scheme_and_count() {
    let ok = |n: &[&str], i: &[&str]| {
        servers::validate(
            &n.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            &i.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )
    };

    assert!(ok(
        &["wss://relay.example", "ws://127.0.0.1:7777"],
        &["https://iroh.example"]
    )
    .is_ok());
    assert!(ok(&[], &[]).is_ok(), "empty means the defaults");
    assert!(
        ok(&["https://relay.example"], &[]).is_err(),
        "a Nostr server is ws or wss"
    );
    assert!(
        ok(&[], &["wss://iroh.example"]).is_err(),
        "an iroh server is http or https"
    );
    assert!(ok(&["wss://"], &[]).is_err());
    assert!(ok(&["not a url"], &[]).is_err());
    assert!(ok(&["wss://relay example"], &[]).is_err());
    assert!(ok(&[], &["https://iroh.example:abc"]).is_err());
    assert!(ok(&[], &["https://[::1]:443"]).is_ok());
    assert!(ok(&[], &["https://[::1"]).is_err());
    assert!(
        ok(&["wss://a.example"; 11], &[]).is_err(),
        "at most ten of one kind"
    );
    assert!(ok(&["wss://a.example"; 10], &[]).is_ok());
}

#[test]
fn stored_servers_read_back_and_an_empty_list_brings_back_the_defaults() {
    let main = Member::genesis();
    let nostr = vec!["wss://relay.example".to_string()];
    let iroh = vec!["https://iroh.example".to_string()];

    main.device
        .db()
        .write(|tx| servers::write(tx, &nostr, &iroh))
        .expect("write");
    let stored = query::read(main.device.db(), |r| servers::read(r)).expect("read");
    assert_eq!(stored.nostr_relays, nostr);
    assert_eq!(stored.iroh_relays, iroh);

    main.device
        .db()
        .write(|tx| servers::write(tx, &[], &[]))
        .expect("write empty");
    let cleared = query::read(main.device.db(), |r| servers::read(r)).expect("read");
    assert_eq!(
        cleared.effective_nostr_relays(),
        servers::default_nostr_relays()
    );
}
