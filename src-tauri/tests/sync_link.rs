//! Integration test for spec 024 User Story 5 (T051): a real main device
//! with its running sync service, and a fresh installation that joins it
//! through the link commands' own code (`LinkHost`, `LinkJoin`), over a real
//! in-process Nostr relay for the rendezvous and real iroh endpoints on
//! loopback for the exchange.
//!
//! Linux only, like `sync_devices.rs` (storage is redirected through
//! `XDG_DATA_HOME`).
#![cfg(target_os = "linux")]

use std::net::Ipv4Addr;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use haex_crdt::rusqlite::params;
use haex_crdt::Database;
use iroh::RelayMode;
use tauri::Manager;
use tokio::sync::Mutex;

use holzi_lib::identity::installation_id_path;
use holzi_lib::instances::paths::{get_app_local_data, get_instance_path};
use holzi_lib::instances::vault_config::vault_config;
use holzi_lib::storage::query::{self, Query};
use holzi_lib::sync::link::host_task::LinkHost;
use holzi_lib::sync::link::join_task::{JoinArgs, JoinConfig, LinkJoin};
use holzi_lib::sync::link::status::{LinkFailure, LinkJoinState, LinkingStage};
use holzi_lib::sync::registry::{SyncRegistry, SyncRuntime};
use holzi_lib::sync::{device_list, keys};

#[path = "common/sync_fixture.rs"]
mod sync_fixture;

/// `XDG_DATA_HOME` is process-wide; tests that set it take turns.
static TURN: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

const NEW_PASSPHRASE: &str = "a-different-passphrase";

/// A fresh installation: its own data directory and app.
struct NewInstallation {
    app: tauri::AppHandle<tauri::test::MockRuntime>,
    join: LinkJoin,
    home: tempfile::TempDir,
}

impl NewInstallation {
    fn new() -> Self {
        let home = tempfile::tempdir().expect("tempdir");
        std::env::set_var("XDG_DATA_HOME", home.path());
        Self {
            app: sync_fixture::mock_app(),
            join: LinkJoin::default(),
            home,
        }
    }

    fn select(&self) {
        std::env::set_var("XDG_DATA_HOME", self.home.path());
    }

    async fn start(
        &self,
        code: &str,
        relay_url: &str,
        search_timeout: Duration,
    ) -> Result<LinkJoinState, holzi_lib::error::HolziError> {
        self.select();
        let emit = Arc::new(|_: &LinkJoinState| {});
        self.join
            .start(
                &self.app,
                emit,
                JoinArgs {
                    code: code.to_string(),
                    vault_name: "linked".to_string(),
                    device_name: "Second laptop".to_string(),
                    passphrase: NEW_PASSPHRASE.into(),
                },
                JoinConfig {
                    nostr_relays: vec![relay_url.to_string()],
                    relay_mode: RelayMode::Disabled,
                    bind_addr: Some((Ipv4Addr::LOCALHOST, 0).into()),
                    search_timeout,
                },
            )
            .await
    }

    /// The linked vault, opened with the passphrase chosen for it.
    fn open_vault(&self) -> Database {
        self.select();
        let path = get_instance_path(&self.app, "linked").expect("path");
        let id_file = installation_id_path(&get_app_local_data(&self.app).expect("data dir"));
        Database::open(vault_config(NEW_PASSPHRASE, &path, &id_file, false)).expect("open linked")
    }

    fn vault_exists(&self) -> bool {
        self.select();
        get_instance_path(&self.app, "linked")
            .expect("path")
            .exists()
    }
}

/// The device's sync service, once it has bound (it starts in the
/// background).
async fn runtime_of(device: &sync_fixture::Instance) -> Arc<SyncRuntime> {
    until("the sync service to come up", || {
        device.app.state::<Arc<SyncRegistry>>().get()
    })
    .await
}

