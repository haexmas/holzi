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

## ChildRegistry (new)

Owned by `VaultGate`. Holds the process id of every child process started for the vault (tool shells,
delegated CLIs, MCP servers), each one its own process group. `register(pid)` returns a guard that
removes the entry on drop. `kill_all()` kills every registered group (Unix `kill(-pid, SIGKILL)`,
Windows `taskkill /T /F`) and makes any later registration kill at once. The drain ladder calls it
at its end and the forced end calls it right before the process ends, so no child outlives the vault
session (FR-003).

## ProcessPresence (new)

Managed state holding the open handle of `<app local data>/presence.lock`, opened for read and write
because Windows refuses locks on append-only handles. `announce(dir, on_alone)`:

1. tries an exclusive `File::try_lock`;
2. when that succeeds, runs `on_alone` (the startup cleanup) while it holds the lock, then calls
   `unlock` and only then `lock_shared`;
3. when it fails, calls `lock_shared` (which waits for a starter that is cleaning up) and skips
   `on_alone`;
4. keeps the shared lock for the life of the process.

A handle that already holds a lock is never locked again, because `std` leaves that unspecified and
possibly deadlocking. There is no atomic exclusive-to-shared downgrade, and none is needed: **no
process starts work before it holds its shared lock**. Whoever sits between `unlock` and
`lock_shared` has nothing to lose, and whoever has work holds a shared lock, which makes every other
exclusive attempt fail. The OS drops the lock when the process ends or crashes, so a crashed process
never blocks the cleanup for later starts. It holds no vault data and is independent of the vault
gate.

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
