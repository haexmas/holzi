# Tasks: Freie Hugging-Face-Modellsuche und Installation

**Input**: Design-Dokumente aus `specs/005-huggingface-model-discovery/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/tauri-commands.md`, `quickstart.md`
**Tests**: Backend-Unit-/Integrationstests und HTTP-Mocks sind Bestandteil des Features; Frontend-Abnahme folgt dem bestehenden manuellen Quickstart.

## Format: `[ID] [P?] [Story] Beschreibung`

- **[P]**: Kann parallel bearbeitet werden, weil Dateien und Abhängigkeiten getrennt sind.
- **[Story]**: Ordnet eine Aufgabe einer User Story zu (`US1`–`US4`).
- Jede Aufgabe nennt konkrete Dateien oder eine klar begrenzte Dateiänderung.

## Phase 1: Setup (Gemeinsame Infrastruktur)

**Zweck**: Gemeinsame Typen, i18n-Fläche und Command-Schnittstellen vorbereiten.

- [ ] T001 [P] Gemeinsame TypeScript-Typen für `HuggingFaceModelResult`, `HuggingFaceFileCandidate`, `InstallPreview` und strukturierte HF-Fehler in `src/composables/useHuggingFace.ts` entsprechend `contracts/tauri-commands.md` definieren.
- [ ] T002 [P] Neue Such-, Detail-, Preview-, Tokenizer-, Hardware-Warnungs-, Download- und Fehlertexte mit identischem Key-Baum in `src/i18n/locales/de.json` und `src/i18n/locales/en.json` ergänzen.
- [ ] T003 [P] Die HF-Discovery- und Installationsansichten unter `src/components/models/` anlegen; Props/Events typisieren und alle sichtbaren Texte über i18n beziehen.
- [ ] T004 [P] Die Backend-/Frontend-Commandnamen, Payloads und Fehlercodes in `src-tauri/src/models/commands.rs`, `src/composables/useHuggingFace.ts` und `specs/005-huggingface-model-discovery/contracts/tauri-commands.md` aufeinander abgleichen, ohne bereits Implementierungslogik zu duplizieren.

**Checkpoint**: Die Feature-Oberfläche und die typisierte Boundary sind vorbereitet; es gibt noch keinen neuen Netzwerkpfad.

## Phase 2: Foundational (Blockierende Voraussetzungen)

**Zweck**: Sichere, testbare gemeinsame Grundlagen für Suche und Installation schaffen.

**⚠️ KRITISCH**: Keine User-Story-Implementierung beginnt vor Abschluss dieser Phase.

- [ ] T005 [P] Tests für Repository-ID-, Revision-, GGUF-Dateiname-, HTTPS-Host- und Slug-Validierung in `src-tauri/src/models/huggingface_tests.rs` bzw. dem zugehörigen Modul schreiben; Pfadtraversal, Steuerzeichen und leere Werte müssen abgedeckt sein.
- [ ] T006 [P] Tests für GGUF-Filter, Quantisierungsnormalisierung samt Provenienz, Deduplizierung, das feste Standardlimit 20 und deterministische Sortierung mit reinen Fixtures in `src-tauri/src/models/huggingface_tests.rs` schreiben.
- [ ] T007 Die bestehende `models`-Migration fortführen und in `src-tauri/src/identity/migrations.rs` eine additive Migration `0015_models_add_huggingface_source` für fehlende HF-Quellefelder (`hf_repo`, `hf_filename`, `hf_revision`, `hf_revision_ref`, `file_sha256`, `integrity_status`, `source_kind`) definieren; Trigger-/CRDT-Version gemäß bestehender Migrationskonvention anpassen.
- [ ] T008 `ModelRow`, `upsert_model`, `get_model`, `list_all_models` und die relevanten Row-Mapper in `src-tauri/src/storage/models.rs` um Source-, Datei-Hash- und Integritätsstatusfelder erweitern und bestehende Katalog-, Provider- und Importzeilen rückwärtskompatibel lesen.
- [ ] T009 [P] Strukturierte Fehlerfälle für ungültige HF-Quelle, nicht unterstütztes Format, fehlenden Tokenizer, notwendige Hardware-Bestätigung, HTTP-Status, Rate-Limit, Timeout und Registrierung in `src-tauri/src/error.rs` ergänzen oder auf vorhandene Varianten abbilden.
- [ ] T010 Die modulare HF-Client-Grenze in `src-tauri/src/models/huggingface.rs` anlegen: bestehendes `reqwest` verwenden, öffentliche Hugging-Face-API-/Resolve-URLs zentral bilden, mutable Refs in Commit-SHAs auflösen, Update-Refs prüfen, Timeouts und Statuscodes übersetzen und keine Tokens/Secrets loggen.
- [ ] T011 Den HF-HTTP-Transport hinter einer testbaren Boundary in `src-tauri/src/models/huggingface.rs` halten und `wiremock`-Fixtures in `src-tauri/src/models/huggingface_tests.rs` für JSON, HTTP-Fehler, Redirect, fehlende Content-Length, Range-Request, SHA-Auflösung und Timeout vorbereiten.
- [ ] T012 Die Quelle-/Datei-Metadaten in der bestehenden Payload-Join- und Registrierungslogik in `src-tauri/src/models/commands.rs` berücksichtigen, ohne `list_installed_models` oder `canonical_model_file` als zweite Pfadquelle zu umgehen.

