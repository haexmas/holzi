# Implementation Plan: Chatfenster und Session-Handling

**Branch**: `004-chat-window-handling` | **Date**: 2026-09-13 | **Spec**: [spec.md](spec.md)

## Summary

Der Chat-Einstieg wird von der zuletzt geöffneten Unterhaltung entkoppelt:
Jeder Einstieg beginnt mit einem leeren, zunächst transienten Session-Entwurf;
erst beim ersten Senden entsteht ein neuer persistierter Thread. Der bestehende
Modell-Resolver aus Spec 002 wird nach dem erfolgreichen Vault-Open im Backend
als nicht-blockierender Preload gestartet. Sein Status wird global im Chat-
Runtime-State gehalten, sodass Workspace und Chat denselben Load sehen.

Die Chat-Seite erhält einen einheitlichen Composer mit kompakten Controls für
Modell, Effort und Freigabe, einer auto-wachsenden Textarea und einem pro
Antwort eingeklappten Reasoning-Accordion. Unterstützt das Modell Reasoning,
wird es automatisch verwendet; ein eigener Reasoning-Schalter entfällt.
Tool-Loop- und Turn-Semantik aus Spec 003 bleiben unverändert.

## Technical Context

**Language/Version**: Rust 1.77.2 / edition 2021, TypeScript 5, Vue 3, Nuxt 4 SPA
**Primary Dependencies**: Tauri 2, tokio, haex-crdt, VueUse, bestehende `@haex/ui`-Komponenten, `@nuxtjs/i18n`
**Storage**: Bestehendes SQLCipher/SQLite über haex-crdt; keine neue Migration für Session-Entwurf, Load-Status oder Accordion-Zustand
**Testing**: `cargo test`, gezielte Rust-Unit-/Integrationstests, `pnpm typecheck`, manuelle Chat-Smoke-Tests gemäß `quickstart.md`
**Target Platform**: Desktop primär; responsive Verhalten für schmale Viewports und mobile WebView mitdenken
**Project Type**: Tauri-Desktop-App mit Rust-Backend und Nuxt-Frontend
**Performance Goals**: Vault-Open blockiert nicht auf Modell-Load; gleicher Modell-Load wird nicht doppelt gestartet; Composer bleibt bei langen Eingaben benutzbar
**Constraints**: Ein aktives Modell und eine aktive Vault pro App-Prozess; `ChatState.operation` serialisiert Modell-/Vault-Operationen; Reasoning ist in diesem Feature nicht persistent
**Scale/Scope**: Single-User, kleine Thread-Historie und ein aktiver Chat-Runtime-State pro Prozess

## Constitution Check

_GATE: Vor Phase 0 und nach Phase 1 erneut prüfen._

| Prinzip                                                 | Status | Begründung                                                                                          |
| ------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------- |
| I. No Secrets in Git                                    | PASS   | Die Änderung führt keine Secrets oder Test-Credentials ein.                                         |
| II. No Local Absolute Paths in Versioned Config         | PASS   | Dokumente verwenden nur repository-relative Pfade; Laufzeitpfade bleiben Resolver-Logik.            |
| III. Project Identity Is Device-Independent             | PASS   | Session-, Load- und UI-Zustände bleiben runtime-/gerätebezogen; Vault-Identität bleibt unverändert. |
| IV. Cross-Repo References Pin Immutable Revisions       | PASS   | Keine neue externe Harness-Referenz.                                                                |
| V. External Sources Are Opt-in Per Project              | PASS   | Kein externer Harness-Content betroffen.                                                            |
| VI. Self-Modifying Instructions Are Always Review-Gated | PASS   | Keine Constitution-, Skill- oder Agent-Instruktion wird geändert.                                   |
| VII. Relay Unavailability Never Blocks Local Work       | PASS   | Session-Entwurf, Composer und lokale Modell-Loads bleiben lokal nutzbar.                            |
| VIII. No Concealment Instructions in Agent Output       | PASS   | Load-, Fehler- und Tool-Zustände bleiben nachvollziehbar.                                           |

**Result**: Alle Gates PASS; kein Complexity-Tracking-Eintrag erforderlich.

## Research- und Designentscheidungen

