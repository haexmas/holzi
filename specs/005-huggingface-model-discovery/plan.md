# Implementation Plan: Freie Hugging-Face-Modellsuche und Installation

**Branch**: `005-huggingface-model-discovery` | **Date**: 2026-09-14 | **Spec**: [spec.md](spec.md)

## Summary

Die bestehende lokale Modellverwaltung wird um einen anonymen Hugging-Face-
Discovery-Client erweitert. Nutzer können öffentliche Repositories suchen,
kompatible GGUF-Dateien auswählen, deren Metadaten prüfen und sie über denselben
atomaren Download-/Registrierungspfad wie Katalogmodelle installieren. Die
kuratierte Qwen3-Auswahl bleibt separat bestehen.

Die Discovery-Logik erhält eine schmale Grenze innerhalb des bestehenden
`src-tauri/src/models`-Bereichs. Sie normalisiert Hub-Antworten, filtert nicht
installierbare Dateien und liefert strukturierte Payloads an das Frontend. Die
UI ergänzt die vorhandene Modellverwaltung bzw. den bestehenden Picker; sie
führt keine zweite lokale Registry ein.

## Technical Context

**Language/Version**: Rust 1.77.2 / edition 2021; TypeScript 5, Vue 3, Nuxt 4 SPA
**Primary Dependencies**: Tauri 2, Tokio, bestehendes `reqwest` 0.12 mit `rustls-tls`, `serde`, bereits gelocktes `sha2` als direkter Hashing-Baustein, `haex-crdt`, bestehende `@haex/ui`-Komponenten und `@nuxtjs/i18n`
**Storage**: Bestehende SQLCipher-/SQLite-Datenbank über haex-crdt; lokale GGUF-Dateien unter dem verwalteten Modellverzeichnis; voraussichtlich additive Metadatenfelder in `models`
**Testing**: Rust-Unit-/Integrationstests, `wiremock` für HTTP-Grenzen, `cargo test`, `pnpm typecheck`, manuelle Verifikation gemäß [quickstart.md](quickstart.md)
**Target Platform**: Tauri-Desktop-App; responsive Modellverwaltung für schmale Fenster
**Project Type**: Tauri-Desktop-App mit Rust-Backend und Nuxt-Frontend
**Performance Goals**: Suchantworten werden begrenzt; UI bleibt während Suche und Download interaktiv; Downloads und HTTP-Parsing blockieren keinen Async-Executor
**Constraints**: Nur öffentliche Hugging-Face-Daten, nur GGUF im ersten Schnitt, keine Secrets oder Hub-Tokens; bestehende Modell- und Fallback-Semantik bleibt erhalten
**Scale/Scope**: Single-User, wenige installierte lokale Modelle, begrenzte Suchergebnislisten; kein eigener Suchindex

## Constitution Check

_GATE: Vor Phase 0 und nach Phase 1 erneut prüfen._

| Prinzip                                                 | Status | Begründung                                                                                                                           |
| ------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------ |
| I. No Secrets in Git                                    | PASS   | Der anonyme Zugriff verwendet keine Zugangsdaten; Tests nutzen keine realen Tokens.                                                  |
| II. No Local Absolute Paths in Versioned Config         | PASS   | Dokumente verwenden nur repository-relative Pfade; lokale Zielpfade werden zur Laufzeit ermittelt.                                   |
| III. Project Identity Is Device-Independent             | PASS   | Hugging-Face-Quellen werden als Repository-/Datei-Metadaten gespeichert; lokale Modellpfade bleiben gerätebezogen.                   |
| IV. Cross-Repo References Pin Immutable Revisions       | PASS   | Es wird keine neue externe Harness-Referenz eingeführt. Hub-Dateien sind Nutzerdaten, keine Projektabhängigkeit.                     |
| V. External Sources Are Opt-in Per Project              | PASS   | Keine neue Harness- oder Skill-Quelle wird zugelassen.                                                                               |
| VI. Self-Modifying Instructions Are Always Review-Gated | PASS   | Keine Agenten-, Skill- oder Constitution-Datei wird geändert.                                                                        |
| VII. Relay Unavailability Never Blocks Local Work       | PASS   | Katalog und installierte Modelle bleiben offline verfügbar; nur Discovery/Download benötigen Netzwerk.                               |
| VIII. No Concealment Instructions in Agent Output       | PASS   | Download-, Validierungs- und Fehlerzustände werden sichtbar und strukturiert behandelt.                                              |
| 500 LoC boundary                                        | PASS   | Discovery, Normalisierung und Commands werden an bestehenden Modulgrenzen gehalten; keine künstliche Aufteilung nur nach Zeilenzahl. |
| Graphify-first authoring                                | PASS   | Vor der Planung wurde Graphify aktualisiert und zu bestehender Modellverwaltung, Commands und Frontend-Modellfluss konsultiert.      |

