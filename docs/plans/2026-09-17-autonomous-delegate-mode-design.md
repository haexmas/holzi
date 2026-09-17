# Autonomous Delegate Mode — Full-Autonomy Extension to CLI Delegate

**Status**: Deferred design capture. Written 2026-09-17 from a session that started by comparing
holzi's architecture to OpenClaw 2.0's ACP/CLI-backend model. Not scoped for implementation.
Extends shipped `specs/007-cli-delegate/` rather than revising it — every acceptance scenario and
functional requirement in that spec continues to hold exactly as shipped unless Autonomous Mode is
explicitly selected.

**Relationship to existing documents**:

- [`specs/007-cli-delegate/spec.md`](../../specs/007-cli-delegate/spec.md) — shipped (merged via PR
  #70, #72, and follow-up fixes). Defines Delegate Backend / Delegate Credential / Delegate
  Invocation and FR-006: every delegate tool call gets the same live, per-tool-call approval as the
  built-in tool loop, with no backend-specific exception. That FR, and the "subagent-style full
  autonomous delegation" exclusion in its Out-of-Scope list, is exactly what this document extends.
- [`2026-09-11-agent-tool-loop-design.md`](./2026-09-11-agent-tool-loop-design.md) §8.1 — rejected
  ACP for Claude Code specifically because of a quoted Agent-SDK ToS clause forbidding third-party
  products from offering claude.ai login or rate limits without prior approval. §9 named
  "Subagent-style full delegation" as a rejected default that "could resurface later as an explicitly
  separate, clearly-labeled feature." This document is that feature.
- OpenClaw 2.0's ACP/CLI-backend split (external research, no repo relationship) was the comparison
  point that surfaced the question this document answers. ACP is not reused here — the same
  Agent-SDK ToS rejection in §8.1 applies to any ACP-based path, autonomous or not, so both variants
  below stay inside the already-accepted raw-CLI approach.

**Sequencing decision**: 007-cli-delegate's shipped default (live per-tool-call approval) is
unchanged and remains the default for every delegate invocation. Autonomous Mode is a second,
explicit opt-in, not a replacement.

---

## 1. What this adds

A second invocation mode for delegate backends (Claude Code, Codex), alongside the shipped default.
In Autonomous Mode, native tool calls the delegate makes proceed without holzi pausing them for
human approval, instead of holzi's live per-tool-call gate. Chosen explicitly per connection or per
request by the operator — never a silent global default — mirroring how `chat.permission_mode` is
already explicit and device-scoped rather than inherited.

Framing: this is not a revision to the Manual/Auto/Plan posture used everywhere else (built-in
tools, local, api_key). Those keep behaving exactly as shipped. Autonomous Mode is delegate-specific
and orthogonal — a choice available only when the selected backend is a connected delegate backend.

## 2. Two variants, matching the operator's "optional gateway" framing

Operator statement that drove this design: agents should keep their autonomy; holzi routing calls
through itself is acceptable only if it is useful for testing specific permissions, otherwise skip
it entirely. Two variants follow directly from that, both operator-selectable per connection or per
request:

- **Variant A — Ungated.** Spawn the delegate CLI with its own native full-autonomy flag. No
  `--permission-prompt-tool` bridge, no `app-server` approval subscription. holzi captures only the
  delegate's final output/transcript. Zero added latency, zero added interception code path — and
  zero per-tool-call visibility beyond whatever the delegate's own transcript happens to expose.
  Confirmed 2026-09-17 against installed CLIs (Claude Code v2.1.274, Codex v0.147.0):
  - **Claude Code**: `--permission-mode bypassPermissions` (also `--dangerously-skip-permissions`,
    not separately tested). No independent OS-level sandbox flag was found in `--help` — under this
    variant, whatever runs is not confined by anything holzi controls beyond the process's own `cwd`
    and env.
  - **Codex** has a richer axis than a single bypass flag: approval policy
    (`-a/--ask-for-approval untrusted|on-request|never`) is independent from sandboxing
    (`-s/--sandbox read-only|workspace-write|danger-full-access`), plus an explicit workspace root
    (`-C/--cd <DIR>`, `--add-dir <DIR>`). `--dangerously-bypass-approvals-and-sandbox` collapses both
    axes to "no approval, no sandbox" in one flag — Codex's own docs call it "EXTREMELY DANGEROUS."
    **`-a never -s workspace-write -C <root>` is very likely the better default for Codex's Variant
    A**: identical "never asks" behavior, but OS-level sandboxing still confines writes/exec to the
    given root instead of relying on holzi to enforce that itself. Not yet exercised end-to-end
    (confirmed from `--help` semantics, not from a live constrained run) — a cheap follow-up spike,
    not a re-opened unknown given `-s`/`-C` are simple, independently documented flags.
- **Variant B — Gated-but-permissive.** Reuse the exact bridge 007-cli-delegate already built —
  Claude Code's `--permission-prompt-tool` MCP server, Codex's `app-server` approval subscription —
  but swap the decision function. Instead of parking on a `oneshot` and waiting for a human (shipped
  default), the bridge auto-responds "allow" immediately for everything **except** an
  operator-configured deny/notify list (e.g. writes outside the invocation's workspace root, network
  egress, credential-file paths), which either hard-blocks or fires a non-blocking notification.
  Same per-call round-trip cost the shipped feature already pays, but for that cost: a full persisted
  tool-call log for free, reusing the exact `ChatMessage::ToolCall`/`ToolResult` rows and
  `tool_source` discrimination the built-in tool loop already writes
  (`2026-09-11-agent-tool-loop-design.md` §3) — no new data-model work.

**Why keep both instead of picking one.** Variant A is "ansonsten auch ohne" exactly — zero
overhead, as close to running the CLI yourself in a terminal as holzi can get. Variant B is "um
bestimmte Berechtigungen zu testen, falls das sinnvoll wäre" — the same practical autonomy (nothing
blocks unless it hits the deny list) plus an audit trail and a narrow safety net, at the cost of
reusing infrastructure that adds one round-trip per tool call. Recommend B as Autonomous Mode's
default, A as an explicit further escalation — a product call, not settled here.

