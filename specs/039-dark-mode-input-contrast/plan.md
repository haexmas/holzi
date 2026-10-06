# Implementation Plan: Dark-Mode-Input-Kontrast

**Branch**: `fix/dark-mode-input-contrast` | **Date**: 2026-10-06 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/039-dark-mode-input-contrast/spec.md`

## Summary

Improve the visibility of non-focused input fields in Dark Mode by adjusting the
existing shared theme token used for input borders and its matching appearance
default. Keep Light Mode, focused states, custom semantic borders, and
user-configured appearance values intact. The change is limited to the shared
stylesheet and its existing token derivation because the reported symptom is
present across the common input styling.

## Technical Context

**Language/Version**: TypeScript/Vue 3 via Nuxt 4; CSS with Tailwind CSS 4

**Primary Dependencies**: Nuxt, Tailwind CSS, tw-animate-css; no new dependency

**Storage**: N/A

**Testing**: Prettier check for the changed stylesheet, Nuxt production build,
and the existing automated checks relevant to appearance/style

**Target Platform**: Desktop Tauri webview and browser-based Nuxt preview

**Project Type**: Desktop application with a Nuxt frontend

**Performance Goals**: No measurable runtime impact; preserve existing rendering
performance

**Constraints**: Do not alter Light Mode, focus indication, component-specific
semantic borders, or persisted appearance settings; do not add dependencies

**Scale/Scope**: One shared stylesheet token and its existing default affecting
all common input fields

## Constitution Check

_GATE: Must pass before Phase 0 research. Re-check after Phase 1 design._

- **PASS** — Work is on the dedicated topic worktree
  `fix/dark-mode-input-contrast`; the primary checkout remains unchanged.
- **PASS** — No secrets, local absolute paths, external references, or new
  dependencies are introduced.
- **PASS** — The active Spec-Kit workflow is represented by this spec, plan,
  tasks, and implementation sequence.
- **PASS** — The change is a small in-place stylesheet edit, so no new named
  function/component/API is authored and no graph query is needed.
- **PASS** — No ADR is needed because this is a reversible visual token tweak
  that does not materially affect a core principle.

## Project Structure

### Documentation (this feature)

```text
specs/039-dark-mode-input-contrast/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Not applicable; documents no persisted model
├── quickstart.md        # Phase 1 validation guide
└── tasks.md             # Phase 2 implementation tasks
```

### Source Code (repository root)

```text
src/
├── assets/css/tailwind.css # Shared theme tokens and base styles
└── lib/appearance/tokens.ts # Appearance defaults kept in sync with CSS

scripts/
└── check-appearance*.ts     # Existing appearance/style checks
```

**Structure Decision**: Keep the existing Nuxt/Tailwind frontend structure. The
implementation belongs in `src/assets/css/tailwind.css` and its existing
`src/lib/appearance/tokens.ts` default, with no new component, service, data
model, or contract.

## Complexity Tracking

No constitution violations or complexity exceptions.
