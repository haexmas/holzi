# Quickstart: Verify Autonomous Delegate Mode manually

Manual verification steps for reviewers/operators before merge. Requires a connected Claude Code
and/or Codex delegate backend (007-cli-delegate) already working — this feature has no value to
verify without that prerequisite in place.

## Preparation

```bash
git switch 009-autonomous-delegate-mode
which claude codex             # at least one required; both preferred for full coverage
claude --version                # live-verified this planning session: 2.1.274
codex --version                  # live-verified this planning session: codex-cli 0.147.0
cd src-tauri && cargo test --lib && cd ..
pnpm typecheck
```

## Already-completed verifications (do not re-derive, treat as regression expectations)

Live-verified during the 2026-09-17 design session
(`docs/plans/2026-09-17-autonomous-delegate-mode-design.md`) and this planning session — see
research.md for the full evidence trail:

1. **Host isolation holds under both vendors' ungated mechanism.** A fake `$HOME` carrying a
   canary `SessionStart` hook (Claude) / a fake `$HOME/.codex/AGENTS.md` (Codex) did not leak into
   an authenticated, isolated session run under `--permission-mode bypassPermissions` /
   `approvalPolicy: "never"` respectively.
2. **Subagent tool calls route through the same approval bridge as top-level calls** — confirmed by
   comparing an identical mutating command (a file write) run at the top level versus inside a
   Task-tool-spawned subagent; both were captured by the same `--permission-prompt-tool` bridge.
3. **Codex's approval posture is a `thread/start` JSON-RPC field (`approvalPolicy`), not a CLI
   flag** — confirmed against the installed `app-server`'s own generated schema. Do not reintroduce
   `-a`/`-s` CLI flags for the `app-server` transport this feature (and 007) uses.

## Scenario 1: Complete a task without per-action approval (User Story 1)

1. Select a connected delegate backend for a new message.
2. Pick `ungated` autonomy mode from the new composer control.
3. Send a request that needs several tool calls to complete (e.g. "list the files in this
   directory, then read the first one and summarize it").
4. **Expect**: the response completes with no approval prompt at any point, the same as if you had
   run the CLI yourself in a terminal with its own bypass flag.
5. Repeat with `gated-permissive` selected instead (no deny rules configured yet).
6. **Expect**: same outcome — no approval prompt — but see Scenario 2 for what should now be
   recorded.
7. Repeat once more with no autonomy mode selected (today's default).
8. **Expect**: the existing live per-tool-call approval prompts appear exactly as before this
   feature existed — no regression.

## Scenario 2: Review what an unsupervised run did (User Story 2)

1. Run a `gated-permissive` request that performs at least two distinct tool calls.
2. Open the conversation history for that turn afterward.
3. **Expect**: every tool call the delegate made is present as its own record, and the turn is
   labeled with the autonomy mode it ran under.
4. Run an `ungated` request instead.
5. **Expect**: only the final assistant response is present — no per-tool-call rows. This is the
   documented tradeoff of `ungated` (spec US2 scenario 2), not a bug.

## Scenario 3: A deny rule holds even under otherwise-full permissiveness (User Story 3)

1. In settings, enable the `network_access` deny rule for delegate connections.
2. Send a `gated-permissive` request that would need network access (e.g. "fetch
   https://example.com and summarize it") using **Codex** first.
3. **Expect**: the network action is blocked; the delegate is told it wasn't available; the turn
   completes rather than crashing.
4. Repeat with **Claude Code**.
5. **Expect**: same outcome via the heuristic tool-name/command-text match (data-model.md) — note in
   the review whether the heuristic held for the exact prompt used, since it is not as structurally
   reliable as Codex's `networkApprovalContext` field (research.md §3).
6. Enable `workspace_escape` instead, and with **Codex specifically**, trigger a _file write_
   (not a command execution) outside the workspace root.
7. **Expect** (spec FR-015, the known asymmetry): the write is denied — not because it was matched
   against the workspace boundary, but because Codex's file-change approval payload cannot be
   evaluated at all, so the fail-safe default applies. Confirm this is what actually happens; if it
   is instead silently allowed, that is a regression against FR-015, not an acceptable gap.

## Scenario 4: Autonomy never carries over (User Story 4)

1. Send one request with `ungated` selected.
2. Send a second, unrelated delegate-backed request without touching the autonomy control again.
3. **Expect**: the second request is prompted for approval exactly as the standard mode would —
   the control should visibly show "standard" again, not still show "ungated."

## Scenario 5: Stop an autonomous run (User Story 5)

1. Start a `gated-permissive` or `ungated` request doing something slow/observable (e.g. a
   multi-file task).
2. Stop it mid-run from the UI.
3. **Expect**: the underlying delegate process actually terminates (check with `ps`/`pgrep
claude`/`pgrep codex` that no orphaned process remains) within the same short window stopping
   already takes for `local`/`api_key` backends.
