# Sync Architecture — Own Devices, Spaces, Data Shares, Blind Relay

**Status**: Design, written 2026-09-27/28 from an operator brainstorm session. Not yet a spec. The
spec cut in §14 is a proposal; every requirement here has to be re-expressed as numbered spec
requirements before code moves.

**Relationship to existing documents**:

- [`2026-09-07-cross-user-sharing-deferred-design.md`](./2026-09-07-cross-user-sharing-deferred-design.md)
  — **superseded in part**. Kept: dropping MLS (§2), NIP-44 v2 key envelopes (§2), the peer relay /
  buffer relay split (§3), "Nostr never carries CRDT deltas" (§4), the relay-readable signed
  membership projection (§4), the table whitelist as exfiltration guard (§6). Replaced: rows scoped
  into spaces through an M:N row register (§6 item 2), the separate per-person federation identity
  (§7; the vault identity plays that role, §4 below), and the HTTPS object-storage IAM sidecars (§5;
  replaced by §12 below).
- [`2026-09-04-v1-scope-design.md`](./2026-09-04-v1-scope-design.md) — §4 (vault = SQLite =
  identity, per-device Nostr/iroh keys) is the base this document builds on. Two lines there move:
  `blob.offer`/file transfer stops being post-v1 for the own-device case (§7), and §5's "iroh never
  carries state" no longer holds, because the `haex-crdt` sync channel runs over iroh (§6). The
  addendum at the end of v1-scope already named this tension.
- Reference implementations (read for lessons, not ported):
  - `haex-vault`: repository `https://github.com/haex-space/haex-vault`, revision
    `8dce379d94e18fcd42c3b73686a06f984ca3f574`.
  - `haex-sync-server`: repository `https://github.com/haex-space/haex-sync-server`, revision
    `e20abeb26991d187fe1a4a8f1bc69029e5b12177`.
  - `haex-crdt` as pinned by holzi: repository `https://github.com/haexmas/haex-crdt`, revision
    `ed230d2c3f58c1b10710b6025ea0ce6c20b8d009`.

---

## 1. Goals

1. **Data sync** of the SQLite vault between the devices running the same vault.
2. **File sync** between the devices running the same vault.
3. Both over **iroh**, secured via **Nostr** keys (identity, authentication, key wrapping).
4. A **blind relay**: an untrusted, multi-tenant server through which vaults of many unrelated users
   sync. It never sees plaintext.
5. **Sharing between different users**, reliable and secure:
   - **Files** through **spaces**, which behave like network folders with folder-wide rights.
   - **Data** (SQLite) through **data shares**, with rights granted individually per user.
   - Content on any foreign server is always encrypted and decryptable only by authorized users.

Non-goals for v1: hiding the participant set from the relay (metadata unlinkability), post-compromise
security, forward secrecy against removed members for content they could already decrypt,
re-delegation between people.

## 2. Operator decisions

| #   | Decision                                                                                                                                                                       | Date       |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------- |
| D1  | A Nostr pubkey identifies one **device**. The same vault runs on several devices at once, each with its own Nostr identity; the **vault identity** is the same on all of them. | 2026-09-27 |
| D2  | **Spaces are network folders for files only.** `read` = read all files in the space; `write` = add new files and modify existing ones.                                         | 2026-09-27 |
| D3  | **SQLite data is not shared via spaces.** Read/write on data is granted individually per user.                                                                                 | 2026-09-27 |
| D4  | Data shares exist for **single entries and whole collections**. Extensions declare what is shareable through **one uniform schema** the core can check for every extension.    | 2026-09-27 |
| D5  | An extension may only declare shareables over **its own tables** (prefix `{public_key}__{extension_name}__`).                                                                  | 2026-09-28 |
| D6  | The **creator of a space or share is its only admin**: invites members, assigns, changes and revokes rights, removes members.                                                  | 2026-09-27 |
| D7  | Only the admin invites. A recipient cannot re-share. Downloading content and sharing it again in their own space is out of scope to prevent.                                   | 2026-09-27 |
| D8  | The **vault identity private key lives on every device** of the vault (inside the SQLCipher database). A lost or stolen device is locked out by rotating the vault identity.   | 2026-09-28 |
| D9  | For v1 the relay guarantee is **"the operator sees no content"**. Per-space pseudonyms are not v1.                                                                             | 2026-09-28 |
| D10 | Concurrent edits of the same file produce a **conflict copy**.                                                                                                                 | 2026-09-28 |
| D11 | **The relay is untrusted.** It never holds a user's own S3 credentials.                                                                                                        | 2026-09-28 |
| D12 | **Both storage backends in v1**: storage provided by the public relay (A) and the user's own S3 (B).                                                                           | 2026-09-28 |
| D13 | **No MLS.** Its ordered epoch chain does not fit an order-free CRDT (§3.3).                                                                                                    | 2026-09-27 |