async fn until<T>(what: &str, mut check: impl FnMut() -> Option<T>) -> T {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Some(found) = check() {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

fn listed(main: &sync_fixture::Instance) -> Vec<([u8; 32], bool)> {
    query::read(&main.database(), |r| {
        let valid = device_list::valid_lists(&device_list::load_all(r)?, &main.vault);
        Ok(device_list::effective(&valid)
            .map(|s| {
                s.list
                    .devices
                    .iter()
                    .map(|d| (d.device_pubkey, d.role == device_list::Role::Main))
                    .collect()
            })
            .unwrap_or_default())
    })
    .expect("read list")
}

/// Who `device` has a session with, named by the devices of `all` (A, B, C), for a failure message.
async fn describe_peers(
    device: &sync_fixture::Instance,
    label: &str,
    all: [&sync_fixture::Instance; 3],
) -> String {
    let runtime = runtime_of(device).await;
    let names = ["A", "B", "C"];
    let peers: Vec<&str> = runtime
        .node
        .connected()
        .into_iter()
        .map(|key| {
            all.iter()
                .position(|d| d.keys.device_pubkey == key)
                .map_or("?", |i| names[i])
        })
        .collect();
    format!("{label} is connected to {peers:?}")
}

async fn awaiting(host: &LinkHost, name: &str) {
    until("the main device to ask", || {
        host.status()
            .filter(|s| {
                s.stage == LinkingStage::AwaitingConfirmation
                    && s.new_device_name.as_deref() == Some(name)
            })
            .map(|_| ())
    })
    .await;
}

/// Runs a link to the point of the user's answer, then answers.
async fn link(
    main: &sync_fixture::Instance,
    fresh: &NewInstallation,
    relay_url: &str,
    answer: impl FnOnce(&LinkHost),
) -> LinkJoinState {
    let runtime = runtime_of(main).await;
    let info = runtime.link.create_code(&runtime).await.expect("a code");
    fresh
        .start(&info.code, relay_url, Duration::from_secs(60))
        .await
        .expect("the join starts");
    awaiting(&runtime.link, "Second laptop").await;
    answer(&runtime.link);
    until("the join to end", || {
        fresh
            .join
            .status()
            .filter(|s| matches!(s, LinkJoinState::Done { .. } | LinkJoinState::Failed { .. }))
    })
    .await
}

/// US5 scenarios 1 to 4 (spec.md), SC-009: a code, a name shown before
/// anything moves, a yes without the main device role: the new installation
/// holds the data, is a linked device on both lists and has no vault secret.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_new_installation_links_as_a_linked_device() {
    let _turn = TURN.lock().await;
    let relay = nostr_sdk::local_relay::MockRelay::run()
        .await
        .expect("mock relay");
    let relay_url = relay.url().await.to_string();
    let main = sync_fixture::create_main("main", &relay_url).await;
    main.write_thread("t1", "from the main device").await;
    let fresh = NewInstallation::new();

    let end = link(&main, &fresh, &relay_url, |host| {
        host.confirm(false).expect("confirm");
    })
    .await;

    assert_eq!(
        end,
        LinkJoinState::Done {
            vault_name: "linked".to_string()
        }
    );
    let vault = fresh.open_vault();
    let (has_secret, title, own_key, list_hash) = query::read(&vault, |r| {
        let secret = keys::vault_secret(r)?.is_some();
        let title: Option<String> = r.query_row(
            "SELECT title FROM chat_threads WHERE id = 't1'",
            params![],
            |row| row.get(0),
        )?;
        let valid = device_list::valid_lists(
            &device_list::load_all(r)?,
            &keys::vault_pubkey(r)?.expect("identity"),
        );
        let devices: Vec<_> = device_list::effective(&valid)
            .map(|s| {
                s.list
                    .devices
                    .iter()
                    .map(|d| (d.device_pubkey, d.role))
                    .collect()
            })
            .unwrap_or_default();
        Ok((
            secret,
            title,
            holzi_lib::sync::envelopes::held_keys(r)?.len(),
            devices,
        ))
    })
    .expect("read the linked vault");
    assert!(!has_secret, "SC-009: no vault secret without the role");
    assert_eq!(title.as_deref(), Some("from the main device"));
    assert!(
        own_key > 0,
        "the content key was wrapped for the new device"
    );
    assert_eq!(list_hash.len(), 2, "both devices are on the list");
    assert!(
        list_hash
            .iter()
            .any(|(_, role)| *role == device_list::Role::Linked),
        "the new device is a linked device"
    );
    assert_eq!(
        listed(&main).len(),
        2,
        "the main device published the list too"
    );
}

/// US5 scenario 5, SC-012: with the role, the vault secret arrives over the
/// link and the new device is a main device.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_new_installation_can_link_as_a_main_device() {
    let _turn = TURN.lock().await;
    let relay = nostr_sdk::local_relay::MockRelay::run()
        .await
        .expect("mock relay");
    let relay_url = relay.url().await.to_string();
    let main = sync_fixture::create_main("main", &relay_url).await;
    let fresh = NewInstallation::new();

    let end = link(&main, &fresh, &relay_url, |host| {
        host.confirm(true).expect("confirm");
    })
    .await;

    assert!(matches!(end, LinkJoinState::Done { .. }), "{end:?}");
    let vault = fresh.open_vault();
    let secret = query::read(&vault, |r| keys::vault_secret(r))
        .expect("read")
        .expect("SC-012: the vault secret arrived");
    let main_secret = query::read(&main.database(), |r| keys::vault_secret(r))
        .expect("read")
        .expect("main holds it");
    assert_eq!(*secret, *main_secret);
    assert_eq!(
        listed(&main).iter().filter(|(_, is_main)| *is_main).count(),
        2
    );
}

