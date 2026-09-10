//! holzi-owned migrations shipped via `MigrationSource`.
//!
//! Contract: `tauri-commands.md` §"holzi-owned data layer". Slices a and b
//! land seven tables: identity (`vault_identity`, `known_devices`) plus the
//! Etappe-2 tables (`providers`, `models`, `chat_threads`, `chat_messages`,
//! `device_downloaded_models_no_sync`) and their supporting indexes.
//!
//! CRDT-tracked tables (no `_no_sync` suffix) get the three metadata
//! columns (`haex_hlc_no_sync`, `haex_column_hlcs_no_sync`,
//! `haex_column_sigs_no_sync`) added by haex-crdt's `CrdtTransformer` at
//! migration time, and the triggers installed by
//! `ensure_triggers_initialized` inside `Database::open`. holzi does NOT
//! call `install_crdt` (Etappe-0 finding: the transformer + trigger
//! installer cover every non-`_no_sync` *table* automatically).
//!
//! That automatic coverage stops at the table level. Each trigger bakes
//! its tracked-*column* list in at creation time, and the installer
//! early-returns on a vault whose stored trigger version is already
//! current — so a migration that adds a column to an existing tracked
//! table must bump [`HOLZI_TRIGGER_VERSION`] as well, or the new column
//! stays untracked on every already-provisioned vault.
//!
//! `device_downloaded_models_no_sync` has the `_no_sync` suffix on the
//! table name so haex-crdt skips it entirely — downloaded GGUF files are
//! per-installation state (their absolute path lives under this host's
//! `AppLocalData/models/`) and must not sync.

use std::collections::BTreeMap;
use std::sync::Arc;

use haex_crdt::{MigrationName, StaticMigrationSource};

/// CRDT trigger-schema version holzi installs on open.
///
/// haex-crdt bakes the tracked-column list into each table's
/// `AFTER UPDATE OF <cols>` trigger when the trigger is created, and
/// `ensure_triggers_initialized` skips the rewrite entirely once the
/// vault's stored version is >= the one passed in. A migration that adds
/// a column to a CRDT-tracked table therefore leaves that column
/// untracked on every already-provisioned vault — writes to it neither
/// mark the row dirty nor record a per-column HLC, so the value never
/// reaches another device.
///
/// Bump this whenever a migration alters the column shape of a
/// CRDT-tracked (non-`_no_sync`) table, so `Database::open` recreates
/// the triggers against the migrated schema.
///
/// - 1: `DEFAULT_TRIGGER_VERSION`, migrations 0001-0008.
/// - 2: `0009_models_add_tokenizer_repo` added a column to `models`.
/// - 3: `0010_providers_add_adapter` added a column to `providers`.
pub const HOLZI_TRIGGER_VERSION: i32 = 3;

