# Implementation Plan: STT Model Choice

**Branch**: `010-stt-model-choice` | **Date**: 2026-09-17 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/010-stt-model-choice/spec.md`

## Summary

Nutzer bekommen für das lokale Speech-to-Text-Modell dieselbe Wahlmöglichkeit, die es für das
lokale Chat/Agent-Modell bereits gibt: ein kleiner, gegen die Geräte-Hardware klassifizierter
Katalog von Whisper-Größen (tiny/base/small), präsentiert als drei Tier-Empfehlungen
(Easy/Sweet/Max) im Onboarding-Wizard (fester dritter Schritt nach Alias und Chat-Modell) und
änderbar in den Settings. Technisch: der bisherige, Whisper-spezifische Speicherpfad
(`WHISPER_DIR`) entfällt zugunsten der bereits bestehenden, backend-agnostischen
`models::paths`-Helfer (gleiche Wurzel wie Chat-GGUFs); die Tier-Auswahl-Logik aus
`catalog::recommend_tiers` wird generalisiert und von beiden Katalogen genutzt; STT-Installationen
werden per Datei-Existenz-Check erkannt statt über die `models`-DB-Tabelle (die von genau einer
Datei pro Slug ausgeht); ein neuer Command leert den warm gehaltenen `VoiceState`-Adapter-Cache,
damit ein Modellwechsel ohne App-Neustart wirkt. Externe/API-basierte Transkriptions-Anbieter
bleiben ausdrücklich außerhalb des Scopes (spec 008 US3, weiterhin ungebaut).

Volle Design-Historie (Brainstorming-Session, alle Entscheidungen inkl. verworfener Alternativen):
[research.md](research.md).

## Technical Context

**Language/Version**: Rust 1.77+ / edition 2021 (Backend, `src-tauri`); TypeScript 5, Vue 3, Nuxt 4
SPA (Frontend)
**Primary Dependencies**: `candle-transformers`/`candle-core` (bestehend, `stt/local.rs`) für
lokale Whisper-Inferenz — keine neue Dependency; bestehende `models::paths`-, `catalog::`-,
`hardware::`- und `storage::preferences`-Module werden erweitert/generalisiert, nicht neu
eingeführt
**Storage**: Keine Schemaänderung. Neue Preference `voice.stt_model_id` über die bestehende
generische Preferences-Infrastruktur (SQLite via `storage::preferences`); STT-Modelldateien im
bestehenden `<AppLocalData>/models/`-Wurzelverzeichnis (Dateisystem, kein DB-Table)
**Testing**: `cargo test` (Rust, Tests in separaten `*_tests.rs`-Dateien neben dem jeweiligen
Modul, Projekt-Konvention); `pnpm typecheck`; kein Vitest/Component-Test-Setup im Projekt vorhanden
— Frontend-Verifikation manuell über `pnpm tauri:dev` (siehe [quickstart.md](quickstart.md))
**Target Platform**: Desktop via Tauri 2 (Linux/macOS/Windows) — Backend-Feature-Gate `llm-cpu`
identisch zum bestehenden Whisper-Adapter; kein aktives Mobile-Build-Target im Repo, Katalog-Format
hält sich trotzdem an dieselben Konventionen wie der LLM-Katalog (der bereits Mobile-Profile
vorsieht) für spätere Wiederverwendbarkeit
**Project Type**: Desktop-App (Tauri: Rust-Backend + Vue/Nuxt-SPA-Frontend) — bestehende
Zwei-Schichten-Struktur, keine neue Projektart
**Performance Goals**: Kein neues Performance-Ziel — Downloadgrößen (150 MB–967 MB) und
CPU-Transkriptionszeit sind dem Nutzer über die Tier-Wahl selbst überlassen (das ist der Zweck der
Funktion), keine harte Obergrenze
**Constraints**: Umschalten des aktiven Modells MUSS ohne App-Neustart wirken (FR-006/SC-004);
Überspringen der Onboarding-Wahl MUSS unverändert zu 100 % funktionierender Diktier-Erfahrung
führen (FR-007/SC-003); keine Whisper-spezifischen Pfad-/Namensannahmen im Storage-Layer (FR-008)
**Scale/Scope**: Drei Katalog-Einträge, ein neuer Preference-Key, fünf neue Tauri-Commands, zwei
neue Vue-Komponenten (Onboarding-Step, Settings-Abschnitt), zwei generalisierte Composables

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Geprüft gegen `.specify/memory/constitution.md` (haex-hive, Version 1.4.0). Keines der acht
NON-NEGOTIABLE-Prinzipien ist berührt:

| Prinzip                                         | Berührt? | Begründung                                                                                     |
| ------------------------------------------------ | :------: | ------------------------------------------------------------------------------------------------ |
| I. No Secrets in Git                             |    ✗    | Keine Credentials/Keys im Scope (externe Provider explizit ausgeschlossen, FR-009).               |
| II. No Local Absolute Paths in Versioned Config  |    ✗    | Katalog-JSON enthält nur Repo-IDs/Revisions/Größen, keine Pfade.                                   |
| III. Project Identity Is Device-Independent      |    ✗    | Nicht berührt — kein Projekt-Identity-Bezug.                                                       |
| IV. Cross-Repo References Pin Immutable Revisions |    ✗    | Erfüllt sogar zusätzlich: `hf_revision` wird für `base`/`small` neu gepinnt (siehe research.md §6), nicht auf `main`. |
| V. External Sources Are Opt-in Per Project        |    ✗    | Kein `.haex-hive.json`-Atom betroffen.                                                             |
| VI. Self-Modifying Instructions Are Review-Gated  |    ✗    | Keine Skill-/Constitution-/Permission-Änderung.                                                    |
| VII. Relay Unavailability Never Blocks Local Work |    ✗    | Feature ist rein lokal (Katalog, Preferences, Dateisystem); kein Relay-Bezug.                      |
| VIII. No Concealment Instructions in Agent Output |    ✗    | Nicht berührt.                                                                                     |

**Ergebnis**: PASS, keine Verletzung, kein Eintrag in Complexity Tracking nötig.

## Project Structure

### Documentation (this feature)

```text
specs/010-stt-model-choice/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md         # Phase 1 output
├── contracts/
│   └── tauri-commands.md
└── tasks.md              # Phase 2 output (/speckit.tasks — not yet created)
```

### Source Code (repository root)

```text
src-tauri/src/
├── hardware/
│   ├── fit.rs                    # unverändert (ModelFitInputs/classify bereits generisch genug)
│   └── tiers.rs                  # NEU: generische pick_three<T>()-Extraktion aus catalog::recommend_tiers
├── catalog/
│   ├── mod.rs                    # recommend_tiers() ruft neu hardware::tiers::pick_three auf
│   └── commands.rs                # unverändert
├── stt/
│   ├── mod.rs                    # unverändert (SttAdapter-Contract)
│   ├── local.rs                  # WHISPER_REPO/REVISION/DIR-Konstanten entfernt; load()/ensure_model_files() nehmen SttCatalogEntry; nutzt models::paths::slug_dir statt eigenem model_dir()
│   ├── stt_catalog.json           # NEU: tiny/base/small-Katalog
│   ├── catalog.rs                 # NEU: entries()/get()/recommend_tiers() für den STT-Katalog (mirrored catalog/mod.rs)
│   └── commands.rs                # NEU: list_stt_catalog, stt_recommend_tiers, list_installed_stt_models, download_stt_model, invalidate_stt_model_cache
├── models/
│   └── paths.rs                   # unverändert — bereits backend-agnostisch, wird von stt/local.rs jetzt mitbenutzt
├── voice.rs                       # resolve_local_adapter liest voice.stt_model_id; VoiceState.whisper invalidierbar
└── lib.rs                         # neue Commands registrieren