Die begründeten Entscheidungen stehen in [research.md](research.md). Runtime-
Typen und Zustandsübergänge stehen in [data-model.md](data-model.md), die
Tauri-Oberfläche in [contracts/tauri-commands.md](contracts/tauri-commands.md).

Die zentrale Reihenfolge ist:

1. `create_instance`/`open_instance` veröffentlichen die neue aktive Vault.
2. Danach startet ein interner, abbruchbarer Default-Model-Preload.
3. Workspace und Chat fragen den globalen Status ab und abonnieren Status-Events.
4. Der Chat erzeugt beim Einstieg nur einen transienten Session-Entwurf.
5. Erst `send_message(threadId: null)` legt den neuen Thread an.

## Project Structure

### Documentation (this feature)

```text
specs/004-chat-window-handling/
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
│   ├── session.rs                 # globaler Model-Load-Status und Generation
│   └── commands.rs                # Load-Internals, Preload, Status-Command/Event
├── instances/
│   ├── create.rs                  # Preload nach Genesis-Publikation starten
│   └── open.rs                    # Preload nach Open/Switch starten
└── lib.rs                         # Status-Command/Event registrieren

src/
├── composables/useChat.ts         # Load-Status und Frontend-Event-Listener
├── pages/chat/[instance].vue      # Session, Composer, Textarea, Reasoning-State
├── pages/workspace/[instance].vue # optionaler dezenter Preload-Status
├── components/chat/               # Composer-Controls und Reasoning-Accordion
└── i18n/locales/{de,en}.json      # neue UI-Texte in beiden Sprachen
```

**Structure Decision**: Die Änderung bleibt in der bestehenden Tauri-/Nuxt-
Struktur. Der Preload gehört zum aktiven Vault-/Chat-Runtime-State, nicht in
die Datenbank. Der transienten Chat-Session wird keine neue Tabelle hinzugefügt.

## Umsetzungsphasen

### Phase 0 - Runtime- und Lifecycle-Grundlage

- Gemeinsame interne Load-Funktion aus `load_model` herauslösen.
- Globalen Load-Status inklusive Load-Generation und Fehlerzustand in
  `ChatState` ergänzen.
- Preload nach erfolgreicher Genesis-/Open-Publikation starten.
- Veraltete Preloads daran hindern, nach einem Vault- oder Modellwechsel den
  aktiven Status zu überschreiben.
- Event-Kanal und Snapshot-Abfrage konsistent machen.

### Phase 1 - Neuer Chat-Einstieg

- Automatische Auswahl des ersten Threads aus `refreshThreads` entfernen.
- Beim Chat-Mount einen transienten neuen Session-Entwurf beginnen.
- Vorhandene Threads weiterhin explizit auswählbar lassen.
- Sicherstellen, dass `send_message` mit `threadId: null` den neuen Thread
  erzeugt und ein verlassener Entwurf keinen leeren Historieneintrag hinterlässt.

### Phase 2 - Composer und Textarea

- Separate Konfigurationszeile in den gemeinsamen Composer-Container verlagern.
- Kompakte, zugängliche Controls für Modell, Effort und Freigabe implementieren;
  ein Reasoning-Control wird nicht mehr angeboten.
- Textarea-Höhe anhand des Inhalts synchronisieren, mit maximal 8 sichtbaren
  Zeilen und internem Scrollen.
- `Enter`-/`Shift+Enter`-Verhalten explizit testen.

### Phase 3 - Reasoning-Darstellung

- Pro Assistant-Nachricht mit Reasoning ein geschlossenes Disclosure-Element
  darstellen.
- Öffnen/Schließen unabhängig pro Nachricht ermöglichen.
- Streaming-Deltas bei geöffnetem Accordion live ergänzen.
- Reasoning- und Accordion-Zustände beim neuen Chat-Einstieg zurücksetzen.

### Phase 4 - Abnahme

- Rust-Tests für Preload-Status, Generation Race und kein Doppel-Load.
- `pnpm typecheck` und i18n-Key-Abgleich für `de`/`en`.
- Manueller Durchlauf aus [quickstart.md](quickstart.md) auf Desktop sowie in
  schmaler Fensterbreite.

## Complexity Tracking

Keine Einträge — die bestehende Runtime-Struktur reicht aus; es wird keine neue
Persistenz- oder Projektstruktur eingeführt.
