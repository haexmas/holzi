<!-- SPECKIT START -->
Aktueller Feature-Plan: [specs/003-agent-tool-loop/plan.md](specs/003-agent-tool-loop/plan.md)
Zugehörige Artefakte: [spec.md](specs/003-agent-tool-loop/spec.md), [research.md](specs/003-agent-tool-loop/research.md), [data-model.md](specs/003-agent-tool-loop/data-model.md), [contracts/tauri-commands.md](specs/003-agent-tool-loop/contracts/tauri-commands.md), [quickstart.md](specs/003-agent-tool-loop/quickstart.md)
Vorheriger Feature-Plan: [specs/002-onboarding-model-prefs/plan.md](specs/002-onboarding-model-prefs/plan.md)
Konvention und Domain-Terme: [docs/adr/0001-device-scoped-data-convention.md](docs/adr/0001-device-scoped-data-convention.md), [CONTEXT.md](CONTEXT.md)
<!-- SPECKIT END -->

## Manifest precedence

When `.haex-hive.json` and `.spaex.json` coexist, `.haex-hive.json` and its
pinned source revisions are authoritative for the effective constitution.
`.spaex.json` is retained as a Spaex-compatible projection and MUST use pins
valid for Spaex's publisher schema; the two manifests MUST NOT be reconciled by
mixing or choosing between their revisions. If their resolved content differs,
follow `.haex-hive.json` and raise the drift for review.
