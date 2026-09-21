//! Integration test for tasks.md T023 (analyze finding G3, spec.md FR-013
//! Edge Cases): disconnecting a `cli_delegate` provider while one of its
//! responses is still streaming must not affect that in-flight response —
//! only a *subsequent* request should be affected.
//!
//! This holds by construction: `CliDelegateAdapter::new` takes `credentials`
//! and `binary` as owned values (not a live database handle), so once a
//! generation has started it no longer looks at the `providers` row at all.
//! This test pins that guarantee rather than merely asserting it by reading
//! the code, so a future refactor that reintroduces a live DB dependency
//! into the streaming path would fail it.

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use haex_crdt::{Database, DatabaseConfig, NoopSignatureProvider, SqlCipherKey};
use uuid::Uuid;

use holzi_lib::adapters::cli_delegate::{CliDelegateAdapter, DelegateChatContext, DelegateVendor};
use holzi_lib::adapters::{ChatMessage, ChatRequest, ChatRole, ProviderAdapter, StreamChunk};
use holzi_lib::identity::{holzi_migration_source, installation_id_path, HolziBootstrap};
use holzi_lib::storage::providers::{self as storage, Provider, ProviderCapability, ProviderKind};

const PASSPHRASE: &str = "cli-delegate-disconnect-in-flight-test";
const STUB_TRANSCRIPT: &str = r#"{"type":"system","subtype":"init"}
{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"still here"}}}
{"type":"result","is_error":false,"subtype":"success","result":"still here","usage":{"input_tokens":1,"output_tokens":2}}
"#;

fn open_vault(dir: &Path) -> Database {
    let db_path: PathBuf = dir.join("vault.db");
    let installation_id = installation_id_path(dir);
    Database::open(DatabaseConfig {
        path: db_path,
        key: SqlCipherKey::new(PASSPHRASE),
        create_if_missing: true,
        bootstrap: Arc::new(HolziBootstrap::new(installation_id).with_alias("test")),
        signature_provider: Arc::new(NoopSignatureProvider),
        migration_source: holzi_migration_source(),
        trigger_version: haex_crdt::DEFAULT_TRIGGER_VERSION,
    })
    .expect("vault open")
}

fn write_stub(dir: &Path) -> PathBuf {
    let path = dir.join("fake-claude");
    let script = format!("#!/bin/sh\ncat <<'EOF'\n{STUB_TRANSCRIPT}EOF\n");
    fs::write(&path, script).expect("write stub script");
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).expect("chmod stub script");
    path
}

#[tokio::test]
async fn disconnect_during_a_response_does_not_affect_that_response() {
    let vault_dir = tempfile::tempdir().expect("vault tempdir");
    let db = open_vault(vault_dir.path());
    let stub_dir = tempfile::tempdir().expect("stub tempdir");
    let stub = write_stub(stub_dir.path());

    let provider_id = Uuid::new_v4();
    let provider = Provider {
        id: provider_id,
        kind: ProviderKind::CliDelegate,
        adapter: Some("claude".to_string()),
        name: "Claude Code".to_string(),
        base_url: Some("claude".to_string()),
        credentials: Some(b"fake-token".to_vec()),
        created_at: 0,
        capability: ProviderCapability::Chat,
    };
    db.with_connection(|conn| {
        storage::insert_provider(conn, &provider).map_err(haex_crdt::Error::from)?;
        Ok(())
    })
    .expect("insert provider");

    // Build the adapter from the provider row exactly as `build_adapter`
    // does, then the row is no longer touched — this is the guarantee
    // under test.
    let adapter = CliDelegateAdapter::new(
        DelegateVendor::Claude,
        provider.credentials.clone().unwrap(),
        stub.to_str().unwrap().to_string(),
        Some(DelegateChatContext {
            pending_tool_approvals: Arc::new(Mutex::new(HashMap::new())),
            emit: Arc::new(|_event: &str, _payload: serde_json::Value| {}),
            database: None,
        }),
    );

    let mut stream = adapter
        .stream_chat(ChatRequest {
            model_id: format!("{provider_id}:claude"),
            thread_id: None,
            system_prompt: None,
            messages: vec![ChatMessage {
                role: ChatRole::User,
                attachments: Vec::new(),
                content: "hi".to_string(),
            }],
            reasoning_requested: false,
            max_new_tokens: None,
            tools: Vec::new(),
            autonomy_mode: Default::default(),
            reasoning_option: Default::default(),
            capabilities: Default::default(),
        })
        .await
        .expect("stream_chat should start");

    // Disconnect while the response is still in flight.
    db.with_connection(|conn| {
        storage::delete_provider(conn, provider_id).map_err(haex_crdt::Error::from)?;
        Ok(())
    })
    .expect("delete provider mid-flight");
    assert!(
        db.with_connection(|conn| {
            storage::get_provider(conn, provider_id).map_err(haex_crdt::Error::from)
        })
        .expect("get_provider query")
        .is_none(),
        "provider row should be gone",
    );

    // The already-running generation must still complete normally.
    let mut text = String::new();
    let mut saw_done = false;
    while let Some(chunk) = stream.next().await {
        match chunk.expect("no stream error expected") {
            StreamChunk::Delta { content, .. } => text.push_str(&content),
            StreamChunk::Done { .. } => {
                saw_done = true;
                break;
            }
            StreamChunk::ToolCalls(_) => panic!("a delegate must never emit ToolCalls"),
            StreamChunk::AgentActivity { .. } => {}
        }
    }

    assert!(saw_done, "in-flight response should still complete");
    assert_eq!(text, "still here");
}
