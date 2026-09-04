# Holzi — v1 Scope and Founding Revisions

**Status**: Draft. Written 2026-09-04, same day as `docs/design/founding.md`. Captures scope decisions and revisions made in a scope-sharpening session immediately after the founding capture.

**Relationship to `founding.md`**: this document is normative for v1 scope and for the identity/storage/mobile revisions listed below. Where it and `founding.md` differ, this document wins for v1. Where it is silent, `founding.md` stands. Both documents are pre-spec: field names, event kinds, wire formats, and identity derivations are working proposals, not settled interfaces, until re-expressed in numbered specs (see Section 5 of `founding.md`).

---

## 1. Purpose

The founding document captures the full architectural vision. This document does two things:

- **Draws the v1 line.** Which parts of `founding.md` are in v1, which are post-v1, and why.
- **Revises three founding areas** whose treatment in `founding.md` no longer matches how v1 should work: the identity/pairing model, the storage layer, and the mobile availability model. Each revision is stated here in enough detail to write specs from; deeper detail is spec-phase work.

## 2. v1 scope summary

| Founding element | v1 status | Note |
| --- | --- | --- |
| Tauri application shell (desktop, server, mobile) | **In v1** | Desktop + server + iOS + Android. Mobile has explicit availability constraints (Section 5). |
| Embedded Nostr relay endpoint per device | **In v1** | Every attested device hosts a relay endpoint. Mobile only while foreground. |
| iroh peer per device | **In v1** | Every attested device runs an iroh peer. Mobile only while foreground. |
| Nostr command events (device-to-device) | **In v1** | Cross-device command routing over Nostr is v1's core value. |
| NIP-17 DMs | **In v1** | Second ingress track survives from founding. |
| `blob.offer` (iroh content-addressed blob transfer) | **Post-v1** | v1 does not ship blob transfer between devices. `blob.offer` and the ticket-lifecycle spec (`founding.md` §5 item 4) are deferred. Cross-device MCP results that would exceed a size threshold are refused in v1 rather than falling back to blobs. |
| `stream.offer` (real-time voice/video/screen streams) | **Post-v1** | No real-time media in v1. `founding.md` §4 item 11 (stream-session semantics) is entirely post-v1. |
| MCP server for external clients (host-local) | **In v1** | IDE/shell/script access is v1. Standard MCP auth per transport. Two auth boundaries to spec (this one plus the cross-device one). |
| MCP-to-Nostr adapter (cross-device MCP invocation) | **In v1** | Follows from cross-device commands + external MCP. |
| Embedded LLM model runner | **In v1** | Runs on desktop, server, and mobile (small preset on mobile). See Section 6. |
| Provider adapters (Anthropic, OpenAI, OpenRouter, Ollama-HTTP, Generic-OpenAI-compatible) | **In v1** | Native Rust trait; see Section 6. |
| Confirmation-authority signed-release model | **In v1** | Cross-device confirmation is a killer use case for the mobile-in-v1 decision. Kept as founding describes. |
| Chat surface in Tauri UI | **In v1** | Primary interaction target. |
| Device attestations, trust store, epoch-based revocation | **In v1** | Baseline for any cross-device work. Revised identity model in Section 4 changes *who signs*, not the trust-store shape. |
| Cross-user sharing (`access.grant`/`access.revoke`, guest events) | **Post-v1** | `founding.md` §6 future-direction, unchanged. |

## 3. What "v1 done" looks like

Two of the operator's own devices, paired into one federation. From each device the operator can:

- Open a chat with a local or provider model of their choice.
- Send a routed command to the other device (`@laptop, do X`) and get a result back.
- Have an IDE / shell / script on either device talk to holzi via its host-local MCP server.
- Confirm a `require-confirmation` write action initiated on one device from the other (signed-release-event round-trip).

No file transfer between devices. No voice/video. No cross-user sharing.

## 4. Revised identity and pairing model

`founding.md` describes a single offline master key held externally (hardware token / NIP-46 / paper backup), signing device attestations directly. This does not match the v1 UX intent, which is that any holzi installation should be able to become an issuer or a joiner without an external signer session.

**Revision.**