/// US5 scenario 7: a decline gives the new installation nothing, leaves the
/// device list as it was, and its vault is gone.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_declined_link_leaves_nothing_behind() {
    let _turn = TURN.lock().await;
    let relay = nostr_sdk::local_relay::MockRelay::run()
        .await
        .expect("mock relay");
    let relay_url = relay.url().await.to_string();
    let main = sync_fixture::create_main("main", &relay_url).await;
    let fresh = NewInstallation::new();

    let end = link(&main, &fresh, &relay_url, |host| {
        host.reject().expect("reject");
    })
    .await;

    assert_eq!(
        end,
        LinkJoinState::Failed {
            reason: LinkFailure::Rejected
        }
    );
    assert!(!fresh.vault_exists(), "FR-025: no vault stays");
    assert_eq!(listed(&main).len(), 1, "the device list is unchanged");
    assert_eq!(runtime_of(&main).await.link.status(), None);
}

/// FR-025: cancelling a join waits for the background task to discard its
/// pending vault before the caller continues.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelling_a_join_waits_until_the_pending_vault_is_removed() {
    let _turn = TURN.lock().await;
    let relay = nostr_sdk::local_relay::MockRelay::run()
        .await
        .expect("mock relay");
    let relay_url = relay.url().await.to_string();
    let main = sync_fixture::create_main("main", &relay_url).await;
    let runtime = runtime_of(&main).await;
    let info = runtime.link.create_code(&runtime).await.expect("a code");
    let fresh = NewInstallation::new();

    fresh
        .start(&info.code, &relay_url, Duration::from_secs(60))
        .await
        .expect("the join starts");
    fresh.join.cancel().await;

    assert_eq!(
        fresh.join.status(),
        Some(LinkJoinState::Failed {
            reason: LinkFailure::ConnectionLost
        })
    );
    assert!(!fresh.vault_exists(), "FR-025: no vault stays after cancel");
    runtime.link.cancel();
}

/// US5 scenario 6: a code that is mistyped, or already used, transfers
/// nothing and says so.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_wrong_or_used_code_transfers_nothing() {
    let _turn = TURN.lock().await;
    let relay = nostr_sdk::local_relay::MockRelay::run()
        .await
        .expect("mock relay");
    let relay_url = relay.url().await.to_string();
    let main = sync_fixture::create_main("main", &relay_url).await;

    // Not even a code: refused at once, without creating a vault.
    let typo = NewInstallation::new();
    let state = typo
        .start("not a code", &relay_url, Duration::from_secs(1))
        .await
        .expect("the input is accepted, the join fails");
    assert_eq!(
        state,
        LinkJoinState::Failed {
            reason: LinkFailure::WrongCode
        }
    );
    assert!(!typo.vault_exists());

    // A code that was used: link once, then try the same code again.
    let first = NewInstallation::new();
    let runtime = runtime_of(&main).await;
    let info = runtime.link.create_code(&runtime).await.expect("a code");
    first
        .start(&info.code, &relay_url, Duration::from_secs(60))
        .await
        .expect("starts");
    awaiting(&runtime.link, "Second laptop").await;
    runtime.link.confirm(false).expect("confirm");
    until("the first join", || {
        first
            .join
            .status()
            .filter(|s| matches!(s, LinkJoinState::Done { .. }))
    })
    .await;

    let second = NewInstallation::new();
    second
        .start(&info.code, &relay_url, Duration::from_secs(3))
        .await
        .expect("starts");
    let end = until("the second join to give up", || {
        second
            .join
            .status()
            .filter(|s| matches!(s, LinkJoinState::Failed { .. }))
    })
    .await;
    assert_eq!(
        end,
        LinkJoinState::Failed {
            reason: LinkFailure::Expired
        }
    );
    assert!(
        !second.vault_exists(),
        "nothing was transferred, no vault stays"
    );
    assert_eq!(listed(&main).len(), 2, "only the first device was added");
}

/// FR-024: a device without the main role cannot show a code, and the
/// backend says so.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_linked_device_cannot_show_a_code() {
    let _turn = TURN.lock().await;
    let relay = nostr_sdk::local_relay::MockRelay::run()
        .await
        .expect("mock relay");
    let relay_url = relay.url().await.to_string();
    let main = sync_fixture::create_main("main", &relay_url).await;
    let linked = sync_fixture::join(&main, &relay_url).await;

    let runtime = runtime_of(&linked).await;
    let refused = runtime.link.create_code(&runtime).await;

    assert!(matches!(
        refused,
        Err(holzi_lib::error::HolziError::NotMainDevice)
    ));
}

