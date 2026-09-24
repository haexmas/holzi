# Data Model: Portable Mode with Protected Data

**Feature**: 014-portable-mode

## Process-only entities

### `PortableMode`

| Field | Rule |
|---|---|
| `kind` | `removable` or `single_file`; selected before instance discovery |
| `root` | validated absolute path owned by the mode; never serialized with secrets |
| `descriptor` | non-secret launch configuration held in process state and passed through feature-013 relaunch; not a third persisted file |
| `protection` | `protected`, `weaker_removable`, or `unavailable`; single-file cannot use the weaker state |

### `PortableSession`

| Field | Rule |
|---|---|
| `mode` | immutable for the process lifetime |
| `session_key` | process-memory only; derived after unlock; never logged or persisted |
| `mounted_root` | validated writable/readable root; loss ends the session without fallback |
| `opened_at` | diagnostics only; must not include passphrases or key material |

## Persistent container entities

### `ProtectedContainer`

The single-file form stores all app-owned persistent data in one opaque item.
The format is intentionally unspecified until the protected-model feasibility
spike chooses an audited implementation. The final contract must define:

- version and magic values that do not identify the product or contained vaults;
- authenticated encryption and key derivation parameters;
- atomic commit and recovery behavior after interruption;
- free-space estimation and maximum growth behavior;
- wrong-key, corruption, and partial-write failure semantics;
- deletion behavior and its documented filesystem-recovery limitation.

## Data-root ownership

The root owns vault files, model bytes, imported/downloaded files, settings and
preferences, caches, diagnostic output, webview state, and helper-process
configuration. User-requested writes to an explicit host path are not owned by
the root and remain outside the guarantee.

## State transitions

```text
unknown → detected → validated → ready → unlocked → closing → released
             └──────→ unavailable (no app-owned writes)
unlocked → storage-lost (safe close, no host fallback)
```

`released` means the session key and active protected handles are gone. A failed
transition must not expose partial plaintext.
