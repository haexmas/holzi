# Contract: Tauri Commands (Rust ↔ Frontend)

**Status**: Working draft against `haex-crdt` Cargo version 0.2.0 at
repository revision `c41ef2e2695da980af56aa586211ec508bf1789b` (not tagged).
Field names and shapes are stable enough to implement against; the
error-mapping table below reflects that pinned crate surface. Any haex-crdt
change to migration IDs, provider trait signatures, or the `Database::open`
shape reopens this contract.

**V1 contract note**: Genesis creates a fresh per-SQLite identity without a
paper-seed or confirmation step. Backup restore uses offline adoption in
`open_instance`; `pair_restored_instance` is the token fallback when no
valid handover attestation is available, including later invalidation. Both update the imported database in place.

**Convention**: All commands are async, return `Result<T, HolziError>` on the Rust side, and are exposed to the frontend via `@tauri-apps/api/core::invoke<T>(name, args)`. Argument keys are `camelCase` from the frontend, mapped to `snake_case` Rust field names by serde.

## Data layer and providers

`haex-crdt` exposes a `Database` handle and requires three provider
implementations from holzi plus a `trigger_version` configuration value. These
providers, the holzi-owned migrations, and the error-mapping layer are the
seams between holzi and the crate. This section documents them normatively;
command sections reference these seams without re-explaining them.

### Provider implementations

The three traits are implemented by holzi in a dedicated
`src-tauri/identity/` module. Extraction into a standalone `haex-identity`
crate is deferred until the trait surface has stabilized against a working
slice.

