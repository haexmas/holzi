# Quickstart: validating unified model capabilities

Runnable checks that prove the feature end to end. Behavior details live in [spec.md](spec.md);
shapes in [data-model.md](data-model.md) and [contracts/](contracts/).

## Prerequisites

- A dedicated worktree on the feature branch (never the primary checkout):
  `git worktree list` shows `.worktrees/012-unified-model-capabilities`.
- Run `pnpm install` for real inside the worktree — do not symlink `node_modules` from another
  checkout (Vite's file-system sandbox breaks on it).
- Dev/build commands that need GTK/WebKit go through the Nix dev shell (`direnv allow`, or
  `nix develop --command …`); `IN_NIX_SHELL` must be set or the host-bridge script silently skips
  its setup.

## 1. Automated checks (CI parity)

From the repository root:

```sh
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test  --manifest-path src-tauri/Cargo.toml          # incl. new capability, storage, request, backfill tests
pnpm lint:rust                                            # clippy, both feature sets, -D warnings
pnpm check:chat-state                                     # replay harness incl. new effort/capability cases
pnpm check:templates
pnpm typecheck && pnpm typecheck:scripts
pnpm lint && pnpm format:check
```

Expected: all green. `cargo build --tests` is the fastest way to enumerate any remaining
`ChatRequest` / `ModelRow` / `ProviderModel` literal that still needs the new fields.

Automated coverage map:

| Spec item               | Where it is proven                                                                                                                                                                                                         |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| FR-002, FR-003, SC-001  | `adapters/anthropic_tests.rs` wire-fixture cases 1–7 (contracts/capabilities-json.md)                                                                                                                                      |
| FR-004, FR-013, FR-014  | `model_capabilities_tests.rs`, `adapters/request_tests.rs` (valid option serialized; stale option omitted)                                                                                                                 |
| FR-006, FR-007, SC-007  | `models/commands_tests.rs` (registration + backfill), `cli_delegate/mod_tests.rs` (Codex model is `None`, not unsupported)                                                                                                 |
| FR-008                  | `chat/commands_tests.rs` (`inspect_attachment` kind in / not in / undetermined)                                                                                                                                            |
| FR-009, FR-010, FR-021  | `storage/models_tests.rs` (round-trip, legacy `NULL`, malformed JSON → `None` + warning, overwrite on refresh, failed refresh keeps row)                                                                                   |
| FR-015 … FR-018, SC-003 | `scripts/check-chat-state.ts`: restore per model; compatible switch keeps own saved option; removed option → Auto + key cleared; out-of-order read cannot overwrite; failed write rolls back; per-connection key isolation |
| SC-005                  | harness asserts no extra capability `invoke` on model switch                                                                                                                                                               |
| FR-019                  | `cargo build` fails if any deleted symbol is still referenced                                                                                                                                                              |

## 2. Manual smoke (real app)

Start the app inside the dev shell: `pnpm tauri:dev`. Use a vault that has (a) a Claude Code
delegate connected **before** this build, (b) at least one local reasoning model, (c) ideally an
API-key Anthropic provider.

**Story 3 first — pre-existing provider is honest.**

1. Open the chat with a Claude model of the pre-existing delegate. Expect: effort control shown
   _disabled_ with the "not yet known — refresh in Settings" label; attachments are declined with
   the "not yet known" reason; no reasoning output is requested.
2. Settings → connected Claude provider → **Refresh models**. Expect progress, then success.
3. Back in chat. Expect: the effort control lists exactly the levels that model supports (a model
   without `xhigh`/`max` shows fewer than five), attachments accept images/PDF per the model.

**Story 1 — composer matches the model.**

4. Switch among Claude models with different supported levels; options change with each.
5. Select a local reasoning model. Expect: disabled control labelled "managed by the model" (the
   disclosed change); reasoning output still streams as before.
6. Select a local non-reasoning model. Expect: no effort control.
7. Select a Codex model (if connected). Expect: "not yet known" state, never "unsupported".

**Story 2 — per-model memory.**

8. Set High on model A and a different level on model B. Switch A ↔ B, then restart the app.
   Expect each model restores its own choice.
9. Choose Auto on model A, restart. Expect Auto.
10. (Removed option) After changing a stored preference to a level the provider no longer offers
    (or refreshing after a provider change), reopen the model. Expect Auto, and the stale key gone.
11. Send a message with High selected on a delegate model; confirm the request used that level
    (delegate `--effort` receives the option id unchanged).

## 3. Record-shape check (one-off)

Capture one live `GET /v1/models` response with an API key **and one with the OAuth bearer the Claude Code delegate uses** (both redacted) and confirm each model's
`capabilities` includes `pdf_input` and the `thinking`/`effort` subtrees the mapping relies on
(research R3, open verification). Add it as a fixture beside the wiremock cases; if `pdf_input` is
absent, follow the per-kind fallback recorded in R3 before merging. If the OAuth response lacks `capabilities`, try `GET /v1/models/{id}` with the same bearer; if neither returns them, amend the spec (FR-003) instead of reintroducing a static table.
