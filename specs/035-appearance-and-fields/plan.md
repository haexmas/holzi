# Implementation Plan: Darstellung und Eingabefelder

**Branch**: `035-ui-foundation` | **Date**: 2026-10-02 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/035-appearance-and-fields/spec.md`

## Summary

Zwei Teile, die sich nur über die Primärfarbe berühren:

1. **Felder (US1)**: Alle Eingabefelder in `src/` wechseln von `ShadcnInput`, `ShadcnTextarea`,
   `ShadcnSelect` und rohen `<input>`/`<select>` auf `UiInput`, `UiInputPassword`, `UiTextarea` und
   `UiSelect` aus haex-ui (gepinnt auf `2dcb8bc`, schon umgestellt). Eine statische Prüfung
   (`check:fields`) hält die alten Feldarten draußen, außer in einer begründeten Positivliste.
2. **Darstellung (US2–US5)**: Die heutigen Tokens in `src/assets/css/tailwind.css` bleiben die
   Standardwerte. Eine reine Bibliothek `src/lib/appearance/` rechnet aus einer gespeicherten
   **Darstellung** (Schema, Akzent, vier Tönungen, Fensterhinweis) für Hell oder Dunkel eine
   Tabelle von CSS-Variablen aus und erzwingt dabei die Kontrastgrenzen (4,5:1 Text, 3:1
   Bedienelemente). Ein Composable `useAppearance` setzt diese Variablen als Inline-Stil auf
   `<html>` (das bestehende `useColorScheme` bleibt für die `dark`-Klasse zuständig). Gespeichert
   wird als eine Vault-Einstellung `appearance.theme` (JSON); Sync, Neuladen nach Sync und
   Vault-Wechsel laufen über den bestehenden Weg des Farbschemas. Export und Import sind eine
   versionierte JSON-Datei, die vollständig geprüft wird, bevor etwas übernommen wird.

Das Ergebnis der Rechnung ist für jede Auswahl **vorab prüfbar**, weil Tönungen die Helligkeit
nicht verschieben (nur Farbton und gedeckelte Sättigung) und die Akzentfarbe ihre Helligkeit aus
dem Kontrast ableitet. Darum misst `check:appearance` Kontraste für alle Farbfelder und
Extremwerte in beiden Schemata, ohne die App zu starten (SC-004); die e2e-Prüfung misst am echten
DOM nach.

## Technical Context

**Language/Version**: TypeScript (Nuxt 4, Vue 3, `strict`), Rust nur für die Registrierung von `tauri-plugin-fs` (eine Zeile, research R7); keine neuen Befehle (Preferences-Befehle `get_pref`/`set_pref` existieren).

**Primary Dependencies**: haex-ui (`@haex-space/ui`, gepinnt auf `2dcb8bc`), Tailwind 4 mit den Tokens aus `src/assets/css/tailwind.css`, `@tauri-apps/plugin-dialog` für Dateiauswahl und `@tauri-apps/plugin-fs` für Lesen und Schreiben der Datei (beide schon im Projekt; `tauri-plugin-fs` steht über das Dialog-Plugin schon in `Cargo.lock` und wird nur direkt eingetragen und registriert, research R7). **Keine neuen Abhängigkeiten**: Farbrechnung (OKLCH ↔ sRGB, WCAG-Kontrast) ist eine kleine eigene Funktion, weil jede Farbbibliothek mehr kostet als die ~120 Zeilen und das Ergebnis testbar sein muss.

**Storage**: Vault-Einstellungen (Tabelle `preferences`, Scope `vault`): `appearance.color_scheme` (bleibt, 023) und neu `appearance.theme` (JSON-String). Keine Migration, keine neue Tabelle.

**Testing**: `scripts/check-appearance.ts` (`node --test`, rein: Farbrechnung, Ableitung, Kontrast für alle Felder, Datei prüfen), `scripts/check-fields.ts` (Feldprüfung), Erweiterung von `check-settings.ts` und `check-agent-actions.ts`, e2e: `settings-color-scheme` (erweitert), neu `appearance-basic`, `appearance-sync-two-devices`, `fields-basic`; bestehende e2e (Passwortmanager, Einstellungen, Chat, Einrichtung) laufen unverändert.

**Target Platform**: Tauri 2 Desktop (Linux, macOS, Windows) und Android/iOS (Mobile-Ziele sind Pflicht); Farbwähler und Dateiauswahl müssen dort funktionieren, siehe research R7.

**Project Type**: Desktop-/Mobile-App (Tauri + Nuxt SPA).

**Performance Goals**: Eine Änderung wirkt im selben Frame-Zyklus wie die Auswahl (reine CSS-Variablen, kein Neuladen); Ableitung < 5 ms je Wechsel.

**Constraints**: Kein Speichern-Knopf; vor dem Öffnen der Vault gelten Standardwerte und nie die Darstellung einer früheren Vault; jede neue Aktion steht im Aktionskatalog (Spec 020/032); Dateien höchstens ~500 Zeilen; Tests in eigenen Dateien; keine Agenten-Attribution.

**Scale/Scope**: ~25 Dateien mit Feldern (Liste in research R6), 1 neue Bibliothek mit 6 Dateien, 1 Composable, 1 Settings-Gruppe mit ~6 Zeilen, 4 Aktionen, 3 e2e-Szenarien.

## Constitution Check

_GATE: Muss vor Phase 0 bestehen. Nach Phase 1 erneut geprüft._

| Prinzip                                          | Bewertung                                                                                            |
| ------------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| I Keine Geheimnisse in Git                       | Berührt nicht; Darstellungsdatei enthält nur Farbwerte.                                              |
| II Keine lokalen absoluten Pfade                 | Berührt nicht; Spec und Plan nennen keine Pfade außer Repo-relativen.                                |
| III Projektidentität geräteunabhängig            | Berührt nicht.                                                                                       |
| IV Fremdreferenzen mit unveränderlicher Revision | haex-ui ist per SHA gepinnt (`2dcb8bc`); die Pin-Änderung ist eine eigene Zeile in `nuxt.config.ts`. |
| V Externe Quellen opt-in                         | Keine neuen Quellen.                                                                                 |
| VI Änderung von Anweisungen nur per Review       | Keine Skills/Konstitution betroffen.                                                                 |
| VII Relay-Ausfall blockiert lokale Arbeit nicht  | Darstellung liegt in der lokalen Vault; Sync ist nachgelagert.                                       |
| VIII Keine Verheimlichungsanweisungen            | Berührt nicht.                                                                                       |

Arbeitsablauf: Worktree `035-ui-foundation`, Konventionelle Commits, PR nach `main`
(Rebase- oder Merge-Commit), keine Agenten-Attribution, Tests getrennt vom Produktivcode.
**Ergebnis: keine Verstöße, kein ADR nötig** (kein Kernprinzip berührt). Nach Phase 1 neu
bewertet: unverändert.

Graphify-Vorgabe: Vor jedem neuen benannten Artefakt in der Umsetzung ist `graphify query` zu
befragen; Kandidaten, die die Umsetzung erweitern statt neu zu bauen, sind schon bekannt:
`useColorScheme` (Anwendung der Klasse und Systembeobachtung), `usePreferences` (Lesen,
Schreiben), `settingsActions`/`settingsActionHandlers` (Aktionen), `SettingsGroup`/`SettingsRow`
(Gruppe), `scripts/e2e/lib/settings.ts` (`textContrast`, `backgroundContrast`, `choose`).

## Project Structure

### Documentation (this feature)

```text
specs/035-appearance-and-fields/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── appearance-actions.md
│   ├── appearance-file.md
│   └── token-map.md
└── tasks.md              # von /speckit-tasks
```

### Source Code (repository root)

```text
src/
├── lib/appearance/
│   ├── oklch.ts             # OKLCH <-> sRGB, Gamut-Begrenzung, Hex lesen/schreiben
│   ├── contrast.ts          # WCAG-Kontrast, Helligkeit nachführen bis Grenze erreicht
│   ├── presets.ts           # Farbfelder (Akzente, Tönungen) mit Kennung, Name-Schlüssel
│   ├── schema.ts            # Darstellung: Typ, Standard, parse (ungültig = nicht gesetzt), Datei
│   ├── derive.ts            # Darstellung + Schema -> Tabelle der CSS-Variablen (Kontrast erzwungen)
│   └── tokens.ts            # Namen der Tokens, die gesetzt werden (Vertrag: contracts/token-map.md)
├── composables/useAppearance.ts   # lädt, wendet an, schreibt; ruft useColorScheme
├── plugins/colorScheme.client.ts  # (besteht) startet Systembeobachtung; ruft auch useAppearance().start()
├── components/settings/
│   ├── AppearanceView.vue         # Kategorie „Darstellung“ (ersetzt ColorSchemeSetting als Inhalt)
│   ├── AccentSetting.vue          # Farbfelder + „+“
│   ├── TintSetting.vue            # eine Zeile Tönung (Fenster, Container, Text, Komponenten)
│   ├── ColorSwatches.vue          # gemeinsame Reihe Farbfelder + eigene Farbe
│   └── AppearanceFileButtons.vue  # Importieren / Exportieren / Zurücksetzen
├── components/wm/Window.vue       # Fensterhinweis: aktive Umrandung folgt Einstellung
├── lib/actions/settingsActions.ts            # + settings.appearance.{set,reset,export,import}
├── stores/settingsActionHandlers.ts          # + Handler
├── i18n/locales/{de,en}.json                 # + settings.appearance.*
└── (Felder) ~25 Dateien laut research R6

