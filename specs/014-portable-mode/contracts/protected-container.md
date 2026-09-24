# Contract: Protected Container

**Feature**: 014-portable-mode

This contract is conditional on the protected-model feasibility gate.

## Security invariants

- No readable vault, model, setting, imported file, cache, or identifying name is
  written outside the container in single-file mode.
- A wrong passphrase fails before returning vault names, metadata, or plaintext.
- The session key exists only after unlock and is released during close/failure.
- Writes are authenticated and atomic from the app's point of view.
- Interrupted writes leave either the previous valid state or a recoverable
  protected state, never a readable partial file.

## Model boundary

The container adapter must expose the local-model loader a supported access form
without writing a readable model to the host filesystem. If the existing loader
cannot consume that form, the adapter must return `ProtectionUnavailable`; it
must not silently stage plaintext.

## User operations

The adapter supports create, open, unlock, list protected entries, read/write
entry streams, commit, close, and delete. It also reports whether enough free
space exists before creation and whether a requested operation would exceed the
available budget.

The final format and library choice are recorded with the feasibility evidence;
this document is not permission to implement an unverified cryptographic format.