**Checkpoint**: Migration, Typen, Fehlerboundary, HTTP-Transport und reine Normalisierungslogik sind testbar und sichern die gemeinsamen Invarianten.

## Phase 3: User Story 1 - Hugging Face durchsuchen (Priority: P1) 🎯 MVP

**Ziel**: Nutzer können öffentliche, installierbare Modell-Repositories suchen und deren konkrete GGUF-Dateien anzeigen.

**Unabhängiger Test**: Query validieren, Mock-Suchergebnis normalisieren, nicht-GGUF-Repositories herausfiltern und Netzwerkfehler retrybar darstellen.

### Tests für User Story 1

- [ ] T013 [P] [US1] Contract-/Commandtests für `search_huggingface_models` mit gültiger Query, zu kurzer Query, leerer Trefferliste, maximal 20 Treffern, HTTP-Fehler und Rate-Limit in `src-tauri/src/models/huggingface_tests.rs` ergänzen.
- [ ] T014 [P] [US1] Detail-Response-Fixtures für Repository ohne GGUF, Repository mit mehreren GGUFs, fehlende Lizenz-/Größen-/Tokenizer-Metadaten und exakte Katalogtreffer in `src-tauri/src/models/huggingface_tests.rs` abdecken.

### Implementierung für User Story 1

- [ ] T015 [US1] `search_huggingface_models` in `src-tauri/src/models/commands.rs` implementieren: Query validieren, öffentliche Treffer auf 20 begrenzen, normalisieren, deduplizieren, deterministisch sortieren und exakte Katalog-Matches mit `catalogMatch`/`catalogEntryId` annotieren.
- [ ] T016 [US1] `get_huggingface_model_details` in `src-tauri/src/models/commands.rs` implementieren und nur installierbare GGUF-Dateikandidaten mit strukturierten Metadaten zurückgeben.
- [ ] T017 [US1] Die neuen Commands in `src-tauri/src/lib.rs` registrieren und Fehlerparameter so serialisieren, dass sie ohne lokalisierte Backend-Texte im Frontend verarbeitet werden können.
- [ ] T018 [US1] `useHuggingFace()` in `src/composables/useHuggingFace.ts` implementieren; explizites Submit, Loading/Error/Retry und das Bewahren der vorherigen Ergebnisliste bei Fehlern abbilden.
- [ ] T019 [US1] `src/components/models/HuggingFaceSearch.vue` und `src/components/models/HuggingFaceResult.vue` mit Suchfeld, Validierung, Ergebnisliste, Empty-/Offline-/Retry-Zustand und Repository-Auswahl implementieren.
- [ ] T020 [US1] Den Einstieg zur freien Suche in `src/pages/settings/[instance].vue` oder einer eingebetteten Modellverwaltungs-Komponente ergänzen, ohne bestehende Alias-/Default-Settings zu verändern.
- [ ] T021 [US1] Suche und Ergebnisdarstellung manuell nach `quickstart.md` prüfen und Query-Validierung, i18n-Key-Verwendung sowie keyboard-nutzbare Auswahl dokumentieren.
- [ ] T063 [US1] Die freie Suche beim Öffnen ohne Query mit einer nach Downloads sortierten Top-10-Ansicht initialisieren; explizite Suchanfragen und Filter müssen weiterhin getrennt funktionieren.

**Checkpoint**: Ein Nutzer kann öffentliche HF-Repositories suchen und passende GGUF-Dateien zur Installation auswählen.