## 3. What does not change

- **Host isolation is non-negotiable and orthogonal to "ungated."** "Autonomous" describes only the
  tool-call-*approval* axis (§2): whether holzi pauses a call for a human decision. It says nothing
  about the tool-call-*discovery* axis, which stays exactly as 007-cli-delegate shipped it in both
  variants, including Variant A: every invocation still gets `CLAUDE_CONFIG_DIR`/`CODEX_HOME` pointed
  at an empty, disposable directory and a disposable `cwd`, so the delegate sees only holzi's own
  instance — holzi's own `CLAUDE.md`/`AGENTS.md`-equivalent context, injected explicitly (spec 007
  FR-009/FR-010) — and never the host's own global config, hooks, plugins, or project files. Removing
  the approval bridge (Variant A) must never be implemented as "drop the isolation wrapper too" — the
  two are independent flags on the spawn, not one on/off switch, and conflating them would silently
  reopen Story 4 of 007-cli-delegate, which is already shipped and tested.
- **Mobile exclusion** (spec 007's rationale: subprocess spawning is categorically unavailable under
  iOS/Android sandboxing) applies identically. Neither variant opens a mobile path.
- **Vault-portable credentials** are reused as-is. Autonomous Mode only changes what happens to
  tool-call interception, not credential handling.
- **The accepted ToS risk posture** from design doc §8.1 (raw CLI, not Agent SDK) is unchanged and
  covers both variants equally — Variant A is not meaningfully different in ToS exposure from
  Variant B; both drive the same raw CLI, only the flags/wiring differ.
- **ACP stays rejected**, for the same reason as before. Variant A uses the CLI's own native bypass
  flags, not an ACP client/agent relationship — it is not a backdoor reintroduction of ACP.

## 4. Open design questions

1. **Workspace confinement.** Partially answered 2026-09-17 for Codex, still open for Claude Code.
   Codex's `-s/--sandbox {read-only,workspace-write,danger-full-access}` plus `-C/--cd`/`--add-dir`
   give an OS-level confinement primitive independent of holzi's own logic — Variant A for Codex can
   keep `workspace-write` sandboxing while still never prompting (`-a never`), so confinement does
   not have to be holzi's problem alone. No equivalent flag surfaced in Claude Code's `--help` — for
   Claude Code, Variant A still has no interception point and no OS-level fallback, so a filesystem
   escape there is bounded only by whatever `cwd`/`--add-dir`-equivalent (none found) or OS-level
   sandboxing (e.g. a container) holzi wraps around the process itself. Variant B *could* enforce a
   workspace-root check inside its deny-list logic for either vendor, since it sees every write/exec
   call before approving. This gap is the strongest argument for defaulting Claude Code specifically
   to B rather than A.
2. **Resource/circuit-breaker — dropped 2026-09-17, operator decision.** Two proposals were floated
   (a flat wall-clock ceiling, then idle-timeout plus a cost ceiling) and both rejected: no concrete
   problem motivates either yet, and the operator has run legitimate coding sessions well over an
   hour, which the first proposal would have cut short for no real benefit. The second proposal's
   cost-ceiling half was also already flagged as unverified and non-trivial (holzi would need to
   accumulate cost itself from token usage against per-model pricing — real, ongoing maintenance
   burden, not a one-time cost). Building either against a hypothetical failure mode contradicts
   Simplicity First, and 007-cli-delegate itself already shipped with no time/cost/call-count ceiling
   at all — this document does not add one for Autonomous Mode either. Revisit only once a concrete
   incident or observed problem motivates a specific mechanism, not preemptively.