## 3. What exists today, and what the references teach

### 3.1 holzi

- No sync transport: no iroh, Nostr, secp256k1 or MLS dependency in `src-tauri/Cargo.toml` or
  `package.json`.
- `haex-crdt` is used for local tracking only; no `scan_*` or `apply_remote_changes` call exists.
  `NoopSignatureProvider` is configured in `src-tauri/src/instances/vault_config.rs`.
- `src-tauri/src/identity/bootstrap.rs` mints a **placeholder** `vault_identity` keypair (random
  bytes shaped like secp256k1 keys); real key generation was explicitly deferred to the sync slice.
- `vault_identity` has no `_no_sync` suffix, so its private key replicates with the vault. This is
  intended under D8, but it must never leave the vault-internal channel (§6).
- `known_devices` (`src-tauri/src/storage/known_devices.rs`) maps installation UUIDs to vault-device
  UUIDs; the vault-device UUID is the HLC node id.

### 3.2 haex-crdt (pinned revision)

- Column-level LWW on uhlc HLCs, metadata columns inline in each business table; deletes are hard
  deletes plus a synced `haex_deleted_rows` log; `_no_sync` tables and columns are excluded.
- Scanner (`src/crdt/scanner/mod.rs`) and apply (`src/crdt/apply/`) are **transport-agnostic**.
- Hooks this design relies on:
  - `ScanFilters` — `origin_node`, `row_pks` allow-list and `column_eq` for sender-side scoping.
  - `ApplyPolicy::prepare_row` (`src/crdt/apply/policy.rs`) for receiver-side admission.
  - `SignatureProvider` (`src/signature.rs`) with `sign_column`, `verify_column` and
    `on_before_apply`; the `sig` JSON is opaque.
- **Defect this design must fix**: `ColumnChange.device_id` is filled with the _scanning_ store's
  device, not the original writer's. Over more than one hop (A → B → C, or via the relay) authorship
  is mislabeled. The true origin is only in the HLC node suffix, and a node id says nothing about
  which vault authored a change in a shared context (§5.4).

### 3.3 haex-vault: why MLS + CRDT was cumbersome

haex-vault ran one openmls group per space (a leaf per device), exported a per-epoch key, and synced
that key table through the CRDT. The friction is structural, not incidental:

- **Two channels with different consistency models.** MLS needs a total order of commits; CRDT
  membership rows merge order-free. A receiver that has not yet applied an ADD sees the same state as
  "member removed", so peers can accept or reject the same commit differently. The code marks this
  `KNOWN DIVERGENCE RISK` (`src-tauri/src/mls/authorization.rs`).
- Epoch gaps detected by matching error strings, leader-held commit buffers, rejoin loops, stale
  group state that permanently locks a device out, and duplicate signature keys on re-invite.
- openmls has no role model; admin/invite policy had to be layered on with extra signatures, and
  external-commit joiners cannot carry the proof-of-possession.
- A leaf per device made multi-device fragile (removing a member by DID removed only one leaf).
- A missing epoch key stalls the whole pull, because the HLC group cannot be applied partially.

**Lesson**: keep key management inside the CRDT's own consistency model. Key epochs become ordinary
CRDT rows that may exist concurrently (§5.2); wrap to the _vault_, not the device (§5.3).

haex-vault patterns worth keeping: the `{public_key}__{extension_name}__` table prefix
(`src-tauri/src/extension/utils.rs`), the space-table whitelist `SPACE_SCOPED_CRDT_TABLES`
(`src-tauri/src/crdt/scanner.rs`), per-object DEKs with chunked AEAD and opaque object names
(`src-tauri/src/file_sync/crypto/envelope.rs`), and ADR 0002's decision that removed members keep
historical read access
(`docs/adr/0002-shared-space-authenticity-and-confidentiality.md`).