scripts/
├── check-appearance.ts            # rein; alle Farbfelder x beide Schemata
├── check-fields.ts                # Positivliste der alten Feldarten
└── e2e/scenarios/
    ├── settings-color-scheme.test.ts    # erweitert
    ├── appearance-basic.test.ts
    ├── appearance-sync-two-devices.test.ts
    └── fields-basic.test.ts
```

**Structure Decision**: Reine Logik in `src/lib/appearance/` (wie `src/lib/settings/colorScheme.ts`
und `src/lib/passwords/`), damit `node --test` sie ohne Nuxt prüft; Zustand und Anwendung in einem
Composable nach dem Muster von `useColorScheme`; Oberfläche als Komponenten unter
`components/settings/` mit dem bestehenden `Group`/`Row`. Kein neuer Rust-Code außer der Registrierung von `tauri-plugin-fs` (research R7).

## Phasen der Umsetzung

Die Spec erlaubt, den COSMIC-Umfang zu staffeln (Annahme „Umfang folgt dem Dialog…“). Der Plan
liefert in dieser Reihenfolge, jede Stufe für sich prüfbar und mergebar:

- **Stufe 1 – Felder (US1)**: Umstellung aller Felder, `check:fields`, e2e `fields-basic`. Unabhängig von der Darstellung.
- **Stufe 2 – Kern der Darstellung (US2 ohne Tönungen, US3, US4 teilweise, US5)**: Bibliothek, Composable, Gruppe mit Akzent, Fenster- und Container-Hintergrund, Schema, Zurücksetzen, Export/Import, Aktionen, Sync, `check:appearance`.
- **Stufe 3 – COSMIC-Rest (FR-013 Rest, FR-024)**: Texttönung, Komponententönung, Fensterhinweis.

Stufe 3 darf ausgelagert werden, ohne Stufe 1 und 2 zu verzögern.

## Complexity Tracking

Keine Verstöße. Eine begründete Abweichung vom einfachsten Weg: **eigene Farbrechnung statt
Bibliothek** (research R3), weil die Vorab-Prüfung aller Farbfelder (SC-004) am selben Code
laufen muss, der in der App rechnet, und weil keine neue Abhängigkeit hinzukommen soll.
