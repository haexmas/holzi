# `haex-crdt` — Extraction Plan

**Status**: Draft. Written 2026-09-04. Companion to [`2026-09-04-v1-scope-design.md`](./2026-09-04-v1-scope-design.md), which decided that holzi consumes `haex-crdt` as a Rust crate dependency and that the crate must exist before holzi's first implementation slice.

**Scope of this document**: how `haex-crdt` gets extracted from `haex-vault` into a standalone repository (`haexmas/haex-crdt`), what its crate boundary is, and what stays behind in `haex-vault`. Not a spec; a refactor + release plan.

**Ownership**: haex-vault owns the source. This plan is written from holzi's side because holzi is the consumer whose timeline depends on it. Cross-link into haex-vault when the extraction actually starts.

---

## 1. Source: what `haex-vault` has today

`haex-vault` is a single Rust crate under `src-tauri/` (Tauri app). No workspace, no `crates/` directory. Storage and CRDT logic sit inside the same crate as the Tauri app itself.

Verified today via a code-mapping pass:

- **Storage**: `rusqlite = "0.40"` with the `bundled-sqlcipher-vendored-openssl` + `functions` + `hooks` features. At-rest encryption is **SQLCipher**, activated via `PRAGMA key`. Home-grown migration engine tied to Drizzle-generated SQL files.
- **CRDT model**: home-grown column-level Last-Write-Wins over SQLite triggers, keyed by `uhlc` Hybrid Logical Clocks. Every CRDT-managed table gets three implicit columns (`haex_hlc`, `haex_column_hlcs`, `haex_column_sigs`) via a `CrdtTransformer` that rewrites `CREATE TABLE` statements. No `automerge`/`yrs`/`loro` — the CRDT is a haex-vault original.
- **Sync transport**: not part of the CRDT core. Iroh QUIC is used for peer sync (`space_delivery/local/`), for large-file transfer (`peer_storage/`), and for backend/cloud file-sync (`file_sync/`). Sync is a separate module that consumes the CRDT layer's scanner and apply APIs.
- **Auth-signed extensions**: `column_sig/` and `registry_row_sig/` add Ed25519 signatures over CRDT column values, tied to UCAN identity. Deeply coupled to `crate::ucan::` and `crate::mls::`.
- **Tauri coupling in CRDT/storage**: narrow. Confined to `HlcService::try_initialize` (reads device ID from `tauri_plugin_store`), the migration loader (`tauri::path::BaseDirectory`), and a scattering of `#[tauri::command]` shims. The internal logic of `trigger.rs`, `scanner.rs`, `cleanup.rs`, `transformer/`, `apply/` uses no `tauri::` imports.
- **Tests**: extensive, mostly inline `#[cfg(test)]` next to the implementation, plus integration tests in `src-tauri/tests/`. The CRDT and storage tests use `Connection::open_in_memory` and `HlcService::new_for_testing` — they do not need a Tauri harness.
- **Change velocity**: the extraction candidates are the calmer surface of haex-vault. Recent churn (103 commits touching `src-tauri/src/{crdt,database}` in ~8 weeks) is concentrated in `column_sig`, sync transport, DoS defences, and UCAN — all of which stay in `haex-vault`.

## 2. Target: what `haex-crdt` is

A standalone Rust crate published as `haexmas/haex-crdt` on GitHub, versioned independently. Consumed by `haex-vault` and `holzi` as a git-tagged dependency (crates.io is deferred until the API stabilizes).

The crate provides:

- Encrypted SQLite storage (SQLCipher).
- A migration engine.
- HLC service (uhlc-based Hybrid Logical Clocks with SQLite-persisted state).
- Column-level LWW CRDT infrastructure: the `CrdtTransformer` that rewrites schemas, the trigger installer that instruments tables, the scanner that reads local changes, the apply pipeline that merges remote changes.
- Cleanup / retention utilities for deleted-row logs.
- A pluggable `SignatureProvider` trait so consumers can add per-column signing without the crate itself depending on a particular identity/auth system.
- A pluggable `DeviceIdProvider` trait so consumers supply the device UUID without the crate depending on a particular OS keystore.
- A migration SQL loader closure so consumers control where migration SQL comes from (bundled resource, disk, embedded, remote fetch).

The crate does **not** provide:

- Sync transport (iroh, WebSocket, cloud). Sync is the consumer's job; the crate exposes scanner + apply APIs that any transport can wire up.
- Identity, UCAN, or MLS. `SignatureProvider` is an interface, not an implementation.
- Any Tauri or async framework opinion beyond what `rusqlite` already implies (blocking I/O; consumers wrap in `spawn_blocking` if they want async).
- Frontend or IPC surface.

