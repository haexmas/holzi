# Extensions reach the agent over MCP and the host over a typed bridge

Status: accepted
Date: 2026-09-21

## Decision

Holzi hosts haextensions (signed web bundles that run in a sandboxed iframe).
Traffic between an extension and holzi runs in two directions. Each direction
has its own protocol, its own code path, and the same permission model.

| Direction | Caller → callee                        | Protocol                                                      | Authorization                                                                  |
| --------- | -------------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| A         | holzi (agent) → extension              | **MCP**: holzi is the client, the extension is the server     | Tool declared in the manifest and confirmed at install; per-call approval gate |
| B         | extension → host operation (file, DB…) | **Typed request/response over the extension's `MessagePort`** | Manifest grants; anything else prompts the user                                |

**Direction A.** An extension declares the tools it offers in its manifest and
serves them over MCP at runtime. Holzi connects as an MCP client and registers
each tool in the existing `ToolRegistry` with source `haextension`. Tools are
`Risky` by default; MCP tool annotations are hints from an untrusted party and
never lower the risk class on their own. A tool call needs an open window of
the extension; holzi opens one minimized when none exists. A call invokes the
extension's functions directly and need not be visible in its UI.

**Direction B.** The extension calls typed host methods (the protocol the
haex-vault SDK already speaks). The frontend only relays JSON between the
iframe port and one Tauri command. A single Rust chokepoint identifies the
extension from its window (never from the payload), checks the permission, and
calls the host function directly. Host operations never pass through the agent.

**One permission model.**

- An extension declares every host operation it needs, and every tool it
  exposes, in its signed manifest. The user confirms both in one dialog at
  install. A manifest change that adds permissions needs a new confirmation.
- A confirmed permission passes without a prompt.
- Any other request prompts the user: allow or deny, with a checkbox to
  remember the choice. A request the manifest never declared is labeled as
  such. An explicit denial takes precedence over a grant.
- A host operation triggered while an agent tool call is running on that
  extension is checked against the _extension's_ grants, and its prompt says
  it was triggered by an agent call.

**The LLM is never reachable from an extension.** The bridge dispatches from a
deny-by-default allowlist of host methods. It offers no MCP sampling, no path
into the chat runtime, and no extension-to-extension calls. A contract test
enumerates the bridge methods and fails if any reaches chat or model code.

## Rationale

The permission model the operator wants — manifest declaration, install-time
confirmation, silent pass for confirmed grants, prompt with "remember" for the
rest — already exists in haex-vault. MCP adds nothing to it, so direction B
keeps that design instead of re-expressing it.

Using MCP for direction B would also weaken the wall around the LLM. A "host
MCP server for extensions" would sit beside the agent's tool registry, one
wiring mistake from exposing it. Separate protocols and code paths make the
wall structural rather than a convention.

Direction B keeps the haex-vault SDK protocol, so the same extensions run in
haex-vault and holzi. The SDK gains an MCP server part for direction A and
loses nothing.

MCP fits direction A: the agent already consumes MCP tools, and `tools/list`
and `tools/call` carry the schemas and results it needs. It fits direction B
poorly: file transfer, streaming, frequent database queries, and push events
are that direction's normal traffic.

## Consequences

- Specs split as: 017 extension host with the permission chokepoint
  (direction B), 018 extension tools for the agent (direction A, plus the SDK
  addition), 019 adding tools to the existing haextensions.
- The manifest schema gains a tools block. That touches `vault-sdk` and
  `haextension` as well as holzi; cross-repo references stay pinned to full
  commit SHAs.
- Holzi does not plan native-webview extension windows. The iframe plus
  port model leaves one chokepoint and needs no per-command Tauri ACL
  allowlist.
- Open for the plan of 018, not yet verified: whether `rmcp` 3.4.0's
  `(Sink, Stream)` transport needs extra feature flags in holzi, the
  extension-side `MessagePort` transport for an MCP server, and how to
  attribute a host operation to an in-flight agent call.
- Prompt handling for direction B needs the queue and duplicate-request
  handling that haex-vault's prompt code already has.

## Considered alternatives

- **MCP in both directions.** Rejected: it blurs the wall around the LLM,
  breaks compatibility with haex-vault's extensions, fits streaming and binary
  traffic poorly, and does not simplify a permission model that is already
  specified.
- **Host operations routed through the agent.** Rejected: the LLM must never be
  reachable through an extension, and host operations must be direct function
  calls.
- **Native-webview windows that call Tauri commands directly.** Rejected for
  holzi: every extension-facing command would need its own ACL entry and its
  own permission check.

## References

Pinned, for porting behavior rather than copying:

- `haex-space/haex-vault` @ `8dce379d94e18fcd42c3b73686a06f984ca3f574`:
  `src-tauri/src/extension/permissions/` (checker and manager),
  `src/composables/usePermissionPrompt.ts` (prompt queue and decisions),
  `src/composables/extensionMessageHandler.ts` (request routing).
- `haex-space/vault-sdk` @ `502593e84b8d289b0986a2777754d6bd8f52da5e`:
  `src/messages.ts` (`MessagePort` handshake), `src/client/external.ts` and
  `src/commands/ai.ts` (existing route for tool-call style requests).
