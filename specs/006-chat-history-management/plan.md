# Implementation Plan: Chat-Historie verwalten

**Branch**: `006-chat-history-management` | **Date**: 2026-09-15 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/006-chat-history-management/spec.md`

## Summary

Die bestehende Thread-Historie wird um drei sichtbare Fähigkeiten ergänzt:
eine laufend aktualisierte vergangene Dauer (`0min`, `1min`, `2h`, `5d`),
Titelbearbeitung und bestätigtes Löschen eines Threads samt Nachrichten. Die
Historienzeile nutzt den verfügbaren Raum für den Titel, richtet die Dauer
rechtsbündig aus und zeigt Bearbeiten/Löschen direkt links neben der Dauer an.
Die Arbeit nutzt die vorhandenen Thread-Modelle und Chat-CRUD-Grenzen. Neue
Persistenz für eine separate Session-Entität ist nicht vorgesehen.

## Technical Context

- **Language/Version**: Rust 1.77.2 / edition 2021, TypeScript 5, Vue 3, Nuxt 4 SPA
- **Primary Dependencies**: Tauri 2, haex-crdt, SQLite/SQLCipher, VueUse, `@nuxtjs/i18n`, vorhandene UI-Komponenten
- **Storage**: Bestehende per-Vault-Tabellen `chat_threads` und `chat_messages` über haex-crdt; keine neue Session-Tabelle
- **Testing**: `cargo test`, gezielte Chat-Rust-Tests, `pnpm typecheck`, `pnpm lint`, manuelle Szenarien aus `quickstart.md`
- **Target Platform**: Desktop primär, mit bedienbarer Darstellung in schmalen Viewports und Touch-Kontexten
- **Project Type**: Tauri-Desktop-App mit Rust-Backend und Nuxt-Frontend
- **Performance Goals**: Dauerangabe ohne zusätzliche Nutzeraktion aktualisieren; Titel- und Löschaktion nach abgeschlossener Persistenz ohne veralteten Verlaufseintrag anzeigen
- **Constraints**: `created_at` ist die alleinige Zeitbasis; sichtbare Einheiten sind `min`, `h`, `d`; neue Texte müssen in Deutsch und Englisch vorhanden sein; laufende Turns dürfen nicht stillschweigend verloren gehen
- **Scale/Scope**: Single-User-Vault, kleine bis mittlere Thread-Historie, ein aktiver Chat-Kontext pro Prozess

## Constitution Check

_GATE: Vor Phase 0 und nach Phase 1 erneut prüfen._

| Prinzip                                                 | Status | Begründung                                                                                                                             |
| ------------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| I. No Secrets in Git                                    | PASS   | Die Spec- und Plan-Artefakte enthalten keine Zugangsdaten oder Schlüssel.                                                              |
| II. No Local Absolute Paths in Versioned Config         | PASS   | Versionierte Dokumente verwenden nur repository-relative Pfade; der Research-Log nennt lokale Pfade nur als Nachweis der Konsultation. |
| III. Project Identity Is Device-Independent             | PASS   | Thread-Identität bleibt an der Vault und ihrer stabilen Thread-ID gebunden, nicht an einen Dateipfad.                                  |
| IV. Cross-Repo References Pin Immutable Revisions       | PASS   | Es werden keine neuen externen Quellen referenziert.                                                                                   |
| V. External Sources Are Opt-in Per Project              | PASS   | Keine Allowlist oder externe Harness-Quelle wird verändert.                                                                            |
| VI. Self-Modifying Instructions Are Always Review-Gated | PASS   | Keine Constitution-, Skill- oder Agent-Anweisung wird geändert.                                                                        |
| VII. Relay Unavailability Never Blocks Local Work       | PASS   | Anzeige, Titeländerung und Löschvertrag bleiben lokale Vault-Funktionen; Synchronisation ist nachgelagert.                             |
| VIII. No Concealment Instructions in Agent Output       | PASS   | Fehler, Bestätigung und Zustandswechsel bleiben sichtbar und nachvollziehbar.                                                          |

**Result**: Alle Gates PASS; kein Complexity-Tracking-Eintrag erforderlich.

## Research- und Designentscheidungen

Die Domänenrecherche und Graphify-Navigation stehen in [research.md](research.md).
Das fachliche Modell steht in [data-model.md](data-model.md), die externen
Command- und Frontend-Verträge in [contracts/tauri-commands.md](contracts/tauri-commands.md).

- `ChatThread` bleibt die bestehende Quelle für Titel, Eröffnungszeit und
  Thread-ID. `ThreadPayload` muss die für die Dauer benötigte Information
  weiterhin liefern.
- Die Historienzeile erhält eine flexible Titelspalte, eine feste
  rechtsbündige Zeitspalte und einen dazwischenliegenden Aktionsbereich. Der
  Aktionsbereich wird bei Hover oder Tastaturfokus eingeblendet und verkürzt
  den Titel im ausgeblendeten Zustand nicht dauerhaft.
- Die Dauer wird als flache, deterministische Präsentationsberechnung aus
  „jetzt minus Eröffnungszeit“ gebildet: ganze Minuten unter einer Stunde,
  ganze Stunden unter einem Tag, ganze Tage ab einem Tag; negative Werte
  werden zu `0min`.
- Die vorhandenen Storage-Kandidaten für Thread-Update und Thread-Löschung
  werden erweitert bzw. über öffentliche Commands genutzt. Es wird keine
  parallele Thread-Datenstruktur eingeführt.
- Die Löschung behandelt Thread und Nachrichten als eine fachliche Aktion und
  aktualisiert erst nach erfolgreicher Persistenz die sichtbare Historie.
- Für den fehlenden Agent-Kontext `CLAUDE.md` wurde kein neues
  Instruktionsdokument angelegt; die Planreferenz bleibt in diesem Feature-
  Verzeichnis und im `.specify/feature.json`-Pointer nachvollziehbar.

## Project Structure

### Documentation (this feature)

```text
specs/006-chat-history-management/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── tauri-commands.md
└── checklists/
    └── requirements.md