## Phase 4: User Story 2 - Datei auswählen und herunterladen (Priority: P1)

**Ziel**: Eine konkrete GGUF-Datei kann sicher über den bestehenden Downloadpfad installiert und anschließend im Chat verwendet werden.

**Unabhängiger Test**: Mock-Repository, konkrete Dateiauswahl, Download-Fortschritt, atomare Veröffentlichung, Registrierung und anschließendes Laden prüfen.

### Tests für User Story 2

- [ ] T022 [P] [US2] Tests für `preview_huggingface_install` mit validierter Quelle, Branch-/Tag-zu-SHA-Auflösung, direktem SHA-Pin, Kollision bestehender Slugs, fehlendem Tokenizer und idempotentem bereits installiertem Source-Key in `src-tauri/src/models/huggingface_tests.rs` ergänzen.
- [ ] T023 [P] [US2] Download-/Registrierungstests in `src-tauri/src/models/commands_tests.rs` oder dem bestehenden Model-Testmodul ergänzen: Erfolg mit gepinnter SHA, Berechnung und Persistenz von `file_sha256`, Abbruch, HTTP-Fehler, kein finaler `.gguf`-Eintrag und Source-Metadaten-Roundtrip einschließlich `hf_revision_ref`. Der Downloadpfad muss außerdem Resume mit Validator und `If-Range`, Neustart ohne Validator, kurze Bodies, fehlendes oder inkonsistentes `Content-Range`, transiente Retry-Statuscodes und das Nicht-Wiederholen permanenter Statuscodes abdecken.
- [ ] T024 [P] [US2] Migration-/Storage-Tests in `src-tauri/tests/model_import.rs` oder einem neuen `src-tauri/tests/huggingface_models.rs` ergänzen, die Katalog-, Import- und HF-Zeilen gemeinsam über `list_installed_models` lesen.

### Implementierung für User Story 2

- [ ] T025 [US2] `preview_huggingface_install` in `src-tauri/src/models/commands.rs` implementieren: Source-Key/Modell-ID, Fit, Tokenizer-Anforderung und `TooBig`-Bestätigung berechnen, ohne Download oder Persistenz auszulösen.
- [ ] T026 [US2] `download_model_from_hf` in `src-tauri/src/models/commands.rs` um aufgelöste Commit-SHA, optionalen Upstream-Ref, Source-Metadaten, Preview-/Fit-Validierung und `forceTooBig` erweitern, dabei Publication-Lock und bestehenden Progress-/Completion-Event-Vertrag beibehalten.
- [ ] T027 [US2] Den Downloadpfad in `src-tauri/src/catalog/mod.rs` bzw. `src-tauri/src/models/commands.rs` so vereinheitlichen, dass Katalog-Qwen3 und freie HF-Dateien denselben HTTPS-/atomaren Schreibpfad nutzen.
- [ ] T028 [US2] `src-tauri/src/storage/models.rs` und `src-tauri/src/models/commands.rs` so verbinden, dass SHA-256 und übrige Metadaten erst nach erfolgreicher finaler Dateiveröffentlichung geschrieben werden und Fehler keine neue Model-Row hinterlassen.
- [ ] T029 [US2] `src/composables/useModels.ts` und `src/composables/useHuggingFace.ts` auf die erweiterten Download-/Preview-Argumente aktualisieren; alle Promises und Download-Listener an der UI-Grenze behandeln.
- [ ] T030 [US2] `src/components/models/HuggingFaceFilePicker.vue` mit Datei-Auswahl, Preview, Tokenizer-Eingabe, Fit-Warnung, expliziter `TooBig`-Bestätigung und Download-Fortschritt implementieren.
- [ ] T031 [US2] Nach erfolgreichem Download die bestehende lokale Modellliste in `src/pages/chat/[instance].vue` bzw. dem gemeinsamen Modell-Refresh aktualisieren und optional das neu installierte Modell laden, ohne `last_active_model_id` durch die Installation zu ändern.

**Checkpoint**: Ein ausgewähltes eigenes GGUF wird atomar installiert, registriert und über den bestehenden Chat-Picker geladen.

## Phase 5: User Story 3 - Metadaten und Runtime-Kompatibilität (Priority: P1)

**Ziel**: Nutzer treffen informierte Entscheidungen; unbrauchbare Formate, unsichere Tokenizer und riskante Hardware-Fits werden sichtbar behandelt.