/// Returns the frozen holzi migration set at the pinned haex-crdt revision.
pub fn holzi_migration_source() -> Arc<StaticMigrationSource> {
    let mut m: BTreeMap<MigrationName, String> = BTreeMap::new();

    // vault_identity — singleton row inserted at Genesis by the bootstrap
    // hook. Never mutated after Genesis; copied along with the .db, so
    // every replica of the same vault shares the row by construction.
    m.insert(
        MigrationName::from("0001_vault_identity"),
        "CREATE TABLE vault_identity (\
            id INTEGER PRIMARY KEY CHECK (id = 1), \
            pubkey BLOB NOT NULL, \
            privkey BLOB NOT NULL\
         );"
        .to_string(),
    );

    // known_devices — one row per (vault × installation). `installation_uuid`
    // is the immutable primary key carried in CRDT `row_pks`; the further
    // columns are plain CRDT-tracked fields. See contract §"Vault identity
    // and device model" for why installation_uuid is a PK, not a mutable
    // column update.
    m.insert(
        MigrationName::from("0002_known_devices"),
        "CREATE TABLE known_devices (\
            installation_uuid TEXT PRIMARY KEY NOT NULL, \
            vault_device_uuid TEXT NOT NULL UNIQUE, \
            alias TEXT, \
            first_seen INTEGER NOT NULL\
         );"
        .to_string(),
    );

    // providers — one row per configured provider. `kind` decides how the
    // row is used: `local` = mistralrs GGUF (credentials NULL, base_url
    // NULL), `api_key` = external HTTP API (credentials required),
    // `cli_delegate` = spawn an external CLI (credentials NULL, base_url
    // holds the CLI binary name or path). Credentials sync per operator
    // decision 2026-09-08 (plan §"Anbietermodelle").
    m.insert(
        MigrationName::from("0003_providers"),
        "CREATE TABLE providers (\
            id TEXT PRIMARY KEY NOT NULL, \
            kind TEXT NOT NULL, \
            name TEXT NOT NULL, \
            base_url TEXT, \
            credentials BLOB, \
            created_at INTEGER NOT NULL\
         );"
        .to_string(),
    );

    // models — cache of the models each provider exposes. For local
    // providers, one row per catalog entry the user has picked or
    // downloaded. `fetched_at` is the last catalog/API refresh in epoch
    // milliseconds; NULL for entries seeded from the built-in catalog
    // before any refresh has happened. `context_window` is best-effort
    // metadata for the pre-flight length check (plan §"Anbietermodelle":
    // "Überschreitet der Verlauf das Kontextfenster … scheitert die
    // Anfrage").
    m.insert(
        MigrationName::from("0004_models"),
        "CREATE TABLE models (\
            id TEXT PRIMARY KEY NOT NULL, \
            provider_id TEXT NOT NULL, \
            name TEXT NOT NULL, \
            context_window INTEGER, \
            fetched_at INTEGER\
         );"
        .to_string(),
    );

    // chat_threads — one row per conversation. `last_provider_id` /
    // `last_model_id` record what was most recently used so a new
    // message can prefill the selector; individual messages carry their
    // own provider/model (plan §"Anbietermodelle": mid-conversation
    // model switch keeps per-message attribution).
    m.insert(
        MigrationName::from("0005_chat_threads"),
        "CREATE TABLE chat_threads (\
            id TEXT PRIMARY KEY NOT NULL, \
            title TEXT NOT NULL, \
            last_provider_id TEXT, \
            last_model_id TEXT, \
            created_at INTEGER NOT NULL, \
            updated_at INTEGER NOT NULL\
         );"
        .to_string(),
    );

    // chat_messages — one row per finalised message. `parent_id` tracks
    // conversation branching (plan §"Datenmodell": Elternbezüge erhalten
    // Verzweigungen). `finish_reason` is one of 'complete' | 'cancelled'
    // | 'error'; in-flight generations are NOT stored here — they live
    // in `app_settings_no_sync` (later slice) and only land here on
    // finalisation.
    m.insert(
        MigrationName::from("0006_chat_messages"),
        "CREATE TABLE chat_messages (\
            id TEXT PRIMARY KEY NOT NULL, \
            thread_id TEXT NOT NULL, \
            parent_id TEXT, \
            role TEXT NOT NULL, \
            content TEXT NOT NULL, \
            provider_id TEXT, \
            model_id TEXT, \
            prompt_tokens INTEGER, \
            completion_tokens INTEGER, \
            finish_reason TEXT, \
            created_at INTEGER NOT NULL\
         );"
        .to_string(),
    );

    // device_downloaded_models_no_sync — this installation's GGUF file
    // registry. NOT synced (plan §"Datenmodell": local file paths, and
    // model bytes stay under `AppLocalData/models/`, never in Git or in
    // the sync payload). `id` matches `models.id` for the corresponding
    // catalog entry. `relative_path` is relative to
    // `AppLocalData/models/` so the same row is portable across the
    // sandboxing rules of desktop, Android and iOS.
    m.insert(
        MigrationName::from("0007_device_downloaded_models_no_sync"),
        "CREATE TABLE device_downloaded_models_no_sync (\
            id TEXT PRIMARY KEY NOT NULL, \
            relative_path TEXT NOT NULL, \
            size_bytes INTEGER NOT NULL, \
            sha256 TEXT, \
            verified_at INTEGER NOT NULL\
         );"
        .to_string(),
    );

    // list_messages and chat history assembly filter by thread and order by
    // creation time. Keep the lookup bounded as a thread grows. This is a
    // new migration so the already-issued 0007 migration remains immutable.
    m.insert(
        MigrationName::from("0008_chat_messages_thread_index"),
        "CREATE INDEX idx_chat_messages_thread \
         ON chat_messages (thread_id, created_at, id);"
            .to_string(),
    );

    // `load_local_model` needs a tokenizer repo to feed mistralrs. Until
    // this migration it was looked up from the built-in catalog only,
    // which meant arbitrary HuggingFace downloads (via
    // `download_model_from_hf` with a caller-supplied `tokenizer_repo`)
    // could not be loaded after a restart — the value was thrown away.
    // Column is NULL for api_key/cli_delegate rows (they do not tokenize
    // locally) and is set on download / import going forward. Pre-existing
    // catalog rows are back-filled lazily by `models::commands` on the
    // next `list_installed_models` call.
    m.insert(
        MigrationName::from("0009_models_add_tokenizer_repo"),
        "ALTER TABLE models ADD COLUMN tokenizer_repo TEXT;".to_string(),
    );

    // API-key providers must retain the adapter/vendor they belong to. A
    // nullable value keeps older rows readable while refresh rejects those
    // rows until they are explicitly migrated to a supported adapter.
    m.insert(
        MigrationName::from("0010_providers_add_adapter"),
        "ALTER TABLE providers ADD COLUMN adapter TEXT;".to_string(),
    );

    Arc::new(StaticMigrationSource(m))
}