## 3. What stays in `haex-vault`

Left behind in `haex-vault` after the extraction:

- `crdt/column_sig/` and `crdt/registry_row_sig/` — Ed25519 signing coupled to `crate::ucan::`. In the new world, these become `haex-vault`'s implementation of `haex-crdt`'s `SignatureProvider` trait, plus their own local storage tables and UCAN plumbing.
- `space_delivery/local/` — iroh + MLS sync loop. Uses `haex-crdt`'s scanner/apply. Stays as `haex-vault`'s sync transport.
- `peer_storage/` — iroh QUIC for large-file peer transfer. Unrelated to CRDT.
- `file_sync/` — cloud/S3-backed backup sync. Consumes CRDT data but is not part of it.
- `owner_sync/`, `mls/`, `ucan/` — identity and encryption plane.
- `external_bridge/`, `extension/` — Tauri-app-scoped concerns.
- All `#[tauri::command]` shims — remain in `haex-vault` as the Tauri entry-point layer over `haex-crdt`'s pure-Rust API.

## 4. Abstraction points

Three coupling surfaces between the extraction candidates and `haex-vault`'s wider system need to be broken by introducing traits/callbacks in the new crate.

### 4.1 `DeviceIdProvider`

Currently `HlcService::try_initialize` reaches through an `&AppHandle` into `tauri_plugin_store`'s `instance.json` to get the persistent device UUID.

New trait in `haex-crdt`:

```rust
pub trait DeviceIdProvider {
    fn device_id(&self) -> Result<Uuid>;
}
```

Consumers implement it however they wish. `haex-vault` wraps its existing store-based lookup; `holzi` provides one backed by its own device-key generation. `haex-crdt` ships a `StaticDeviceId(Uuid)` implementation for tests and simple consumers.

### 4.2 `SignatureProvider`

Currently `crdt/scanner.rs` and `crdt/commands/apply/` call directly into `crate::ucan::` for signing/verifying per-column Ed25519 signatures. This coupling is what makes the current code un-extractable.

New trait in `haex-crdt`:

```rust
pub trait SignatureProvider {
    fn sign_column(&self, preimage: &[u8]) -> Result<Vec<u8>>;
    fn verify_column(&self, preimage: &[u8], sig: &[u8], author: &AuthorId) -> Result<()>;
    fn author_id(&self) -> AuthorId;
}

pub struct NoopSignatureProvider;
```

