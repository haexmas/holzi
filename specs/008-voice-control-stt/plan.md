# Implementation Plan: Voice Control (Local Speech-to-Text)

**Branch**: `008-voice-control-stt` | **Date**: 2026-09-16 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/008-voice-control-stt/spec.md`

## Summary

Der Chat bekommt eine Push-to-Talk-Sprachaufnahme: Mic-Kontrolle in der Chat-Ansicht, lokale
Transkription über ein mitgeliefertes Whisper-Modell (candle-transformers, teilt sich den
bestehenden candle-Stack mit der lokalen Chat-Inferenz aus `mistralrs`), Transkript landet im
Eingabefeld und wird — per Default — automatisch abgeschickt. Drei feste Wörter (STOP, HALT,
ABBRECHEN) werden lokal, deterministisch und unabhängig vom Assistenten-Zustand erkannt und
brechen einen laufenden Turn sofort ab. Transkription wird als zweite Capability (`chat` |
`transcription`) auf der bestehenden `Provider`/`ProviderAdapter`-Abstraktion ergänzt, statt ein
paralleles System zu bauen; Nutzer können zusätzlich einen externen Transkriptions-Dienst
konfigurieren. Volles Design: [docs/plans/2026-09-16-voice-control-stt-design.md](../../docs/plans/2026-09-16-voice-control-stt-design.md).

Ausdrücklich außerhalb des Scopes: neue sprachgesteuerte App-Kommandos jenseits der drei
Interrupt-Wörter. Alles andere läuft unverändert durch die bestehende Chat-/Agent-Tool-Loop
(spec 003).

## Technical Context

**Language/Version**: Rust 1.77.2 / edition 2021 (Backend); TypeScript 5, Vue 3, Nuxt 4 SPA
(Frontend)
**Primary Dependencies**: Tauri 2; `candle-transformers` (neu, gleiche Version wie das bereits
über `mistralrs` gepinnte `candle-core`/`candle-nn` 0.10.2) für die lokale Whisper-Inferenz; `cpal`
(neu) für plattformübergreifende Mikrofon-Aufnahme; bestehendes `reqwest` 0.12 mit `rustls-tls`
für den externen Transkriptions-Adapter; bestehende `Provider`/`ProviderAdapter`-Infrastruktur
(`src-tauri/src/providers/`, `src-tauri/src/adapters/`); bestehende `@haex/ui`-Komponenten und
`@nuxtjs/i18n`
**Storage**: Bestehende SQLCipher-/SQLite-Datenbank über haex-crdt; additive
Capability-Unterscheidung auf der bestehenden `providers`-Tabelle (kein neues Storage-Schema);
Audio selbst wird nicht persistiert (FR-020) — nur In-Memory-Puffer während Aufnahme/Transkription
**Testing**: `cargo test`; feste Audio-Fixture mit bekanntem Transkript für den lokalen
Whisper-Adapter (kein reales Mikrofon); `wiremock` für die HTTP-Grenze des externen
Transkriptions-Adapters; `pnpm typecheck`; das Repository hat aktuell keinen Frontend-Test-Runner
(kein `vitest`/Component-Test-Setup) — Frontend-Verifikation läuft wie bei den bestehenden Specs
über `pnpm typecheck` plus manuelle Verifikation gemäß [quickstart.md](quickstart.md), statt dafür
neu ein Test-Framework einzuführen
**Target Platform**: Tauri Desktop (Linux/macOS/Windows) und Mobile (iOS/Android) — beide von
Beginn an, da Aktivierung reines Push-to-Talk ist (kein Streaming, keine plattformspezifische
Always-on-Mic-Logik nötig). Die Mobile-Zusage gilt nur, wenn der in T032–T034 definierte
Readiness-Gate (Berechtigungen, cpal-Backends und beide Build-Ziele) erfolgreich ist.
**Project Type**: Tauri-App mit Rust-Backend und Nuxt-Frontend
**Performance Goals**: SC-001 — Transkript erscheint innerhalb von 5s nach Loslassen der
Mic-Kontrolle für eine typische Ein-Satz-Nachricht; SC-002 — der lokale Interrupt-Fast-Path
stoppt eine laufende Antwort innerhalb von 1s nach dem finalen lokalen Audio-Frame, unabhängig vom
Zustand des Assistenten oder dem gewählten STT-Provider
**Constraints**: Lokale Transkription MUSS ohne Netzwerkzugriff funktionieren (FR-003); kein
Audio-Persisting (FR-020); Sprachsteuerung nur im Vordergrund und nur in der Chat-Ansicht
(FR-018); das mitgelieferte lokale Modell muss ohne Download sofort nutzbar sein (FR-010);
Interrupt-Erkennung darf nicht vom Zustand/der Verfügbarkeit des Assistenten abhängen (FR-008)
**Scale/Scope**: Single-User-App; genau eine aktive Transkriptionsquelle zur Zeit; kurze
Push-to-Talk-Äußerungen (Sekunden bis maximal ~60s gedeckelt), kein Streaming/Dauerlisten

**Feature boundary**: `voice` ist ein unabhängiges Cargo-Feature. Es aktiviert `cpal`, die
Aufnahme und die Voice-Commands; `llm-cpu` aktiviert zusätzlich den gebündelten Candle-Whisper-
Adapter. Damit kompilieren `--no-default-features` und externe Transkription mit
`--no-default-features --features voice` ohne ungelöste `cpal`- oder Candle-Referenzen. Wenn nur
`voice` aktiv ist, bleibt der externe STT-Pfad verfügbar und die lokale Quelle meldet
`LocalSttUnavailable`; im normalen Default-Build sind beide Features aktiv.

**Mobile readiness gate**: Vor einer Implementierung, die iOS/Android als unterstützt ausweist,
müssen Tauri-Mikrofonberechtigungen in den jeweiligen Plattform-Manifesten erklärt, die
`cpal`-Backends für beide Zielplattformen verifiziert und erfolgreiche iOS- und Android-Builds
ausgeführt werden (T032–T034). Schlägt ein Gate fehl, wird die Plattform aus dem Zielumfang
genommen, bis die Lücke durch eine überprüfbare Änderung geschlossen ist.

## Constitution Check

_GATE: Vor Phase 0 und nach Phase 1 erneut prüfen._

| Prinzip                                                 | Status | Begründung                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| I. No Secrets in Git                                    | PASS   | Externe STT-Zugangsdaten laufen über den bestehenden Credential-Storage-Pfad der Provider; Tests verwenden keine echten Schlüssel.                                                                                                                                                                                                                                                                                                                                                                                                                               |
| II. No Local Absolute Paths in Versioned Config         | PASS   | Alle Dokumente nutzen repo-relative Pfade.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| III. Project Identity Is Device-Independent             | PASS   | Kein neues Identitätskonzept; die Geräteklassen-Erkennung für die Modellgröße ist bereits bestehende, gerätelokale Logik aus spec 002.                                                                                                                                                                                                                                                                                                                                                                                                                           |
| IV. Cross-Repo References Pin Immutable Revisions       | PASS   | `candle-transformers`/`cpal` sind normale Cargo-Abhängigkeiten mit gepinnter Version, keine Harness-Content-Referenz.                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| V. External Sources Are Opt-in Per Project              | PASS   | Keine neue Harness-/Skill-Quelle.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| VI. Self-Modifying Instructions Are Always Review-Gated | PASS   | Keine Constitution-/Skill-/Permission-Datei wird geändert.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| VII. Relay Unavailability Never Blocks Local Work       | PASS   | Die lokale Transkription funktioniert vollständig offline; nur der optionale externe Adapter braucht Netzwerk, und nur wenn der Nutzer ihn explizit aktiviert.                                                                                                                                                                                                                                                                                                                                                                                                   |
| VIII. No Concealment Instructions in Agent Output       | PASS   | Fehlerzustände (fehlende Mikrofon-Berechtigung, gescheiterte Transkription, ungültige externe Zugangsdaten) werden sichtbar gemeldet (FR-013, FR-015).                                                                                                                                                                                                                                                                                                                                                                                                           |
| 500 LoC boundary                                        | PASS   | Audio-Capture, STT-Adapter-Trait, lokaler Whisper-Adapter, externer Adapter und Interrupt-Matcher sind als separate, schmale Module geplant (siehe Projektstruktur unten); keine künstliche Aufteilung nur wegen Zeilenzahl.                                                                                                                                                                                                                                                                                                                                     |
| Graphify-first authoring                                | PASS   | Vor der Planung gegen `graphify-out/graph.json` konsultiert: `ProviderAdapter`/`Provider` (`src-tauri/src/providers/mod.rs`) und `CancellationToken` in `ChatState`/`session.rs` (`src-tauri/src/chat/session.rs`) bestätigt als die zu erweiternden Kandidaten; für Audio-Aufnahme, lokale Whisper-Inferenz und Interrupt-Matching existiert im Graphen kein verwandter Kandidat (Suche nach "audio/cpal/microphone/whisper/speech-to-text" ergab keine Treffer außerhalb unrelated Text-Streaming-Code) — diese Teile sind bewusst neue, eigenständige Module. |

**Result**: Alle Gates PASS; kein Complexity-Tracking-Eintrag erforderlich.

## Project Structure

### Documentation (this feature)

```text
specs/008-voice-control-stt/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── checklists/
│   └── requirements.md
└── contracts/
    └── tauri-commands.md
