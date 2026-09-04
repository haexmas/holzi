# Contract: Tauri Commands (Rust ↔ Frontend)

**Status**: Draft, spec-phase working proposals. Field names, error variants, and argument shapes MUST be reviewed once `haex-crdt`'s own initialization API is finalized. Nothing here is a settled interface.

**Convention**: All commands are async, return `Result<T, HolziError>` on the Rust side, and are exposed to the frontend via `@tauri-apps/api/core::invoke<T>(name, args)`. Argument keys are `camelCase` from the frontend, mapped to `snake_case` Rust field names by serde.

## Commands

### `list_instances`

Scans `<AppLocalData>/instances/` for `.db` files (excluding the `.trash/` subdirectory) and returns metadata.

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

**Postconditions on success**: `<name>.db` exists, is unlocked, is bound as the active instance in `AppState`. Backend emits `instance-list-changed`.

**Postconditions on failure**: no partial file on disk. If a file was created before failure, backend deletes it. A `.pending` marker (empty file next to the `.db` during Genesis) is removed on startup if orphaned.

**Failure modes**:

- `HolziError::NameConflict { name }` — a `.db` with that name already exists.
- `HolziError::InvalidName { reason }` — regex mismatch.
- `HolziError::WeakPassphrase { reason }` — policy failure.
- `HolziError::InstanceAlreadyActive` — another instance is currently active in `AppState`.
- `HolziError::PairingTokenInvalid { reason }` — Join mode only.
- `HolziError::FingerprintMismatch` — Recover mode only.
- `HolziError::CrdtInit { reason }` — passthrough from `haex-crdt`.

---

### `open_instance`

Opens an existing `.db`, unlocks SQLCipher, starts Nostr relay + iroh peer, marks active in `AppState`.

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

**Preconditions**: `<name>.db` exists; no active instance (call `close_instance` first).

**Postconditions on success**: SQLCipher unlocked; Nostr relay listening; iroh peer online; `AppState.active_instance = Some(...)`. Backend emits `instance-list-changed` (last-access bumped).

**Failure modes**:

- `HolziError::NotFound { name }` — no such file.
- `HolziError::WrongPassphrase` — SQLCipher rejected. **The frontend MUST NOT expose whether the error was `NotFound` vs `WrongPassphrase`** (FR-021); it renders both as a generic "Öffnen fehlgeschlagen". The typed error is for logs and telemetry only.
- `HolziError::InstanceAlreadyActive` — see above.
- `HolziError::Io` — read error.

---

### `close_instance`

Closes the currently-active instance: shuts down Nostr relay, disconnects iroh peer, drops the `haex-crdt` handle, clears `AppState.active_instance`. Idempotent — succeeds if nothing is active.

```rust
#[tauri::command]
pub async fn close_instance(
    state: State<'_, AppState>,
) -> Result<(), HolziError>;
```

**Failure modes**:

- `HolziError::CloseFailed { reason }` — a subsystem shutdown returned an error. State is left as clean as possible; caller may retry.

---

### `import_instance_file`

Copies an external `.db` file into `<AppLocalData>/instances/`. Validates it is a readable SQLCipher database before copying.

```rust
#[tauri::command]
pub async fn import_instance_file(
    app: AppHandle,
    args: ImportInstanceArgs,
) -> Result<ImportInstanceResult, HolziError>;

pub struct ImportInstanceArgs {
    pub source_path: String,        // Absolute path chosen by user via plugin-dialog
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

**Notes**:

- Source path is trusted from the frontend ONLY because it came from `@tauri-apps/plugin-dialog`, which the OS gates. The command still validates the path is a regular file and that the extension is `.db`.
- Copy is atomic (write to temp path, `rename` into place). On any error the temp file is deleted.
- The source file is never modified or moved.

**Failure modes**:

- `HolziError::NotAValidInstance { reason }` — file is not readable, wrong magic, or SQLCipher rejects the header check.
- `HolziError::NameConflict { name }` — only when `on_conflict = Abort`.
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

**Failure modes**:

- `HolziError::NotFound { name }`.
- `HolziError::InstanceActive { name }` — refuses to trash the running instance.
- `HolziError::Io`.

---

### `forget_instance` *(v1 optional)*

Removes an instance from the frontend list without touching the file — a no-op in this design because the list is a directory scan, not a stored preference. Included for API-shape parity with haex-vault; may be removed if not surfaced in v1 UI.

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
