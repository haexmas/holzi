use std::net::Ipv4Addr;

use super::*;
use crate::sync::link::code::LinkCode;

fn addr() -> EndpointAddr {
    let keys = DeviceKeys::generate();
    let id = iroh::EndpointId::from_bytes(&keys.endpoint_id).expect("id");
    EndpointAddr::from_parts(id, [TransportAddr::Ip((Ipv4Addr::LOCALHOST, 4000).into())])
}

#[test]
fn the_host_opens_what_the_new_device_sealed_and_learns_who_it_is() {
    let code = LinkCode::generate();
    let (sk, pk) = code.rendezvous_keys().expect("keys");
    let newcomer = DeviceKeys::generate();
    let meeting = LinkMeeting::new(&addr());

    let event = build(&newcomer, &meeting, &pk).expect("build");
    let (sender, opened) = open(&event, &sk).expect("open");

    assert_eq!(sender, newcomer.device_pubkey);
    assert_eq!(opened, meeting);
    assert_eq!(
        opened.endpoint_addr().expect("addr").id,
        meeting.endpoint_addr().expect("addr").id
    );
    assert!(opened.is_fresh(now_ms()));
}

#[test]
fn a_meeting_for_another_code_cannot_be_opened() {
    let (_, pk) = LinkCode::generate().rendezvous_keys().expect("keys");
    let (other_sk, _) = LinkCode::generate().rendezvous_keys().expect("keys");
    let event = build(&DeviceKeys::generate(), &LinkMeeting::new(&addr()), &pk).expect("build");

    assert!(open(&event, &other_sk).is_err());
}

#[test]
fn a_presence_meeting_is_not_a_link_meeting() {
    use crate::sync::presence::{build as build_presence, PresenceContent};
    let code = LinkCode::generate();
    let (sk, pk) = code.rendezvous_keys().expect("keys");
    let keys = DeviceKeys::generate();
    let content = PresenceContent::own(keys.device_pubkey, keys.endpoint_id, None, Vec::new(), 1);
    let event = build_presence(&keys, &content, &pk).expect("build");

    assert!(matches!(open(&event, &sk), Err(PresenceError::WrongKind)));
}

#[test]
fn a_meeting_is_only_fresh_within_the_bounds() {
    let mut meeting = LinkMeeting::new(&addr());
    meeting.ts = 1_000_000;

    assert!(meeting.is_fresh(1_000_000));
    assert!(meeting.is_fresh(1_000_000 + MAX_AGE_MS));
    assert!(!meeting.is_fresh(1_000_000 + MAX_AGE_MS + 1));
    assert!(meeting.is_fresh(1_000_000 - MAX_FUTURE_MS));
    assert!(!meeting.is_fresh(1_000_000 - MAX_FUTURE_MS - 1));
}
