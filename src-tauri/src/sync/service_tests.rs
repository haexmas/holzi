use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use tauri::test::{mock_builder, mock_context, noop_assets};

use super::*;
use crate::sync::test_support::Member;

fn mock_app() -> AppHandle<tauri::test::MockRuntime> {
    mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app")
        .handle()
        .clone()
}

fn deps(member: &Member) -> SyncDeps<tauri::test::MockRuntime> {
    SyncDeps {
        replica: Arc::clone(&member.device.replica),
        keys: member.keys.clone(),
        vault: member.vault,
        relay_mode: RelayMode::Disabled,
        nostr_relays: Vec::new(),
        bind_addr: Some((Ipv4Addr::LOCALHOST, 0).into()),
        app: mock_app(),
    }
}

#[tokio::test]
async fn a_closing_gate_before_start_refuses() {
    let gate = VaultGate::with_runtime(tokio::runtime::Handle::current());
    gate.request_close();
    let member = Member::genesis();
    assert!(SyncService::start(&gate, deps(&member)).is_err());
}

#[tokio::test]
async fn a_closing_gate_ends_the_bound_service_within_budget() {
    let member = Member::genesis();
    let gate = VaultGate::with_runtime(tokio::runtime::Handle::current());
    gate.begin_session().expect("session");
    SyncService::start(&gate, deps(&member)).expect("start");

    gate.request_close();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !gate.is_idle() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the service ends once the close starts");
}
