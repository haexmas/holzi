# Implementation Plan: Frontend Onboarding — Landing, Create, Open, Connect

**Branch**: `001-frontend-onboarding` | **Date**: 2026-09-04 | **Spec**: [`spec.md`](./spec.md)
**Input**: Feature specification from [`./spec.md`](./spec.md)

## Summary

Deliver the holzi Tauri app's first-impression surfaces: a landing page with three primary actions (Anlegen, Öffnen, Verbinden) mirroring haex-vault's landing pattern, plus a Zuletzt-verwendet list and an Unlock sheet for existing instances. Frontend is Nuxt 4 (SPA) with Tailwind v4, shadcn-vue components (copy-in), Pinia stores, `@nuxtjs/i18n`, and `@nuxt/icon` fed from a locally-bundled Lucide icon set. Backend is a set of five Tauri commands over the `haex-crdt` layer that manage `<name>.db` files under `<AppLocalData>/instances/`, plus a `close_instance` for safe hand-off between active instances. This slice is the minimum surface required to reach the walking-skeleton described in `docs/plans/2026-09-04-v1-scope-design.md §11` (two devices paired into one federation, exchanging a ping).

## Technical Context

**Language/Version**:
- Frontend: TypeScript 5.x, Vue 3.5, Nuxt 4.
- Backend: Rust (stable, per `rust-toolchain.toml` to be added), Tauri 2.

**Primary Dependencies**:
- Nuxt modules: `@nuxtjs/i18n`, `@nuxt/icon`, `@pinia/nuxt`, `@vueuse/nuxt`.
- Styling: Tailwind v4 via `@tailwindcss/vite`, `tw-animate-css`.
- UI: `shadcn-vue` (Radix Vue-based, copy-in), `class-variance-authority`, `tailwind-merge`, `lucide-vue-next` (via `@nuxt/icon` iconify-json bundle).
- Tauri plugins: `@tauri-apps/plugin-dialog` (file picker for Öffnen), `@tauri-apps/plugin-store` (only for non-secret preferences — not for instance list), `@tauri-apps/plugin-fs` (limited to `AppLocalData` scope).
- Rust-side: `haex-crdt` (workspace path or crates.io once extracted), `tauri`, `serde`, `ts-rs` (for type sharing), `thiserror`.

**Storage**:
- Instance DBs: `<AppLocalData>/instances/<name>.db`, SQLCipher-encrypted, `haex-crdt`-managed.
- No frontend-side persistence of instance state. Recent-list is a directory scan, not a JSON file.

**Testing**:
- Unit: Vitest for Vue composables and Pinia stores.
- E2E: Playwright for the landing → Anlegen → Unlock → Close → Reopen loop.
- Backend: `cargo test` for Rust commands; ts-rs bindings validated by a `test:constants` equivalent.

**Target Platform**: Desktop (Linux, macOS, Windows), Android, iOS. SPA mode (`ssr: false`) required because Tauri renders into a native WebView, not against a Node server.

**Project Type**: Desktop-app (Tauri) with an embedded web frontend. Uses the "Web application" project structure inside `src/` (frontend) + `src-tauri/` (backend), matching `haex-vault`'s layout.

**Performance Goals**:
- Landing interactive ≤500ms on desktop, ≤1500ms on mid-range Android (SC-003).
- Unlock ≤2s desktop / ≤4s mobile (SC-004).
- No blocking network requests during app boot.

**Constraints**:
- Offline-capable: zero external CDN or icon-API traffic (SC-006, FR-027).
- App must never write outside `<AppLocalData>/instances/` for instance data.
- The frontend must never construct or receive filesystem paths for instances (FR-025).
- Only one active instance at runtime (FR-022) — enforced backend-side, surfaced as a typed error.

**Scale/Scope**:
- v1 typical operator: 1–3 instances on desktop, 1 on mobile. UI must render acceptably up to ~50 instances (SC-007 covers 10+; 50 is a soft ceiling).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

The repo's constitution reference is `.haex-hive.json → com.github.haexmas.haex-hive.constitution` (revision `336eaf1e`). This spec does not attempt to import and re-render the full constitution — its NON-NEGOTIABLE principles apply session-wide per `~/.claude/CLAUDE.md` haex-hive detection block. Gates that touch this feature:

