<!-- SPECKIT START -->
Aktueller Feature-Plan: [specs/002-onboarding-model-prefs/plan.md](specs/002-onboarding-model-prefs/plan.md)
Zugehörige Artefakte: [spec.md](specs/002-onboarding-model-prefs/spec.md), [research.md](specs/002-onboarding-model-prefs/research.md), [data-model.md](specs/002-onboarding-model-prefs/data-model.md), [contracts/tauri-commands.md](specs/002-onboarding-model-prefs/contracts/tauri-commands.md), [quickstart.md](specs/002-onboarding-model-prefs/quickstart.md)
Konvention und Domain-Terme: [docs/adr/0001-device-scoped-data-convention.md](docs/adr/0001-device-scoped-data-convention.md), [CONTEXT.md](CONTEXT.md)
<!-- SPECKIT END -->

## Manifest precedence

When `.haex-hive.json` and `.spaex.json` coexist, `.haex-hive.json` and its
pinned source revisions are authoritative for the effective constitution.
`.spaex.json` is retained as a Spaex-compatible projection and MUST use pins
valid for Spaex's publisher schema; the two manifests MUST NOT be reconciled by
mixing or choosing between their revisions. If their resolved content differs,
follow `.haex-hive.json` and raise the drift for review.
