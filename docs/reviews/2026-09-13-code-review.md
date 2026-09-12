# Full code review — 2026-09-13

Reviewed baseline: `4670cfb` (main), approximately 18,000 lines of application
source and tests. The review covered all Rust subsystems, Vue pages/components,
composables, persistence, Tauri configuration, build scripts and the three
numbered specs. Independent passes checked repository standards and specification
behavior. Existing local agent-configuration edits and generated binding changes
are excluded from the fixes.

## Confirmed findings and corrections

| Priority | Trigger and previous behavior | Correction |
| --- | --- | --- |
| P1 | Importing an already managed GGUF onto itself truncated the model; interrupted replacement exposed partial final files. | Copy to a unique staging file and rename only after success. |
| P1 | Switching vaults during a download/import registered the model in the new vault. | Capture the original database before any await and pass it through registration. |
| P1 | A second send could overwrite abort handles while a tool round was running; vault switches retained the old credential-bearing adapter. | Reserve the entire turn/model-load/vault-transition operation; reject overlapping fresh work and clear the session on successful vault transitions. Idempotent replays remain available. |
| P1 | Genesis reported success when its pending marker could not be removed; startup later deleted the supposedly healthy vault. | Remove the marker before publishing the active vault and propagate failure. |
| P2 | The initial HTTP request bypassed retry handling and could not be stopped while awaiting response headers. | Reuse cancellable stream-start/retry logic for every step, register cancellation before the first await, and carry the same retry budget into the first stream. |
| P2 | Fast tool rounds or a backwards clock put child messages before their parents. | Enforce a timestamp later than the persisted parent at the common message-insertion boundary. |
| P2 | A failed thread update left a final assistant message committed despite reporting failure. | Commit the final message and thread update in one SQLite transaction. |
| P2 | Late permission replies after cancellation were rejected as unknown requests. | Retain cancellation tombstones; late replies are harmless while truly unknown IDs remain errors. |
| P2 | Newly created chats were missing from the sidebar; switching threads lost visible streaming tokens. | Refresh thread metadata after accepted sends and route stream events by their generating thread. |
| P2 | Multi-round answers appeared concatenated before their tool rows; completed IPC replays could leave the composer busy forever. | Reconcile against persisted history on turn completion and recognize terminal messages on idempotent replay. |
| P2 | Permission dialogs obscured Stop, cancelled requests remained visible, and cancelled answers had no distinct status. | Add Stop to the dialog and initial-send state, clear completed approvals, and translate the cancelled status in both locales. |
| P2 | Leaving a running chat stranded its approval wait; late listener registration survived unmount. | Cancel owned work on unmount and dispose subscriptions that complete after unmount. |
| P2 | A failed permission-mode save left the UI displaying a mode that was never persisted. | Serialize saves and restore the previously persisted selection on failure. |
| P2 | Closing/reopening onboarding drawers reset their in-flight submission flag; unlock completion could emit a subsequently selected vault name. | Keep submission ownership until completion and use the backend's returned vault name. |
| P2 | The documentation checker scanned installed dependencies and generated artifacts. | Select tracked and non-ignored repository Markdown through Git, preserving required-file checks. |

## Verification

Regression coverage includes real SQLCipher databases, injected persistence
failure, backwards timestamps, import self-replacement, two-vault registration,
stream-start retry exhaustion, cancellation before response headers, late
permission replies, and operation ownership until turn completion. Frontend
checks replay the real Vue script setup with IPC replaced at its boundary.
The documentation regression runs the real checker in a temporary Git repository.

Commands used:

```sh
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --jobs 2
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --jobs 2
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --jobs 2 -- -D warnings
pnpm typecheck
pnpm generate
node scripts/check-chat-state.mjs
python3 scripts/ci/test_check_docs.py
python3 scripts/ci/check-docs.py
```

Results: 130 Rust tests passed with default CPU features (2 GGUF tests ignored),
129 passed without default features, 11 frontend replay tests passed, and the
documentation regression passed. Rustfmt, Clippy with warnings denied, Nuxt
typecheck, static generation, documentation validation and patch whitespace
checks all passed.

The tests requiring an operator-provided GGUF remain ignored. CUDA/Metal hardware
and a native interactive WebView were not exercised. The installed Node version
22.17 emits the repository's engine warning (minimum supported 22.19); frontend
checks still run. No new dependencies were added.

## Maintainability exceptions and follow-up split plan

The following existing files exceed the Spaex 500-line review boundary. They
remain intact in this behavior-fix PR to keep ownership and event-order changes
reviewable against their regression coverage. These are explicit temporary
exceptions, not a claim that size alone justifies splitting functions.

- `src-tauri/src/chat/commands.rs`: lifecycle, send admission, turn execution and
  permission handling are coupled today. A separate mechanical PR should first
  move model lifecycle/resolution, then send persistence, then turn execution
  into owning modules, preserving the registered command surface and tests.
- `src/pages/chat/[instance].vue`: separate sidebar/model-management UI from the
  chat transcript and composer; move event/session state together into one
  composable after preserving the executable replay tests.
- `src-tauri/tests/chat_tool_loop.rs`: split approval/cancellation cases from
  retry cases, retaining one small fixture module for their real database and
  scripted adapter boundaries.
- `src-tauri/src/adapters/anthropic.rs` and `anthropic_tests.rs`: separate SSE
  decoding and its malformed-stream tests from HTTP request construction and
  pagination tests while retaining the provider-adapter interface.
- `src-tauri/src/providers/mod.rs`: move adapter construction/legacy repair out
  of command CRUD and model-refresh orchestration into the provider boundary.

Existing Rust formatting drift is corrected in its own mechanical commit.
Deferred product features (federation and user-configurable MCP servers) are not
implemented by this review.
