# Data Model: Vault Lifecycle Isolation

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Research**: [research.md](research.md)

Nothing here is persisted. All entities are runtime state of one app process, plus two argument
types that carry the passphrase. There is no schema change and no migration.

## VaultGate (managed state, one per app process)

The single owner of "may requests be answered, and what is still running".

| Field    | Type                      | Meaning                                                                               |
| -------- | ------------------------- | ------------------------------------------------------------------------------------- |
| `phase`  | `VaultPhase`              | Where the process is in its one-session life. Guarded by one short-held std `Mutex`.  |
| `cancel` | `CancellationToken`       | Fired once when the close starts. Children are handed to tasks that need to clean up. |
| `tasks`  | `TaskTracker`             | Counts session-scoped tasks, blocking work and every live `VaultDb`. Closed on close. |
| `aborts` | `Mutex<Vec<AbortHandle>>` | Abort handles of registered async tasks, used by the second rung of the drain.        |
| `policy` | `ClosePolicy`             | What ends the process: relaunch or exit. Fixed per build (see research R2).           |

Validation rules:

- `phase` only moves forward (`Idle` → `Active` → `Closing`), and `Idle` → `Closing` is allowed
  (closing before any vault is open, for example a window close on the unlock screen).
- `cancel` fires at most once, on the first transition into `Closing`.
- A second close request changes nothing and reports that a close is already running (FR-002).

## VaultPhase

```text
        begin_session()                request_close()
Idle ───────────────────► Active ─────────────────────────► Closing ──► (process ends)
  │                                                            ▲
  └────────────────────────── request_close() ─────────────────┘
```

| Phase     | Answers requests                                  | `open`/`create`                     |
| --------- | ------------------------------------------------- | ----------------------------------- |
| `Idle`    | yes (vault commands fail with `NoActiveInstance`) | allowed, moves the gate to `Active` |
| `Active`  | yes                                               | refused with `VaultAlreadyActive`   |
| `Closing` | only the app-scoped allow-list                    | refused with `VaultClosed`          |

`begin_session()` is called by `open_instance`/`create_instance` only after the database has opened.
A failed open leaves the gate in `Idle`, so the user can retry a wrong passphrase.

## ClosePolicy

`Relaunch` (default in release builds) or `Exit`. Chosen by one function; debug builds return `Exit`
until the relaunch under the dev runner is verified (research R2). Window close and quit use `Exit`
regardless, because the user asked to leave. The forced end uses the same policy: `Exit` calls
`std::process::exit(0)` and `Relaunch` calls `tauri::process::restart`, about 0.5 s after the normal
request, and only if the process is still alive.

## CloseOutcome / DrainOutcome

Returned by the background close task and used for tests and logging. They are never shown to the
user, because the process ends either way.

| Value               | Meaning                                                                      |
| ------------------- | ---------------------------------------------------------------------------- |
| `Drained`           | The tracker emptied within the cooperative window (about 1 s).               |
| `DrainedAfterAbort` | The tracker emptied after registered tasks were aborted (within about 3 s).  |
| `Stuck`             | Something un-abortable was still running at the 3 s limit. The process ends. |

## VaultDb (replaces the bare `Arc<Database>` returned by `active_database`)

| Field    | Type               | Meaning                                                              |
| -------- | ------------------ | -------------------------------------------------------------------- |
| `db`     | `Arc<Database>`    | The one connection's handle.                                         |
| `_token` | `TaskTrackerToken` | Keeps the tracker non-empty while any clone of this handle is alive. |

`Clone` (callers clone into `spawn_blocking` closures) and `Deref<Target = Database>`. When the last
clone drops, the tracker can empty and the drain can take the `Arc` out of `AppState` and drop it.

## AppState (existing, encapsulated)

`active_instance` becomes private to `state.rs`. Access goes through methods:

| Method            | Used by           | Behavior                                                        |
| ----------------- | ----------------- | --------------------------------------------------------------- |
| `database(gate)`  | `active_database` | Returns a `VaultDb` or `VaultClosed` / `NoActiveInstance`.      |
| `install(handle)` | `open`, `create`  | Publishes the opened database; only valid while gate is `Idle`. |
| `take()`          | the close task    | Removes the handle so the drain can drop it after clones end.   |

## Secret-carrying argument types

`OpenInstanceArgs` and `CreateInstanceArgs` (existing, changed).

| Field        | Before   | After               | Rule                                                                       |
| ------------ | -------- | ------------------- | -------------------------------------------------------------------------- |
| `passphrase` | `String` | `Zeroizing<String>` | Erased on drop; never cloned; excluded from `Debug` (prints `<redacted>`). |

## PublicationLock (existing helper, extended)

Returned by `acquire_model_publication_lock`. Holds the in-process mutex guard for the slug **and** an
exclusive lock on `<models>/.locks/<slug>.lock`. Both release on drop. Acquiring polls
asynchronously so a close can cancel the wait.

## ProcessPresence (new)

Managed state holding the open handle of `<app local data>/presence.lock`. `announce(dir, on_alone)`
uses a platform adapter with two explicit operations: `try_lock_exclusive` and
`downgrade_to_shared`. The adapter must convert the held exclusive lock to a shared lock in one
OS-level operation (on Unix, the `flock(LOCK_SH)` conversion; on Windows, the corresponding native
lock conversion), with no unlock/relock interval. It must not call a generic lock method twice on
the same already-locked handle, because that is unspecified and can deadlock on some platforms.

When exclusive acquisition succeeds, `announce` runs `on_alone` (the startup cleanup) while still
holding exclusive ownership, then performs the atomic downgrade and retains that handle for the
life of the process. When exclusive acquisition fails, it waits for a shared lock and skips
`on_alone`. The OS drops the lock when the process ends or crashes, so a crashed process never
blocks cleanup for later starts. It holds no vault data and is independent of the vault gate.

## Frontend: no closing state

The frontend keeps no state about a close. The lock control calls `close_instance` and nothing else.
The backend replaces the page (research R4), which discards every store and variable of the vault
session at once, and the process ends shortly after.

## Error kinds (wire shape unchanged: `{ kind, ...fields }`)

| Kind                        | New | Raised when                                                        | UI text key                        |
| --------------------------- | --- | ------------------------------------------------------------------ | ---------------------------------- |
| `VaultClosed`               | yes | A request arrives, or a long-running request is cut, after a close | `errors.vaultClosed`               |
| `VaultAlreadyActive`        | yes | `open`/`create` while a session exists                             | `errors.vaultAlreadyActive`        |
| `VaultAlreadyOpenElsewhere` | no  | Another app process holds the vault (exists; needs UI mapping)     | `errors.vaultAlreadyOpenElsewhere` |
