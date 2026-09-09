# holzi

A portable, isolated personal-agent app. Runs as a Tauri application on any device the operator owns (desktop, server, iOS, Android).

- All LLM credentials and model configuration live inside holzi. Opens on any host, works even if the host has no LLM installed.
- Nostr relay endpoint plus iroh peer in one process, forming a closed federation with the operator's other holzi installations.
- Exposes an MCP server for external clients. Standard MCP auth applies.
- Local-first, user-chosen models. No automatic model routing.
- Mobile participates as a full peer while foreground; unreachable while backgrounded is a normal presence state, not an error.

## Design

The design corpus lives under [docs/](docs/) and the numbered specs under [specs/](specs/). Founding architecture first, then v1 scope revisions on top of it.

- [plans/README.md](plans/README.md) — advisory roadmap for the first desktop MVP; the numbered specs remain normative.
- [docs/design/founding.md](docs/design/founding.md) — founding architecture. Partially superseded by the v1 scope document below (see its front matter for the pointers).
- [docs/plans/2026-09-04-v1-scope-design.md](docs/plans/2026-09-04-v1-scope-design.md) — draws the v1 line and revises identity/pairing, storage, and mobile. Normative for v1 where it differs from founding.
- [docs/plans/2026-09-04-haex-crdt-extraction-plan.md](docs/plans/2026-09-04-haex-crdt-extraction-plan.md) — extraction of the SQLite + CRDT-sync layer into the standalone `haex-crdt` crate (shipped as v0.1.0; holzi consumes it as a Rust dependency).
- [docs/plans/2026-09-07-cross-user-sharing-deferred-design.md](docs/plans/2026-09-07-cross-user-sharing-deferred-design.md) — deferred design for cross-user shared spaces. Not v1; captured so closed-federation work does not foreclose it.
- [specs/001-frontend-onboarding/](specs/001-frontend-onboarding/) — first numbered spec (Landing / Anlegen / Öffnen / Verbinden / Unlock).

## Status

Etappe 1 (App und Instanzlebenszyklus) merged 2026-09-09. Etappe 2 (Anbieter und Modellkatalog) und Etappe 3 (Nutzbarer Chat) in Arbeit. Identity model settled on per-instance keys stored inside the encrypted SQLite (no federation-root, no paper-seed). External `haex-crdt` crate provides the SQLite + CRDT-sync foundation.

## Development setup

The repository uses the Claude speckit integration. After a fresh clone,
restore the ignored local `/speckit.*` entrypoints with:

```bash
specify init --here --ai claude --offline
```

### Local inference build

The default `cargo build` enables the `llm-cpu` feature, which pulls
`mistralrs = 0.8.1` and its candle/tokio dependency tree. The first
build downloads and compiles many crates; expect several minutes on a
cold cache. Iterating on non-LLM code can skip that path with
`cargo build --no-default-features` (the `llm` module then compiles as
an empty gate).

For GPU inference, opt into one of the stacked features:

```bash
# NVIDIA GPU — requires nvcc + CUDA toolkit >= 12.0 at build time.
# The driver alone is not enough (Etappe 0 finding #3).
cargo build --features llm-cuda

# Apple GPU.
cargo build --features llm-metal
```

**First-load warning on CUDA hosts.** Under CUDA the very first model
load per host takes 30-45 s while nvcc's JIT cache in
`~/.nv/ComputeCache` warms. Subsequent loads on the same hardware are
~4 s. The chat UI shows a dedicated "GPU wird für dieses Modell
optimiert" state for that first load (Etappe 0 finding #4).

### Running the local-inference integration test

The wrapper's real-runtime test is skipped by default because it needs
a GGUF file on disk. To exercise it, point `HOLZI_TEST_GGUF` at any
GGUF and add `--ignored`:

```bash
HOLZI_TEST_GGUF=~/path/to/model.gguf \
  cargo test --manifest-path src-tauri/Cargo.toml \
  --test local_inference -- --ignored
```

`HOLZI_TEST_GGUF_TOKENIZER` overrides the tokenizer repo id (default:
`Qwen/Qwen2.5-0.5B-Instruct`).
