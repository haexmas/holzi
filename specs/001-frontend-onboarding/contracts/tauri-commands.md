# Contract: Tauri Commands (Rust ↔ Frontend)

**Status**: Draft, spec-phase working proposals. Field names, error variants, and argument shapes MUST be reviewed once `haex-crdt`'s own initialization API is finalized. Nothing here is a settled interface.

**V1 contract note**: Genesis creates a fresh per-SQLite identity without a
paper-seed or confirmation step. Backup restore uses `open_instance` followed
by `pair_restored_instance`, updating the same imported database in place.

**Convention**: All commands are async, return `Result<T, HolziError>` on the Rust side, and are exposed to the frontend via `@tauri-apps/api/core::invoke<T>(name, args)`. Argument keys are `camelCase` from the frontend, mapped to `snake_case` Rust field names by serde.

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

Creates a new `.db` under `<AppLocalData>/instances/`, initializes `haex-crdt` with the given passphrase, runs mode-specific initialization.

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
    pub mode: CreateMode,
}

pub enum CreateMode {
    Genesis,
    Join { token: String },
}

pub struct CreateInstanceResult {
    pub info: InstanceInfo,
}
```

**Frontend call**:

```ts
const result = await invoke<CreateInstanceResult>('create_instance', {
  args: {
    name: 'laptop-home',
    passphrase: '…',
    mode: { type: 'Genesis' },  // or { type: 'Join', token }
  },
})
```

**Preconditions**: `name` matches `^[A-Za-z0-9][A-Za-z0-9_\-]{0,63}$`; passphrase meets min-length policy; no active instance in `AppState`.

**Postconditions on success**: `<name>.db` exists, is unlocked, is bound as the active instance in `AppState`, and has fresh per-instance identity keys plus a Genesis self-record (for Genesis) or mutually signed peer records (for Join). The Nostr relay and iroh peer are already running before the result is returned. Backend emits `instance-list-changed`.

**Postconditions on failure**: no partial file on disk. If a file was created before failure, backend deletes it. A `<name>.db.pending` marker (empty file next to the `.db` during Genesis) is removed with its sibling on startup if orphaned.

**Failure modes**:

- `HolziError::NameConflict { name }` — a `.db` with that name already exists.
- `HolziError::InvalidName { reason }` — regex mismatch.
- `HolziError::WeakPassphrase { reason }` — policy failure.
- `HolziError::InstanceAlreadyActive` — another instance is currently active in `AppState`.
- `HolziError::PairingTokenInvalid { reason }` — Join mode only.
- `HolziError::CrdtInit { reason }` — passthrough from `haex-crdt`.

---

### `confirm_create` *(historical — not in v1)*

Finalizes a successful Genesis flow after the operator confirms that the paper-seed was recorded. It is the commit point for the `.pending` marker; the active runtime remains open.

```rust
#[tauri::command]
pub async fn confirm_create(
    app: AppHandle,
    state: State<'_, AppState>,
    args: ConfirmCreateArgs,
) -> Result<InstanceInfo, HolziError>;

pub struct ConfirmCreateArgs {
    pub name: String,
}
```

**Preconditions**: `<name>.db` is the active instance created by the current Genesis flow and its `.pending` marker exists.

**Postconditions**: the marker is removed atomically, the instance remains active, and `instance-list-changed { reason: 'confirmed', affectedName: name }` is emitted. A confirmed instance is never removed by startup orphan cleanup.

**Failure modes**: `NotFound`, `InstanceMismatch`, or `Io`. `InstanceMismatch` means that a different instance is active than the pending Genesis instance named by the command. Failure leaves the marker and database intact so confirmation can be retried.

---

### `abort_create` *(historical — not in v1)*

Cancels a pending Genesis flow. It shuts down the runtime, removes the pending database and marker, clears `AppState.active_instance`, and emits `instance-list-changed { reason: 'aborted' }` only after both files are gone.

```rust
#[tauri::command]
pub async fn abort_create(
    app: AppHandle,
    state: State<'_, AppState>,
    args: AbortCreateArgs,
) -> Result<(), HolziError>;