**Result**: Alle Gates PASS; kein Complexity-Tracking-Eintrag erforderlich.

## Research- und Designentscheidungen

Die Begründungen stehen in [research.md](research.md). Domänenobjekte,
Persistenzänderungen und Zustandsübergänge stehen in [data-model.md](data-model.md);
die Tauri-Grenze steht in [contracts/tauri-commands.md](contracts/tauri-commands.md).

## Project Structure

### Documentation

```text
specs/005-huggingface-model-discovery/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
└── contracts/
    └── tauri-commands.md
```

### Source Code

```text
src-tauri/src/
├── models/
│   ├── commands.rs              # Discovery-/Download-Commands und Payload-Join
│   ├── download.rs              # bestehender atomarer Downloadpfad
│   ├── paths.rs                 # bestehende Slug-/Dateiname-Validierung
│   └── huggingface.rs           # HTTP-Client, Normalisierung und GGUF-Filter
├── catalog/
│   └── mod.rs                   # kuratierte Qwen3-Katalogdaten und Match-Abgleich
├── hardware/
│   └── ...                      # bestehende Fit-Klassifikation wiederverwenden
├── storage/
│   └── models.rs                # Quelle, SHA-256 und Integritätsstatus persistieren
├── identity/
│   └── migrations.rs            # models-Migrationen
└── lib.rs                       # neue Commands registrieren

src/
├── composables/
│   ├── useModels.ts              # bestehender Installations-Contract erweitern
│   └── useHuggingFace.ts         # Discovery-State und Tauri-Aufrufe
├── components/models/
│   ├── HuggingFaceSearch.vue
│   ├── HuggingFaceResult.vue
│   └── HuggingFaceFilePicker.vue
├── pages/settings/[instance].vue # Einstieg/Einbettung der Modellverwaltung
├── pages/chat/[instance].vue     # ggf. Wiederverwendung nach Installation
└── i18n/locales/{de,en}.json     # Such-, Metadaten-, Warn- und Fehlertexte
```

**Structure Decision**: Die Erweiterung bleibt in den vorhandenen `models`,
`storage`, Settings- und Chat-Grenzen. Ein eigener Discovery-Adapter ist eine
echte externe Systemgrenze und verhindert, dass HTTP-/Hub-Parsing in den
Tauri-Commands oder Vue-Seiten dupliziert wird. Die bestehende
`download_model_from_hf`-Routine und `canonical_model_file` bleiben die
autoritativen Installations-/Pfadgrenzen.

## Graphify-Konsultation und Erweiterungskandidaten

Die Abfrage zu Modellverwaltung, Hugging-Face-Download und Picker hat folgende
Kandidaten ergeben:

