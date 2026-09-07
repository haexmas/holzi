# holzi

A portable, isolated personal-agent app. Runs as a Tauri application on any device the operator owns (desktop, server, iOS, Android).

- All LLM credentials and model configuration live inside holzi. Opens on any host, works even if the host has no LLM installed.
- Nostr relay endpoint plus iroh peer in one process, forming a closed federation with the operator's other holzi installations.
- Exposes an MCP server for external clients. Standard MCP auth applies.
- Local-first, user-chosen models. No automatic model routing.
- Mobile participates as a full peer while foreground; unreachable while backgrounded is a normal presence state, not an error.

## Design

The design corpus lives under [docs/](docs/) and the numbered specs under [specs/](specs/). Founding architecture first, then v1 scope revisions on top of it.

- [docs/design/founding.md](docs/design/founding.md) — founding architecture. Partially superseded by the v1 scope document below (see its front matter for the pointers).
- [docs/plans/2026-09-04-v1-scope-design.md](docs/plans/2026-09-04-v1-scope-design.md) — draws the v1 line and revises identity/pairing, storage, and mobile. Normative for v1 where it differs from founding.
- [docs/plans/2026-09-04-haex-crdt-extraction-plan.md](docs/plans/2026-09-04-haex-crdt-extraction-plan.md) — extraction of the SQLite + CRDT-sync layer into the standalone `haex-crdt` crate (shipped as v0.1.0; holzi consumes it as a Rust dependency).
- [docs/plans/2026-09-07-cross-user-sharing-deferred-design.md](docs/plans/2026-09-07-cross-user-sharing-deferred-design.md) — deferred design for cross-user shared spaces. Not v1; captured so closed-federation work does not foreclose it.
- [specs/001-frontend-onboarding/](specs/001-frontend-onboarding/) — first numbered spec (Landing / Anlegen / Öffnen / Verbinden / Unlock).

## Status

Draft. No implementation code yet. Speckit adopted 2026-09-04; spec 001 merged. Identity model settled on per-instance keys stored inside the encrypted SQLite (no federation-root, no paper-seed). External `haex-crdt` crate provides the SQLite + CRDT-sync foundation.

## Development setup

The repository uses the Claude speckit integration. After a fresh clone,
restore the ignored local `/speckit.*` entrypoints with:

```bash
specify init --here --ai claude --offline
```