**Unabhängiger Test**: Fixtures für gültig/ungültig, fehlende Metadaten und Hardware-Grenzfälle gegen Parser, Preview und UI-Warnungen prüfen.

### Tests für User Story 3

- [ ] T032 [P] [US3] Parser-Tests für fehlende Lizenz, fehlende Content-Length, unbekannte Quantisierung, fehlendes Kontextfenster und nicht auflösbaren Tokenizer in `src-tauri/src/models/huggingface_tests.rs` ergänzen.
- [ ] T033 [P] [US3] Hardware-Fit-Tests für `Fits`, `Tight`, `TooBig` und `Unknown` mit deterministischen Modelldaten in `src-tauri/src/models/huggingface_tests.rs` ergänzen.
- [ ] T034 [P] [US3] Fehler- und Accessibility-Szenarien der Preview-/Tokenizer-/Warnungsansicht manuell gemäß `quickstart.md` prüfen.

### Implementierung für User Story 3

- [ ] T035 [US3] Metadaten-Normalisierung in `src-tauri/src/models/huggingface.rs` vervollständigen: Lizenzstatus, Dateigröße, Quantisierung, Kontextfenster und belastbare Tokenizer-Hinweise ohne geratenen Default; für Quantisierung/Kontext die Provenienz und den sicherheitsrelevanten Range-Request-Fallback aus dem Contract liefern.
- [ ] T036 [US3] Den bestehenden Hardware-Fit-Klassifikator in `src-tauri/src/hardware/` über einen testbaren Adapter mit HF-Dateigröße und Kontextfenster versorgen; keine zweite Fit-Implementierung erzeugen.
- [ ] T037 [US3] `src/components/models/HuggingFaceFilePicker.vue` so erweitern, dass fehlende Metadaten ehrlich angezeigt werden, `TooBig` eine zweite Bestätigung verlangt und fehlender Tokenizer den Download blockiert.
- [ ] T038 [US3] Strukturierte Fehlerübersetzung in `src/composables/useHuggingFace.ts` und lokalisierte Meldungen in `src/i18n/locales/de.json`/`src/i18n/locales/en.json` vervollständigen.
- [ ] T039 [US3] Sicherheitsvalidierung an `src-tauri/src/models/paths.rs`/`src-tauri/src/models/huggingface.rs` integrieren und sicherstellen, dass Repository- und Dateinamen keinen lokalen Pfad außerhalb des Modellverzeichnisses erreichen.

**Checkpoint**: Format-, Pfad-, Tokenizer- und Hardware-Risiken sind vor dem Download sichtbar und werden korrekt behandelt.

## Phase 6: User Story 4 - Installierte eigene Modelle verwalten (Priority: P2)

**Ziel**: Freie HF-Modelle verhalten sich nach der Installation wie alle anderen lokalen Modelle.

**Unabhängiger Test**: HF-Modell und Katalogmodell gemeinsam auflisten, beide laden, HF-Modell entfernen und Fallback prüfen.

### Tests für User Story 4

- [ ] T040 [P] [US4] Regressionstest in `src-tauri/tests/huggingface_models.rs` für gemeinsames Auflisten, Laden und Löschen eines HF-Modells neben einem Qwen3-Katalogmodell ergänzen.
- [ ] T041 [P] [US4] Regressionstest für `chat.last_active_model_id` und die Spec-002-Fallback-Kette nach Entfernung eines frei installierten Modells in `src-tauri/tests/huggingface_models.rs` ergänzen.

### Implementierung für User Story 4

- [ ] T042 [US4] Source-Anzeige (`catalog`, `huggingface`, `imported`) in der gemeinsamen Modellliste unter `src/composables/useModels.ts` und den bestehenden Settings-/Chat-Modellgruppen ergänzen.
- [ ] T043 [US4] `delete_installed_model` und den lokalen Refresh in `src-tauri/src/models/commands.rs` prüfen/erweitern, sodass nur die lokale Datei gelöscht wird und HF-/Provider-Metadaten erhalten bleiben.
- [ ] T044 [US4] Sicherstellen, dass `load_model` und der Session-Resolver in `src-tauri/src/chat/` vor jedem lokalen Runtime-Load den vollständigen SHA-256-Hash berechnen, gegen `models.file_sha256` prüfen und nur bei Erfolg laden; frei installierte Modelle bleiben ohne erneute Hugging-Face-Anfrage nutzbar.
- [ ] T045 [US4] Die dauerhaft erreichbare gemeinsame Modellverwaltung in `src/pages/settings/[instance].vue` oder den bestehenden Modellkomponenten um aktiven Modellwechsel, Quelle, Entfernen-Bestätigung, Katalog-Match-Link, Update-Hinweis und Fehlerzustand ergänzen.

