use std::sync::Arc;

use haex_crdt::{Database, DatabaseConfig, NoopSignatureProvider, SqlCipherKey};

use crate::identity::{
    holzi_migration_source, installation_id_path, HolziBootstrap, HOLZI_TRIGGER_VERSION,
};
use crate::model_capabilities::{ModelCapabilities, ReasoningControl};
use crate::state::{ActiveInstanceHandle, AppState};
use crate::storage::models::{self, SourceKind};
use crate::vault_gate::VaultGate;

use super::{register_downloaded, RegisterDownloadedArgs};

#[tokio::test]
async fn registration_keeps_the_vault_captured_before_a_transfer() {
    let (dirs, original, replacement, model_file, destination) =
        tokio::task::spawn_blocking(|| {
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
            let model_file = dirs[0].path().join("model.gguf");
            std::fs::write(&model_file, [0u8; 24]).expect("write fake model bytes");
            let destination = dirs[0].path().join("published.gguf");
            (
                dirs,
                databases.remove(0),
                databases.remove(0),
                model_file,
                destination,
            )
        })
        .await
        .expect("open join");

    let state = AppState::default();
    state
        .install(
            ActiveInstanceHandle {
                name: "original".into(),
                database: Arc::clone(&original),
            },
            || Ok(()),
        )
        .expect("install the original vault");
    let captured = state.database().expect("active");
    state
        .switch_to(
            ActiveInstanceHandle {
                name: "replacement".into(),
                database: Arc::clone(&replacement),
            },
            || {},
        )
        .expect("switch to the replacement vault");

    register_downloaded(RegisterDownloadedArgs {
        db: captured,
        id: "downloaded".into(),
        name: "Downloaded".into(),
        relative: "downloaded/model.gguf".into(),
        staged_path: model_file,
        destination,
        size_bytes: 24,
        context_window: None,
        tokenizer_repo: Some("tokenizer/repo".into()),
        source_kind: SourceKind::Imported,
        hf_repo: None,
        hf_filename: None,
        hf_revision: None,
        hf_revision_ref: None,
    })
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

/// Registration is the one creation site every download/import path shares,
/// so it must record the capabilities of a local model (spec 012): reasoning
/// derived from the id, and no attachment support.
#[tokio::test]
async fn registration_records_capabilities_derived_from_the_local_model_id() {
    let dir = tokio::task::spawn_blocking(|| tempfile::tempdir().expect("dir"))
        .await
        .expect("dir join");
    let root = dir.path().to_path_buf();
    let db = {
        let root = root.clone();
        tokio::task::spawn_blocking(move || {
            Arc::new(
                Database::open(DatabaseConfig {
                    path: root.join("vault.db"),
                    key: SqlCipherKey::new("model-capabilities-registration-test"),
                    create_if_missing: true,
                    bootstrap: Arc::new(HolziBootstrap::new(installation_id_path(&root))),
                    signature_provider: Arc::new(NoopSignatureProvider),
                    migration_source: holzi_migration_source(),
                    trigger_version: HOLZI_TRIGGER_VERSION,
                })
                .expect("open vault"),
            )
        })
        .await
        .expect("open join")
    };

    for id in ["Qwen3-4B-Instruct", "Qwen2.5-0.5B-Instruct"] {
        let staged = root.join(format!("{id}.staged"));
        tokio::fs::write(&staged, [0u8; 8])
            .await
            .expect("stage bytes");
        register_downloaded(RegisterDownloadedArgs {
            db: VaultGate::new()
                .vault_db(Arc::clone(&db))
                .expect("open gate"),
            id: id.into(),
            name: id.into(),
            relative: format!("{id}/model.gguf"),
            staged_path: staged,
            destination: root.join(format!("{id}.gguf")),
            size_bytes: 8,
            context_window: None,
            tokenizer_repo: None,
            source_kind: SourceKind::Imported,
            hf_repo: None,
            hf_filename: None,
            hf_revision: None,
            hf_revision_ref: None,
        })
        .await
        .expect("register model");
    }

    tokio::task::spawn_blocking(move || {
        let read = |id: &str| {
            db.with_connection(|conn| Ok(models::get_model(conn, id)?))
                .expect("read model")
                .expect("registered row")
                .capabilities
        };
        let reasoning = read("Qwen3-4B-Instruct").expect("determined");
        assert_eq!(reasoning, ModelCapabilities::local("Qwen3-4B-Instruct"));
        assert_eq!(reasoning.reasoning, Some(ReasoningControl::ModelManaged));
        assert_eq!(reasoning.accepted_attachment_kinds, Some(Vec::new()));

        let plain = read("Qwen2.5-0.5B-Instruct").expect("determined");
        assert_eq!(plain.reasoning, Some(ReasoningControl::Unavailable));
        drop(db);
        drop(dir);
    })
    .await
    .expect("assert join");
}
