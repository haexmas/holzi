# Contract: Portable Runtime Boundary

**Feature**: 014-portable-mode

## Startup ordering

1. Detect the non-secret launch descriptor.
2. Validate mode, root availability, write permission, and required components.
3. Install the selected root in process state.
4. Resolve instance, model, preferences, cache, diagnostics, webview, and helper
   paths below that root.
5. Only then list or open vaults.

Any failure before step 5 returns a plain-language error and performs no fallback
write to the computer's ordinary app-data or temporary locations.

## Path policy

All app-owned path functions receive or derive the selected root. Callers must not
construct portable paths from `app_local_data_dir`, `temp_dir`, or the current
working directory. A path outside the root is allowed only when it is an explicit
user destination for an agent operation and is marked outside the guarantee.

## Relaunch and close

- A close/relaunch preserves the non-secret mode descriptor through the relaunch
  configuration; the app does not create a third file beside the executable and
  protected container.
- Unlock material and session keys are not part of the descriptor.
- `close` flushes and closes protected handles before releasing the active vault.
- Lost storage ends the session; no alternate path is attempted.

## Required error classes

The UI must distinguish unavailable/read-only storage, missing runtime component,
insufficient space, wrong passphrase, damaged container, and unsupported
protection capability. The backend returns structured errors; localization stays
in the frontend.