pub struct AbortCreateArgs {
    pub name: String,
}
```

The command is idempotent for an already-cleaned pending flow. It MUST NOT remove a confirmed instance.

---

### `open_instance`

Opens an existing `.db`, validates and unlocks SQLCipher, starts Nostr relay + iroh peer, and marks active in `AppState`. If another instance is active, the command performs the close-and-open switch as one state-locked operation, but validates the requested credentials before closing the current runtime. An import-pending copy is a restore, not a normal open: after credential validation, it MUST rekey `instance_identity` and retire the copied keys before any relay or iroh endpoint starts; the fresh identity becomes active with `restore_pairing_required` set and the federation view hands off to `pair_restored_instance`.

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

**Preconditions**: `<name>.db` exists and is not a pending Genesis database. An imported database with an import-pending marker may be opened; its SQLCipher credential validation is completed by this command. A database with a restore-pending marker is an already-rekeyed restore and may be reopened without rekeying again.

**Postconditions on success**: SQLCipher unlocked; Nostr relay listening; iroh peer online; `AppState.active_instance = Some(...)`; the database mtime is refreshed to the current time as the persisted `lastAccess`; and any ordinary import-pending marker is removed. For an imported copy, rekey-on-restore has completed before startup, the copied keys were never used to start a network endpoint, `restore_pairing_required = true`, and the restore marker remains until pairing succeeds. If another instance was active, it is fully closed before the new one becomes visible. Backend emits `instance-list-changed` (last-access bumped).

**Atomic switch and rollback**: the state lock is held throughout the operation. First, `open_instance` validates the requested file and SQLCipher credentials while the current runtime and `AppState.active_instance` remain unchanged. If that validation fails, no close, state transition, mtime refresh, or active-instance event occurs. For an import-pending copy, the rekey transaction durably records the restore-pairing-required state before any requested runtime service starts; the backend then writes and fsyncs the restore-pending marker before removing the import marker. If interrupted between the database transaction and marker publication, the persisted restore state lets the next open recreate the marker without rekeying. A rekey failure leaves the copy and import marker unchanged. Only after validation and rekey succeed may the command shut down the previous runtime and activate the requested fresh identity. If a later startup step fails, every service started for the requested instance is stopped, its database handle is dropped, and `AppState.active_instance` is cleared. The restore-pending marker and fresh identity remain, so a subsequent `open_instance` recognizes the already-rekeyed restore, skips rekey entirely, and starts only the fresh identity for another pairing attempt. A failed unlock attempt retains the copy and import marker so a subsequent attempt can retry; deletion requires an explicit discard action or conclusive validation that the file is not a holzi instance. The previous instance is not silently resumed after a post-validation startup failure.

**Marker lifecycle**: for `<name>.db`, the sibling `<name>.import-pending` marker means that the copied database has not yet passed credential validation. Successful validation durably records the restore-pairing-required state after fresh identity keys have replaced and retired the copied identity, then publishes `<name>.restore-pending` and removes the import marker. On a later open, `<name>.restore-pending` or the persisted restore-pairing-required state means rekey already completed: the backend MUST skip rekey, MUST never start the copied identity, and MUST return `restore_pairing_required = true` until `pair_restored_instance` succeeds. A successful pairing transaction removes the restore marker and clears the state. Startup and open reject inconsistent marker/database combinations without starting a network endpoint; they retain a valid restore database for retry after interruption.

**Failure modes**:

- `HolziError::NotFound { name }` — no such file.
- `HolziError::WrongPassphrase` — SQLCipher rejected. **The frontend MUST NOT expose whether the error was `NotFound` vs `WrongPassphrase`** (FR-021); it renders both as a generic "Öffnen fehlgeschlagen". The typed error is for logs and telemetry only.
- `HolziError::NotAValidInstance { reason }` — the database or SQLCipher format is invalid. An import-pending copy remains available unless the backend has conclusively established that it is not a holzi instance; a passphrase-related failure must never delete it.
- `HolziError::Io` — read error.

---

### `pair_restored_instance`

Completes pairing for an imported database that `open_instance` has rekeyed and
placed in `restore-pairing-required` state. It updates that active database in
place; it MUST NOT copy a file or call `create_instance`.

```rust
#[tauri::command]
pub async fn pair_restored_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    args: PairRestoredInstanceArgs,
) -> Result<InstanceInfo, HolziError>;