**Checkpoint**: Eigene Modelle sind nach Neustart auffindbar, ladbar, entfernbar und vollständig in die bestehende Fallback-Semantik integriert.

## Phase 7: Update- und Integritätsabschluss

**Zweck**: SHA-Pinning, Upstream-Updates und die verpflichtende lokale
Dateiprüfung vor jedem Load vollständig implementieren, bevor die finale
Abnahme beginnt.

- [ ] T053 [P] [US4] Tests für `check_huggingface_model_updates` mit unveränderter SHA, neuer SHA, direktem SHA-Pin ohne Update-Ref, Offline-/HTTP-Fehler und gelöschtem Repository ergänzen.
- [ ] T054 [US4] `check_huggingface_model_updates` in `src-tauri/src/models/commands.rs` implementieren und in `src-tauri/src/lib.rs` registrieren; nur gespeicherte öffentliche Refs prüfen, Status strukturiert liefern und lokale Dateien/Metadaten bei Fehlern unverändert lassen.
- [ ] T055 [US4] Einen Update-Installationspfad für dieselbe Modell-ID ergänzen, der die neue SHA über den bestehenden atomaren Downloadpfad installiert und erst danach Datei, `hf_revision`, `file_sha256` und Status aktualisiert.
- [ ] T056 [US4] `useHuggingFace()` und die gemeinsame Modellverwaltung um sichtbare Update-Badges, alte/neue SHA, Retry und eine ausdrückliche „Update installieren“-Aktion erweitern; direkte SHA-Pins ohne Ref als nicht automatisch prüfbar anzeigen.
- [ ] T057 [P] [US4] Quickstart- und manuellen Abnahmetest für ein simuliertes Upstream-Update ergänzen: Benachrichtigung erscheint, altes Modell bleibt bis zur Bestätigung nutzbar, fehlgeschlagene Aktualisierung lässt den alten Stand intakt.
- [ ] T058 [P] [US4] Integritäts-Fixtures für unveränderte, ausgetauschte, korrumpierte und fehlende lokale Dateien sowie fehlenden gespeicherten Hash ergänzen.
- [ ] T059 [US4] Eine wiederverwendbare SHA-256-Datei-Hash-Funktion in `src-tauri/src/models/` ergänzen (den bereits im Lockfile vorhandenen `sha2`-Baustein direkt verwenden) und alle Downloads, Updates und Importe erst nach erfolgreicher Hash-Berechnung als ladbar registrieren.
- [ ] T060 [US4] `load_model` in `src-tauri/src/chat/commands.rs` um die verpflichtende Vorabprüfung erweitern und in `src-tauri/src/lib.rs` auf den bestehenden Contract abstimmen; Hashing und Dateizugriff über `spawn_blocking` ausführen und bei Mismatch strukturierte Fehler mit erwarteter/aktueller SHA liefern.
- [ ] T061 [US4] Den bestätigungspflichtigen Override-Pfad `load_model_with_integrity_override` in `src-tauri/src/chat/commands.rs` implementieren und registrieren: aktuelles Modell laden, `integrity_status = 'untrusted'` setzen, erwartete SHA unverändert lassen und keine stille Hash-Aktualisierung erlauben.
- [ ] T062 [US4] Den Integritätsdialog in der Modellverwaltung implementieren: „trotzdem als unsicher laden“, „dasselbe HF-Modell erneut herunterladen bzw. lokal neu importieren“ und „anderes Modell auswählen“ mit lokalisierter Erklärung und Fehlerbehandlung anbieten.

**Checkpoint**: Jeder lokale Load ist vor dem Runtime-Start integritätsgeprüft;
Abweichungen führen zu einer bewussten Nutzerentscheidung.

## Phase 8: Polish & Cross-Cutting Concerns

**Zweck**: Gesamtvalidierung, Dokumentation und Regression über alle Modellpfade.