src/
├── i18n/locales/
│   ├── en.json                     # neue onboarding.sttModel.*/settings.sttModel.*-Keys
│   └── de.json                     # dito
├── composables/
│   ├── useCatalog.ts               # refaktoriert zu generischer Factory, exportiert weiterhin useCatalog
│   ├── useSttCatalog.ts            # NEU: dünner Export derselben Factory für den STT-Katalog
│   ├── useModels.ts                # betroffene Funktionen (listInstalledAsync/downloadFromCatalogAsync) zu Factory extrahiert
│   └── useSttModels.ts             # NEU: dünner Export derselben Factory für STT-Installationen
├── pages/onboarding/[instance].vue # step-Union erweitert um 'sttModel', dritter Schritt verdrahtet
├── components/onboarding/
│   ├── ModelChoiceStep.vue         # unverändert
│   └── SttModelChoiceStep.vue      # NEU: mirrored ModelChoiceStep.vue für STT-Tiers
├── pages/settings/[instance].vue   # neue SettingsSttModelSetting-Komponente eingebunden
└── components/settings/
    ├── DefaultModelSetting.vue     # unverändert
    └── SttModelSetting.vue         # NEU: mirrored DefaultModelSetting.vue für STT
```

**Structure Decision**: Bestehende Zwei-Schichten-Struktur (`src-tauri` Rust-Backend,
`src` Nuxt/Vue-Frontend) unverändert. Keine neuen Top-Level-Verzeichnisse — jede neue Datei landet
neben ihrem bestehenden Pendant (`stt/` neben `catalog/`, `SttModelSetting.vue` neben
`DefaultModelSetting.vue`), damit der Vergleich mit dem gespiegelten LLM-Pfad beim Review trivial
bleibt.

## Complexity Tracking

*Keine Einträge — Constitution Check hat keine Verletzung ergeben.*