pub struct PairRestoredInstanceArgs {
    pub name: String,
    pub token: String,
}
```

The federation view invokes this command when the operator selects
**Wiederherstellung verbinden** and scans or pastes the parent's pairing token.
The QR and text paths pass the same token string as the normal Verbinden flow.

**Preconditions**: the named instance is the active rekeyed import, its restore
marker is present, and no restore pairing has completed.

**Postconditions on success**: the command verifies the token and transcript,
writes the mutually signed `peer_instances` records into the same database,
removes the import and restore markers, clears `restore_pairing_required`, and
emits `instance-list-changed { reason: 'restore-paired', affectedName: name }`.
No second database is created.

**Rollback on failure**: an expired, consumed, or rejected token rolls back all
peer-record writes and leaves the same database, fresh identity, restore marker,
and active runtime available for retry. No second database is created and the
previous instance is not reopened implicitly.

**Failure modes**: `NotFound`, `InstanceMismatch`, `PairingTokenInvalid`,
`CrdtInit`, or `Io`.

---

### `close_instance`

Closes the currently-active instance: shuts down Nostr relay, disconnects iroh peer, drops the `haex-crdt` handle, clears `AppState.active_instance`. Idempotent — succeeds if nothing is active.

```rust
#[tauri::command]
pub async fn close_instance(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), HolziError>;
```

After a successful close of an active instance, emit `instance-list-changed { reason: 'closed', affectedName: name }`. The event requires the `AppHandle` dependency; an idempotent close with no active instance emits no mutation event.

**Failure modes**:

- `HolziError::CloseFailed { reason }` — a subsystem shutdown returned an error. State is left as clean as possible; caller may retry.

---

### `import_instance_file`

Copies an external `.db` file into `<AppLocalData>/instances/`. Structural validation happens before copying; SQLCipher credential validation is completed by `open_instance`.

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
    pub pending_validation: bool,      // True until rekey and restore pairing complete
}
```

**Postconditions on success**: the destination file and import-pending marker exist, and `instance-list-changed { reason: 'imported', affectedName: info.name }` is emitted after both are in place so the Pinia store can immediately resync.

**Notes**:

- `source_path` is the external file path returned by `@tauri-apps/plugin-dialog`; it is the only path accepted from the frontend. It is not a managed-instance path. The command validates that it is a regular file and that the extension is `.db`.
- SQLCipher page validation is deferred until `open_instance`, where the operator supplies the passphrase. The copied file is marked by a sibling `<name>.import-pending` marker; after rekey, `<name>.restore-pending` remains until `pair_restored_instance` succeeds. Failed unlock or pairing attempts retain the copied file and its current marker for retry; only an explicit discard action or conclusive validation that the file is not a holzi instance may remove them. The source file is never touched.
- Copy uses a crash-safe publish protocol: write the database to a temporary file in the managed directory and fsync it; write and fsync `<name>.import-pending`; fsync the directory; atomically rename the temporary database to `<name>.db`; then fsync the directory again before emitting the import event. The marker is durable before the database is published, so a crash at the rename boundary cannot leave an unmarked imported database that `open_instance` could start normally. A marker without its database is treated as an incomplete import and is cleaned up or retried without network startup. On any pre-publish error the temporary file and marker are deleted.
- For `ConflictPolicy::Overwrite`, the command checks the target name while holding the `AppState` lock and rejects replacement if that name is the active instance. The frontend must close that instance explicitly before retrying overwrite.
- The source file is never modified or moved.

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

**Postconditions on success**: the source database is absent from the managed root, the trashed copy exists under `.trash/`, and `instance-list-changed { reason: 'trashed', affectedName: name }` is emitted only after the move completes.

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

    #[error("An instance is already active")]
    InstanceAlreadyActive,

    #[error("Instance '{name}' is not the pending active creation")]
    InstanceMismatch { name: String },

    #[error("Instance '{name}' is active; close it before this operation")]
    InstanceActive { name: String },

    #[error("File is not a valid holzi instance: {reason}")]
    NotAValidInstance { reason: String },

    #[error("Pairing token invalid: {reason}")]
    PairingTokenInvalid { reason: String },

    #[error("Failed to close active instance: {reason}")]
    CloseFailed { reason: String },

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