- **No secrets in git**: instance DBs live under `<AppLocalData>/`, never inside the repo. Fixtures for tests use throwaway paths under `tmp/`. ✅
- **Local-only artifacts**: all icons, fonts, translations, and templates are bundled at build time. No runtime CDN. ✅ (FR-027, SC-006)
- **User confirmation for destructive actions**: hard-delete of an instance file is not exposed in v1 UI (FR-023 offers only list-remove and trash). ✅
- **Reproducible builds**: `pnpm-lock.yaml` committed; Rust `Cargo.lock` committed. ✅ (standard for the Tauri stack)

No violations to justify in Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/001-frontend-onboarding/
├── spec.md                          # Feature specification (P1..P3 stories)
├── plan.md                          # This file
├── research.md                      # Rationale for stack choices and haex-vault deltas
├── quickstart.md                    # Fresh-clone → running landing in <5 minutes
├── contracts/
│   ├── tauri-commands.md            # Rust command signatures + error enum
│   ├── events.md                    # Tauri event topics emitted by backend
│   └── types.md                     # Shared TS/Rust type definitions
└── tasks.md                         # Actionable task list per user story
```

### Source Code (repository root)

```text
src/                                 # Nuxt frontend (SPA)
├── app.vue                          # Root — DnD/Toast providers, layout wrapper
├── pages/
│   ├── index.vue                    # Landing (Anlegen/Öffnen/Verbinden + list)
│   └── federation/
│       └── [instance].vue           # Placeholder for post-unlock; not this spec
├── components/
│   ├── ui/                          # shadcn-vue copy-ins → <UiButton>, <UiSheet>, ...
│   │   ├── button/Button.vue
│   │   ├── card/Card.vue
│   │   ├── sheet/Sheet.vue
│   │   ├── dialog/Dialog.vue
│   │   ├── input/Input.vue
│   │   ├── label/Label.vue
│   │   ├── radio-group/RadioGroup.vue
│   │   └── sonner/Sonner.vue
│   └── onboarding/                  # Feature components → <OnboardingCreateSheet>, ...
│       ├── CreateSheet.vue          # Anlegen — Genesis + Recover sub-modes
│       ├── OpenSheet.vue            # Öffnen — file picker + copy
│       ├── ConnectSheet.vue         # Verbinden — QR/token pairing
│       ├── UnlockSheet.vue          # Passphrase entry for existing instance
│       ├── PaperSeedDisplay.vue     # Read-once seed presentation
│       └── InstancesList.vue        # Zuletzt-verwendet list
├── composables/
│   ├── useInstance.ts               # Bridge to Tauri commands + AppState
│   └── useAppVersion.ts
├── stores/
│   └── instances.ts                 # Pinia — instance list + active instance
├── i18n/
│   └── locales/
│       ├── de.json                  # Default
│       └── en.json                  # Fallback
├── types/
│   └── bindings/                    # ts-rs generated types
├── lib/
│   └── utils.ts                     # cn() helper for Tailwind class merging
└── assets/
    └── css/
        └── tailwind.css             # Tailwind + shadcn-vue CSS variables

src-tauri/
├── Cargo.toml
├── tauri.conf.json                  # Bundle id, allowlist scope for AppLocalData
├── build.rs
└── src/
    ├── main.rs                      # Tauri app entry, command registration
    ├── error.rs                     # HolziError enum (thiserror)
    ├── state.rs                     # AppState — mutex-guarded active instance handle
    ├── instances/
    │   ├── mod.rs
    │   ├── paths.rs                 # get_instance_path, get_instances_directory
    │   ├── crud.rs                  # create_instance, open_instance, close_instance
    │   ├── list.rs                  # list_instances (directory scan)
    │   ├── import.rs                # import_instance_file (Öffnen)
    │   ├── trash.rs                 # move_instance_to_trash
    │   └── events.rs                # emit_instance_list_changed
    └── pairing/
        ├── mod.rs
        └── join.rs                  # Verbinden backend — token, transcript, co-sign

tests/                               # Vitest
├── stores/
│   └── instances.spec.ts
└── composables/
    └── useInstance.spec.ts

e2e/                                 # Playwright
└── onboarding.spec.ts
```

**Structure Decision**: Adopt haex-vault's directory layout (`src/` + `src-tauri/`, Pinia under `src/stores/`, Tauri plugins consumed from `@tauri-apps/plugin-*`). Component auto-import uses Nuxt's `pathPrefix: true` so `src/components/ui/button/Button.vue` becomes `<UiButton>` and `src/components/onboarding/CreateSheet.vue` becomes `<OnboardingCreateSheet>` — no manual imports in templates.

## Complexity Tracking

> No Constitution Check violations. Table intentionally left empty.
