# Implementation Plan: Navigation im Tab (Vor/Zurück je Tab)

**Branch**: `020-tab-navigation` | **Date**: 2026-09-25 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/020-tab-navigation/spec.md`

## Summary

Jeder Tab der Workspace-Shell (Spec 015) bekommt einen Ort (Pfad plus Query) und
eine eigene lineare Vor/Zurück-Historie. Apps melden Routen an, auch
verschachtelt; die Titelleiste bekommt Zurück/Vor mit Verlaufsliste; Maustasten,
Alt+Pfeil und System-Zurück wirken nach festen Zielregeln. Alle Shell-Aktionen
laufen künftig über eine Aktions-Registry mit festen Standardbelegungen.

Technischer Ansatz (Begründungen in [research.md](./research.md)):

- **Eigener Tab-Router** (R1, R3–R5): reine Historien-Reducer und ein
  Pfad-Matcher unter `src/lib/shell/`, `useTabRouter()`, `ShellRouterView`,
  `ShellLink`. Kein zweiter vue-router, keine Closures in der Historie.
- **Historie im Store** (R2, R12): Feld `history` in der bestehenden, nie
  persistierten `TabRuntime`; keine Migration, keine neuen Tauri-Commands.
- **Eingaben mit eindeutigem Ziel** (R6, R7): Titelleisten-Knöpfe, ein globaler
  `keydown`-Listener, Maustasten am Fenster-Element, System-Zurück über
  `onBeforeRouteLeave` der Shell-Host-Seite samt Sperr-Eintrag.
- **Aktionen statt „Commands“** (R8): `src/lib/shell/actions.ts` und
  `keybindings.ts`; die 015-Komponenten rufen `shell.runAction()`.
- **Chat** (R10): Routen `/` und `/thread/:id`, Anbindung in einem neuen
  `useChatNavigation.ts`, weil `ChatApp.vue` an der 500-Zeilen-Grenze steht.
- **Keine neuen Abhängigkeiten** (R14).

## Technical Context

**Language/Version**: TypeScript 6 (strict), Vue 3.5, Nuxt 4.5.2 (SPA,
`ssr: false`), Node 22.19; Rust unverändert

**Primary Dependencies**: nur Vorhandenes — Pinia 4, `@vueuse/core` 15,
haex-ui-Layer (shadcn-vue/reka-ui: `ShadcnDropdownMenu`, `ShadcnTooltip`,
Toast), Tailwind 4, `@lucide/vue`, `@nuxtjs/i18n`

**Storage**: keine Änderung (Historie nur im Speicher, FR-011)

**Testing**: neues `pnpm check:shell-navigation` (Node-Type-Stripping, Vorbild
`check-shell-state.ts`); Regression `check:shell-state`, `check:chat-state`,
`check:templates`, `typecheck`, `typecheck:scripts`, `lint`, `format:check`;
manuell nach [quickstart.md](./quickstart.md)

**Target Platform**: Tauri-Desktop (Linux, macOS, Windows). Android-Geste über
denselben Pfad vorbereitet, end-to-end erst mit Android-Target prüfbar (R7)

**Project Type**: desktop-app (Nuxt-SPA-Frontend + Rust-Backend in einem Tauri-Projekt)

**Performance Goals**: SC-003 — ≤ 100 ms von Zurück/Vor bis zur sichtbaren
Ansicht bei geladenen Daten (reine Zustandsänderung, kein I/O außer dem
Nachladen von Chat-Nachrichten, das bereits heute gecacht wird)

**Constraints**: Dateien ≤ 500 Zeilen (`ChatApp.vue` steht bei 499 →
`useChatNavigation.ts`; `check-shell-state.ts` steht bei 829 → neue Prüfungen in
eigene Datei, R13); Module in `src/lib/shell/` relativ mit `.ts`-Endung, ohne
`~/`-Alias (Node-Harness); Historie ≤ 50 Einträge je Tab

**Scale/Scope**: ≤ 100 Tabs × ≤ 50 Einträge je Gerät — vernachlässigbarer
Speicher; 12 Aktionen, 2 Standardbelegungen

## Constitution Check

_GATE: Muss vor Phase 0 bestehen. Nach Phase 1 erneut geprüft — Ergebnis unten._

Geprüft gegen `.specify/memory/constitution.md` (v1.4.0) und die
spaex-Constitution `.spaex/constitution.md`.

| Prinzip / Vorgabe                                                          | Status | Begründung                                                                                                                                                                                  |
| -------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| I Keine Geheimnisse in Git                                                 | ✅     | Keine Geheimnisse berührt                                                                                                                                                                   |
| II Keine lokalen absoluten Pfade in versionierter Konfiguration            | ✅     | Nur repo-relative Pfade                                                                                                                                                                     |
| III Projektidentität geräteunabhängig                                      | ✅     | Berührt nicht                                                                                                                                                                               |
| IV Cross-Repo-Referenzen an unveränderliche SHAs gepinnt                   | ✅     | haex-vault `8dce379d94e18fcd42c3b73686a06f984ca3f574` in Spec und Research                                                                                                                  |
| V Externe Quellen nur per Opt-in                                           | ✅     | Keine neue Quelle                                                                                                                                                                           |
| VI Selbstverändernde Anweisungen review-pflichtig                          | ✅     | Keine Änderung an Constitution/Skills; `CONTEXT.md` läuft durch den PR                                                                                                                      |
| VII Relay-Ausfall blockiert lokale Arbeit nicht                            | ✅     | Rein lokal                                                                                                                                                                                  |
| VIII Keine Verheimlichung in Agent-Ausgaben                                | ✅     | –                                                                                                                                                                                           |
| Workflow: speckit-Stufen, PR auf `main`, Conventional Commits, kein Squash | ✅     | specify → plan → tasks → implement; Topic-Branch im Worktree                                                                                                                                |
| ADR bei prinzipienrelevanter Entscheidung                                  | ✅     | Keine nötig (R16)                                                                                                                                                                           |
| Test-Code in separaten Dateien                                             | ✅     | `scripts/check-shell-navigation.ts`                                                                                                                                                         |
| Worktree je Änderung                                                       | ✅     | `.worktrees/020-tab-navigation`                                                                                                                                                             |
| 500-LoC-Grenze                                                             | ✅     | Neue Dateien klein; `ChatApp.vue` bekommt nur einen Composable-Aufruf; neue Prüfungen nicht in `check-shell-state.ts`                                                                       |
| Graphify vor neuen benannten Artefakten                                    | ✅     | Abfrage in R8: keine Router/Keybinding-Artefakte; „Command“ belegt → Name „Aktion“                                                                                                          |
| `ponytail:`-Kommentar bei bewusster Vereinfachung                          | ✅     | Geplant an R7 (Android erst mit Target geprüft) und R6-Fallback (Zeigerposition)                                                                                                            |
| Nicht-triviale Logik hinterlässt einen ausführbaren Check                  | ✅     | `check:shell-navigation`                                                                                                                                                                    |
| Keine Selbstreferenzen von Agenten in Artefakten/Commits                   | ✅     | Wird bei Commits eingehalten                                                                                                                                                                |
| **Phasen-Disziplin**                                                       | ⚠️     | Setzt 015 im Einsatz voraus. Planen ist erlaubt; **`/speckit-implement` erst nach Merge von 015**, dann Rebase auf `main`; `plans/README.md` bekommt einen Eintrag (Aufgabe in tasks) — R15 |

**Ergebnis vor Phase 0**: kein unbegründeter Verstoß; ein ⚠️ dokumentiert.

**Ergebnis nach Phase 1**: unverändert. Das Design fügt keine Abhängigkeit, keine
Persistenz und keinen Tauri-Command hinzu.

## Project Structure

### Documentation (this feature)

```text
specs/020-tab-navigation/
├── plan.md                              # Dieses Dokument
├── research.md                          # Phase 0 (R1–R16)
├── data-model.md                        # Phase 1: Ort, Historie, Routen, Aktionen, Übergänge
├── quickstart.md                        # Phase 1: automatische und manuelle Validierung
├── contracts/
│   ├── tab-navigation-contract.md       # Routen, useTabRouter, ShellRouterView/ShellLink, openApp(at)
│   └── shell-actions.md                 # Aktionsliste, runAction, Tastatur, Maus, System-Zurück
├── checklists/requirements.md           # Spec-Qualitätscheckliste
└── tasks.md                             # Phase 2 — NICHT von /speckit-plan erzeugt
```

### Source Code (repository root)

Stand nach Merge von 015 (Pfade aus dem Branch `015-workspace-shell`).

```text
src/
├── lib/shell/                           # reine Module (relativ, mit .ts)
│   ├── navigation.ts                    # NEU: TabLocation, TabHistory, push/replace/go/removeEntry, Gleichheit, Parsen
│   ├── routeMatch.ts                    # NEU: Pfadmuster, verschachteltes Matching, Parameter
│   ├── actions.ts                       # NEU: ShellActionDefinition + Liste
│   ├── keybindings.ts                   # NEU: KeyboardEvent → Chord, Plattform, Auflösung
│   ├── types.ts                         # + TabRuntime.history
│   ├── layoutState.ts                   # openApp(…, at?), hydrate legt Start-Historie an
│   └── tabs.ts                          # addTab(…, at?)
├── stores/shell.ts                      # + navigate/back/forward/go je Tab, runAction, systemBack, openApp(at)
├── composables/
│   ├── useTabRouter.ts                  # NEU: App-Schnittstelle (inert außerhalb der Shell)
│   ├── useShellTab.ts                   # + openApp(appId, at?)
│   ├── useShellKeyboard.ts              # NEU: globaler keydown → runAction
│   └── useChatNavigation.ts             # NEU: Chat-Routen ↔ selectThread/newChat/Thread-Anlage/Löschen
├── components/shell/
│   ├── ShellNavButtons.vue              # NEU: Zurück/Vor + langer Druck/Rechtsklick
│   ├── ShellHistoryMenu.vue             # NEU: Verlaufsliste (Dropdown)
│   ├── ShellRouterView.vue              # NEU: rendert den Eintrag der eigenen Tiefe
│   ├── ShellLink.vue                    # NEU: Link mit push/replace, aria-current
│   ├── appRoutes.ts                     # ersetzt appComponents.ts (Routentabellen je App)
│   ├── ShellWindow.vue                  # + ShellNavButtons, Maustasten, data-shell-window-id
│   ├── ShellTabPanel.vue                # rendert ShellRouterView statt direkter App-Komponente
│   └── ShellTabBar/ShellTabListMenu/ShellWindowControls/ShellLauncher/…  # Aufrufe über runAction
├── components/apps/ChatApp.vue          # + useChatNavigation (eine Zeile Aufruf)
├── pages/workspace/[instance].vue       # + onBeforeRouteLeave/Sperr-Eintrag, ?at=, useShellKeyboard
├── pages/{chat,settings,federation}/[instance].vue  # unverändert (open=)
└── i18n/locales/{de,en}.json            # + shell.nav.*, shell.actions.*, shell.chat.thread

