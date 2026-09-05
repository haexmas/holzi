# Contract: Tauri Commands (Rust ↔ Frontend)

**Status**: Draft, spec-phase working proposals. Field names, error variants, and argument shapes MUST be reviewed once `haex-crdt`'s own initialization API is finalized. Nothing here is a settled interface.

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
    Recover { seed: String, expected_fingerprint: String },
    Join { token: String },
}

pub struct CreateInstanceResult {
    pub info: InstanceInfo,
    pub paper_seed: Option<String>,       // Some for Genesis, None otherwise
    pub root_fingerprint: Option<String>, // Some for Recover (matched), None otherwise
    pub requires_confirmation: bool,      // True for Genesis until confirm_create
}
```

**Frontend call**:

```ts
const result = await invoke<CreateInstanceResult>('create_instance', {
  args: {
    name: 'laptop-home',
    passphrase: '…',
    mode: { type: 'Genesis' },  // or { type: 'Recover', seed, expectedFingerprint }
  },
})
```

**Preconditions**: `name` matches `^[A-Za-z0-9][A-Za-z0-9_\-]{0,63}$`; passphrase meets min-length policy; no active instance in `AppState`.

**Postconditions on success**: `<name>.db` exists, is unlocked, is bound as the active instance in `AppState`, and the Genesis `.pending` marker remains until `confirm_create`. The Nostr relay and iroh peer are already running before the result is returned. Backend emits `instance-list-changed`.

**Postconditions on failure**: no partial file on disk. If a file was created before failure, backend deletes it. A `<name>.db.pending` marker (empty file next to the `.db` during Genesis) is removed with its sibling on startup if orphaned.

**Failure modes**:

- `HolziError::NameConflict { name }` — a `.db` with that name already exists.
- `HolziError::InvalidName { reason }` — regex mismatch.
- `HolziError::WeakPassphrase { reason }` — policy failure.
- `HolziError::InstanceAlreadyActive` — another instance is currently active in `AppState`.
- `HolziError::PairingTokenInvalid { reason }` — Join mode only.
- `HolziError::FingerprintMismatch` — Recover mode only.
- `HolziError::CrdtInit { reason }` — passthrough from `haex-crdt`.

---

### `confirm_create`

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

### `abort_create`

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

Opens an existing `.db`, validates and unlocks SQLCipher, starts Nostr relay + iroh peer, and marks active in `AppState`. If another instance is active, the command performs the close-and-open switch as one state-locked operation, but validates the requested credentials before closing the current runtime.

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

**Preconditions**: `<name>.db` exists and is not a pending Genesis database. An imported database with an import-pending marker may be opened; its SQLCipher credential validation is completed by this command.

**Postconditions on success**: SQLCipher unlocked; Nostr relay listening; iroh peer online; `AppState.active_instance = Some(...)`; the database mtime is refreshed to the current time as the persisted `lastAccess`; and any import-pending marker is removed. If another instance was active, it is fully closed before the new one becomes visible. Backend emits `instance-list-changed` (last-access bumped).

**Atomic switch and rollback**: the state lock is held throughout the operation. First, `open_instance` validates the requested file and SQLCipher credentials while the current runtime and `AppState.active_instance` remain unchanged. If that validation fails, no close, state transition, mtime refresh, or active-instance event occurs. Only after validation succeeds may the command shut down the previous runtime and activate the requested runtime. If a later startup step fails, every service started for the requested instance is stopped, its database handle is dropped, and `AppState.active_instance` is cleared. For an import-pending copy, a failed unlock attempt—including an incorrect passphrase—retains the copy and marker so a subsequent `open_instance` attempt can retry; deletion requires an explicit discard action or conclusive validation that the file is not a holzi instance. The previous instance is not silently resumed after a post-validation startup failure.

**Failure modes**:

- `HolziError::NotFound { name }` — no such file.
- `HolziError::WrongPassphrase` — SQLCipher rejected. **The frontend MUST NOT expose whether the error was `NotFound` vs `WrongPassphrase`** (FR-021); it renders both as a generic "Öffnen fehlgeschlagen". The typed error is for logs and telemetry only.
- `HolziError::NotAValidInstance { reason }` — the database or SQLCipher format is invalid. An import-pending copy remains available unless the backend has conclusively established that it is not a holzi instance; a passphrase-related failure must never delete it.
- `HolziError::Io` — read error.

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
    pub pending_validation: bool,      // True until open_instance validates the passphrase
}
```

**Postconditions on success**: the destination file and import-pending marker exist, and `instance-list-changed { reason: 'imported', affectedName: info.name }` is emitted after both are in place so the Pinia store can immediately resync.

**Notes**:

- `source_path` is the external file path returned by `@tauri-apps/plugin-dialog`; it is the only path accepted from the frontend. It is not a managed-instance path. The command validates that it is a regular file and that the extension is `.db`.
- SQLCipher page validation is deferred until `open_instance`, where the operator supplies the passphrase. The copied file is marked by a sibling `<name>.import-pending` marker. Failed unlock attempts retain both the copied file and marker for retry; only an explicit discard action or conclusive validation that the file is not a holzi instance may remove them. The source file is never touched.
- Copy is atomic (write to temp path, `rename` into place) and creates the import-pending marker as part of the same backend-owned operation. On any error the temp file and marker are deleted.
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

    #[error("Recovered fingerprint does not match expected")]
    FingerprintMismatch,

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
