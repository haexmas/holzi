# holzi

A portable, isolated personal-agent app. Runs as a Tauri application on any device.

- All LLM credentials and model configuration live inside holzi. Opens on any host, works even if the host has no LLM installed.
- Nostr relay endpoint plus iroh peer in one process, forming a closed federation with the operator's other holzi installations.
- Exposes an MCP server for external clients. Standard MCP auth applies.
- Local-first, user-chosen models. No automatic model routing.

## Design

See [docs/design/founding.md](docs/design/founding.md) for the founding architecture.

## Status

Draft. No code yet. Founding design captured 2026-09-04.
