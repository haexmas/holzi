# Contract: Tauri Commands (Rust ↔ Frontend)

**Status**: Working draft against an unreleased `haex-crdt` revision that
includes the PR removing `reconcile_device_id` plus a pre-HLC bootstrap hook.
The currently released `v0.1.0` does not provide that integration. Field names
and shapes are stable enough to implement against; the error-mapping table
below reflects the current crate surface. Any haex-crdt change to migration
IDs, provider trait signatures, bootstrap ordering, or the `Database::open`
shape reopens this contract.

**V1 contract note**: Genesis creates a fresh vault, generating a per-vault
identity keypair (see [Vault identity and device model](#vault-identity-and-device-model)).
Backup restore is a plain `open_instance` of a copied `.db` — no rekey, no
attestation, no restore-pairing handshake. The copy carries the vault
identity and the source's `known_devices` rows; the pre-HLC bootstrap reuses
the local installation row when present or adds a local-only row with a fresh
vault-scoped device UUID when the copy came from another installation.

**Convention**: All commands are async, return `Result<T, HolziError>` on the Rust side, and are exposed to the frontend via `@tauri-apps/api/core::invoke<T>(name, args)`. Argument keys are `camelCase` from the frontend, mapped to `snake_case` Rust field names by serde.

## Data layer and providers

`haex-crdt` exposes a `Database` handle and requires three provider
implementations from holzi plus a `trigger_version` configuration value. These
providers, the holzi-owned migrations, and the error-mapping layer are the
seams between holzi and the crate. This section documents them normatively;
command sections reference these seams without re-explaining them.

### Vault identity and device model

**Three layered identities**, each with a distinct scope:

- **Vault identity** — one keypair per vault, generated at Genesis and stored
  in the vault DB (`vault_identity` singleton). Copied along with the `.db`,
  so every replica of the same vault holds the same vault identity by
  definition. Used to authenticate an install to other replicas of the same
  vault via proof-of-possession on connection setup: the initiator signs a
  per-connection challenge, the receiver verifies against the vault public
  key it also holds. No shared secret over the wire, no persisted attestation.
- **Installation UUID** — one random UUID per holzi installation on a host,
  stored in a single file **outside** any vault, at `<AppLocalData>/installation-id`.
  Shared by every vault opened by that installation. **Never leaves the
  device**, never enters any vault DB, never touches the wire. Its sole
  purpose is to serve as the local key for looking up "which vault-device
  row belongs to this installation" — see `known_devices` below. This is
  the mechanism that makes two vaults on one host unlinkable across sync:
  their vault-device UUIDs are independently random, and the installation
  UUID that ties them together locally never surfaces beyond the local
  filesystem.
- **Vault-device UUID** — one per (vault × installation), stored as a row in
  the vault's `known_devices` table. The `installation_uuid` used to find the
  local row is a local-only column and is excluded from every CRDT payload;
  `vault_device_uuid`, alias, timestamps, and endpoint metadata form the
  synchronizable device payload. This UUID is the HLC node id for this replica
  of this vault. Two vaults opened by the same installation get independent
  random vault-device UUIDs; two installations opening the same vault (via a
  copied `.db`) get independent random vault-device UUIDs.

The lookup on every open is: read the local installation UUID → find the
matching local-only `known_devices.installation_uuid` row in the vault → use
that row's vault-device UUID as HLC node id. A copied database from the same
installation therefore reuses its row; a copy from another installation has
no matching local row and receives a fresh UUID. The bootstrap transaction
must leave the local-only column out of CRDT payloads.

### Pre-HLC known-device bootstrap

The current `haex-crdt` open sequence needs one consumer-facing extension. After
crate and consumer migrations have run, but before HLC initialization or any
signed bootstrap write, the database opener MUST run a single atomic
pre-HLC bootstrap transaction. That operation:

1. Reads the installation UUID from `<AppLocalData>/installation-id` (minting
   and fsyncing the file if needed).
2. Looks up the local-only `known_devices.installation_uuid` column.
3. Reuses the matching `vault_device_uuid`, or mints a fresh one and inserts a
   complete `known_devices` row with all CRDT metadata and the local-only
   lookup value. The local-only column is excluded from the CRDT projection.
4. For Genesis, creates the vault identity keypair and singleton
   `vault_identity` row in the same unsigned bootstrap phase. For an imported
   database, it verifies that the copied vault identity is present.
5. Commits both results atomically, or rolls both back.

The hook returns the persisted vault-device UUID and the vault identity needed
to construct the providers. HLC initialization then consumes that UUID through
`DeviceIdProvider::device_id()`. The provider MUST be a pure, already-resolved
provider for this open: it returns the UUID it was given and MUST NOT open a
database connection or transaction. `Database::open` MUST NOT initialize HLC or
perform signed bootstrap writes until this hook has committed.

### Provider implementations

Three traits implemented by holzi in a dedicated `src-tauri/identity/`
module. Extraction into a standalone `haex-identity` crate is deferred until
the trait surface has stabilized against a working slice.

- **`DeviceIdProvider`** — returns the current replica's vault-device UUID.
  The pre-HLC bootstrap performs the installation-file read, local lookup, mint,
  and transaction. The provider is then constructed from the persisted result;
  its `device_id()` only returns that UUID, so it has no database or transaction
  dependency. This guarantees that the UUID committed by bootstrap is the one
  HLC uses as its node id for this open.
- **`SignatureProvider`** — signs the canonical column preimage supplied to
  `sign_column`, using the vault identity keypair. Verification is a later
  sync concern; holzi ships the signatures now so the deferred sync layer
  needs no migration over populated tables.
- **`MigrationSource`** — enumerates the holzi-owned migrations below.
  haex-crdt's own bookkeeping migrations (`_no_sync` tables and
  `_no_trigger` metadata columns) are separate and shipped by the crate.

Holzi also passes **`trigger_version: i32`** in `DatabaseConfig`. This is
haex-crdt's own trigger-schema version; holzi passes
`haex_crdt::DEFAULT_TRIGGER_VERSION` verbatim and bumps it only in lockstep
with a crate-required upgrade.

**Crate integration uses the provider-authoritative open hook.** The unreleased
haex-crdt revision containing PR #23 no longer arbitrates device IDs —
`reconcile_device_id` is gone — but the pre-HLC bootstrap hook is still required
so holzi can resolve and persist `known_devices` before HLC starts. The released
`v0.1.0` surface is not sufficient for this contract.

### holzi-owned data layer

Tables holzi ships via its `MigrationSource`. Installed as CRDT-tracked
tables via `install_crdt(table, opts)` unless marked `_no_sync`. The MVP
ships only the first two; the rest are named here so the deferred sync layer
lands on already-CRDT-tracked tables.

- **`vault_identity`** (singleton row) — the vault identity keypair (private
  key + public key). Present from Genesis; copied along with the `.db`, so
  every replica of the same vault shares this row. Private key material
  never leaves the encrypted DB.
- **`known_devices`** — one row per (vault × installation) that has ever
  opened this vault: `installation_uuid` is a local-only lookup column matching
  the value stored in `<AppLocalData>/installation-id`; it is never included in
  CRDT payloads. `vault_device_uuid` (the HLC node id for this replica), alias,
  first-seen timestamp, and (later, with sync) an iroh node id are the
  synchronizable fields. Rows arriving from another replica have no local
  installation lookup value. This boundary keeps two vaults on one host
  unlinkable across sync while allowing a copied database on the same
  installation to reuse its local row.

Post-MVP additions (mentioned here to reserve names, not shipped): a
capability/grant table for cross-device authorization once sync lands, and
whatever the deferred cross-user-sharing design decides on for revocation.

### Node-ID collision detection (post-MVP)

The vault-device UUID must be unique per replica or CRDT conflict resolution
stops being deterministic. With `known_devices` keyed by installation UUID,
the invariant holds by construction as long as installation UUIDs are
unique. It can break only when the installation UUID file itself is
duplicated: VM clone, `rsync -a` of the entire app data directory,
full-system backup restore. Both replicas then look up the same
`known_devices` row and get the same vault-device UUID.

Holzi cannot detect this at open time on a single-install host. Once the
sync layer is in place, the invariant MUST be enforced by the sync layer:
on the first exchange with any peer, if the peer's advertised vault-device
UUID matches the local one, both sides raise a hard error and refuse to
sync until one side is regenerated. This is a post-MVP requirement, listed
here so the sync design accounts for it from day one.

**Never permitted**: the same `.db` open in two processes at once. `fs2`
prevents it on one host. Across a shared network mount or a cloud-sync
folder it cannot be prevented reliably — services like Dropbox replicate
byte ranges rather than transactions — so this is a documented
anti-requirement, and startup SHOULD warn when an instance path looks like a
known sync folder.

### Error mapping

The direct `haex-crdt::Error` variants map to `HolziError` as follows. Errors
returned through haex-crdt's internal `DatabaseError` conversion are surfaced
as `CrdtInit { reason }` until the crate exposes a typed top-level variant.

| haex-crdt variant | HolziError variant | Notes |
| --- | --- | --- |
| `Sqlite(error)` | `CrdtSqlite { reason: error.to_string() }` | SQL/SQLCipher failure |
| `Io(error)` | `CrdtIo { reason: error.to_string() }` | I/O failure |
| `Hlc(reason)` | `CrdtHlc { reason }` | HLC/provider failure |
| `DeviceIdMismatch { expected, supplied }` | `DeviceIdMismatch { expected, supplied }` | not expected in normal operation — holzi's à-la-carte integration (see [Provider implementations](#provider-implementations)) does not go through `reconcile_device_id`; this maps only a defensive `HlcService::initialize_in_place` self-check |
| `SignatureVerificationFailed { first_failed_change }` | `CrdtSignatureVerificationFailed { first_failed_change }` | remote batch is rejected atomically |
| `UnexpectedSignatureUnderNoop` | `CrdtUnexpectedSignatureUnderNoop` | transport/provider configuration error |
| `MigrationMissingFromSource { journal, name }` | `MigrationMissingFromSource { journal, name }` | catastrophic; source and journal are retained |
| `MigrationContentDrift { name, expected, found }` | `MigrationContentDrift { name, expected, found }` | catastrophic; abort |
| `MigrationCompatibility { reason }` | `MigrationCompatibility { reason }` | incompatible legacy schema |
| `CrdtAlreadyInstalled { table }` | `CrdtAlreadyInstalled { table }` | logic error in holzi's bootstrap |
| `Message(reason)` | `CrdtInit { reason }` | catch-all for the pinned crate's database-layer conversion |

At the pinned revision, filesystem-level lock collisions are converted inside
haex-crdt to `Error::Message`, so Holzi MUST NOT claim a typed
`VaultAlreadyOpenElsewhere` mapping from the crate yet. Once haex-crdt exposes
that condition as a typed top-level error, Holzi may map it to the distinct
`VaultAlreadyOpenElsewhere` variant. It remains distinct from
`InstanceAlreadyActive` (a process-internal `AppState.active_instance ==
Some(_)`).

## Commands

### `list_instances`

Scans `<AppLocalData>/instances/` for `.db` files (excluding the `.trash/` subdirectory and pending Genesis files) and returns metadata.

```rust
#[tauri::command]
pub async fn list_instances(
    app: AppHandle,
) -> Result<Vec<InstanceInfo>, HolziError>;
```

**Frontend call**:

```ts
const list = await invoke<InstanceInfo[]>('list_instances')
```

**Returns**: `InstanceInfo[]` sorted by `lastAccess` descending. See `types.md`.

**Failure modes**:

- `HolziError::PathResolution` — `AppLocalData` cannot be resolved (rare, catastrophic).
- `HolziError::Io` — directory read failed (permissions).

---

### `create_instance`

Creates a new vault under `<AppLocalData>/instances/`: opens a fresh SQLCipher DB, runs holzi's migrations, and generates the vault identity keypair. No pairing, no join — the MVP has one command for creating a vault.

```rust
#[tauri::command]
pub async fn create_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    args: CreateInstanceArgs,
) -> Result<CreateInstanceResult, HolziError>;

pub struct CreateInstanceArgs {
    pub name: String,
    pub passphrase: String,
}

pub struct CreateInstanceResult {
    pub info: InstanceInfo,
}
```

**Preconditions**: `name` matches `^[A-Za-z0-9][A-Za-z0-9_\-]{0,63}$`; passphrase meets min-length policy; no active instance in `AppState`.

**Internal choreography**:

1. Validate `name` and passphrase policy in-process (haex-crdt accepts opaque strings; policy is holzi's job).
2. Ensure `<AppLocalData>/installation-id` exists — read it if present, otherwise mint a fresh v4 UUID and write it (fsync). This file is one-time-per-installation, not per-vault; it may already exist from earlier holzi use on this host.
3. Write `<name>.db.pending` as an empty marker, then create `<name>.db` as the candidate path in `<AppLocalData>/instances/`. Marker presence controls publication.
4. Run the pre-HLC bootstrap after migrations. In one unsigned transaction, generate the vault identity keypair and insert the singleton `vault_identity` row, then look up or mint the local-only `known_devices.installation_uuid` row with its complete CRDT metadata. No HLC initialization or signed write may happen before this transaction commits.
5. Construct a `DeviceIdProvider` from the persisted vault-device UUID and a `SignatureProvider` from the bootstrapped vault identity, then call `Database::open` with `create_if_missing: true`. The provider performs no database I/O; HLC receives exactly the UUID committed by bootstrap.
6. Bind `AppState.active_instance = Some(info)`.
7. Flush the DB and directory, remove `<name>.db.pending`, flush the directory again. Marker removal is the durable commit point.
8. Emit `instance-list-changed`.

**Postconditions on success**: `<name>.db` exists and is unlocked and active; `<AppLocalData>/installation-id` exists (it may have already existed); the `vault_identity` and self-`known_devices` rows are written; the pending marker is gone.

**Postconditions on failure**: On any failure before step 7, drop every `Arc<Database>` clone, verify the file lock is released, and remove `<name>.db` and `<name>.db.pending`. Leave `<AppLocalData>/installation-id` in place — it is shared with any other vaults on this host and MUST NOT be deleted by a per-vault operation. A crash between steps 3 and 7 leaves the pending marker; startup orphan-cleanup removes it together with the sibling `.db`.

**Failure modes**:

- `HolziError::NameConflict { name }` — a `.db` with that name already exists.
- `HolziError::InvalidName { reason }` — regex mismatch.
- `HolziError::WeakPassphrase { reason }` — policy failure.
- `HolziError::InstanceAlreadyActive` — another instance is currently active in `AppState`.
- `HolziError::MigrationMissingFromSource { .. }` / `HolziError::MigrationContentDrift { .. }` / `HolziError::MigrationCompatibility { reason }` / `HolziError::CrdtSqlite { reason }` / `HolziError::CrdtIo { reason }` / `HolziError::CrdtHlc { reason }` / `HolziError::CrdtAlreadyInstalled { table }` / `HolziError::CrdtInit { reason }` — see [Error mapping](#error-mapping). All are catastrophic during `create_instance`; the backend rolls back per postconditions above.

---

### `open_instance`

Opens an existing `.db` (whether created locally, imported from another install, or restored from backup): unlocks SQLCipher, wires the CRDT layer, and marks the instance active in `AppState`. There is no rekey, no restore-pairing handoff, no adoption gate — the vault identity in the DB is authoritative, and the vault-device UUID for this replica comes from the `known_devices` lookup keyed by the local installation UUID (inserted with a fresh UUID on first open of this vault by this installation).

```rust
#[tauri::command]
pub async fn open_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    args: OpenInstanceArgs,
) -> Result<InstanceInfo, HolziError>;

pub struct OpenInstanceArgs {
    pub name: String,
    pub passphrase: String,
}
```

**Preconditions**: `<name>.db` exists and is not a pending Genesis database (Genesis is identified by the sibling `<name>.db.pending` marker).

**Internal choreography**:

1. Ensure `<AppLocalData>/installation-id` exists (mint and fsync if absent).
2. Run the pre-HLC bootstrap after migrations. It reads the installation UUID, looks it up in the local-only `known_devices.installation_uuid` column, and either reuses the found `vault_device_uuid` or mints a fresh one, inserts the complete row, and commits. Construct the pure `DeviceIdProvider` from that result, then call `Database::open` with `create_if_missing: false`.
3. If another instance was active, stop its runtime; then bind `AppState.active_instance = Some(info)` and refresh the DB mtime as `lastAccess`.
4. Emit `instance-list-changed`.

**Postconditions on success**: SQLCipher unlocked; `AppState.active_instance = Some(...)`; DB mtime refreshed; the local-only `known_devices` column contains a row matching the local installation UUID. If another instance was active, it is fully closed before the new one becomes visible.

**Atomic switch and rollback**: the state lock is held throughout. Validate the requested file and SQLCipher credentials while the previous runtime remains active; on validation failure, no close or state transition occurs. Only after the new candidate opens successfully does the previous runtime stop. On candidate startup failure, drop every candidate handle and keep the previous runtime; the copy remains on disk for retry. A `known_devices` row inserted during a failed candidate open remains in the DB — it is harmless (an unused entry for this installation) and will be picked up on the next successful open. A failed unlock retains the file; deletion requires an explicit discard action or conclusive validation that the file is not a holzi instance.

**Failure modes**:

- `HolziError::NotFound { name }` — no such file.
- `HolziError::WrongPassphrase` — SQLCipher rejected. **The frontend MUST NOT expose whether the error was `NotFound` vs `WrongPassphrase`** (FR-021); it renders both as a generic "Öffnen fehlgeschlagen". The typed error is for logs and telemetry only.
- `HolziError::NotAValidInstance { reason }` — the database or SQLCipher format is invalid.
- `HolziError::Io` — read error.
- Any haex-crdt-mapped variant listed in [Error mapping](#error-mapping) if migrations or CRDT installation fail.

---

### `close_instance`

Closes the currently-active instance: shuts down Nostr relay, disconnects iroh peer, drops the last `Arc<Database>` clone (haex-crdt closes the SQLCipher handle and releases its `fs2` lock as a consequence of the last-Arc drop, not through a separate crate call), clears `AppState.active_instance`. Idempotent — succeeds if nothing is active.

```rust
#[tauri::command]
pub async fn close_instance(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), HolziError>;
```

**Postcondition on success**: no `Arc<Database>` clone survives inside `AppState` or any subsystem holzi owns. If a subsystem is still holding a clone at drop time, `close_instance` MUST error with `CloseFailed` rather than returning `Ok`; a returned `Ok` implies the file lock is released and a subsequent `open_instance` of the same file will succeed on this host.

After a successful close of an active instance, emit `instance-list-changed { reason: 'closed', affectedName: name }`. The event requires the `AppHandle` dependency; an idempotent close with no active instance emits no mutation event.

**Failure modes**:

- `HolziError::CloseFailed { reason }` — a subsystem shutdown returned an error. State is left as clean as possible; caller may retry.

---

### `import_instance_file`

Copies an external `.db` file into `<AppLocalData>/instances/`. Structural validation happens before copying; SQLCipher credential validation is completed by `open_instance`. The copy carries the source's `known_devices` rows unchanged. On first open, the pre-HLC bootstrap reuses a matching local-only installation row when the source came from the same installation, otherwise it inserts a new local row with a fresh vault-device UUID. The local-only `installation_uuid` column is never included in CRDT payloads.

```rust
#[tauri::command]
pub async fn import_instance_file(
    app: AppHandle,
    state: State<'_, AppState>,
    args: ImportInstanceArgs,
) -> Result<ImportInstanceResult, HolziError>;

pub struct ImportInstanceArgs {
    pub source_path: String,        // External path returned by plugin-dialog; never a managed path
    pub on_conflict: ConflictPolicy,
}

pub enum ConflictPolicy {
    Rename,      // Append -2, -3, … (default)
    Overwrite,   // Replace existing (frontend must confirm first)
    Abort,       // Fail with NameConflict
}

pub struct ImportInstanceResult {
    pub info: InstanceInfo,
    pub renamed_from: Option<String>,  // Set if on_conflict=Rename triggered
}
```

**Postconditions on success**: the destination `<name>.db` exists, and `instance-list-changed { reason: 'imported', affectedName: info.name }` is emitted after the durable copy completes.

**Notes**:

- `source_path` is the external file path returned by `@tauri-apps/plugin-dialog`; it is the only path accepted from the frontend. It is not a managed-instance path. The command validates that it is a regular file and that the extension is `.db`.
- SQLCipher page validation is deferred until `open_instance`, where the operator supplies the passphrase. The pre-HLC bootstrap reuses the matching local-only `known_devices.installation_uuid` row for a same-installation copy, or inserts one with a fresh vault-device UUID for a copy from another installation. That local-only lookup column is excluded from CRDT payloads.
- Copy uses a crash-safe publish protocol: write the database to a temporary file in the managed directory and fsync it; fsync the directory; atomically rename the temporary database to `<name>.db`; then fsync the directory again before emitting the import event. For `ConflictPolicy::Overwrite`, stage as `<name>.db.importing`, rename the old `<name>.db` to an operation-specific backup, atomically rename the staged file into `<name>.db`, fsync the directory, and only then remove the backup. A crash mid-import leaves either the original target intact or a `.importing` file that startup deletes without opening it. The source file is never modified or moved.
- For `ConflictPolicy::Overwrite`, the command checks the target name while holding the `AppState` lock and rejects replacement if that name is the active instance. The frontend must close that instance explicitly before retrying overwrite. On open, the new file reuses a matching local installation row when present; otherwise bootstrap inserts one with a fresh vault-device UUID.

**Failure modes**:

- `HolziError::NotAValidInstance { reason }` — the source is not a regular `.db` file or fails structural validation before copying.
- `HolziError::NameConflict { name }` — only when `on_conflict = Abort`.
- `HolziError::InstanceActive { name }` — `Overwrite` targets the active instance.
- `HolziError::Io` — copy failed.

---

### `move_instance_to_trash`

Soft-deletes an instance by moving `<name>.db` into `<AppLocalData>/instances/.trash/<name>-<timestamp>.db`. Refuses if the target is currently the active instance.

```rust
#[tauri::command]
pub async fn move_instance_to_trash(
    app: AppHandle,
    state: State<'_, AppState>,
    args: TrashInstanceArgs,
) -> Result<(), HolziError>;

pub struct TrashInstanceArgs {
    pub name: String,
}
```

**Postconditions on success**: the source database is absent from the managed root, the trashed `.db` exists under `.trash/`, and `instance-list-changed { reason: 'trashed', affectedName: name }` is emitted only after the move completes. `<AppLocalData>/installation-id` is unaffected — it is installation-scoped, not per-vault. If the operator later restores the trashed file (out of scope for v1), the `known_devices` row for this installation is still inside the file and will be reused; a new row is inserted only if the file is opened on a different installation.

**Failure modes**:

- `HolziError::NotFound { name }`.
- `HolziError::InstanceActive { name }` — refuses to trash the running instance.
- `HolziError::Io`.

---

### `forget_instance` *(out of v1 scope)*

Not exposed in v1. Because the list is a directory scan, removing an item while retaining its file would require persistent exclusion metadata. The only v1 removal action is `move_instance_to_trash`.

## Error type

```rust
#[derive(thiserror::Error, Debug, serde::Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(tag = "kind")]
pub enum HolziError {
    #[error("Instance '{name}' already exists")]
    NameConflict { name: String },

    #[error("Invalid instance name: {reason}")]
    InvalidName { reason: String },

    #[error("Passphrase does not meet policy: {reason}")]
    WeakPassphrase { reason: String },

    #[error("Wrong passphrase")]
    WrongPassphrase,

    #[error("No instance named '{name}'")]
    NotFound { name: String },

    #[error("An instance is already active in this process")]
    InstanceAlreadyActive,

    #[error("Vault file is locked by another process on this host")]
    VaultAlreadyOpenElsewhere,

    #[error("Instance '{name}' is active; close it before this operation")]
    InstanceActive { name: String },

    #[error("File is not a valid holzi instance: {reason}")]
    NotAValidInstance { reason: String },

    #[error("Failed to close active instance: {reason}")]
    CloseFailed { reason: String },

    // Mapped from haex-crdt Error variants — see "Error mapping" above.
    #[error("SQLCipher error from haex-crdt: {reason}")]
    CrdtSqlite { reason: String },

    #[error("I/O error from haex-crdt: {reason}")]
    CrdtIo { reason: String },

    #[error("HLC error from haex-crdt: {reason}")]
    CrdtHlc { reason: String },

    #[error("Device UUID mismatch: expected {expected}, supplied {supplied}")]
    DeviceIdMismatch { expected: String, supplied: String },

    #[error("Migration {name} from {journal} is missing from MigrationSource")]
    MigrationMissingFromSource { journal: String, name: String },

    #[error("Migration {name} content drift between database and MigrationSource")]
    MigrationContentDrift {
        name: String,
        expected: String,
        found: String,
    },

    #[error("Legacy schema is incompatible: {reason}")]
    MigrationCompatibility { reason: String },

    #[error("Remote CRDT signature verification failed at change #{first_failed_change}")]
    CrdtSignatureVerificationFailed { first_failed_change: usize },

    #[error("Unexpected non-empty signature under NoopSignatureProvider")]
    CrdtUnexpectedSignatureUnderNoop,

    #[error("CRDT already installed for table {table}")]
    CrdtAlreadyInstalled { table: String },

    #[error("haex-crdt init failed: {reason}")]
    CrdtInit { reason: String },

    #[error("Path resolution failed: {reason}")]
    PathResolution { reason: String },

    #[error("I/O error: {reason}")]
    Io { reason: String },
}
```

## Frontend error handling

- `WrongPassphrase` and `NotFound` MUST both surface to the operator as a single generic message (FR-021). Do not branch UI on the discriminator.
- All other errors surface via `sonner` toast with the localized message from `i18n/locales/{de,en}.json` under `errors.<kind>`.
- Fallback for unknown discriminators: log to console + generic toast.