scripts/check-shell-navigation.ts        # NEU
package.json                             # + check:shell-navigation
.github/workflows/ci.yml                 # + Schritt check:shell-navigation
CONTEXT.md                               # + Ort, Tab-Historie, Shell-Aktion
plans/README.md                          # + Roadmap-Eintrag 020
```

**Structure Decision**: Wie in 015: reine, ohne Nuxt testbare Module unter
`src/lib/shell/`, dünne Store-Erweiterung und Composables, Vue-Bausteine unter
`components/shell/`. Kein Backend-Anteil.

## Anforderungsabdeckung

| Anforderungen                                             | Umsetzung                                                                        |
| --------------------------------------------------------- | -------------------------------------------------------------------------------- |
| FR-001–009 (Ort, Historie, Push/Replace, Grenzen)         | `navigation.ts`, `routeMatch.ts`, `useTabRouter`, `ShellRouterView`, `ShellLink` |
| FR-010–011 (Lebensdauer)                                  | `TabRuntime.history`, `syncTabRuntime`, `hydrate`                                |
| FR-012–014 (Öffnen an einem Ort, Legacy, unbekannter Ort) | `openApp(…, at?)`, `useShellTab().openApp`, `?at=`, Wurzel-`ShellRouterView`     |
| FR-015–016 (Knöpfe, Verlaufsliste)                        | `ShellNavButtons`, `ShellHistoryMenu`, `ShellWindow`                             |
| FR-017 (Tastatur)                                         | `keybindings.ts`, `useShellKeyboard`                                             |
| FR-018 (Maus)                                             | `ShellWindow` (`mouseup`/`auxclick`), Fallback Zeigerposition                    |
| FR-019–020 (System-Zurück, Webview-Historie)              | `pages/workspace/[instance].vue`, `shell.systemBack()`                           |
| FR-021 (Titel)                                            | Store: Titel-Kette R9, Eintrags-Titel beim Verlassen                             |
| FR-022–023 (Zugänglichkeit, i18n)                         | ARIA an Knöpfen und Menü, `de.json`/`en.json`                                    |
| FR-024–026 (Aktionen)                                     | `actions.ts`, `shell.runAction`, Umstellung der 015-Komponenten                  |
| FR-027 (Chat)                                             | `useChatNavigation.ts`, `appRoutes.ts`                                           |

## Reihenfolge der Umsetzung (Grobplan für `/speckit-tasks`)

0. **Gate**: 015 gemerged, Branch auf `main` rebased, `plans/README.md`-Eintrag.
1. **Reine Module** `navigation.ts`, `routeMatch.ts`, `actions.ts`,
   `keybindings.ts` samt `check:shell-navigation` (Test zuerst).
2. **Store**: `TabRuntime.history`, `openApp`/`addTab` mit Ort, Navigation je
   Tab, `runAction`, `systemBack`; Store-Tests im selben Skript.
3. **Vue-Bausteine**: `useTabRouter`, `ShellRouterView`, `ShellLink`,
   `appRoutes.ts` (Apps ohne Routen verhalten sich wie bisher).
4. **Titelleiste**: `ShellNavButtons`, `ShellHistoryMenu`; Plattform-Spike
   Maustasten (quickstart §3), dann Maus-Anbindung.
5. **Tastatur und System-Zurück**: `useShellKeyboard`, Host-Seite.
6. **Aktionen**: 015-Komponenten auf `runAction` umstellen.
7. **Chat**: Routen, `useChatNavigation.ts`, `check:chat-state` unverändert grün.
8. **i18n, `CONTEXT.md`, CI-Schritt**, quickstart-Durchlauf.

## Complexity Tracking

Keine Abweichungen von der Constitution.