```

### Source Code

```text
src-tauri/src/
├── chat/
│   └── thread_commands.rs       # Rename-/Delete-Commands und Payload-Validierung
├── storage/
│   ├── chat_threads.rs           # Thread-Titel und Thread-Löschoperation
│   └── chat_messages.rs          # zugehörige Nachrichten beim Löschen
└── lib.rs                        # neue Commands registrieren

src/
├── composables/useChat.ts        # Thread-Verträge und CRUD-Aufrufe
├── pages/chat/[instance].vue     # Historienzeilen, Editier-/Löschzustand
└── i18n/locales/{de,en}.json     # neue sichtbare Texte in beiden Sprachen

src-tauri/src/chat/*_tests.rs     # Backend-Validierung und Persistenzfehler
src-tauri/tests/*chat*.rs         # Thread-/Nachrichten-Löschvertrag
scripts/check-chat-state.mjs      # bestehende Frontend-State-Regressionen erweitern
```

**Structure Decision**: Die Änderung bleibt in den bestehenden Chat-, Storage-
und i18n-Grenzen. Die Duration ist flüchtige UI-Projektion; Titel und Löschen
bleiben persistierte Thread-Aktionen. Neue generische UI- oder Storage-
Abstraktionen werden nur eingeführt, wenn die vorhandenen Kandidaten die
Anforderung nicht tragen.

## Umsetzungsphasen

### Phase 0 - Vertrag und Persistenz

- Rename- und Delete-Verträge inklusive Eingabevalidierung und strukturierten
  Fehlerfällen festlegen.
- Vorhandene Thread-Update- und Löschkandidaten wiederverwenden.
- Sicherstellen, dass eine bestätigte Löschung Thread und Nachrichten zusammen
  entfernt und keine Teiländerung sichtbar veröffentlicht.
- Backend-Tests für gültige Änderungen, ungültige Titel, Abbruch und
  Persistenzfehler ergänzen.

### Phase 1 - Historien-UI und Duration

- Thread-Zeilen um die sichtbare, rechtsbündige Dauer und den links daneben
  eingeblendeten Aktionsbereich ergänzen; der Titel nutzt den übrigen Raum.
- Deterministische Duration-Regeln sowie Aktualisierung an Minuten-, Stunden-
  und Tagesgrenzen abdecken.
- Titel-Editiermodus mit Speichern/Verwerfen und zugänglichen Zuständen
  integrieren.
- Löschbestätigung, aktiven Thread und Fehlerzustände integrieren.
- Deutsche und englische i18n-Schlüssel ergänzen und Fokus-/Touch-Fälle
  berücksichtigen.

### Phase 2 - Abnahme

- `cargo test` und gezielte Thread-/Chat-Tests ausführen.
- `pnpm typecheck`, `pnpm lint` und den bestehenden Chat-State-Check ausführen.
- Den manuellen Durchlauf aus [quickstart.md](quickstart.md) auf Desktop und
  in schmaler Fensterbreite dokumentieren.
- Constitution Check nach dem Design erneut bestätigen.

## Complexity Tracking

Keine Einträge — die bestehende Chat- und Storage-Struktur trägt die
Anforderungen ohne neue Persistenzschicht oder externe Abhängigkeit.