- **Genesis moment.** On first install, a holzi instance can start a fresh federation. It generates a federation-root keypair in-app and displays a versioned **paper-seed** bundle containing the seed material and the federation-root public-key fingerprint. The root private key is deterministically derived from the seed with a domain-separated, versioned KDF; recovery derives the keypair again and verifies that the resulting public key matches the fingerprint in the bundle before restoring signing capability. A mismatch aborts recovery. The operator is required to record and store the paper-seed before setup completes; the seed is the sole federation-scope recovery secret and is the only secret artifact that ever leaves the federation's encrypted state boundary.
- **Symmetric pairing.** Any already-attested device with a current `pairing-authority` capability may pair a new device. A parent with only `confirmation-authority` cannot issue or accept a pairing attestation. Pairing runs by short-lived one-time token (QR code or copy/paste string) issued by the parent device and scanned by the joiner. The joiner signs a canonical, token-bound pairing transcript with its new device key, and the parent signs the same transcript as the joiner's attestation; both signatures must verify against the identities named in the transcript, the parent capability must be current at the transcript's epoch, the token must be unused and unexpired, and the transcript must name the current federation epoch. Only then is the attestation accepted. After pairing, the joiner is a full peer with pairing authority of its own only if that capability was held by the parent and explicitly granted.
- **CRDT-synced attestation ring.** The federation state (attested-device registry, revocation epochs, capability grants including `confirmation-authority` and `pairing-authority`, plus capability-scoped paper-seed-derived signing material) lives in the encrypted CRDT store described in Section 5. Authority-bearing devices receive only the signing material needed by their current capabilities. A device paired with neither attestation capability receives a redacted federation-state view and never receives the paper-seed, a derived private key, or an exportable signing handle. Alias assignment is unique within a federation: concurrent attestations proposing the same alias are detected by the alias plus the attestation identities, and the canonical winner is the lexicographically smallest attestation ID (with the full ID as the deterministic tie-breaker). The losing/conflicting attestation remains recorded, and the alias remains unroutable while any conflict exists; commands addressed to it fail with an explicit alias-conflict result until a subsequent attestation assigns a free alias.
- **Compromise model.** Because authority-bearing devices hold federation signing material, compromise of one of those devices can compromise the federation's ability to issue new attestations. Response depends on whether the attacker also holds the SQLCipher passphrase of that device. Without the passphrase, the at-rest signing material remains unusable; response is fast revocation via epoch bump (as in `founding.md`), initiated from any surviving device. With the passphrase, individual revocation cannot outrun an attacker who has `pairing-authority` — attacker-signed branches would appear faster than they can be individually revoked — and the operator resets the federation entirely (see *Federation reset* below). A device with neither attestation capability holds only its own device keys and cannot extend trust or approve protected writes; its compromise has bounded blast-radius regardless of passphrase. The paper-seed remains untouched by device compromise, since it is not stored on any device.
- **Recovery.**
  - If at least one attested device survives, a new device joins normally via pairing; federation state re-syncs via CRDT.
  - If no attested device survives, the operator imports the paper-seed into a fresh install, which restores signing capability. Attested-device state cannot be reconstructed from the paper-seed alone; the operator effectively re-founds and re-pairs surviving devices, if any.
- **Federation reset (compromise recovery).** The response to a confirmed compromise of an authority-bearing device that also includes the SQLCipher passphrase is a full federation reset, not incremental revocation. Any trusted device generates a fresh federation-root keypair and paper-seed. The old federation is deprecated at a cutoff epoch communicated out-of-band to every trusted device. Clean devices join the new federation via normal pairing against the new root. State that must survive (chat history, skills, memory) is exported from the old federation on a device the operator still trusts and imported into the new one manually. Old federation artifacts (paper-seed, attestations) are considered untrusted and are not carried over. This is the panic-button path, not daily hygiene: for well-scoped compromises without passphrase, epoch-bumped revocation remains the response.
- **What remains from `founding.md`.** The trust-store shape (`alias, nostr_pubkey, iroh_node_id, valid_until, capabilities, epoch`) is unchanged. Revocation events merge as an add-wins union keyed by each event's unique identity, so all concurrent revocation targets remain recorded before effective trust is derived. Revocation epochs merge monotonically: the merged federation epoch is always the maximum observed epoch and can never decrease. Attestations and capability grants whose epoch is below that current merged epoch are rejected, so delayed CRDT state cannot restore a revoked device or capability. The `attestation` event and revocation event are unchanged in shape; what changes is *who signs them* — any attested device with `pairing-authority`, not an external master.
- **Attestation-capability slots.** `pairing-authority` and `confirmation-authority` are attestation-capability slots, delegable and revocable through the same attestation flow. Delegation is non-escalating: an authority may grant only capabilities it currently holds, so `pairing-authority` cannot grant `confirmation-authority`. Genesis-device holds both by default. A protected write requires a current `confirmation-authority` attestation. The two slots are independent: a parent may pair a joiner with any subset of its own capabilities, including neither — the concrete motivation is temporary installs on untrusted hosts (a laptop in an internet cafe, a shared workstation), which the operator pairs without either slot so the device participates in chat and reads federation state but cannot invite further devices or approve protected writes. A joiner with neither slot may read registry metadata (aliases, endpoint identities, validity, capabilities, and current epoch), revocation and capability records, presence, and chat/session state, but receives no usable federation signing material and may sign only device-scoped traffic permitted by its own attestation. A separate concern is wiping that temporary device's local state on session end so nothing remains on the host; that is tracked as *Ephemeral Session Mode* in Section 12.