```

### Source Code (repository root)

```text
src-tauri/src/
├── audio/
│   ├── mod.rs                   # cpal-Aufnahme: Start/Stop, In-Memory-PCM-Puffer, Max-Dauer-Cap
│   └── audio_tests.rs
├── stt/
│   ├── mod.rs                   # CanonicalPcm + schmaler SttAdapter-Trait
│   ├── local.rs                 # LocalWhisperAdapter (candle-transformers, gebündeltes Modell,
│   │                             #   Tier-Wahl über bestehende Hardware-Erkennung aus spec 002)
│   ├── local_tests.rs           # feste Audio-Fixture, bekanntes Transkript, kein Netzwerk
│   ├── external.rs              # ExternalSttAdapter (HTTP, vendor discriminator + Credentials)
│   ├── external_tests.rs        # wiremock-Grenze, Fehler-Mapping analog AnthropicAdapter
│   └── interrupt.rs             # reine Matcher-Funktion: Transkript -> Option<InterruptCommand>
│       interrupt_tests.rs
├── providers/
│   └── mod.rs                   # Capability-Feld (chat | transcription) auf Provider/CRUD ergänzt
├── storage/
│   └── providers.rs             # Migration: Capability-Spalte auf der bestehenden providers-Tabelle
├── chat/
│   └── session.rs                # Interrupt-Matcher an das bestehende CancellationToken angebunden
└── lib.rs                        # neue Tauri-Commands registrieren (Aufnahme starten/stoppen,
                                   #   STT-Provider verwalten)