- **`DeviceIdProvider`** — returns the current instance's device UUID.
  `Database::open` requires this provider before it opens the database, so the
  provider cannot discover a fresh UUID from `haex_crdt_configs_no_sync` during
  that same open. Holzi MUST therefore generate and retain the UUID before
  constructing `DatabaseConfig`; the post-open `instance_identity_no_sync`
  row mirrors the value for application use (see
  [Device-ID model](#device-id-model)).
- **`SignatureProvider`** — signs the canonical column preimage supplied to
  `sign_column`, using the instance keypair from
  `instance_identity_no_sync`. It MUST NOT sign with the
  federation-attestation keypair; that key is reserved for signing device
  attestations at bootstrap.
- **`MigrationSource`** — enumerates the holzi-owned migrations listed in
  [holzi-owned data layer](#holzi-owned-data-layer). haex-crdt's own
  bookkeeping migrations (the `_no_sync` tables and `_no_trigger` metadata
  columns) are separate and shipped by the crate itself.

Holzi also passes **`trigger_version: i32`** in `DatabaseConfig`. This is
haex-crdt's own CRDT trigger-schema version — bumping it triggers an in-place
rewrite via `ensure_triggers_initialized` on the next open. Holzi passes
`haex_crdt::DEFAULT_TRIGGER_VERSION` verbatim; it MUST NOT be bumped for
holzi-side schema changes (those are `MigrationSource` migrations), only in
lockstep with a crate-required upgrade.

### holzi-owned data layer

Tables holzi ships via its `MigrationSource`. These sit next to (not inside)
haex-crdt's `_no_sync` bookkeeping tables. Tables containing private or
installation-local state MUST use the `_no_sync` suffix and MUST NOT be passed
to `install_crdt`; all other tables listed here are installed as CRDT-tracked
tables via `install_crdt(table, opts)`.

- **`instance_identity_no_sync`** (singleton row) — the per-database
  instance's private Nostr+iroh keypair, its device UUID, and the private
  federation-attestation keypair (secp256k1). Private key material and
  restore-pairing state (`restore_pairing_required`) MUST remain local and
  MUST never enter the CRDT or sync transport. The attestation public data
  needed by a peer is written to `peer_instances`; the private key is never
  wrapped for delivery.
- **`peer_instances`** — the attested-device registry. One row per peer with
  `alias`, `nostr_pubkey`, `iroh_node_id`, current attestation epoch,
  capabilities (including `pairing-authority`, `confirmation-authority`,
  `sharing-authority`), and `valid_until`. Mutually signed on both sides
  during pairing.
- **`revocation_epochs`** — monotonic epoch counter per device. Revocation
  bumps the epoch atomically; in-flight iroh transfers against a stale epoch
  are cancelled by the accept handler on the next chunk.
- **`capability_grants`** — cross-device write-action grants (e.g., which
  device holds `confirmation-authority` at time T). Referenced by relay-side
  intent-hold logic.
- **`attested_devices`** — historical attestation records for audit and
  rollback; write-once, no updates.

### Device-ID model

Two UUIDs, layered:

1. **Installation UUID** — one per holzi installation on a host, stored in
   userspace outside any vault. Never sent over the wire.
2. **Device UUID** — a fresh random v4 UUID generated for each new database,
   retained by Holzi before `Database::open`, then recorded by haex-crdt in
   `haex_crdt_configs_no_sync` and mirrored in `instance_identity_no_sync`.
   Two vaults on the same host are not linkable via this ID.

**Holzi-owned mapping for ordinary reopen**: Holzi stores a non-secret mapping
from instance name to device UUID in an atomically replaced JSON index at
`<AppLocalData>/instance-index.json`. Creation writes a `pending` mapping and
fsyncs the index before constructing `DatabaseConfig`; after the database and
pending marker commit, the mapping becomes `ready`. `open_instance` reads the
`ready` mapping before calling `Database::open` and passes that UUID through a
stable `DeviceIdProvider`; it MUST NOT mint a new UUID during ordinary reopen.
Startup removes `pending` mappings whose creation marker or database is absent.
If a mapping is missing, corrupt, or not `ready`, `open_instance` returns an
explicit validation error and leaves the database and any currently active
instance untouched; it MUST NOT guess a UUID or overwrite the database's
recorded value.

**Ordinary reopen** — a database whose UUID this index already knows — passes
that UUID straight through and MUST NOT mint a new one.

**Adopting a copied database is a distinct, intended flow.** Importing a `.db`
creates a new replica, whether the source is on this machine or another one
and whether its UUID is already in the local index. The source remains
untouched and may still run, so its UUID MUST NOT be reused for the copy.
Only ordinary reopen of the original managed database retains its UUID.
Copy adoption and token/QR pairing must end in the same authorized state,
so adoption is not merely "accept the mismatch" — it MUST also:

1. Prepare a **new device UUID** (the CRDT node identity must be
   unique per replica, or conflict resolution stops being deterministic).
2. Prepare a **new signing keypair**, retiring the copied one only in the
   attestation/completion transaction below. Without this,
   two replicas would sign with the same key and writes could not be
   attributed.
3. Prepare **new Nostr and iroh endpoint keys**, which must be unique per
   device.

At the pinned revision, `initialize_in_place` runs before device-id
reconciliation and seeds the HLC from the last persisted timestamp. That
supports a future adoption implementation; it does not itself implement
adoption or prove crash safety. The adopting revision MUST verify that the
first new write advances past the copied timestamp and uses the new node ID,
and that UUID, keys, local index and restore markers recover consistently
after interruption before any endpoint or application write starts.
Sources at the pinned revision: [open lifecycle](https://github.com/haexmas/haex-crdt/blob/c41ef2e2695da980af56aa586211ec508bf1789b/src/database/mod.rs#L129)
and [HLC initialization](https://github.com/haexmas/haex-crdt/blob/c41ef2e2695da980af56aa586211ec508bf1789b/src/crdt/hlc.rs#L140).

**What the pinned revision actually does**: `reconcile_device_id` rejects a
mismatch outright; there is no adopt API. Until a supporting revision is
reviewed and pinned, `open_instance` MUST reject every unprocessed
import-pending copy before rekey, application writes or network startup,
including copies with a known source UUID. It returns
`HolziError::CrdtInit { reason: "copied database adoption unavailable in pinned haex-crdt" }`
and retains the copy, marker, index and currently active instance unchanged.
This capability error MUST NOT be treated as conclusive file invalidity.
`DeviceIdMismatch { expected, supplied }` remains reserved for an actual
crate-reported mismatch; a missing index entry supplies neither a verified
expected UUID nor a permitted replacement UUID.

This dependency limitation applies only to copy adoption. Token/QR join
uses `create_instance` to create an empty database with a fresh UUID and
does not require adoption.

**How an adopted replica is authorized, without a token.** Operator decision
of 2026-09-08: adoption with a valid handover attestation MUST complete locally
offline, with no reachable parent. A missing or unusable signing key, or a
source that cannot authorize the grant under the existing peer-registry rules,
uses the token fallback. Neither path may start the copied identity.

The adoption transaction MUST follow this order:

1. Prepare the fresh device UUID, CRDT signing keypair, Nostr keypair and iroh
   keypair without activating them. Keep the copied keys available until commit.
2. Sign a versioned, domain-separated canonical handover payload using the
   **copied Nostr instance signing key** corresponding to the source
   `nostr_pubkey` already trusted in `peer_instances`. A CRDT column signature
   or an unregistered attestation key is not a substitute for this authorization.
   The payload MUST bind the source device UUID and `nostr_pubkey`, its grant
   and revocation epoch, the adopted device UUID, CRDT signing public key,
   `nostr_pubkey`, `iroh_node_id`, alias, capabilities, `valid_until`, grant
   epoch, a unique attestation ID and adoption timestamp. Private keys and
   local import-operation evidence MUST NOT enter this payload.
3. In the recoverable adoption transaction, write the complete signed payload
   into the adopted `peer_instances` entry **before** replacing the singleton
   local identity and retiring all copied private keys. Commit the attestation,
   exact prepared private keys, fresh UUID and local completion record together.
   There MUST be no durable state with retired copied keys but missing
   attestation or missing corresponding new private keys. A pre-commit crash
   rolls back the entire transaction; a post-commit retry reuses its identity
   and attestation. Publish the target index and markers as specified by
   `open_instance` before application writes or endpoint startup.

If no valid attestation can be produced, the same atomic identity/completion
transition instead records the explicit token-fallback outcome and
`restore_pairing_required = true`; it does not publish an authorized peer grant.
A signing-key/authorization limitation permits this fallback; a failed database
write, fsync or incomplete transaction does not. Storage failures retain the
import for retry and MUST NOT retire keys without a committed outcome.

**Receiver validation and first contact.** Before granting ordinary Nostr,
iroh or CRDT access, a receiver MUST validate the complete handover payload
against its own effective trust store, not against self-asserted trust in the
incoming row. The source grant must be current, unexpired and unrevoked and
carry `pairing-authority`; the granted capabilities must be a subset of the
source's currently held capabilities and the expiry must not outlive the
source grant. Both source and adopted grant epochs must pass the existing
monotonic revocation rules; stale grants, replay of a revoked identity and
unauthorized epoch advances are rejected. An adoption timestamp is informational
and cannot prove that a grant preceded revocation. Validation of a derived
source must reach an already trusted authorization anchor; unknown or cyclic
self-assertions cannot bootstrap trust.

The first-contact handshake carries the attestation before ordinary sync and
proves possession of the attested new endpoint keys; the CRDT verification key
is taken only from that validated binding. No full sync or application access
is allowed merely to obtain the peer row. Identical valid attestations are
idempotent; changing any signed field invalidates them. Subsequent ingress
rechecks effective trust, including the source authorization on which a derived
grant depends, so revocation is not bypassed by a previously accepted record.
The receiver MUST show a newly accepted derived replica in the UI.

Offline completion uses the copy's locally available trust snapshot; it cannot
guarantee acceptance by a peer that already knows a later revocation or expiry.
Such a peer rejects access and the UI reports the authorization failure without
rolling back adoption or reusing copied keys. Token reauthorization of that
same fresh identity uses the existing restore-pairing fallback, explicitly
recorded locally before `pair_restored_instance` is offered. Merely being offline
or unreachable MUST NOT trigger fallback. The supporting dependency/protocol
review must fix the canonical wire encoding, version/domain and first-contact
proof with positive and tamper/replay test vectors before adoption is enabled.

**Trust and revocation.** A readable database copy already exposes its stored
history and credentials; attestation additionally grants ongoing membership
and access to future synchronized data within the granted capabilities. Changing
the SQLCipher passphrase does not revoke that membership. Only a currently
authorized `pairing-authority` peer may publish a revocation under the existing
monotonic epoch rules. Any peer's UI MUST surface newly accepted derived
replicas; observing an unexpected one does not itself grant revocation authority.
Revoking only one child does not remove the copied source key's ability to
request more grants: compromise response must also revoke the source and its
dependent grants, or use the existing federation-wide reset. Token fallback
depends on inability to produce a valid attestation, regardless of whether the
source device still exists or is reachable.

**Never permitted**, independent of the above: the same `.db` open in two
processes at once. `fs2` prevents it on one host. Across a shared network
mount or a cloud-sync folder it cannot be prevented reliably, because such
services replicate byte ranges rather than transactions — that case is a
documented anti-requirement, and startup SHOULD warn when an instance path
looks like a known sync folder.

### Error mapping

The direct `haex-crdt::Error` variants map to `HolziError` as follows. Errors
returned through haex-crdt's internal `DatabaseError` conversion are surfaced
as `CrdtInit { reason }` until the crate exposes a typed top-level variant.

| haex-crdt variant | HolziError variant | Notes |
| --- | --- | --- |
| `Sqlite(error)` | `CrdtSqlite { reason: error.to_string() }` | SQL/SQLCipher failure |
| `Io(error)` | `CrdtIo { reason: error.to_string() }` | I/O failure |
| `Hlc(reason)` | `CrdtHlc { reason }` | HLC/provider failure |
| `DeviceIdMismatch { expected, supplied }` | `DeviceIdMismatch { expected, supplied }` | surfaced to UI |
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

**Internal choreography** (holzi-orchestrated, not a single haex-crdt call):

1. Validate `name` and passphrase policy in-process (haex-crdt accepts opaque strings; policy is holzi's job).
2. Generate a fresh v4 device UUID for this database (see [Device-ID model](#device-id-model)); do not derive it from the installation UUID. Atomically persist a `pending` name-to-UUID entry in `<AppLocalData>/instance-index.json` and fsync the index before opening the database.
3. Write `<name>.db.pending` as an empty marker, then create `<name>.db` as the candidate path. Both are inside `<AppLocalData>/instances/`; the marker and pending index entry, not a database-file rename, control publication.
4. Call `Database::open(DatabaseConfig { path, key: SqlCipherKey::new(passphrase), create_if_missing: true, device_id: Arc::new(...), signature_provider: Arc::new(...), migration_source: Arc::new(...), trigger_version: HOLZI_TRIGGER_VERSION })`. `device_id` is a `DeviceIdProvider`, not a UUID field. This opens the SQLCipher file, runs haex-crdt's own `_no_sync` bookkeeping migrations, and runs the holzi migrations from [holzi-owned data layer](#holzi-owned-data-layer) transactionally.
5. Generate the per-instance Nostr and iroh keypairs plus the federation-attestation secp256k1 keypair; write them (and the device UUID) into the `instance_identity_no_sync` singleton row.
6. For `CreateMode::Genesis`: write a Genesis self-record into `peer_instances` with the full local capability set (`pairing-authority`, `confirmation-authority`, `sharing-authority`) and epoch 0.
7. For `CreateMode::Join`: validate the token, perform the pairing handshake against the parent (see `spec 002` for the parent-side surface), and write the mutually-signed `peer_instances` rows returned by the transcript.
8. Start the Nostr relay and iroh peer bound to the new instance identity.
9. Bind `AppState.active_instance = Some(info)`.
10. Flush the database and directory, remove `<name>.db.pending`, atomically promote the index entry from `pending` to `ready`, and flush the index and directory again. Marker removal plus the ready mapping is the durable commit point for the whole flow; no database-file rename is used.
11. Emit `instance-list-changed`.

**Postconditions on success**: `<name>.db` exists, is unlocked, is bound as the active instance in `AppState`, and has fresh per-instance identity keys plus a Genesis self-record (for Genesis) or mutually signed peer records (for Join). The Nostr relay and iroh peer are already running before the result is returned. The `.pending` marker is gone.

**Postconditions on failure**: no partial file remains that startup would treat as a real instance. For `CreateMode::Join`, any parent-side `peer_instances` publication is staged until the local commit or undone with a compensating transaction before cleanup. On any failure before step 10, Holzi stops every relay and iroh service started for the candidate, clears `AppState.active_instance`, drops every `Arc<Database>` clone it owns, verifies the database lock is released, removes the candidate `.db`, marker, and pending index entry, and only then returns the error. If a remote peer publication cannot be synchronously undone, the join is recorded as failed and a compensating revocation is published before the local files are removed. If the process crashes between steps 3 and 10, startup orphan-cleanup finds the `.pending` marker, removes it together with its sibling `.db`, and removes the matching pending index entry.

**Failure modes**:

- `HolziError::NameConflict { name }` — a `.db` with that name already exists.
- `HolziError::InvalidName { reason }` — regex mismatch.
- `HolziError::WeakPassphrase { reason }` — policy failure.
- `HolziError::InstanceAlreadyActive` — another instance is currently active in `AppState`.
- `HolziError::PairingTokenInvalid { reason }` — Join mode only.
- `HolziError::DeviceIdMismatch { .. }` / `HolziError::MigrationMissingFromSource { .. }` / `HolziError::MigrationContentDrift { .. }` / `HolziError::MigrationCompatibility { reason }` / `HolziError::CrdtSqlite { reason }` / `HolziError::CrdtIo { reason }` / `HolziError::CrdtHlc { reason }` / `HolziError::CrdtSignatureVerificationFailed { .. }` (Join only, on remote-change apply) / `HolziError::CrdtUnexpectedSignatureUnderNoop` (Join only) / `HolziError::CrdtAlreadyInstalled { table }` / `HolziError::CrdtInit { reason }` — see [Error mapping](#error-mapping). All are catastrophic during `create_instance` (fresh DB); the backend rolls back per postconditions above.

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

Opens an existing `.db`, validates and unlocks SQLCipher, starts Nostr relay + iroh peer, and marks active in `AppState`. If another instance is active, the command performs the close-and-open switch as one state-locked operation, but validates the requested credentials before closing the current runtime. An import-pending copy follows the adoption transaction in [Device-ID model](#device-id-model) before any relay or iroh endpoint starts. With a valid handover attestation it opens the normal federation view offline with `restore_pairing_required = false`. Only the token fallback sets that flag and offers `pair_restored_instance`.

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

**Preconditions**: `<name>.db` exists and is not a pending Genesis database. An imported database with an import-pending marker may be opened; its SQLCipher credential validation is completed by this command. A database with a restore-pending marker may be reopened without rekeying only after verifying the matching local completion proof below.

**Pinned-revision gate**: the import-pending success path below is conditional
on the adoption capability in [Device-ID model](#device-id-model). At the
current pin, reject unprocessed imports with the capability error defined
there. A known source UUID does not bypass this gate. After the dependency
upgrade, adopt a fresh UUID and rotate all signing/endpoint keys before
startup; persist the attested or token-fallback outcome and target name-to-new-UUID mapping
without modifying the source mapping. Only restore completion proven for this local import generation remains
authoritative for retries, so neither adoption nor rekey repeats.

**Local import-generation proof**: every staging operation allocates a fresh
random operation ID in the durable local import marker/journal, including
Rename imports. Source-embedded restore state is not proof of local completion.
The adoption/rekey transition records this operation ID, the new UUID and
the attested or token-fallback outcome in an encrypted completion record,
independent of `restore_pairing_required`. The target index records the same
operation ID and UUID; a restore marker records them only for the fallback.
Resume without adoption/rekey only when the database's
completion record matches the current locally recorded operation and its new
UUID. If interrupted before index/restore-marker publication, the matching
local import journal/marker plus committed completion record permit finishing
that publication before startup. A mismatching pre-existing restore flag or
operation ID belongs to the source: this is still an unprocessed import and
must pass the adoption gate. Missing or contradictory local evidence fails
closed while retaining the copy. All restore-state precedence rules below
are subject to this generation check; a copied flag alone never overrides a
fresh import marker.

**Postconditions on success**: SQLCipher unlocked; Nostr relay and iroh peer started under the existing identity for an ordinary reopen or the fresh identity for an adopted copy, without requiring network reachability; `AppState.active_instance = Some(...)`; the database mtime is refreshed as the persisted `lastAccess`; and the import-pending marker is durably removed. For an attested copy, `restore_pairing_required = false` and no restore marker remains. For the fallback, `restore_pairing_required = true` and the restore marker remains until pairing succeeds. If another instance was active, it is fully closed before the new one becomes visible. Backend emits `instance-list-changed` (last-access bumped).

**Atomic switch and rollback**: the state lock is held throughout the operation. First, validate the requested file and SQLCipher credentials while the current runtime and `AppState.active_instance` remain unchanged. Validation failure causes no close, state transition, mtime refresh or active-instance event. For an import-pending copy, commit the adoption transaction and its completion outcome before candidate startup. Publish and fsync the target index and, only for the fallback, the restore marker; then remove the import marker and fsync the directory. A matching completion record allows an interrupted publication to finish without repeating adoption, even when `restore_pairing_required = false`. A transaction failure retains the copied identity and import marker for retry; a storage failure MUST NOT be converted into a token fallback.

After validation and any required adoption succeed, start the requested SQLCipher, relay and iroh runtime as a private candidate while the current runtime remains active. Only after every requested startup step succeeds may the backend stop the previous runtime and publish the candidate. On candidate startup failure, stop every candidate service and drop its handle; retain the previous runtime and active state without a half-switched event or mtime update. Keep the committed fresh identity, completion record, target mapping and any fallback marker. Retry preserves the same UUID, keys, attestation and outcome; it neither rotates again nor asks an attested replica to pair. A failed unlock retains the copy and markers; deletion requires an explicit discard action or conclusive validation that the file is not a holzi instance.

**Marker lifecycle**: `<name>.import-pending` identifies a local import whose adoption or publication is unfinished. All recovery decisions require the local import-generation proof above; marker presence alone is insufficient.

- **Attested completion**: durably record the attestation, fresh identity and completion with `restore_pairing_required = false`; publish the target index before removing the import marker. No `<name>.restore-pending` marker is needed. If publication is interrupted, finish it from the matching completion record without rekeying or entering token pairing.
- **Token fallback**: durably record the fresh identity and completion with `restore_pairing_required = true`; publish and fsync the target index and `<name>.restore-pending` before removing the import marker. Matching completion evidence wins over a stale import marker. Reopen preserves the identity and pairing flag until `pair_restored_instance` commits.
- **Later invalidation**: after verifying newer trust evidence that invalidates an attested grant, the backend holds the state lock, atomically changes the existing completion outcome and pairing flag to token fallback without changing UUID or keys, then publishes and fsyncs the matching restore marker. Emit `instance-list-changed` with reason `restore-pairing-required` only after durable publication. A crash before marker publication is recovered from the matching database completion and target-index proof. Unauthenticated rejection messages or reachability failures cannot cause this transition.
- **Fallback pairing completion**: atomically write the peer records and clear the pairing flag, retaining the completion record with its successful outcome; then durably remove the restore marker. After interruption, a matching completed transaction permits stale-marker cleanup without pairing again.

Startup and open reject contradictory or unverifiable marker/database combinations without starting an endpoint and retain the copy for recovery. Source-embedded completion or restore state never substitutes for proof of the current local import generation.

**Failure modes**:

- `HolziError::NotFound { name }` — no such file.
- `HolziError::WrongPassphrase` — SQLCipher rejected. **The frontend MUST NOT expose whether the error was `NotFound` vs `WrongPassphrase`** (FR-021); it renders both as a generic "Öffnen fehlgeschlagen". The typed error is for logs and telemetry only.
- `HolziError::NotAValidInstance { reason }` — the database or SQLCipher format is invalid. An import-pending copy remains available unless the backend has conclusively established that it is not a holzi instance; a passphrase-related failure must never delete it.
- `HolziError::Io` — read error.
- `HolziError::CrdtInit` — copied-database adoption is unavailable at the pinned revision; retain the imported copy and marker for retry after a dependency upgrade or explicit discard.

---

### `pair_restored_instance`

Completes the token fallback for an imported database that
`open_instance` has rekeyed, with `restore-pairing-required` set during adoption
or after verified later invalidation of the attestation.
It updates that active database in
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
marker and matching completion proof are present, and `restore_pairing_required` is true. This also permits reauthorization after an attested replica was rejected against newer trust state, without rekeying or creating another database.

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
`Io`, or any of the haex-crdt-mapped variants listed in [Error mapping](#error-mapping).

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

Copies an external `.db` file into `<AppLocalData>/instances/`. Structural validation happens before copying; SQLCipher credential validation is completed by `open_instance`.

This command may stage a structurally valid source as import-pending,
regardless of whether the local index knows its UUID. Against the pinned
haex-crdt revision, `open_instance` MUST reject every unprocessed imported
copy with the adoption capability error in [Device-ID model](#device-id-model).
It MUST NOT reuse the source UUID or silently supply a replacement to
`Database::open`. Successful staging does not imply a usable replica.

That restriction tracks the **dependency**, not the intended scope: adopting a
copied database is a supported way to add a replica once haex-crdt exposes an
adopt API, and it then follows the three-step adoption in
[Device-ID model](#device-id-model) — new device UUID, new signing keypair, new
endpoint keys. Nothing here may be read as limiting holzi to one machine.

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
    pub pending_validation: bool,      // True until adoption and attestation or fallback pairing complete
}
```

**Postconditions on success**: the destination file and import-pending marker exist, and `instance-list-changed { reason: 'imported', affectedName: info.name }` is emitted after both are in place so the Pinia store can immediately resync.

**Notes**:

- `source_path` is the external file path returned by `@tauri-apps/plugin-dialog`; it is the only path accepted from the frontend. It is not a managed-instance path. The command validates that it is a regular file and that the extension is `.db`.
- SQLCipher page validation is deferred until `open_instance`, where the operator supplies the passphrase. The copied file starts with a sibling `<name>.import-pending` marker. Attested adoption completes offline and removes it after durable index publication; only the fallback publishes `<name>.restore-pending` until `pair_restored_instance` succeeds. See the `open_instance` marker lifecycle for crash recovery. Failed unlock or pairing attempts retain the copied file and its current marker for retry; only an explicit discard action or conclusive validation that the file is not a holzi instance may remove them. The source file is never touched.
- Copy uses a crash-safe publish protocol: for Rename, write the database to a temporary file in the managed directory and fsync it; write and fsync `<name>.import-pending` with a fresh operation ID and staged-file digest; fsync the directory; atomically rename the temporary database to `<name>.db`; then fsync the directory again before emitting the import event. For Overwrite, the backend instead allocates an operation id, writes the staged database to `<name>.db.importing.<operation-id>`, writes a generation-bound `<name>.import-pending.<operation-id>` marker containing the staged-file digest, and fsyncs an overwrite journal before touching the existing `<name>.db`. The recoverable commit renames the old database to an operation-specific backup, renames the staged generation into `<name>.db`, fsyncs the directory, records the committed phase, and durably publishes the ordinary `<name>.import-pending` marker carrying the committed operation ID before removing the backup, operation-specific marker, and journal. Startup/open never applies a generation-specific marker to an old target: with an unfinished journal it verifies the digest and either completes the staged generation or restores/retains the old target, without network startup. A marker without its matching database is treated as an incomplete import and is cleaned up or retried without network startup. On any pre-publish error the temporary file and marker are deleted.
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

    #[error("An instance is already active in this process")]
    InstanceAlreadyActive,

    #[error("Vault file is locked by another process on this host")]
    VaultAlreadyOpenElsewhere,

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