**Explicitly out of the revision.** NIP-46 remote-signer support and hardware-token custody are not in v1. They can return post-v1 as alternative Genesis-moment root-key custody options; the paper-seed will remain the universal recovery path.

## 5. Storage, at-rest encryption, and CRDT sync via `haex-crdt`

`founding.md` §4 items 5 and 8 (encrypted-at-rest choice, multi-device routing) are resolved by consuming an extracted library.

- **`haex-crdt`.** The SQLite + CRDT-sync layer currently living inside `haex-vault` is extracted into a standalone Rust crate named `haex-crdt`. Both `haex-vault` and `holzi` consume it as a Rust library dependency. This extraction is v1-blocking: no holzi implementation slice starts until `haex-crdt` exists as an importable crate.
- **At-rest encryption is inherited.** `holzi` does not choose its own at-rest scheme; it uses whatever `haex-crdt` provides (working assumption: SQLCipher, to be confirmed during extraction). If `haex-crdt` changes its scheme, `holzi` moves with it.
- **What lives in `haex-crdt`-synced state.** Federation-scope encrypted state that must be identical across attested devices: attested-device registry, revocation-epoch table, capability grants, chat/session history, skills/memory (post-v1 scope but reserved). Paper-seed-derived signing material is a separate capability-scoped record: it is replicated only to authority-bearing devices and is absent from the redacted view of an empty-capability device. The record-level ACL and encryption mechanism are spec-phase work.
- **What does not live in `haex-crdt`.** Ephemeral runtime state (open iroh sessions, current relay connections), local-only preferences that should not sync (device alias, local model file paths), and any per-device secret keys (Nostr device keypair, iroh NodeId keypair) that are generated on each installation and must never cross the CRDT boundary.
- **Three cross-device channels, disjoint by role.** With `haex-crdt` in play, holzi has three cross-device channels rather than the two in `founding.md`:
  1. **Nostr** — semantic events: presence, capability advertisement, commands, LLM prompts/responses, DMs, control-plane events, confirmation intents and releases.
  2. **iroh** — bulk bytes: `blob.offer` (post-v1) and `stream.offer` (post-v1). No v1 traffic on this channel except peer keep-alive.
  3. **`haex-crdt` sync** — federation state deltas: attestation ring, revocation-epoch changes, capability changes, chat/session state.
  These channels do not share ownership. `haex-crdt` is authoritative for chat/session records and their stable message IDs; Nostr events are transport-only envelopes for prompts, responses, and control messages, and are not a second chat history. A received event is applied to the CRDT record keyed by its message ID exactly once; replays and duplicate deliveries are ignored. Nostr never carries CRDT deltas; `haex-crdt` never carries a command intent; iroh never carries state. Which transport `haex-crdt` uses internally (its own WS bridge, iroh, or something else) is a `haex-crdt`-internal decision, not a holzi-visible one.

## 6. Provider layer

Native Rust `LlmAdapter` trait with five implementations, hardcoded in v1. No Python sidecar, no plugin surface.

1. **Anthropic** — direct Messages-API adapter. Native support for prompt caching and extended-thinking as those Anthropic features stabilize.
2. **OpenAI** — direct Chat-Completions adapter for canonical OpenAI endpoints.
3. **OpenRouter** — direct adapter, exposes the OpenRouter model catalog behind one API key.
4. **Ollama-HTTP** — adapter for an externally running Ollama on the host or on another operator device.
5. **Generic-OpenAI-compatible** — user supplies a base URL and optional API key. Escape hatch for LiteLLM-as-proxy, LM Studio, Azure-OpenAI, custom vLLM/TGI deployments, and Ollama's OpenAI-compat endpoint.

