# Implementation Plan: Mehrfachinstanzen für Apps

**Branch**: `030-app-multi-instance` | **Date**: 2026-09-29 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/030-app-multi-instance/spec.md`

## Summary

Chat wird von einer Einzelinstanz- zu einer Mehrfachinstanz-App (spec 015
FR-016/017); Einstellungen bleibt unverändert Einzelinstanz. Der
generische Mehrfachinstanz-Mechanismus (`AppDefinition.multiInstance`,
`openApp`/`addTab`/`hydrate`) existiert bereits vollständig und ist bereits
mit einer synthetischen Test-App abgedeckt (spec 015 User Story 7,
`scripts/check-wm-state.ts` T051/T052) — diese Spec ändert an diesem
Mechanismus nichts. Die Umsetzung ist auf eine Registry-Änderung plus
Regressionstests mit der echten `system.chat`-App begrenzt (Begründung in
[research.md](./research.md)).

## Technical Context

**Language/Version**: TypeScript 6 (strict), Vue 3.5, Nuxt 4.5.2 (SPA)

**Primary Dependencies**: keine neuen — nur bestehende
`src/lib/wm/{apps,layoutState,tabs}.ts`

**Storage**: SQLCipher-Vault über haex-crdt, unverändert; Sitzungsdaten
(spec 022, `wm_sessions_no_sync`) speichern Tabs bereits generisch nach
App-ID, keine Schema-Änderung nötig (siehe research.md R1)

**Testing**: `node --test` gegen `scripts/check-wm-state.ts` (neue Fälle mit
der echten `WM_APPS`-Registry statt nur der synthetischen Test-App),
`pnpm lint`, `pnpm typecheck`, `format:check`; ein neues Tauri-e2e-Szenario
für den sichtbaren Nutzerpfad (zwei Chat-Tabs, ein Einstellungs-Tab bleibt
einzeln)

**Target Platform**: Tauri-Desktop (Linux, macOS, Windows); Android ohne
Besonderheiten

**Project Type**: desktop-app (Nuxt-SPA in einem Tauri-Projekt); reines
Frontend, kein Rust-Anteil

**Performance Goals**: keine neuen (unverändertes Öffnen-Verhalten für
Mehrfachinstanz-Apps, bereits performant getestet)

**Constraints**: Keine Änderung an `openApp`/`addTab`/`hydrate` oder ihren
bestehenden Tests (T051/T052) — nur an der Registry (`WM_APPS`) und den
beiden betroffenen Spec-Dokumenten (015, 020)

**Scale/Scope**: Eine Zeile Registry-Änderung
(`src/lib/wm/apps.ts`); keine neue Datei außer Tests

## Constitution Check

_GATE: Muss vor Phase 0 bestehen. Nach Phase 1 erneut geprüft — Ergebnis unten._

Geprüft gegen `.specify/memory/constitution.md` (v1.4.0) und die
spaex-Constitution `.spaex/constitution.md`.

| Prinzip / Vorgabe                                               | Status | Begründung                                                    |
| --------------------------------------------------------------- | ------ | ------------------------------------------------------------- |
| I Keine Geheimnisse in Git                                      | ✅     | Keine Geheimnisse berührt                                     |
| II Keine lokalen absoluten Pfade in versionierter Konfiguration | ✅     | Nur repo-relative Pfade                                       |
| III Projektidentität geräteunabhängig                           | ✅     | Berührt nicht                                                 |
| IV Cross-Repo-Referenzen an unveränderliche SHAs gepinnt        | ✅     | Kein Cross-Repo-Bezug                                         |
| V Externe Quellen nur per Opt-in                                | ✅     | Keine neue Abhängigkeit                                       |
| VI Selbstverändernde Anweisungen review-pflichtig               | ✅     | Keine Änderung an Constitution/Skills                         |
| VII Relay-Ausfall blockiert lokale Arbeit nicht                 | ✅     | Rein lokal                                                    |
| VIII Keine Verschleierungs-Anweisungen                          | ✅     | Berührt nicht                                                 |
| speckit-Workflow eingehalten                                    | ✅     | specify → (dieser plan) → tasks → implement, mit Review-Gates |

Ergebnis nach Phase 1: unverändert.

## Project Structure

### Documentation (this feature)

```text
specs/030-app-multi-instance/
├── plan.md              # diese Datei
├── research.md          # Phase 0
├── data-model.md         # Phase 1
├── quickstart.md         # Phase 1
└── tasks.md              # Phase 2 ($speckit-tasks, noch nicht erstellt)
```

### Source Code (repository root)

```text
src/lib/wm/
└── apps.ts                # `system.chat`: multiInstance false -> true

scripts/
└── check-wm-state.ts       # neue Testfälle mit der echten WM_APPS-Registry

scripts/e2e/scenarios/
└── chat-multi-instance.test.ts   # neu: zwei Chat-Tabs, Einstellungen bleibt einzeln

specs/015-workspace-shell/spec.md   # FR-017 amendment block (wie Spec 022 es bei FR-023 tat)
specs/020-tab-navigation/spec.md    # Assumptions-Eintrag amendment block
```

**Structure Decision**: Keine neue Datei außer Tests; die Registry-Änderung
bleibt in der bestehenden `apps.ts`. Beide betroffenen Vorgänger-Specs
bekommen einen Änderungsvermerk im selben Stil, den Spec 022 bereits für
FR-023 aus Spec 015 verwendet hat (Blockzitat vor der geänderten
Anforderung, Original-Wortlaut bleibt als historischer Stand erhalten).

## Complexity Tracking

Keine Verstöße gegen die Constitution Check-Gates; Tabelle entfällt.
