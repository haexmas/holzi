# Implementation Plan: Portable Mode with Protected Data

**Branch**: `014-portable-mode` | **Date**: 2026-09-24 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/014-portable-mode/spec.md`

## Summary

Portable mode is a process-level data-root selection with two distributions:
removable-storage mode and a single-file mode backed by one protected container.
The first implementation slice must prove that model bytes can be consumed while
the host disk never receives readable user data. If that proof fails, removable
storage may expose an explicitly weaker mode, but the single-file mode is blocked.

The design extends the existing path resolvers and instance lifecycle rather than
adding a second vault or model system. One resolved `PortableDataRoot` is chosen
before instance discovery, passed to instance/model/preferences/cache/log paths,
and carried in the relaunch configuration required by feature 013 without
creating a third app-owned file.

## Technical Context

**Language/Version**: Rust (Tauri 2, repository toolchain), TypeScript/Vue 3,
Node scripts for deterministic checks, Windows first.

**Existing infrastructure to extend**:

- `src-tauri/src/instances/paths.rs` owns managed instance paths.
- `src-tauri/src/models/paths.rs` owns model roots and filenames.
- `src-tauri/src/instances/startup.rs` resolves startup state.
- `src-tauri/src/state.rs` owns process-level active-instance state.
- feature 013 owns one-vault-per-process, relaunch, and secret lifecycle rules.
- SQLCipher already protects vault database contents; model and other files are
  currently ordinary files below application-local data.

**Protection approach**: Do not invent an encrypted filesystem or write a new
cryptographic primitive. Phase 0 must evaluate whether the existing local-model
loader can consume decrypted bytes without materializing a readable model file.
A protected container implementation may use an existing audited library only
after the spike proves the boundary and its failure behavior. The container
format, key derivation, nonce handling, and memory erasure become normative in
the contract only after that gate succeeds.

**Distribution**: Windows is the first target. The single-file form is a
non-installer executable; it must not register services, shortcuts, startup
entries, file associations, or use a system-wide installer. Other platforms
remain an explicit follow-up unless the same boundary is verified.

**Testing**: Rust unit/integration tests in dedicated test files; deterministic
path and mode checks in `scripts/`; black-box filesystem tests for portable
runs; existing normal-installation tests remain the regression gate.

**Constraints**:

- No fallback from the portable root to OS app-data, temp, cache, or log paths.
- The portable decision is made before any instance/model discovery or directory
  creation.
- Passphrases and derived session keys never enter logs or persisted config.
- Agent-requested writes to user-selected host files remain possible and are
  outside the portable guarantee, with that limit shown to the user.
- Keep production files below 500 LoC where practical; split path, container,
  and lifecycle responsibilities by existing module boundaries.

## Constitution Check

| Principle                                      | Status                      | Evidence / gate                                                                                                 |
| ---------------------------------------------- | --------------------------- | --------------------------------------------------------------------------------------------------------------- |
| No secrets in Git                              | PASS                        | No keys, passphrases, or fixtures containing secrets are committed.                                             |
| No local absolute paths in versioned artifacts | PASS                        | Plans and contracts use repository-relative paths only.                                                         |
| Cross-repo refs use immutable SHAs             | PASS                        | No new cross-repo reference is needed for the design slice.                                                     |
| Worktree and topic branch                      | PASS                        | Work occurs in `.worktrees/014-portable-mode` on `014-portable-mode`.                                           |
| Separate test files                            | PASS                        | Rust tests stay in `*_tests.rs`; filesystem checks stay under `scripts/` or `tests/`.                           |
| Non-trivial logic has a runnable check         | PASS                        | Every path/protection boundary gets a deterministic check before implementation is complete.                    |
| Security boundary is explicit                  | PASS                        | Single-file mode is blocked until the no-readable-host-data gate passes.                                        |
| Phase discipline                               | NEEDS OPERATOR CONFIRMATION | The roadmap must record when portable mode is allowed to implement after feature 013. Planning may proceed now. |

## Design Gates

### Gate 0: protected-model feasibility

The first implementation task builds a minimal protected-storage fixture and
tries the real local-model loading path. It must establish all of the following:

1. The model is stored encrypted or inside the protected container at rest.
2. The loader receives usable plaintext only in memory or an equivalent
   OS-supported non-persistent handle.
3. Wrong keys fail before model metadata or bytes are exposed.
4. Download/import, hashing, cancellation, and cleanup do not create a readable
   staging file on the host disk.

If any item fails, record the exact limitation. Implement removable-storage
weak-mode messaging and portable-root isolation, but do not implement or expose
the single-file protected-container mode.

### Gate 1: complete data-root audit

Before changing path code, enumerate every process-owned write/read surface:
instances, models, preferences, installation identity, webview state, caches,
diagnostics, update/download staging, child-process configuration, and temporary
files. A portable run is not complete until every surface either resolves below
`PortableDataRoot` or is deliberately classified as an operating-system trace
named in the limits statement.

## Project Structure

### Design artifacts

```text
specs/014-portable-mode/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── contracts/
│   ├── runtime-boundary.md
│   └── protected-container.md
├── quickstart.md
├── checklists/requirements.md
└── tasks.md
```

### Expected implementation surface

```text
src-tauri/src/
├── portable/
│   ├── mod.rs                 # mode detection and startup boundary
│   ├── paths.rs               # PortableDataRoot and child-path policy
│   ├── container.rs           # protected container adapter, after Gate 0
│   ├── state.rs               # process-only mode/session-key state
│   └── *_tests.rs             # dedicated boundary and failure tests
├── instances/paths.rs         # resolve through the selected root
├── models/paths.rs            # resolve through the selected root
├── instances/startup.rs       # reject unsafe startup before writes
├── device/commands.rs         # portable installation identity handling
└── state.rs                   # retain portable configuration for relaunch

