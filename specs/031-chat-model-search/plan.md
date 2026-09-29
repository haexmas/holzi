# Implementation Plan: Modellsuche im Chat

**Branch**: `031-chat-model-search` | **Date**: 2026-09-29 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/031-chat-model-search/spec.md`

## Summary

Die Modellauswahl im Chat-Composer (`ComposerSettingsPopover.vue`) bekommt ein
Suchfeld, das die angezeigte Liste in Echtzeit auf fehlertolerante Treffer
gegen Anbieter- und Modellname einschränkt, Anbieter-Gruppierung und
Tastaturbedienung bleiben erhalten.

Technischer Ansatz (Begründungen in [research.md](./research.md)):

- **Fehlertoleranter Abgleich** (R1): `fuse.js` (neue, kleine,
  abhängigkeitsfreie Bibliothek) statt einer selbstgeschriebenen
  Teilfolgen-Suche, weil FR-003/SC-002 auch einzelne Tippfehler
  (Buchstabenvertauschung, nicht nur Auslassung) abdecken müssen — das
  verlangt einen echten Distanz-Score, den eine reine
  Teilfolgen-Prüfung nicht liefert.
- **Einbindung in die bestehende Auswahl** (R2): Ein Textfeld direkt in
  `ShadcnSelectContent`, das Modell- und Anbieter-Liste per bestehendem
  `v-for`/`v-if` filtert, statt eines neuen Combobox-Bausteins — letzterer
  würde eine neue Komponentenfamilie im haex-ui-Layer (separates Repo)
  voraussetzen, die es dort noch nicht gibt.
- **Leerer Treffer** (R3): bestehendes Hinweis-Muster (kurzer Text statt
  leerer Fläche), keine neue Komponente.

## Technical Context

**Language/Version**: TypeScript 6 (strict), Vue 3.5, Nuxt 4.5.2 (SPA)

**Primary Dependencies**: `fuse.js` (neu, siehe R1), `reka-ui`
(`ShadcnSelect*` aus dem haex-ui-Nuxt-Layer, bestehend), `@nuxtjs/i18n`

**Storage**: N/A — es wird ausschließlich über bereits geladene
`ModelGroup[]` (Anbieter-/Modellnamen) gefiltert, kein neuer Datenbestand

**Testing**: `node --test` gegen die reine Filter-/Score-Funktion (neues
`scripts/check-chat-model-search.ts`, Muster wie `scripts/check-settings.ts`);
`pnpm lint`, `pnpm typecheck`, `npx tsc --project tsconfig.scripts.json
--noEmit`, `format:check`; ein neues Tauri-e2e-Szenario für die
Tastaturbedienung (Muster wie `scripts/e2e/scenarios/*.test.ts`)

**Target Platform**: Tauri-Desktop (Linux, macOS, Windows); Android ohne
Besonderheiten (dieselbe Vue-Komponente)

**Project Type**: desktop-app (Nuxt-SPA in einem Tauri-Projekt); reines
Frontend, kein Rust-Anteil

**Performance Goals**: Filterung MUSS ohne wahrnehmbare Verzögerung bei jedem
Tastenanschlag erfolgen (SC-001: Modell in unter 5 s finden); bei den heute
üblichen Listengrößen (siehe Scale/Scope) reicht ein Neu-Berechnen des
gesamten Fuse-Index pro Tastenanschlag, kein Debouncing nötig

**Constraints**: `ComposerSettingsPopover.vue` bleibt unter der
500-Zeilen-Richtgrenze (heute 293 Zeilen); keine neue Komponente im
haex-ui-Layer (separates Repo, siehe R2); keine Regression der bestehenden
Tastaturbedienung (FR-007)

**Scale/Scope**: Typischerweise wenige bis einige Dutzend Modelle über
wenige Anbieter (SC-001 prüft mit 30); eine einzelne Vue-Komponente plus eine
neue reine Hilfsfunktion

## Constitution Check

_GATE: Muss vor Phase 0 bestehen. Nach Phase 1 erneut geprüft — Ergebnis unten._

Geprüft gegen `.specify/memory/constitution.md` (v1.4.0) und die
spaex-Constitution `.spaex/constitution.md`.

| Prinzip / Vorgabe                                               | Status | Begründung                                                                       |
| --------------------------------------------------------------- | ------ | -------------------------------------------------------------------------------- |
| I Keine Geheimnisse in Git                                      | ✅     | Keine Geheimnisse berührt                                                        |
| II Keine lokalen absoluten Pfade in versionierter Konfiguration | ✅     | Nur repo-relative Pfade                                                          |
| III Projektidentität geräteunabhängig                           | ✅     | Berührt nicht                                                                    |
| IV Cross-Repo-Referenzen an unveränderliche SHAs gepinnt        | ✅     | Kein Cross-Repo-Bezug (haex-ui-Layer-Pin in `nuxt.config.ts` bleibt unverändert) |
| V Externe Quellen nur per Opt-in                                | ✅     | `fuse.js` ist eine gewöhnliche npm-Abhängigkeit, kein Harness-Atom               |
| VI Selbstverändernde Anweisungen review-pflichtig               | ✅     | Keine Änderung an Constitution/Skills                                            |
| VII Relay-Ausfall blockiert lokale Arbeit nicht                 | ✅     | Rein lokal, kein Netzwerk                                                        |
| VIII Keine Verschleierungs-Anweisungen                          | ✅     | Berührt nicht                                                                    |
| speckit-Workflow eingehalten                                    | ✅     | specify → (dieser plan) → tasks → implement, mit Review-Gates                    |

Ergebnis nach Phase 1: unverändert, siehe [research.md](./research.md) für
die Abwägung der neuen Abhängigkeit.

## Project Structure

### Documentation (this feature)

```text
specs/031-chat-model-search/
├── plan.md              # diese Datei
├── research.md          # Phase 0
├── data-model.md         # Phase 1
├── quickstart.md         # Phase 1
└── tasks.md              # Phase 2 ($speckit-tasks, noch nicht erstellt)
```

### Source Code (repository root)

```text
src/
├── components/chat/
│   └── ComposerSettingsPopover.vue   # bekommt das Suchfeld
├── lib/chat/
│   └── modelSearch.ts                 # neu: reine Filter-/Score-Funktion über fuse.js
└── i18n/locales/
    ├── de.json                        # neue Strings: Platzhalter, Kein-Treffer-Hinweis
    └── en.json

scripts/
└── check-chat-model-search.ts         # neu: node:test gegen modelSearch.ts

scripts/e2e/scenarios/
└── chat-model-search.test.ts          # neu: Tastaturbedienung Ende-zu-Ende
```

**Structure Decision**: Bestehende Struktur (`src/components/chat/`,
`src/lib/`, `scripts/`, `scripts/e2e/scenarios/`) wird beibehalten; die neue
reine Logik liegt in `src/lib/chat/` neben den übrigen Chat-Hilfsmodulen
(Analogie zu `src/lib/settings/search.ts`), nicht in der Komponente selbst,
damit sie ohne Vue mit `node --test` prüfbar ist (Muster
`scripts/check-settings.ts`).

## Complexity Tracking

Keine Verstöße gegen die Constitution Check-Gates; Tabelle entfällt.
