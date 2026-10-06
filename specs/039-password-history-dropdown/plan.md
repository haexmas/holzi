# Implementation Plan: Kompakter Passwortverlauf

**Branch**: `feat/password-history-dropdown` | **Date**: 2026-10-06 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/039-password-history-dropdown/spec.md`

## Summary

Die Verlaufansicht des Passwortmanagers wird von der bisherigen Timeline-neben-Inhalt-
Darstellung auf einen einspaltigen Ablauf umgestellt. Der vorhandene Verlaufsselektor wird
als zugängliche Dropdown-Auswahl dargestellt; der ausgewählte Stand bleibt darunter sichtbar.
Die bestehende Lade-, Auswahl-, Geheimnis- und Wiederherstellungslogik wird wiederverwendet.
Die Zeitzeile im Standkopf wird typografisch zu einer dezenten Metainformation zurückgenommen.

## Technical Context

**Language/Version**: Vue 3 / TypeScript 6, Vue Single-File Components

**Primary Dependencies**: Nuxt 4, bestehende globale `UiSelect`-Komponente und
`@lucide/vue`-basierte Icon-Komponenten

**Storage**: Keine Änderung; bestehende Passwort-Verlaufsdaten aus dem Vault-Service

**Testing**: Bestehende Node-Checks, `pnpm check:templates`, `pnpm typecheck`, `pnpm lint`

**Target Platform**: Tauri-Desktop-App mit Webview

**Project Type**: Desktop-Webanwendung

**Performance Goals**: Kein zusätzlicher Backend-Aufruf beim Öffnen; pro Auswahl weiterhin
genau der bestehende Ladevorgang für den gewählten Stand

**Constraints**: Keine Änderung an Backend-Verträgen, Datenmodell, Navigation oder
Geheimnisbehandlung; deutsche und englische Übersetzungen bleiben synchron

**Scale/Scope**: Zwei bestehende Passwortmanager-Komponenten plus betroffene Übersetzungen
und Spezifikationsartefakte

## Constitution Check

_GATE: Must pass before implementation._

- **Spaex / Spec Kit workflow**: PASS. Die Änderung hat eine eigene Spec, einen Plan und
  ausführbare Tasks; die Implementierung folgt erst danach.
- **Worktree/topic branch**: PASS. Die Arbeit erfolgt in `feat/password-history-dropdown` im
  registrierten Worktree `.worktrees/password-history-dropdown`.
- **No secrets / portable config**: PASS. Es werden keine Geheimnisse oder lokalen Pfade in
  versionierte Projektdateien geschrieben.
- **Reuse before new code**: PASS. Der vorhandene History-Selektor, die bestehende `UiSelect`
  und die bestehenden Datenlade-/Reset-Funktionen werden erweitert bzw. wiederverwendet.
- **Tests separate from production**: PASS. Es werden keine Tests in Produktionsdateien ergänzt;
  bestehende Checks und Template-Prüfungen werden ausgeführt.
- **ADR requirement**: PASS. Die reine Präsentationsänderung berührt keine Core-Principle-
  Entscheidung und benötigt keinen ADR.

## Project Structure

```text
specs/039-password-history-dropdown/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── checklists/requirements.md
└── tasks.md

src/components/passwords/
├── HistoryTab.vue       # Einspaltiges Layout und bestehende Auswahl-/Ladelogik
├── HistoryTimeline.vue  # Wiederverwendeter Selektor als Dropdown
└── HistorySnapshot.vue  # Dezente Zeit-Metainformation

src/i18n/locales/
├── de.json
└── en.json
```

**Structure Decision**: Die Änderung bleibt in den bestehenden Passwort-Komponenten. Es wird
keine neue Komponente und kein neues Daten- oder Service-Modul eingeführt, weil der bestehende
Verlaufsselektor bereits die Auswahl- und Tastaturlogik besitzt und lediglich seine Darstellung
wechselt.

## Design Details

1. `HistoryTab.vue` entfernt die responsive Grid-Klasse und rendert Selektor und Snapshot als
   normale vertikale Folge.
2. `HistoryTimeline.vue` behält die bestehenden Zeitformatierer und `select`-Emission bei,
   ersetzt die Timeline-Schaltflächen aber durch die globale `UiSelect`-Komponente. Die
   Optionen zeigen den exakt formatierten Zeitpunkt; die Reihenfolge bleibt unverändert.
3. `HistorySnapshot.vue` verwendet für `savedAt` eine kleine, normalgewichtete
   `text-muted-foreground`-Darstellung statt einer dominanten Überschrift.
4. Übersetzungstexte bleiben unverändert, sofern kein zusätzlicher zugänglicher Labeltext
   benötigt wird; der bestehende Schlüssel `passwords.history.states` wird als Label genutzt.

## Complexity Tracking

Keine Verfassungsverstöße oder zusätzlichen Komplexitätskosten.
