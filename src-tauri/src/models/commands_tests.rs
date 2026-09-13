use std::sync::Arc;

use haex_crdt::{Database, DatabaseConfig, NoopSignatureProvider, SqlCipherKey};

use crate::identity::{
    holzi_migration_source, installation_id_path, HolziBootstrap, HOLZI_TRIGGER_VERSION,
};
use crate::state::{ActiveInstanceHandle, AppState};
use crate::storage::models;

use super::register_downloaded;

#[tokio::test]
async fn registration_keeps_the_vault_captured_before_a_transfer() {
    let (dirs, original, replacement) = tokio::task::spawn_blocking(|| {
        let dirs = [
            tempfile::tempdir().expect("original dir"),
            tempfile::tempdir().expect("replacement dir"),
        ];
        let mut databases = Vec::new();
        for dir in &dirs {
            databases.push(Arc::new(
                Database::open(DatabaseConfig {
                    path: dir.path().join("vault.db"),
                    key: SqlCipherKey::new("model-registration-test"),
                    create_if_missing: true,
                    bootstrap: Arc::new(HolziBootstrap::new(installation_id_path(dir.path()))),
                    signature_provider: Arc::new(NoopSignatureProvider),
                    migration_source: holzi_migration_source(),
                    trigger_version: HOLZI_TRIGGER_VERSION,
                })
                .expect("open vault"),
            ));
        }
        (dirs, databases.remove(0), databases.remove(0))
    })
    .await
    .expect("open join");

    let state = AppState::new();
    *state.active_instance.lock().expect("state") = Some(ActiveInstanceHandle {
        name: "original".into(),
        database: Arc::clone(&original),
    });
    let captured = Arc::clone(
        &state
            .active_instance
            .lock()
            .expect("state")
            .as_ref()
            .expect("active")
            .database,
    );
    *state.active_instance.lock().expect("state") = Some(ActiveInstanceHandle {
        name: "replacement".into(),
        database: Arc::clone(&replacement),
    });

    register_downloaded(
        captured,
        "downloaded",
        "Downloaded",
        "downloaded/model.gguf",
        24,
        None,
        Some("tokenizer/repo".into()),
    )
    .await
    .expect("register after switch");

    tokio::task::spawn_blocking(move || {
        assert!(original
            .with_connection(|conn| Ok(models::get_model(conn, "downloaded")?))
            .expect("original model")
            .is_some());
        assert!(replacement
            .with_connection(|conn| Ok(models::get_model(conn, "downloaded")?))
            .expect("replacement model")
            .is_none());
        drop(state);
        drop(original);
        drop(replacement);
        drop(dirs);
    })
    .await
    .expect("assert join");
}