src/
├── components/portable/       # first-start statement, mode/unlock/errors
├── composables/usePortableMode.ts
└── pages/index.vue             # select/continue portable mode before unlock

scripts/
├── check-portable-paths.ts
└── check-portable-filesystem.ts

docs/
└── security/portable-mode.md  # user-facing limits and operator guidance
```

## Phased Implementation

### Phase 0 — feasibility and audit

1. Inventory all process-owned paths and child processes; document each owner in
   `research.md` and the runtime contract.
2. Build the protected-model spike against the existing model loader and record
   a pass/fail result with observable filesystem evidence.
3. Confirm the feature-013 relaunch contract and secret-erasure boundary.
4. Update the plan and tasks if the spike changes the supported scope. No UI may
   advertise the single-file form before this phase passes.

### Phase 1 — portable root and removable-storage mode

1. Detect portable launch configuration before app-data resolution.
2. Introduce one root resolver and route instance, model, preferences, cache,
   diagnostics, webview state, and child-process configuration through it.
3. Refuse unavailable/read-only/full roots without creating a host fallback.
4. Carry only non-secret relaunch configuration needed to reopen the same root;
   do not create a third app-owned file beside the executable and container.
5. Add filesystem tests proving normal and portable installations are isolated.

### Phase 2 — protection boundary

1. Implement the approved protected-container adapter only if Gate 0 passes.
2. Derive a session key after unlock, keep it process-local, and erase it on
   close/failure according to feature 013.
3. Route model import/download/open and metadata through the protected boundary.
4. Reject wrong keys, corrupted containers, partial writes, and disk-full cases
   without exposing plaintext or leaving partial artifacts.
5. Add outside-the-app marker scans for names and content categories.

### Phase 3 — single-file distribution and UI

1. Produce the Windows non-installer distribution with no system-wide changes.
2. Add first-start setup for the container location and free-space check.
3. Add unlock, delete, cleanup, unavailable-root, missing-component, and refusal
   states before vault discovery.
4. Show the limits statement before unlock and document deliberate host-file
   writes as outside the guarantee.
5. Keep normal installation behavior and messages unchanged.

### Phase 4 — validation and handoff

1. Run the full normal-installation suite and portable path/filesystem checks.
2. Execute the Windows no-admin quickstart with removable storage and single-file
   runs, including crash, relaunch, copy, wrong-passphrase, and deletion cases.
3. Record unsupported platforms and known OS traces in the security document.
4. Re-run the constitution and cross-artifact analysis, then prepare a PR with
   the feasibility evidence attached.

## Complexity Tracking

| Risk / deviation                                        | Why it is necessary                                                                               | Upgrade or exit path                                                                                   |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| A protected container may require a new storage adapter | Existing model APIs are path-based and cannot promise host-disk secrecy by configuration alone.   | Prefer an existing audited library; otherwise block the single-file mode rather than weakening FR-018. |
| Portable mode must own more than vault files            | Webview state, cache, logs, and helper processes can leak traces even when the vault is portable. | Keep the path audit as a maintained boundary test and add owners when new storage surfaces appear.     |
| Removable-storage weak mode may be needed               | The spec explicitly allows it only when the feasibility gate fails.                               | Label every unprotected category and keep the single-file form unavailable.                            |