Plus **embedded runner** as a first-class local backend, not an adapter row: on desktop and server holzi ships a runner (candidate stacks: llama.cpp / candle / mistral.rs, chosen at implementation time based on mobile-portability) and a default small model. On mobile the runner ships with a smaller preset (Phi-3-mini-class or Qwen-tiny-class); battery/storage cost is accepted, iOS App Store size restrictions are respected. Model choice at runtime is always explicit; no automatic routing between local and provider adapters.

## 7. Mobile as full peer with foreground-only availability

`founding.md` §4 item 7 (mobile scope for v1) is answered: iOS and Android are in v1. `founding.md`'s "device equals relay equals Tauri application" one-to-one identity is preserved on mobile, with a documented availability constraint.

- **Full-peer while foreground.** When the mobile app is in the foreground, the mobile device hosts its Nostr relay endpoint and its iroh peer exactly as desktop does. It accepts inbound Nostr command events from the operator's other devices, serves NIP-17 wrappers, participates in `haex-crdt` sync.
- **Unreachable while backgrounded.** When the mobile OS backgrounds and suspends the app, the relay endpoint and iroh peer are unreachable. This is *not* an error state and must not surface as one. Other devices in the federation observe the mobile device as unreachable and continue.
- **Presence events carry availability class.** Each device's presence event declares an availability class — working proposals: `always-on` (typical desktop or server), `foreground-only` (typical mobile). Availability class is normative for sender behavior: a command targeted at a currently-unreachable `foreground-only` device does not wait on long relay timeouts; the sender returns `queued` immediately while retaining the command intent.
- **Delivery semantics for mobile targets.** Command intents addressed to a currently-unreachable `foreground-only` device are persisted at the sender's home relay with status `queued`, a TTL, and a stable intent ID. The same intent ID is reused for retries, so the target executes an intent at most once. Unreachable targets enter `queued`; when the intent is delivered after a fresh presence event, it moves to `accepted` and may then become `completed` or `failed`; if its TTL elapses while queued, it moves to `expired`. A sender can query every status by intent ID. Exact TTL and the presence-event kind carrying availability class are spec-phase work.
- **iroh sessions to mobile.** iroh streams to a mobile peer terminate on background; there is no session resumption across the sleep/wake boundary in v1. This is aligned with `stream.offer` being post-v1: v1 has almost no iroh traffic to a mobile peer to speak of.
- **Push notifications are not in v1.** Waking a mobile app via APNs/FCM to receive a delayed command is post-v1. In v1, the operator opens the mobile app to check queued intents (like most messengers already require for practical delivery on iOS anyway).

## 8. Confirmation-authority

`require-confirmation` and the signed-release-event model from `founding.md` are in v1, unchanged in shape. Restated for completeness:

- Any capability class may be marked `require-confirmation` in policy.
- The relay holds intents in that class until it receives a signed release event.
- The release event is signed by a device whose current attestation carries the `confirmation-authority` capability. That capability is delegable and revocable through the attestation flow (Section 4 above; the flow itself is symmetric-cross-signing in v1, not master-signed).
- The release event names the exact intent ID.
- Receivers verify the release-signer's attestation binding, the current epoch, and the intent scope before accepting the release; then the intent forwards.

The killer v1 use case: an operator confirms from their phone a `require-confirmation` action initiated on their laptop.

## 9. Relay implementation choice

`founding.md` §4 item 4 is not fully decided; a directional choice is captured.

- Direction: **embedded Rust relay with SQLite backend**, participating in the broader Nostr ecosystem's kinds and NIPs rather than a minimalist purpose-built one. Rationale: the operator wants to leverage Nostr ecosystem tooling and standards over time; standing on an existing Rust relay reduces boilerplate and Nostr-spec-drift.
- Candidate libraries: `nostr-rs-relay` (github.com/scsibug/nostr-rs-relay), `rnostr` (github.com/rnostr/rnostr), or building on top of `nostr-sdk` (github.com/rust-nostr/nostr). Exact choice is a spike task inside the first implementation slice, not a design-time decision.
- The relay must be embeddable in-process on desktop, server, iOS, and Android. If no candidate meets that bar, the spike proposes either a fork or a minimal purpose-built path, and this document's directional choice is revisited.

## 10. Dependency map

Two parallel workstreams gate holzi v1's first implementation slice:

- **haex-hive schema migration** (in progress, in `~/Projekte/haex-hive/`). Until the migration lands, holzi's declared `com.github.haexmas.atoms.graphify-first-authoring` atom does not take effect. Blocks *tooling*, not code — holzi can proceed on design/spec work without it.
- **`haex-crdt` extraction from `haex-vault`** (planned). Until `haex-crdt` exists as an importable Rust crate, holzi cannot start its first implementation slice. Blocks *code*.

Both are outside this repository. Neither is a holzi task. This document flags them so future readers know why holzi's implementation timeline waits.

## 11. First implementation slice — walking-skeleton sketch

A concrete first slice, once both blockers above are cleared. Not a spec; a sketch of what to bring up first.

**Target.** Two desktop devices, paired into a fresh federation, exchanging a ping-pong command.

**Steps in the slice.**

1. Holzi Tauri app boots on device A. First-run wizard generates a fresh federation-root keypair inside `haex-crdt`-encrypted state, derives and displays the paper-seed bundle, requires operator to confirm they have recorded it.
2. Device A generates a device-scoped Nostr keypair and iroh NodeId; Device A and the federation root both sign the canonical symmetric-cross-signing transcript, which is stored as the attestation event in `haex-crdt`.
3. Device A's embedded Nostr relay endpoint comes up, announces a Presence event with `always-on` availability class.
4. Holzi boots on device B. First-run wizard offers "join existing federation." Device A displays a short-lived pairing QR (containing a one-time token and a Nostr contact hint). Device B scans it.
5. Device B generates its own Nostr/iroh identities and signs the canonical pairing transcript; Device A signs the same transcript as the joiner-attestation; `haex-crdt` synchronizes the attestation ring between the two devices.
6. Device B's relay endpoint comes up, publishes its Presence.
7. From Device A's UI, the operator sends a `ping` command targeting `@device-b` (alias resolution + attestation-epoch binding as in `founding.md` explicit-targeting). The relay routes the command to Device B; Device B echoes a `pong` result.

**Explicitly out of this slice.** LLM chat, external MCP server, MCP-to-Nostr adapter, confirmation-authority flow, embedded model runner, provider adapters, mobile.

Those come as follow-on slices, each with its own spec.

## 12. What remains open

Not decided by this document; still spec-phase work.

- **From `founding.md` §4, still open:** item 1 (event-kind numbering), item 2 (attestation event wire format + replay protection), item 3 (ingress ACL policy language), item 6 (master-key-custody options; this document narrows v1 to paper-seed only, but a broader custody-story returns post-v1), item 9 (comparison due-diligence).
- **From this document, still open:** presence-event kind and availability-class wire format; sender queue TTL for `foreground-only` targets; embedded-runner stack selection (llama.cpp vs candle vs mistral.rs); exact `haex-crdt` crate boundaries and its own transport.
- **Ephemeral Session Mode.** Temporary installs on untrusted hosts (an internet cafe laptop, a shared workstation) need a session-scoped lifecycle. Regardless of whether the eventual implementation uses an attested empty-capability device or an unattested guest peer, the parent issues a short-lived session lease with an explicit expiry bound to the temporary device or guest-session identity. On a normal session end, the parent publishes a revocation targeting that lease or attestation before the temporary device wipes its local haex-crdt store, per-device keypairs, and cached federation state. If the parent cannot reach every peer, the lease expiry is fail-closed; peers stop accepting the session identity when the expiry passes. A crash, forced termination, or power loss skips the clean-end revocation but has the same bounded-expiry outcome, and the operator can issue an explicit revocation from any surviving authority after recovery. Peers validate both revocation and expiry, so a copied device keypair cannot remain accepted beyond the session lease or a subsequent epoch bump. Open sub-questions are the exact lease/heartbeat wire format and whether the guest or empty-capability model ships in v1; the remote cleanup and fail-closed lifetime are requirements for either model.
- **haex-crdt extraction plan** itself: which parts of `haex-vault` move out, what the new crate's public API looks like, how `haex-vault` continues to work post-extraction. Owned by the `haex-vault` project, not holzi.

## 13. Next actions for holzi (this repo)

Once this document is committed:

1. Wait on haex-hive migration and `haex-crdt` extraction.
2. Start the "Project structure and spec workflow" question (`founding.md` §5 item 1): whether to adopt speckit as haex-hive does, adopt it lightly, or use a different approach. This can begin before the two blockers clear.