### 3.4 haex-sync-server: shape yes, data model no

The **shape** is right: multi-tenant, stores ciphertext, co-hosts an iroh relay. The **data model**
is not what holzi wants:

- One row per cell with `table_name`, `row_pks`, `column_name`, `hlc_timestamp`, `device_id`,
  author and epoch in cleartext (`src/db/schema.ts`); only the value is encrypted. The server sees
  the app schema, activity per device, and natural-key PKs.
- Server-side LWW merge in `POST /sync/push` (`src/routes/sync.ts`) — the server must understand
  the change format.
- Registration requires an email address; the encrypted key backup is recoverable by email OTP.
- UCAN authorization (`src/middleware/ucanAuth.ts`) has no revocation list: a removed member keeps
  access until the UCAN expires.
- Blob storage (`src/routes/storage.ts`) is one bucket per user, not per space, and the server holds
  full MinIO credentials.

## 4. Identity model

| Key                                   | Scope      | Held by                        | Used for                                                                                                                  |
| ------------------------------------- | ---------- | ------------------------------ | ------------------------------------------------------------------------------------------------------------------------- |
| **Device key** (secp256k1, Nostr)     | one device | that device only (`_no_sync`)  | NIP-42 auth at relays, signing changes and file-index entries, signing the device's iroh NodeId                           |
| **iroh endpoint key** (ed25519)       | one device | that device only               | QUIC transport; bound to the device key by a signed statement                                                             |
| **Vault identity** (secp256k1, Nostr) | one vault  | every device of the vault (D8) | device attestations, admin signatures on member lists, receiving NIP-44 key envelopes, the vault's public npub for grants |