| Kandidat                                                                                       | Bewertung                                                | Erweiterung                                                                                                                            |
| ---------------------------------------------------------------------------------------------- | -------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/models/commands.rs` (`download_model_from_hf`)                                  | passt: bestehender Download-/Registrierungsboundary      | Discovery-Commands und normalisierte Installationsübergabe; erwartete Einsparung ca. 40–60 Zeilen gegenüber einem zweiten Downloadpfad |
| `src-tauri/src/models/paths.rs` (`validate_slug`, `validate_filename`, `canonical_model_file`) | passt: bestehende lokale Sicherheits-/Dateiauswahlgrenze | unverändert wiederverwenden; kein paralleler Pfad-Validator                                                                            |
| `src-tauri/src/storage/models.rs` (`ModelRow`, `upsert_model`)                                 | passt: gemeinsame Modellmetadaten                        | Source-Felder ergänzen; kein separates HF-Register                                                                                     |
| `src/composables/useModels.ts` (`downloadFromHfAsync`)                                         | passt: bestehender Installationsaufruf                   | Preview-/Source-Argumente erweitern; kein zweites Download-Composable                                                                  |
| `src/pages/chat/[instance].vue` (Modellgruppen/Refresh)                                        | passt: bestehender Picker-Refresh                        | installierte HF-Modelle über dieselbe `InstalledModel`-Liste anzeigen                                                                  |

Der unabhängige Kandidat `src-tauri/src/models/huggingface.rs` kapselt nur die
neue externe HTTP-Grenze (z. B. `search`, `details`, `normalize_files`) und ist
kein Duplikat eines bestehenden Artefakts. Ein konkreter Rewrite-Call-Site ist
`download_model_from_hf`: Der neue Settings-Flow ruft nach
`preview_huggingface_install` denselben Command mit dem normalisierten
`HuggingFaceInstallRequest` auf. Das Backend mappt diese UI-Absicht vor dem
Command-Aufruf in `DownloadFromHfArgs`, übernimmt die aus dem Preview
abgeleitete `modelId` und löst die Revision über die Datenmodell-Konvertierung
in eine konkrete Commit-SHA auf. So entsteht kein zweiter Datei-Download und
die UI kann keine abweichende Modell-ID oder unaufgelöste Revision
einschleusen.

## Umsetzungsphasen

### Phase 0 - HTTP- und Metadaten-Contract

- Hugging-Face-Client auf Basis des bereits vorhandenen `reqwest` definieren.
- Such- und Repository-Antworten in interne, präzise Rust-Typen normalisieren.
- Öffentliche HTTPS-Hub-URLs, Repository-IDs und Dateinamen validieren.
- GGUF-Erkennung, Ergebnislimit, Deduplizierung und deterministische Sortierung
  als pure bzw. mockbare Logik festlegen; das Standardlimit beträgt 20 Treffer
  pro Seite.
- Tokenizer-Auflösung als explizites Ergebnis modellieren: sicher ermittelt,
  vorgefüllt, oder Nutzerangabe erforderlich.
- Quantisierungs-/Kontext-Provenienz (`hub_metadata`, Dateiname, GGUF-Header,
  unbekannt) im normalisierten Ergebnis mitführen. Ein Range-Request auf den
  GGUF-Header ist nur für eine sicherheitsrelevante `TooBig`-Entscheidung nötig.
- Mutable HF-Refs vor jedem Download in eine konkrete Commit-SHA auflösen;
  optionalen Branch/Tag zusätzlich für spätere Update-Prüfungen speichern.
- Nach atomarer Veröffentlichung bzw. Import den SHA-256-Hash berechnen und in
  `models.file_sha256` speichern. Der Ladepfad muss unmittelbar vor jedem
  Runtime-Load denselben Hash erneut berechnen und bei Abweichung den Nutzer
  entscheiden lassen.

### Phase 1 - Persistenz und Backend-Commands

- Prüfen, welche HF-Quellfelder die bestehende `models`-Tabelle bereits abdeckt;
  fehlende Repository-/Datei-/Revisionsfelder einschließlich
  `hf_revision_ref` additiv und CRDT-kompatibel ergänzen.
- `file_sha256` und `integrity_status` in der gemeinsamen `models`-Zeile
  persistieren. Der SHA beschreibt den Dateiinhalt und ist daher unabhängig vom
  Gerät; ein lokaler Mismatch wird als Nutzerentscheidung behandelt.
- Bestehenden `download_model_from_hf`-Pfad so erweitern, dass Discovery-
  Payloads direkt verwendet werden können und die bestehende Registrierung,
  Publication-Lock- und Progress-Semantik erhalten bleibt.
- Neue Commands für Suche, Repository-/Datei-Details und optionalen
  Installations-Preview registrieren.
- Einen Update-Check für installierte HF-Modelle mit `hf_revision_ref`
  registrieren; er vergleicht nur öffentliche Refs und verändert lokale Daten
  erst nach einer ausdrücklichen Update-Installation.
- Strukturierte Fehler für HTTP, Timeout, Rate-Limit, Validierung,
  fehlenden Tokenizer und Hardware-Bestätigung an die Frontend-Grenze liefern.
- Keine Installation als erfolgreich registrieren, bevor die finale GGUF-Datei
  atomar veröffentlicht und der Metadatensatz geschrieben ist.
- Katalog-/HF-Downloads und Importe erst nach erfolgreicher SHA-256-Berechnung
  als ladbar markieren; beim lokalen Load vor `LocalModel::load` eine vollständige
  Hash-Prüfung in `spawn_blocking` erzwingen.
- Historische Dateien ohne Hash als `unknown` behandeln und im Integritätsdialog
  dieselben drei Entscheidungen anbieten: unsicher laden, erneut herunterladen
  bzw. neu importieren oder ein anderes Modell auswählen.

### Phase 2 - Frontend-Modellverwaltung

- Eine dauerhaft erreichbare, zusammenhängende Modellverwaltung in Settings
  oder einer gemeinsamen Modellseite bereitstellen. Sie enthält Katalog,
  freie Suche, installierte Modelle, aktives Modell, Löschen und Update-Hinweise
  unabhängig davon, ob das Onboarding bereits abgeschlossen ist.
- In dieser Oberfläche einen Einstieg „Modell von Hugging Face suchen“ ergänzen.
- Suche mit explizitem Submit, Loading-, Empty-, Offline- und Retry-Zustand
  bauen; nicht bei jedem Tastaturanschlag Netzwerkanfragen starten.
- Treffer als Repository-Karten und konkrete GGUF-Dateien als auswählbare
  Einträge darstellen.
- Größe, Quantisierung, Kontextfenster, Lizenzstatus und Hardware-Fit vor dem
  Download anzeigen; `TooBig` erfordert eine zweite Bestätigung.
- Tokenizer-Repository vor dem Download anzeigen und bei Unsicherheit als
  Pflichtfeld abfragen.
- Bestehende Download-Fortschrittsanzeige wiederverwenden und nach Abschluss
  installierte Modelle aktualisieren sowie optional direkt laden.
- Beim Öffnen bzw. expliziten Aktualisieren der Modellverwaltung den
  Upstream-Stand verfolgen, neue SHAs sichtbar melden und eine manuelle
  Aktualisierung über denselben atomaren Downloadpfad anbieten.
- Integritätsstatus in der installierten Modellliste anzeigen; bei fehlender
  Referenz, Dateifehler oder Hash-Mismatch das Laden zunächst blockieren und
  einen lokalisierte Integritätsdialog mit drei Entscheidungen anbieten.
- Freie Suchtreffer mit exaktem Katalog-Repository und Dateinamen als
  Katalogtreffer markieren und zum Katalogeintrag verlinken; keine automatische
  ID-Zusammenführung.
- Alle neuen Texte in `de` und `en` ergänzen.

### Phase 3 - Regression und Abnahme

- Parser-/Filter-/Sortier- und Validierungstests mit Fixtures ergänzen.
- HTTP-Suche, Timeout, Fehlerstatus, Redirect und fehlende Metadaten mit
  `wiremock` abdecken.
- Download-Abbruch und atomare Sichtbarkeit testen.
- Prüfen, dass Katalog-Qwen3, Provider-Modelle, lokale Importe und die
  Spec-002-Fallback-Kette unverändert funktionieren.
- `cargo test`, `pnpm typecheck`, `git diff --check` und manuellen Quickstart
  ausführen.

## Constitution Check (nach Phase 1)

| Prinzip                               | Status | Re-Check                                                                                                              |
| ------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------- |
| Keine Secrets / keine lokalen Pfade   | PASS   | HF-Zugriff bleibt anonym; Zielpfade werden ausschließlich aus validierten Werten gebildet.                            |
| Input-Validation und Fehlerbehandlung | PASS   | Repository, Datei, URL, Dateiformat, Tokenizer und Fehlerzustände liegen an der Boundary.                             |
| Async Rust                            | PASS   | HTTP-/Dateioperationen bleiben async; CPU-/Dateisystem-Parsing wird nicht unkontrolliert auf dem Executor ausgeführt. |
| Bestehende Artefakte erweitern        | PASS   | `useModels`, `download_model_from_hf`, `models.rs`, `paths.rs` und vorhandene UI-Flows werden wiederverwendet.        |
| Testbarkeit                           | PASS   | HTTP-Grenze ist mockbar; Parser und Filter sind ohne Netzwerk testbar.                                                |

**Result**: Keine neue Abweichung von Constitution oder Projektstruktur.

## Complexity Tracking

Keine Einträge — die neue Discovery-Grenze entspricht einer bestehenden
externen Systemgrenze und ersetzt keine bestehende Modellverwaltung.
