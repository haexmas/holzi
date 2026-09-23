//! One app process serves at most one vault session (spec 013 US2, FR-010 to FR-012, SC-003), over
//! the mock runtime (a real `AppHandle<MockRuntime>`, so path resolution is the genuine article,
//! per `open_instance_core`/`create_instance_core` being generic over `R: Runtime`): `open_instance`
//! and `create_instance` refuse before touching any file once a vault is active or a close is
//! under way, and a failed unlock never leaves the gate stuck.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use tauri::test::{mock_builder, mock_context, noop_assets};
use tokio::sync::Mutex;

use holzi_lib::chat::session::ChatState;
use holzi_lib::instances::{create_instance_core, open_instance_core};
use holzi_lib::state::AppState;
use holzi_lib::vault_gate::{VaultGate, VaultPhase};
use holzi_lib::HolziError;

/// `XDG_DATA_HOME` is process-wide, so tests that set it take turns. `tokio::sync::Mutex::new`
/// is not `const`, hence the `LazyLock` (a plain `static` initializer would not compile).
static TURN: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

const PASSPHRASE: &str = "correct-horse-battery";

async fn create(
    app: &tauri::AppHandle<tauri::test::MockRuntime>,
    state: &AppState,
    chat: &ChatState,
    name: &str,
) -> Result<(), HolziError> {
    create_instance_core(app, state, chat, name, PASSPHRASE.into())
        .await
        .map(|_| ())
}

async fn open(
    app: &tauri::AppHandle<tauri::test::MockRuntime>,
    state: &AppState,
    chat: &ChatState,
    name: &str,
    passphrase: &str,
) -> Result<(), HolziError> {
    open_instance_core(app, state, chat, name, passphrase.into())
        .await
        .map(|_| ())
}

/// Every file under `root`, as a path relative to it plus its byte size, sorted — so two snapshots
/// can be compared regardless of the order a directory happens to list entries in.
fn snapshot_files(root: &Path) -> Vec<(PathBuf, u64)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(PathBuf, u64)>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if let Ok(meta) = entry.metadata() {
                out.push((
                    path.strip_prefix(root).expect("under root").into(),
                    meta.len(),
                ));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn mock_app() -> tauri::AppHandle<tauri::test::MockRuntime> {
    mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app")
        .handle()
        .clone()
}

#[tokio::test]
async fn open_and_create_refuse_while_active_and_touch_no_file() {
    let _turn = TURN.lock().await;
    let data_home = tempfile::tempdir().expect("data home");
    std::env::set_var("XDG_DATA_HOME", data_home.path());

    let gate = VaultGate::new();
    let state = AppState::new(gate.clone());
    let chat = ChatState::with_children(gate.children());
    let app = mock_app();

    create(&app, &state, &chat, "first")
        .await
        .expect("create the first instance");
    assert_eq!(gate.phase(), VaultPhase::Active);
    let before = snapshot_files(data_home.path());
    assert!(
        !before.is_empty(),
        "creating the first instance did write files"
    );

    assert!(matches!(
        create(&app, &state, &chat, "second").await,
        Err(HolziError::VaultAlreadyActive)
    ));
    assert_eq!(
        snapshot_files(data_home.path()),
        before,
        "a refused create must not touch the filesystem"
    );

    assert!(matches!(
        open(&app, &state, &chat, "first", PASSPHRASE).await,
        Err(HolziError::VaultAlreadyActive)
    ));
    assert_eq!(
        snapshot_files(data_home.path()),
        before,
        "a refused open must not touch the filesystem"
    );

    std::env::remove_var("XDG_DATA_HOME");
}

#[tokio::test]
async fn open_and_create_refuse_while_closing_and_touch_no_file() {
    let _turn = TURN.lock().await;
    let data_home = tempfile::tempdir().expect("data home");
    std::env::set_var("XDG_DATA_HOME", data_home.path());

    let gate = VaultGate::new();
    let state = AppState::new(gate.clone());
    let chat = ChatState::with_children(gate.children());
    let app = mock_app();
    // Simulates a close under way without running the real close protocol (which would end this
    // test process) — the gate is the one thing `open_instance`/`create_instance` consult.
    gate.request_close();
    let before = snapshot_files(data_home.path());
    assert!(before.is_empty(), "nothing was ever created");

    assert!(matches!(
        create(&app, &state, &chat, "first").await,
        Err(HolziError::VaultClosed)
    ));
    assert_eq!(snapshot_files(data_home.path()), before);

    assert!(matches!(
        open(&app, &state, &chat, "first", PASSPHRASE).await,
        Err(HolziError::VaultClosed)
    ));
    assert_eq!(snapshot_files(data_home.path()), before);

    std::env::remove_var("XDG_DATA_HOME");
}

/// FR-010: a failed unlock (wrong passphrase) must not leave the gate stuck — an open that
/// follows, on the very same still-`Idle` gate, must still succeed.
#[tokio::test]
async fn a_failed_open_leaves_the_gate_idle_and_a_later_open_succeeds() {
    let _turn = TURN.lock().await;
    let data_home = tempfile::tempdir().expect("data home");
    std::env::set_var("XDG_DATA_HOME", data_home.path());

    // A first app process creates the vault on disk, then (as its own process would) never opens
    // it again — its own gate is left `Active`, irrelevant to the rest of this test.
    let creator_gate = VaultGate::new();
    let creator_state = AppState::new(creator_gate.clone());
    let creator_chat = ChatState::with_children(creator_gate.children());
    let creator_app = mock_app();
    create(&creator_app, &creator_state, &creator_chat, "vault-a")
        .await
        .expect("create vault-a");
    // Releases haex-crdt's advisory file lock (the last `Arc<Database>` clone drops with the
    // handle): a real second process would never have held it open in the first place, and
    // without this the opener's own `Database::open` would hit that lock, not the passphrase.
    drop(creator_state.take().expect("take the creator's handle"));

    // A second, independent process (fresh gate, same on-disk data) is the one under test.
    let opener_gate = VaultGate::new();
    let opener_state = AppState::new(opener_gate.clone());
    let opener_chat = ChatState::with_children(opener_gate.children());
    let opener_app = mock_app();

    let wrong_result = open(
        &opener_app,
        &opener_state,
        &opener_chat,
        "vault-a",
        "the-wrong-passphrase",
    )
    .await;
    assert!(
        matches!(wrong_result, Err(HolziError::WrongPassphrase)),
        "expected WrongPassphrase, got {wrong_result:?}"
    );
    assert_eq!(
        opener_gate.phase(),
        VaultPhase::Idle,
        "a failed unlock must not move the gate at all"
    );

    open(
        &opener_app,
        &opener_state,
        &opener_chat,
        "vault-a",
        PASSPHRASE,
    )
    .await
    .expect("the correct passphrase now succeeds");
    assert_eq!(opener_gate.phase(), VaultPhase::Active);

    std::env::remove_var("XDG_DATA_HOME");
}