- **Device attestation**: the vault identity signs `{vault_npub, device_npub, iroh_node_id,
issued_at}`. Every verifier (own devices, other users' devices, the relay) resolves "device D acts
  for vault V" through this attestation.
- **Grants name vaults, never devices.** The granter does not learn how many devices the recipient
  has, and the recipient adds devices without the granter acting.
- **Device revocation (D8)**: because every device holds the vault private key, a stolen device can
  mint attestations for itself. Locking one device out therefore means **rotating the vault
  identity**: new keypair, re-attest the remaining devices, re-publish the new npub to every
  counterparty (share and space admins re-wrap keys to it). This is the accepted cost; the SQLCipher
  passphrase is what protects a lost device's database.
- Presence and addressing: each device publishes its current iroh `NodeAddr` as an encrypted Nostr
  event addressed to its own vault identity. This replaces iroh's default pkarr/DNS discovery, so no
  third-party discovery infrastructure is required.

## 5. Common building blocks

All three planes (own devices, spaces, data shares) use the same primitives.

### 5.1 Sealed batch

The unit that travels over iroh and through the relay:

```
SealedBatch {
  scope_id,        // vault id, space id or share id (opaque on the relay)
  key_id,          // which content key encrypted this batch
  ciphertext,      // XChaCha20-Poly1305 over the canonical payload below
}
payload = {
  author_device_npub, author_vault_npub,
  changes: [ColumnChange with per-column sig],   // one or more complete HLC groups
}
```

- Batches never split an HLC transaction group (`paginate_changes` already guarantees this).
- Application-level encryption is used **also on direct iroh links** between own devices. Transport
  encryption would suffice there, but one format means any node — a third own device or the relay —
  can store and forward a batch without seeing plaintext, and there is only one code path.

### 5.2 Key epochs as CRDT rows

```
scope_keys(scope_id, key_id, epoch, created_by_vault, created_at_hlc,
           envelopes: { recipient_vault_npub -> NIP-44 v2 envelope })
```

- A rotation inserts a new row; it does not replace a chain link. Two concurrent rotations produce
  two valid keys — no fork to resolve.
- Every ciphertext names its `key_id`. A receiver decrypts with whatever key it holds; it never needs
  a globally agreed "current" epoch to read.
- A sender encrypts with the deterministically highest key (`epoch`, then `key_id`) it knows.
- Only the scope's admin writes `scope_keys` for spaces and data shares (D6), so concurrent rotations
  can only come from the admin's own devices. For the vault-internal scope the vault key is static
  and rotates only with the vault identity.
- Envelope format: as specified in the deferred design §2 (canonical JSON `{content_key, epoch,
scope_id}` in NIP-44 v2, envelope bound to scope, epoch and recipient).

### 5.3 Wrap to the vault, not the device

Envelopes are addressed to the recipient's **vault identity**. The first device of the recipient
vault that sees the envelope unwraps it and stores the content key in its own vault. Vault-internal
sync (§6) distributes it to that user's other devices. Consequences:

- Multi-device is entirely separated from cross-user key management.
- Rotation costs O(recipient vaults), not O(devices).

### 5.4 Authorship

- Every change carries `author_device_npub` and `author_vault_npub` inside the sealed payload; the
  per-column signature is made with the device key over `(scope_id, table, pks, column, hlc,
author_vault_npub, value)`.
- Receivers verify signature → attestation (device ∈ vault) → capability of that vault in that
  scope. This is the real `SignatureProvider` for shared scopes; `NoopSignatureProvider` remains
  valid only for the vault-internal scope over an authenticated iroh link.
- `haex-crdt` must stop stamping the scanning device as `device_id` for relayed changes (§3.2). The
  upstream fix belongs in `haexmas/haex-crdt`.
- **`created_by`**: every shareable table (data shares) and every file-index entry (spaces) carries
  an immutable `created_by` vault npub, set on insert by the core and never writable afterwards.
  Receivers reject any change to it. It backs the "delete own entries" rule (§11).

### 5.5 Cursors

- Between peers: a **version vector** of the highest HLC seen per origin node, not a single "last
  pushed HLC". Only a vector stays correct when changes arrive over several paths (A → B → C, relay).
  `ScanFilters.origin_node` supports scanning per origin.
- At the relay: a **server-assigned monotonic sequence number** per mailbox (§10.2). Nostr's
  wall-clock `since` is not used.

## 6. Plane 1: own-device data sync over iroh

- **Scope**: the whole vault except `_no_sync` tables. No permission checks between own devices;
  trust comes from the vault identity.
- **Transport**: an iroh ALPN, working name `holzi-sync/1`. Handshake: both sides present their
  device attestation and prove possession of the device key; the peer accepts only attestations
  signed by its own vault identity.
- **Exchange**: swap version vectors, stream the missing sealed batches in both directions, apply
  through `apply_remote_changes`.
- **Discovery**: the encrypted presence event (§4). The relay also runs an **iroh relay** for NAT
  traversal, as haex-sync-server already co-hosts one.
- **Vault identity private key**: replicates only on this plane. The sender-side filter for spaces,
  data shares and relay snapshots MUST exclude `vault_identity` and every other vault-secret table,
  enforced as a structural allow-list rather than a deny-list.
- **Via the relay**: the vault-internal scope also has a relay mailbox (§10), encrypted under the vault
  key, so two own devices that are never online at the same time still converge.

## 7. Own-device file sync

Own-device file sync is a space with exactly one member, the vault itself. It uses the same
machinery as §8, so there is no second implementation. A user can mark folders to sync across
devices; the file index lives in the vault-internal scope.

## 8. Plane 2: spaces (network folders for files)

### 8.1 Model

- A space is a file tree plus a member list: `{vault_npub → capabilities}`, written only by the
  admin (D6).
- **File index**: a small CRDT scoped to the space. One entry per file:

  ```
  space_files(space_id, file_id, path, size, mtime, plaintext_blake3,
              object_id, dek, created_by, modified_by, deleted)
  ```

  The entry is encrypted under the space key like every other batch. The file's **DEK lives in the
  entry**; no sidecars are needed on the storage side.

- **Objects**: file content encrypted in chunks under the per-file DEK (the `HXFE` envelope pattern
  from haex-vault). `object_id` is the hash of the ciphertext, so storage cannot swap content
  undetected. **Objects are immutable**: modifying a file writes a new object and points the entry
  at it; the old object is garbage-collected. Storage never sees an overwrite.
- **Transfer**: objects travel as iroh-blobs (BAO-verified, resumable) between peers, or through a
  storage backend (§12). A device fetches an object from whichever source has it.

### 8.2 Rights

| Capability | Meaning in a space                                                     | Enforced by                                                   |
| ---------- | ---------------------------------------------------------------------- | ------------------------------------------------------------- |
| `read`     | read all files                                                         | cryptography (holds the space key) + storage gate             |
| `write`    | add files, modify any file, delete **own** files (`created_by` = self) | receivers (signature + capability) + storage gate for uploads |
| `delete`   | delete any file                                                        | receivers + storage gate                                      |

Delete semantics are the proposal in §11 and still need operator confirmation.

### 8.3 Conflicts

Two concurrent modifications of the same `file_id` (neither HLC dominates the other's base) produce a
**conflict copy** (D10): the losing version becomes a new entry `name.conflict.<device>.<ts>.ext`
with its own `file_id`. No silent LWW loss of file content.

### 8.4 Membership changes

- **Invite**: the admin adds `{vault_npub → caps}`, wraps the current space key to the invitee, and
  publishes a new signed member list to the relay (§10.3). The invite reaches the invitee as a Nostr
  DM (NIP-17) carrying the space id and relay hints.
- **Change rights / remove**: the admin updates the member list, publishes it with a higher epoch,
  then **rotates the space key** and wraps it to the remaining members.
- **What revocation means**: the storage gate refuses immediately; new files are unreadable to the
  removed member; files it already had keys for stay readable to it. Optional full re-encryption is
  a later feature.

## 9. Plane 3: data shares (SQLite, per user)

### 9.1 Shareable declaration (uniform schema)

Extensions declare what can be shared; the core computes and enforces everything else. The extension
never touches sync, keys or ACLs.

```yaml
# extension manifest
shareable:
  - type: calendar.calendar # a collection
    root: { table: calendars, pk: id }
    label: name # shown in the generic share dialog
    includes:
      - { table: events, fk: calendar_id, as: calendar.event }
  - type: calendar.event # a single entry
    root: { table: events, pk: id }
    label: title
    includes:
      - { table: attendees, fk: event_id }
      - { table: reminders, fk: event_id }
```

Table names above are logical; the core resolves them to the extension's prefixed physical tables.

**Validation at install and update (D5)** — the extension is rejected, not just the declaration, if:

- any `root` or `includes` table lies outside the extension's own prefix
  `{public_key}__{extension_name}__`;
- any FK edge points outside that prefix (no reaching into core tables or other extensions' tables);
- a table is not CRDT-tracked, or is a `_no_sync` table.

### 9.2 Share model

```
shares(share_id, owner_vault, type, root_pks, created_at)
share_grants(share_id, grantee_vault, capabilities, epoch)   -- written only by the owner (D6)
scope_keys(...)                                              -- §5.2, per share
```

- **Membership of a row in a share** is derived: the root row plus every row reachable through the
  declared FK edges. A new event in a shared calendar is automatically part of the share; there is no
  row register.
- **Sender-side guard**: only declared tables and only rows reachable from the root leave the vault
  for that share (`ScanFilters` with `column_eq` / `row_pks`).
- **Receiver-side guard** (`ApplyPolicy::prepare_row`): an incoming row is admitted only if its table
  belongs to the share type's extension, it is reachable from the share root, and the author vault
  holds the needed capability (§11). A new row from a grantee (an item added to a shared shopping
  list) must carry an FK into the share.
- **Where received data lands**: in the recipient's copy of the same extension tables, tagged with
  the originating `share_id` in a core-owned mapping. It then syncs to the recipient's own devices
  through plane 1.
- **The ACL has a single writer** (the owner vault). Concurrent ACL edits can only come from the
  owner's own devices and resolve by ordinary LWW. No concurrent admins, no forks.

### 9.3 Overlapping shares

With D4, one row can be inside two shares (an event shared individually and through its calendar). A
write is validated against the share it arrived through. Forwarding it into the other share is done
by the owner vault, so it is delayed until an owner device is online. Accepted for v1.

## 10. Blind relay ("Holzi Relay")

### 10.1 Role

A headless, multi-tenant server. It is **untrusted** (D11): it can delete or withhold, but it can
neither read nor forge. It is not a peer: no vault, no passphrase, no apply.

It bundles three services:

1. **Mailboxes** for sealed batches, one per scope (vault-internal, space, data share). Served over
   the same iroh ALPN family, so the relay is a "blind peer" that speaks the sync protocol but only
   stores and forwards.
2. An **iroh relay** for NAT traversal.
3. Optionally a **Nostr relay** for presence, invites and signaling.

### 10.2 Mailboxes

- Append-only; each accepted batch gets a monotonic sequence number; clients pull `after <seq>`.
- **No server-side merge.** Table names, columns, PKs and HLCs stay inside the ciphertext — the
  opposite of haex-sync-server's per-cell rows.
- **Compaction is client-driven**: a member with `write` uploads an encrypted snapshot of the scope
  at a sequence number; the relay then drops older batches. The snapshot carries the original
  per-column signatures (haex-crdt keeps them in `haex_column_sigs`), so a snapshot writer cannot
  forge other members' data. Receivers verify snapshots exactly like batches.
- No accounts, no email. An identity is a key. Quota is per vault npub; admission is by operator
  invite code in v1, with payment (Cashu/Lightning) as a later option.

### 10.3 Authorization at the relay: signed member lists, not UCAN

The relay must decide who may read from and write to a mailbox. It does this with a **signed member
list** that the admin uploads (the deferred design's §4 projection):

```
MemberList { scope_id, epoch, grants: [{vault_npub, caps}], issued_at, expires_at, admin_sig }
```

- A request authenticates with the **device key** (NIP-42 challenge) and presents its **device
  attestation**. The relay checks the admin signature, that the attested vault is in the current
  list, and that the capability fits (`read` for pulls, `write` for pushes).
- A higher epoch replaces a lower one; missing, expired or invalid lists fail closed.
- **Revocation is immediate**: the admin uploads a new list. No revocation list is needed, unlike
  UCAN tokens that stay valid until expiry.
- **Why not UCAN**: UCAN's value is verifiable _delegation chains_. With D6/D7 every chain has exactly
  one link (admin → member), and a one-link UCAN is a signed grant. Holding the current list at the
  relay gives immediate revocation, and it uses the same secp256k1 keys as Nostr instead of a second
  `did:key` Ed25519 key system. Functionally it is still a capability system.
- **The relay gate is coarse and only defense in depth.** It can only check read vs. write. The fine
  rules (`created_by`, delete-own, FK into share) are enforced by every receiving client after
  decryption, so a relay that lets something through still cannot get it applied.

### 10.4 What the relay sees (D9)

Sees: IP addresses, timing, sizes, scope ids, the member list per scope (vault npubs + capabilities),
device npubs through attestations. Does not see: any content, table or column names, PKs, HLCs, file
names, file sizes beyond ciphertext length. Batches may be padded to size buckets. Per-scope
pseudonyms are a post-v1 option.

## 11. Capabilities

| Capability | Data share                                        | Space                                            |
| ---------- | ------------------------------------------------- | ------------------------------------------------ |
| `read`     | read all rows of the share                        | read all files                                   |
| `write`    | create rows, modify any row, delete **own** rows  | add files, modify any file, delete **own** files |
| `delete`   | delete any row of the share                       | delete any file                                  |
| admin      | implicit for the creator only; not grantable (D6) | same                                             |

- "Own" means `created_by` equals the author's vault (§5.4).
- **Proposed, pending operator confirmation**: this three-flag split (delete-own inside `write`,
  separate `delete`). The operator named three options on 2026-09-27; this is the recommended one.
- **Revocation vs. concurrent writes — proposed rule**: a change encrypted under key epoch _e_ is
  valid if its author vault held the needed capability in the member list of epoch _e_. Revocation
  therefore acts forward only; a member writing concurrently with its removal may have that last
  write accepted. The alternative, keeping a batch log per scope and recomputing affected cells on
  membership change, is correct but much heavier. Needs operator confirmation.

## 12. Storage backends for spaces (D12)

The client side is identical for both backends: same immutable, opaque, encrypted objects addressed
by ciphertext hash from a signed file index. Only the way a device obtains access differs.

### A. Storage provided by the public relay

- The relay operator runs the storage (S3-compatible). The relay _is_ the storage provider, so its
  access is unavoidable and harmless: it holds only ciphertext, and forgery is detected.
- Access: the relay checks the member list and issues short-lived presigned URLs; traffic goes
  directly between device and storage.
- Delete: the relay records the uploading vault per object and allows `DELETE` for the uploader or a
  `delete` holder.

### B. The user's own S3

- **The relay never receives the credentials** (D11).
- **One bucket per space**, with **two scoped tokens**: read-only and read-write. The admin creates
  them in their vault and wraps them with NIP-44 to members according to capability, inside the
  encrypted space data.
- Devices access S3 directly with those tokens; the relay is not involved for files.
- Revocation: rotate the affected token and redistribute it, then rotate the space key.
- **Limit**: S3 cannot enforce "delete own files only"; a read-write token can delete any object.
  File-index tombstones remain client-checked. Physically deleted objects are recovered through
  **bucket versioning**, which the setup flow MUST enable.
- Provider support for bucket-scoped tokens must be verified per provider. Without it, the fallback
  is an admin device issuing presigned URLs over iroh (available only while that device is online).

## 13. Threat model

| Property                 | Secured by                                                                                                        | Relay / storage can                                                                                  |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Confidentiality          | encryption; keys only in members' vaults                                                                          | nothing                                                                                              |
| Integrity / authenticity | per-chunk AEAD, ciphertext-hash object ids, per-column device signatures, attestations, admin-signed member lists | nothing — forgery is rejected by receivers                                                           |
| Authorization            | receivers enforce all rules; relay gate is additional                                                             | let an unauthorized request through (still not applied) or refuse a valid one                        |
| Availability             | redundancy: direct iroh sync, several own devices, snapshots                                                      | delete, withhold, serve stale state — detected by version vectors and sequence gaps, not preventable |
| Removed member           | key rotation, relay gate, token rotation                                                                          | — ; the removed member keeps what it could already decrypt (accepted, as in haex-vault ADR 0002)     |
| Stolen device            | SQLCipher passphrase; vault identity rotation (D8)                                                                | —                                                                                                    |

## 14. Proposed spec cut

Spec numbers 017–021 are reserved for agent control; the next free numbers start at 024.

| Spec | Scope                                                                                                                                                                            | Depends on                                 |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| 024  | Real secp256k1 vault identity and device keys, device attestations, iroh endpoint, presence via Nostr, `holzi-sync/1` between own devices (plane 1), authorship fix in haex-crdt | —                                          |
| 025  | Own-device file sync: file index, encrypted immutable objects, iroh-blobs, conflict copies                                                                                       | 024                                        |
| 026  | Blind relay: mailboxes, sequence cursors, client snapshots, signed member lists, iroh relay, storage backend A                                                                   | 024                                        |
| 027  | Spaces: member lists, invites, capabilities, key epochs, revocation, rotation                                                                                                    | 025, 026                                   |
| 028  | Data shares: shareable declaration schema, prefix validation, closure computation, sender/receiver guards                                                                        | 024, 026, extension table storage in holzi |
| 029  | Storage backend B: own S3, scoped tokens, versioning                                                                                                                             | 027                                        |

Structural items that belong in 024 even though only the vault-internal scope exists then: the real
`SignatureProvider` seam, the sealed-batch format with `scope_id`/`key_id`, the allow-list that keeps
vault secrets on plane 1, and `created_by` support in the core.

## 15. Open questions

1. **Delete semantics** (§11) — confirm the three-flag proposal.
2. **Revocation vs. concurrent writes** (§11) — confirm the forward-only rule.
3. **Admin loss**: if the admin vault is lost entirely, the space or share is frozen (content stays,
   membership cannot change). Is a "transfer admin" feature needed, and when?
4. **Vault identity rotation flow** (D8): how counterparties learn the new npub. Inside existing
   shares the old identity can sign a hand-over statement, but only if the key is not the
   compromised one.
5. **Snapshot authority**: whether any `write` member may upload a compaction snapshot, or only the
   admin.
6. **Relay discovery and trust configuration**: how a vault picks relays; whether a space may use
   several relays for redundancy.
7. **Wire formats and event kinds**: sealed batch, member list, attestation, presence, invite DM.
8. **Rotation cost**: O(recipient vaults) per membership change — measure before large spaces appear.
9. **Mobile**: plane 1 while the app is foreground-only (v1-scope availability classes).
10. **Extension storage in holzi**: data shares (028) need extension-owned, prefixed CRDT tables,
    which holzi does not have yet.