`haex-vault` implements the trait against its UCAN/DID stack (reusing today's `column_sig` + `registry_row_sig` code, now living inside haex-vault). `holzi` uses `NoopSignatureProvider` in v1 (no per-column signing needed while the closed federation trusts every attested device equally); it can graduate to a real provider post-v1 if needed. The crate's storage schema reserves the `haex_column_sigs` column even for no-op providers, so a future upgrade does not require a migration.

### 4.3 Migration SQL loader

Currently the migration engine calls `tauri::path::BaseDirectory::Resource` to locate SQL files bundled into the Tauri app.

New API in `haex-crdt`:

```rust
pub trait MigrationSource {
    fn load_migration(&self, name: &str) -> Result<String>;
    fn list_migrations(&self) -> Result<Vec<MigrationName>>;
}
```

`haex-vault` wraps its Tauri resource lookup. `holzi` provides an implementation backed by its own bundled resources (Tauri or otherwise). A `StaticMigrationSource(HashMap<String, String>)` ships with the crate for tests.

### 4.4 Optional: event callbacks

Today `crdt::mod::notify_dirty_tables_changed` emits a Tauri event when a scan detects dirty tables. In the crate this becomes a callback the consumer registers:

```rust
pub type DirtyTablesCallback = Arc<dyn Fn(&[String]) + Send + Sync>;
```

`haex-vault` registers a callback that fires a Tauri event. `holzi` registers one that pokes its own state store or MCP surface.

## 5. Module inventory (haex-vault → haex-crdt)

Working proposal; final paths decided at extraction time.

| haex-vault path | haex-crdt path | Notes |
| --- | --- | --- |
| `src-tauri/src/database/mod.rs` (minus `#[tauri::command]`s) | `src/db/mod.rs` | Re-exports adjusted |
| `src-tauri/src/database/core/*` | `src/db/core/*` | Whole subtree |
| `src-tauri/src/database/vault_lock.rs` | `src/db/lock.rs` | `fs2`-based file lock |
| `src-tauri/src/database/migrations/*` (minus Tauri loader) | `src/db/migrations/*` | Loader becomes trait consumer |
| `src-tauri/src/database/connection_context.rs` | `src/db/connection_context.rs` | |
| `src-tauri/src/database/row.rs`, `stats.rs`, `constants.rs`, `paths.rs`, `listing.rs`, `maintenance.rs`, `import_delete.rs`, `error.rs` | `src/db/*` | Direct move |
| `src-tauri/src/crdt/hlc.rs` (minus AppHandle path) | `src/crdt/hlc.rs` | Uses `DeviceIdProvider` |
| `src-tauri/src/crdt/trigger.rs` | `src/crdt/trigger.rs` | Direct move |
| `src-tauri/src/crdt/scanner.rs` | `src/crdt/scanner.rs` | Direct move |
| `src-tauri/src/crdt/cleanup.rs` | `src/crdt/cleanup.rs` | Direct move |
| `src-tauri/src/crdt/transformer/*` | `src/crdt/transformer/*` | Direct move |
| `src-tauri/src/crdt/insert_transformer.rs` | `src/crdt/insert_transformer.rs` | Direct move |
| `src-tauri/src/crdt/commands/apply/*` (minus command shims) | `src/crdt/apply/*` | Rename from `commands/apply` |
| `src-tauri/src/crdt/column_sig/*` | **stays in haex-vault** | Becomes `SignatureProvider` impl |
| `src-tauri/src/crdt/registry_row_sig/*` | **stays in haex-vault** | Same |
| Table name constants used by CRDT (`TABLE_CRDT_*`, `COL_CRDT_*`) | `src/db/constants.rs` | These name generic CRDT bookkeeping tables |
| Vault-specific constants (`vault_settings_key`) | **stays in haex-vault** | |

Tests travel with the modules. Integration tests in `src-tauri/tests/` that touch column_sig or registry_row_sig split: pure CRDT round-trips move to `tests/` in the new crate; sig-specific vectors stay in haex-vault.

## 6. Public API sketch

Not final; a working shape for reviewers.

```rust
// Opening + configuring a store
pub struct StoreConfig {
    pub path: PathBuf,
    pub key: SqlCipherKey,      // wrapper around a byte string
    pub create_if_missing: bool,
    pub device_id: Arc<dyn DeviceIdProvider>,
    pub signature_provider: Arc<dyn SignatureProvider>,
    pub migration_source: Arc<dyn MigrationSource>,
    pub on_dirty_tables: Option<DirtyTablesCallback>,
}

pub struct Store { /* opaque */ }

impl Store {
    pub fn open(config: StoreConfig) -> Result<Self>;
    pub fn with_connection<R>(&self, f: impl FnOnce(&Connection) -> Result<R>) -> Result<R>;
    pub fn hlc(&self) -> &HlcService;
    pub fn apply_migrations(&self) -> Result<()>;
    pub fn install_crdt(&self, table: &str) -> Result<()>; // installs columns + triggers
    pub fn scan_local_changes(&self, scope: ScanScope) -> Result<LocalChanges>;
    pub fn apply_remote_changes(&self, changes: RemoteChanges) -> Result<ApplyReport>;
    pub fn cleanup_deleted_rows(&self, retention: RetentionPolicy) -> Result<CleanupReport>;
}

// SQL rewriting for CRDT-managed tables
pub struct CrdtTransformer;
impl CrdtTransformer {
    pub fn transform_create_table(sql: &str) -> Result<String>;
    pub fn transform_insert(sql: &str) -> Result<String>;
}
```

Consumers wire `Store` up, define their own tables (running each `CREATE TABLE` through `CrdtTransformer::transform_create_table` first, or via `install_crdt` after creation for existing tables), and rely on the `Store` for both local operations and sync-scanning.

## 7. Extraction sequence (source-of-change side, in `haex-vault`)

Concrete steps whoever owns the extraction will take. Not this PR's work; captured so the timing is legible.

1. **Refactor pass in `haex-vault`, no new repo yet.** Introduce the three traits (`DeviceIdProvider`, `SignatureProvider`, `MigrationSource`) inside `haex-vault` itself. Rewrite `HlcService`, migration loader, and column_sig call-sites to go through the traits. Ship no functional change. Verify with existing tests.
2. **Cut the crate as a `haex-vault` sub-crate first.** Turn `haex-vault` into a Cargo workspace; create `crates/haex-crdt/` and move the identified modules there. `haex-vault` imports it as a `path` dependency. All existing tests keep passing.
3. **Move to standalone repo.** Create `haexmas/haex-crdt` on GitHub, copy the crate over with its full git history if possible (`git filter-repo` or manual), open a first release tag (`v0.1.0`). `haex-vault` switches its dependency from `path` to `git = "...", tag = "v0.1.0"`.
4. **First integration for `holzi`.** `holzi`'s first implementation slice adds `haex-crdt` as a git dependency, implements its own `DeviceIdProvider`, uses `NoopSignatureProvider`, supplies a `MigrationSource` for its own schema.

Steps 1 and 2 are the invasive ones inside `haex-vault`. Step 3 is a repo-mechanics operation. Step 4 is holzi's job.

## 8. Testing strategy

- The crate's own test suite is the inline `#[cfg(test)]` tests moved from `haex-vault` plus the pure-CRDT integration tests. All use in-memory SQLite (`Connection::open_in_memory`) or a temp-file SQLCipher DB.
- Tests that require UCAN identity or MLS group state stay in `haex-vault`.
- After Step 2 above, `haex-vault`'s full test suite must still pass with `haex-crdt` as a workspace member. This is the acceptance bar for Step 3.
- `holzi` writes its own small integration test that opens a `haex-crdt` `Store`, defines a toy CRDT-managed table, writes and re-reads a row. This is the smoke test that `haex-crdt` works from a consumer that is not `haex-vault`.

## 9. Versioning, release, distribution

- Semver from the start.
- `v0.1.0` is the initial release once `haex-vault` fully uses the standalone repo. Behavior is expected to be identical to pre-extraction `haex-vault`; the version reflects "this is a new artifact and its API may still change" rather than an API bump.
- Git tags on `haexmas/haex-crdt` are the distribution mechanism. Both consumers use `git = ..., tag = "vX.Y.Z"` in their `Cargo.toml`.
- crates.io publication is deferred until the API has been used by more than one non-example consumer and has settled. Not v1-blocking for holzi.
- License: matches `haex-vault`'s license (to confirm during extraction).

## 10. Open questions

- **Ed25519 signature schema forward-compat.** The three CRDT metadata columns (`haex_hlc`, `haex_column_hlcs`, `haex_column_sigs`) exist regardless of the `SignatureProvider` used. Is `haex_column_sigs` always populated (empty for `NoopSignatureProvider`), or omitted? Impact: schema compatibility between vaults with different providers. Decided during Step 1.
- **`AuthorId` shape.** UCAN uses DIDs; a simpler `holzi` federation could use device pubkeys directly. Whether `haex-crdt`'s `AuthorId` is a `String`, an enum, or a trait matters for how tightly downstream code binds to it. Working proposal: `String` newtype, treated as opaque by the crate.
- **Retention / tombstone policy.** `haex-vault` has its own retention logic in `crdt/cleanup.rs` tuned to space membership. Does the crate expose a `RetentionPolicy` enum with variants like `TimeBased`, `Manual`, or does it hand the tombstone table to the consumer to reap? Working proposal: parameterized retention, defaults to time-based.
- **Async story.** `rusqlite` is blocking. `haex-vault` today wraps calls in Tokio `spawn_blocking` at the command boundary. Does `haex-crdt` stay blocking-only and let consumers wrap, or provide a thin async wrapper? Working proposal: blocking-only in v0; async wrapper post-v1 if needed.
- **Multi-consumer compatibility for the migration engine.** Today the migration journal is haex-vault-specific. If `haex-crdt` owns it, does each consumer maintain its own journal path/table namespace? Working proposal: yes — `MigrationSource` names the migration set; each consumer has its own set; the CRDT bookkeeping tables are shared.
- **Repo-history preservation.** `git filter-repo` to keep commit history of the moved files vs a clean-start commit in the new repo. Working proposal: preserve history when reasonably practical; clean-start is the fallback.

## 11. What this plan does not do

- Does not schedule the work. Timing is up to whoever owns `haex-vault`.
- Does not commit to the exact API. The traits and `Store` sketch are working proposals; the final shape is decided during Step 1.
- Does not decide whether `holzi` writes its own tests against `haex-crdt`'s pre-1.0 API or waits. Working assumption: it does, and lives with the churn.

## 12. Next actions

For holzi (this repo), immediate:

- None until Step 1 or Step 2 above has begun in `haex-vault`.

For `haex-vault` (whoever owns it):

- Step 1 (introduce the three traits inside `haex-vault`, no new repo yet) is the smallest reversible move and unblocks everything after it. It is a self-contained refactor that ships with no functional change and can happen at any time.

For the operator:

- When ready to trigger the extraction, coordinate with the `haex-vault` project. This plan can be cross-linked from a `haex-vault` issue or ADR at that point.
