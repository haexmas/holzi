# Research: Portable Mode with Protected Data

**Feature**: 014-portable-mode
**Date**: 2026-09-24

## R1. One data-root boundary

**Decision**: Add one process-owned `PortableDataRoot` boundary and make the
existing instance and model path modules resolve children from it. The boundary
is selected before instance discovery and is passed to preferences, identity,
cache, diagnostics, webview state, and helper-process configuration.

**Rationale**: `instances/paths.rs` and `models/paths.rs` are the existing path
owners. A second portable storage implementation would leave ordinary callers
able to bypass the guarantee.

**Alternatives considered**: Environment-variable overrides at each call site
were rejected because they are easy to omit and difficult to test exhaustively.

## R2. Mode detection and relaunch

**Decision**: The launcher supplies a non-secret portable-mode descriptor as
launch configuration, and the process bootstrap validates it before any app-data
directory is created. Relaunch receives the same descriptor through the
feature-013 process configuration; it is not written as a third app-owned file.
A missing, unreadable, or inconsistent root is a hard startup error with no host
fallback.

**Rationale**: The mode must survive a close/relaunch while secrets must not be
persisted. The descriptor identifies a location, not a vault key.

**Alternatives considered**: Infer mode from the current working directory;
rejected because shortcuts and shell launchers can change it.

## R3. Removable storage before protected-container mode

**Decision**: Implement removable-root isolation independently from the protected
container. If the model feasibility gate fails, expose only a clearly labelled
weaker removable mode and never expose the single-file mode.

**Rationale**: The specification explicitly permits a weaker removable mode but
forbids readable user data on the computer's own disk.

## R4. Protected model feasibility is a release gate

**Decision**: Use the current local-model loader in a black-box spike. The spike
must prove that model bytes, filenames, metadata, download staging, hashing, and
cleanup do not create readable host-disk data. No container format or crypto API
is committed before this result.

**Rationale**: Current model loading is path-based. A path to a decrypted file on
the host disk would violate FR-018 even if the container itself is encrypted.

**Alternatives considered**: Mount an encrypted volume; rejected because it
requires drivers or administrator rights. Encrypt each file and decrypt to a
temporary file; rejected because the temporary file is the exact forbidden leak.

## R5. Key lifecycle

**Decision**: The session key is derived only after unlock, held in process state,
never serialized or logged, and released before the active instance is dropped.
Wrong-key and corruption errors are indistinguishable from the outside until a
valid unlock is established.

**Rationale**: Reuse feature 013's passphrase and process-secret rules rather
than creating a portable-specific secret store.

## R6. Single-file distribution

**Decision**: Windows first, as a non-installer executable. It may create exactly
one user-selected protected container; no additional app-owned file is created
beside it and no system-wide registration is performed. Other platforms need a
separate capability check and are out of the first implementation slice.

**Rationale**: The threat model targets shared Windows computers and the spec
requires no elevation or installer changes.

## R7. Host traces and deliberate user writes

**Decision**: The limits statement names OS-managed traces (paging, hibernation,
crash reports, recent files, download/security scans, and device logs). Agent
writes explicitly requested by the user use their requested host path and are
outside the guarantee; portable routing must not silently redirect them.

**Rationale**: The app cannot control a hostile OS and must not change the
meaning of a user-approved file operation.