src/
├── components/chat/
│   └── VoiceInputControl.vue     # Mic-Kontrolle neben dem Send-Button, Zustände idle/recording/
│                                 #   transcribing/error, sichtbares Signal bei externer Quelle
├── stores/
│   └── voiceSettings.ts          # aktive Transkriptionsquelle, Auto-Send-Toggle
└── pages/models/                 # bestehende "Modelle verwalten"-Fläche um Transkriptions-Tab
                                   #   erweitert (kein neuer eigenständiger Screen)
```

**Structure Decision**: Neue, schmale `audio/` und `stt/` Module statt Erweiterung bestehender
`chat/` oder `providers/`-Dateien — die Provider/CRUD-Erweiterung selbst bleibt im bestehenden
`providers/mod.rs`, aber die eigentliche Aufnahme- und Transkriptionslogik hat keinen verwandten
Kandidaten im Graphen (siehe Constitution Check) und bekommt daher eigene Module. Der
Interrupt-Matcher ist bewusst kein Adapter, sondern eine reine Funktion nahe am bestehenden
Cancellation-Pfad in `chat/session.rs`, nicht im `stt/`-Modul selbst — er ist unabhängig davon,
welcher STT-Adapter den Text geliefert hat.

## Complexity Tracking

_Keine Gate-Verletzungen — dieser Abschnitt entfällt._