3. **Subagent visibility — resolved 2026-09-17, subagent calls do route through the bridge.**
   Tested against Claude Code v2.1.274 with a purpose-built stdio MCP server standing in for
   `permission_mcp_server.rs`/`approval_bridge.rs` (same protocol, logs every call, always allows).
   Methodology used a positive control to avoid a false negative: an initial attempt with the
   top-level agent told to spawn a subagent running a bare `echo` command logged *zero* approval
   calls — but a control run putting that same bare `echo` directly at the top level (no subagent at
   all) *also* logged zero calls, showing the command itself was never gated under
   `--permission-mode default`, independent of subagent involvement. Re-run with a mutating command
   (`echo hello > file.txt`, which the earlier control confirmed *does* trigger approval at the top
   level) routed through a subagent instead: the bridge received exactly one `Bash` call carrying
   that command, `subagent_stats` confirmed one subagent spawned and completed, and the file was
   written only after the bridge's "allow" response — i.e. the subagent's own mutating action
   genuinely paused on and was gated by the same external approval tool as a top-level call. Variant
   B's audit-trail/deny-list value proposition holds for at least one level of subagent nesting
   (`max_depth: 1` in the observed run). Not tested: whether a subagent that itself spawns a further
   subagent (depth 2+) still routes through the same bridge — plausible given the mechanism is a
   session-wide CLI flag rather than a per-agent setting, but not empirically confirmed.
4. **Cross-device visibility of what an autonomous run did.** Files an agent changes on disk exist
   only on that one device (same "files never live in the CRDT-synced database" principle as
   [`2026-09-07-cross-user-sharing-deferred-design.md`](./2026-09-07-cross-user-sharing-deferred-design.md)
   §5). With Variant B, the turn/tool-call transcript is an ordinary `chat_messages` row set and
   therefore syncs per-vault like any other conversation — other paired devices see *that* an
   autonomous run happened and roughly what it did, even without the changed files themselves. With
   Variant A, only the delegate's final-output text (the turn's assistant message) syncs — a
   summary, not a tool-by-tool record.
5. **Exact CLI flags for Variant A — resolved 2026-09-17.** Flag names and the isolation regression
   check are both confirmed against installed CLIs (Claude Code v2.1.274, Codex v0.147.0); see §2.
   Isolation test method: a fake `$HOME` (Claude) / fake `$HOME/.codex` (Codex) carrying a canary —
   a `SessionStart` hook writing a sentinel file for Claude, an `AGENTS.md` instructing the model to
   reply with a fixed marker string for Codex — while `CLAUDE_CONFIG_DIR`/`CODEX_HOME` pointed at a
   separate, genuinely isolated directory seeded with a short-lived copy of the real OAuth
   credential (deleted immediately after each run). Both runs authenticated successfully (proving
   the session genuinely initialized, not merely failing closed before reaching the canary) and
   neither canary fired: no sentinel file, and Codex replied with the requested plain "OK" rather
   than the canary marker. **Isolation holds under both vendors' full-autonomy flags** —
   `--permission-mode bypassPermissions` did not reintroduce host-`$HOME` discovery, and an explicit
   `CODEX_HOME` override was not bypassed by the `$HOME/.codex` default fallback. This was
   specifically the risk flagged by the operator mid-design ("holzi darf nicht direkt den
   Host-Claude/Codex nutzen, sondern seine eigene Instanz/CLAUDE/AGENTS.md") — confirmed closed, not
   just assumed. Not covered by this spike: whether *skills* or *plugins* discovery (as opposed to
   settings/hooks/AGENTS.md) behaves the same way under the bypass flags — 007's own research only
   checked this for its default (gated) invocation shape, not for either Variant A flag combination.
6. **Where this posture lives in the UI/data model — proposed resolution, needs operator sign-off.**
   Recommend **per-request, not a persisted device preference**. Spec 007's `Delegate Invocation` is
   already a per-request entity (backend choice itself is made per request, FR-004) — the autonomy
   mode fits naturally as a second selector shown only when a delegate backend is chosen, defaulting
   every time to `Gated` (007's shipped behavior, unchanged). Deliberately *not* modeled as a
   `chat.permission_mode`-style sticky device preference: that pattern is right for a general risk
   posture the operator sets once and forgets, but wrong for "let this agent run unsupervised" — a
   silently-persisted autonomous default is the failure mode most worth avoiding here (operator turns
   it on for one task, forgets, a later unrelated request runs autonomously unexpectedly). The chosen
   mode should persist on that turn's own record (alongside the existing backend-identity metadata,
   FR-005) so conversation history shows which mode a given delegate turn actually ran under — no new
   storage mechanism, reuses the same place backend attribution already lives.

## 5. What this explicitly does not do

- Does not change 007-cli-delegate's shipped default in any way.
- Does not revisit the ACP rejection — both variants stay inside the already-accepted raw-CLI
  approach.
- Does not extend to local/api_key backends or the built-in host-CLI tool. This document is scoped
  to delegate backends only; loosening the built-in tool loop's own approval model is a separate,
  unrelated question this document takes no position on.