- [ ] T046 [P] i18n-Key-Parität für `de`/`en` und alle neuen Modellkomponenten prüfen.
- [ ] T047 [P] Bestehende Qwen2.5-Verweise in aktiven kuratierten Spec-/Quickstart-Dokumenten gegen Qwen3 prüfen und nur veraltete Vorschlagsbeispiele aktualisieren.
- [ ] T048 [P] `specs/005-huggingface-model-discovery/quickstart.md` gegen den finalen UI- und Command-Flow aktualisieren, einschließlich SHA-Pinning, Update-Benachrichtigung, expliziter Update-Installation und Navigation nach dem Onboarding.
- [ ] T049 Vollständige Rust-Suite mit `cargo test` ausführen und fehlende Boundary-/Persistenztests ergänzen.
- [ ] T050 `pnpm typecheck` ausführen und TypeScript-/Vue-Vertragsabweichungen beheben.
- [ ] T051 Manuellen Quickstart auf Desktop und schmalem Viewport ausführen; Abweichungen und bewusste Folgearbeiten im Spec-/Review-Dokument festhalten.

## Abhängigkeiten und Ausführungsreihenfolge

### Phasenabhängigkeiten

- Setup (Phase 1) kann sofort beginnen.
- Foundational (Phase 2) hängt von Setup ab und blockiert alle User Stories.
- US1 und US2 sind beide P1; US1 liefert Discovery, US2 nutzt deren normalisierte Datei-Contracts.
- US3 ergänzt US2 um die Preview-/Kompatibilitätsgrenzen und sollte vor produktiver Freigabe des Downloads abgeschlossen sein.
- US4 hängt auf der gemeinsamen Installation, Modellliste und Löschsemantik auf.
- Update-/Integritätsabschluss (Phase 7) hängt von US2/US4 und der gemeinsamen
  Modellregistrierung ab.
- Polish (Phase 8) hängt von Phase 7 ab; `cargo test`, Typecheck und der
  manuelle Quickstart dürfen erst danach als finale Abnahme gelten.

### User-Story-Abhängigkeiten

- **US1**: Nach Phase 2 unabhängig testbar.
- **US2**: Benötigt die Discovery-Typen und den Backend-Transport aus Phase 2; die UI kann erst nach US1 sinnvoll über Suchresultate demonstriert werden.
- **US3**: Benötigt Preview-/Dateikandidaten aus US1/US2, bleibt aber als Parser-/Fit-Test unabhängig prüfbar.
- **US4**: Benötigt die erfolgreiche gemeinsame Registrierung aus US2.

### Parallelmöglichkeiten

- T001–T004 können parallel vorbereitet werden.
- T005, T006, T009 und T011 können parallel erstellt werden, solange sie nur die vereinbarte Boundary testen.
- T013 und T014 können parallel geschrieben werden.
- T022–T024 können parallel vorbereitet werden.
- T032–T034 können parallel vorbereitet werden.
- T040 und T041 können parallel geschrieben werden.
- T053 und T058 können parallel als Test-Fixtures vorbereitet werden; T054–T062
  hängen an den jeweiligen Backend-/Frontend-Boundaries.
- T046–T048 sind parallel möglich, sobald Phase 7 abgeschlossen ist;
  T049–T052 bilden danach die finale Abnahme.

## Implementierungsstrategie

### MVP zuerst

1. Setup und Foundational abschließen.
2. US1 als reine öffentliche Suche mit GGUF-Filter fertigstellen.
3. US2 mit Preview, Download und gemeinsamer Registrierung fertigstellen.
4. US1/US2 unabhängig testen und erst danach Verwaltungspolish aus US4 ergänzen.

### Inkrementelle Lieferung

1. Discovery ohne Installation demonstrieren.
2. Eine konkrete GGUF-Datei sicher installieren und im Chat laden.
3. Metadaten-/Hardware-/Tokenizer-Grenzen schließen.
4. SHA-Integritätsprüfung und Nutzerentscheidungen bei Dateimismatch validieren.
5. Entfernen, Neustart und Fallback validieren.

### Hinweise

- `[P]` wird nur bei getrennten Dateien oder echten Test-/Dokumentationsparallelitäten verwendet.
- Es wird keine neue Dependency eingeplant; `reqwest`, `wiremock` und bestehende Modell-/Pfadgrenzen werden wiederverwendet.
- Frontend-Verifikation bleibt manuell, weil im Repository kein etablierter Frontend-E2E-Teststack vorhanden ist.
- Keine Aufgabe ändert spaex-, Spec-Kit- oder Agenten-Instruktionen.