/// US6, SC-010 (spec.md): with three devices, the main device removes one. The
/// removed device gets nothing new any more, the other two keep syncing, and
/// both lists no longer name it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_removed_device_gets_nothing_new_while_the_others_keep_syncing() {
    use holzi_lib::sync::commands::{remove_device_now, this_device, ThisDevice};

    let _turn = TURN.lock().await;
    let relay = nostr_sdk::local_relay::MockRelay::run()
        .await
        .expect("mock relay");
    let relay_url = relay.url().await.to_string();
    let a = sync_fixture::create_main("main", &relay_url).await;
    let b = sync_fixture::join(&a, &relay_url).await;
    let c = sync_fixture::join(&a, &relay_url).await;

    // All three find each other and sync: the removal is of a working device.
    tokio::time::timeout(Duration::from_secs(40), async {
        loop {
            a.write_thread("warm-up", "warm-up").await;
            if b.thread_title("warm-up").is_some() && c.thread_title("warm-up").is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    })
    .await
    .expect("all three devices sync before the removal");

    let runtime = runtime_of(&a).await;
    remove_device_now(&runtime, b.keys.device_pubkey)
        .await
        .expect("the main device removes the linked device");

    // What the main device writes now reaches the device that stays; the
    // removed one, which would have got it a moment ago, does not.
    tokio::time::timeout(Duration::from_secs(40), async {
        loop {
            a.write_thread("after-removal", "after-removal").await;
            if c.thread_title("after-removal").is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    })
    .await
    .expect("the remaining device keeps syncing");
    if b.thread_title("after-removal").is_some() {
        panic!(
            "FR-027 broken: B got after-removal. {} | {} | {} | A lists {} | B lists {} | C lists {}",
            describe_peers(&a, "A", [&a, &b, &c]).await,
            describe_peers(&b, "B", [&a, &b, &c]).await,
            describe_peers(&c, "C", [&a, &b, &c]).await,
            listed(&a).len(),
            listed(&b).len(),
            listed(&c).len(),
        );
    }
    assert_eq!(listed(&a).len(), 2);
    let c_sees = until("the remaining device to learn the new list", || {
        let devices = listed(&c);
        (devices.len() == 2).then_some(devices)
    })
    .await;
    assert!(
        c_sees
            .iter()
            .all(|(device, _)| *device != b.keys.device_pubkey),
        "FR-034: the removed device is off the list of the others"
    );

    // The removed device learns of it the next time it reaches one that knows: its reconnect
    // dials the devices it still lists, and the refusal brings the new list along (FR-034). The
    // service does that on a 30 second tick; the test runs the same step instead of waiting.
    let removed_runtime = runtime_of(&b).await;
    let removed = tokio::time::timeout(Duration::from_secs(40), async {
        loop {
            holzi_lib::sync::reconnect_missing(&removed_runtime.node, &removed_runtime.replica)
                .await;
            let place = holzi_lib::storage::query::read(&b.database(), |r| {
                this_device(r, &b.keys.device_pubkey)
            })
            .expect("read place");
            if place == ThisDevice::Removed {
                break place;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        let rows = holzi_lib::storage::query::read(&b.database(), |r| {
            holzi_lib::sync::presence::load_all(r)
        })
        .expect("read presence");
        let names = [a.keys.device_pubkey, b.keys.device_pubkey, c.keys.device_pubkey];
        let known: Vec<String> = rows
            .iter()
            .map(|row| {
                let who = ["A", "B", "C"][names.iter().position(|k| *k == row.device_pubkey).unwrap_or(0)];
                format!("{who}: address {}", row.endpoint_addr.is_some())
            })
            .collect();
        panic!(
            "the removed device did not learn that it was removed: B lists {} devices; {}; B knows {known:?}",
            listed(&b).len(),
            "see above",
        )
    });
    assert_eq!(removed, ThisDevice::Removed);
}

/// US6 scenario 9 at the backend: a linked device cannot remove anything,
/// and no device removes itself.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_a_main_device_removes_and_never_itself() {
    use holzi_lib::sync::commands::remove_device_now;

    let _turn = TURN.lock().await;
    let relay = nostr_sdk::local_relay::MockRelay::run()
        .await
        .expect("mock relay");
    let relay_url = relay.url().await.to_string();
    let a = sync_fixture::create_main("main", &relay_url).await;
    let b = sync_fixture::join(&a, &relay_url).await;

    let main_runtime = runtime_of(&a).await;
    let itself = remove_device_now(&main_runtime, a.keys.device_pubkey).await;
    assert!(matches!(
        itself,
        Err(holzi_lib::error::HolziError::InvalidInput { .. })
    ));

    let linked_runtime = runtime_of(&b).await;
    let refused = remove_device_now(&linked_runtime, a.keys.device_pubkey).await;
    assert!(matches!(
        refused,
        Err(holzi_lib::error::HolziError::NotMainDevice)
    ));
    assert_eq!(listed(&a).len(), 2, "nothing was removed");
}
