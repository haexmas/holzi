# Phase 0 Research: STT Model Choice

Keine `NEEDS CLARIFICATION`-Marker im Technical Context — die Kernentscheidungen wurden bereits
vor dem Schreiben der Spec im direkten Gespräch mit dem Operator getroffen. Dieses Dokument fasst
sie im Research-Format zusammen und ergänzt die zwei Punkte, die erst während der Planung konkret
wurden (gepinnte Whisper-Revisionen für `base`/`small`, Cache-Invalidierung).

## 1. Scope: lokale Größen-Tiers statt externer Anbieter

**Decision**: Diese Spec deckt ausschließlich die Wahl zwischen mehreren lokalen Whisper-Größen
(tiny/base/small) ab — analog zur bestehenden LLM-Katalog-Tier-Wahl.

**Rationale**: Spec 008 US3 ("externer Transkriptions-Dienst als Alternative zum gebündelten
lokalen Whisper") existiert bereits als eigene, ungebaute User Story (`specs/008-voice-control-stt/tasks.md`
T022–T028) und deckt einen anderen Bedarf ab (Cloud-Genauigkeit/Sprachabdeckung vs. Geräte-Ressourcen).
Beide Achsen in einer Spec zu vermischen hätte den Scope unnötig aufgebläht.

**Alternatives considered**: Beides gemeinsam umsetzen — verworfen, weil US3 einen komplett
anderen Mechanismus braucht (Credentials, HTTP-Adapter, Off-Device-Warnhinweis) und unabhängig
priorisiert werden kann.

## 2. Hardware-Fit-Tiers (Easy/Sweet/Max) statt flacher Liste

**Decision**: Die drei Whisper-Größen werden — wie beim LLM-Katalog — gegen die Geräte-Hardware
klassifiziert (`hardware::classify`/`Fit`) und zu drei Tier-Empfehlungen (Easy/Sweet/Max)
verdichtet, nicht als ungefilterte Liste präsentiert.

**Rationale**: Operator-Entscheidung — volle UX-Parität zum LLM-Onboarding-Schritt war explizit
gewünscht, auch wenn der Fit bei diesen kleinen Modellgrößen (150 MB–967 MB) auf den meisten
Geräten ohnehin `Fits` ergibt. Die Klassifikationslogik bleibt unverändert (`ModelFitInputs`
akzeptiert bereits `context_window: Option<u64>`; für Whisper-Einträge wird `None` übergeben, was
intern auf den konservativen 4096-Default fällt).

**Alternatives considered**: Flache Liste mit Geschwindigkeit/Genauigkeit-Label statt Hardware-Fit
— technisch einfacher und ehrlicher (STT-Größen sind kaum RAM-limitiert), aber vom Operator
zugunsten der UX-Konsistenz mit dem LLM-Fluss verworfen.

## 3. Generische Tier-Auswahl-Logik statt Duplikation

**Decision**: Der Sortier-/Auswahl-Algorithmus aus `catalog::recommend_tiers`
(`src-tauri/src/catalog/mod.rs:150`) wird in eine generische Hilfsfunktion extrahiert, die sowohl
vom bestehenden LLM-Katalog als auch vom neuen STT-Katalog aufgerufen wird.

**Rationale**: Der Doc-Kommentar in `stt/local.rs:14-19` benennt diese Lücke bereits explizit
("`catalog::recommend_tiers` is hardcoded to the LLM catalog's `CatalogEntry` type... Building a
general-purpose tier selector is out of scope for this pass"). Diese Spec macht genau das jetzt
nach — als kleine, chirurgische Extraktion, nicht als Neubau.

**Alternatives considered**: Algorithmus für den STT-Katalog duplizieren — verworfen (DRY-Verstoß,
zwei Stellen mit identischer, nicht-trivialer Sortier-/Fallback-Logik).

## 4. Speicherort: gemeinsames, backend-agnostisches `models::paths` — kein `WHISPER_DIR`

**Decision**: STT-Modelle liegen unter demselben Wurzelverzeichnis wie Chat-Modelle
(`<AppLocalData>/models/<slug>/`, `MODELS_DIRECTORY` unverändert "models"), aufgelöst über die
bestehenden generischen Helfer `models::paths::slug_dir`/`model_file_path`.

**Rationale**: Operator-Entscheidung, in zwei Schritten präzisiert: zuerst der Wunsch nach einem
gemeinsamen, nicht Whisper-spezifischen Verzeichnis (weil das lokale STT-Backend nicht dauerhaft
Whisper bleiben muss), dann die Vereinfachung, dass der bereits existierende Name "models" dafür
ausreicht — keine Umbenennung zu "llmModels" nötig. `models::paths` war bereits vollständig
backend-agnostisch (`<root>/<slug>/<filename>`); der bisherige `WHISPER_DIR`/`model_dir()`-Sonderweg
in `stt/local.rs` war unnötig.

**Alternatives considered**:

- Neues Wurzelverzeichnis `llmModels` — verworfen, da der Operator den bestehenden Namen
  ausdrücklich beibehalten wollte.
- Separates STT-eigenes Wurzelverzeichnis (z. B. weiterhin `whisper/`) — verworfen, widerspricht
  der Anforderung, keine Backend-spezifische Struktur festzuschreiben.
- Migration einer bereits unter dem alten `<AppLocalData>/whisper/tiny/<rev>/`-Pfad
  heruntergeladenen Installation in den neuen Slug-Pfad — ursprünglich vorgesehen, dann verworfen:
  es gibt keine Nutzer und damit keine bestehenden Installationen, die migriert werden müssten.
  Der alte Pfad wird nirgends mehr referenziert; ein fehlendes Modell wird immer per normalem
  Download unter dem kanonischen Slug-Pfad neu geladen.

## 5. Keine Vereinheitlichung mit der `models`-DB-Tabelle

**Decision**: STT-Installationen werden nicht als Zeilen in der bestehenden `models`-Tabelle
(`InstalledModel`/`device_downloaded_models_no_sync`) geführt, sondern weiterhin über
Existenzprüfung der benötigten Dateien im Slug-Verzeichnis erkannt.

**Rationale**: Das bestehende Schema (`relative_path`, `file_sha256`) geht von genau einer Datei
pro Modell-Slug aus — passend zu GGUF (Single-File-Format). Der Whisper-Adapter lädt über
`candle-transformers`' natives HF-Format und braucht drei Dateien (`config.json`,
`tokenizer.json`, `model.safetensors`), weil das der native Ladepfad dieser Implementierung ist,
nicht weil Whisper das verlangt. Eine "eine Zeile = mehrere Dateien"-Erweiterung des Schemas wäre
eine echte, nicht triviale Änderung (inkl. Integritätsprüfung pro Datei) und steht in keinem
Verhältnis zum Nutzen für drei kleine, öffentliche, unveränderliche Modell-Assets.

**Alternatives considered**: Nur die Gewichtsdatei (`model.safetensors`) als `InstalledModel`-Zeile
registrieren, Config/Tokenizer als adapter-interne Implementierungsdetails behandeln — technisch
möglich, aber inkonsistent (eine DB-Zeile, die nicht das ganze installierte Modell beschreibt) und
bringt keinen Mehrwert, da `list_installed_models`/`useModels.ts` ohnehin nicht für die
Onboarding-/Settings-STT-Anzeige gebraucht wird (siehe Punkt 7).

## 6. Gepinnte Revisionen für `whisper-base`/`whisper-small`

**Decision**: Analog zu `whisper-tiny` (bereits gepinnt auf
`169d4a4341b33bc18d8881c4b69c2e104e1cc0af`) werden `base` und `small` auf den zum Zeitpunkt dieser
Planung aktuellen `main`-Commit von `openai/whisper-base` bzw. `openai/whisper-small` gepinnt:

| Tier  | Repo                   | Revision (SHA)                                         | `model.safetensors` | `config.json` | `tokenizer.json` |
| ----- | ---------------------- | ------------------------------------------------------ | ------------------: | ------------: | ---------------: |
| tiny  | `openai/whisper-tiny`  | `169d4a4341b33bc18d8881c4b69c2e104e1cc0af` (bestehend) |       151 061 672 B |       1 983 B |      2 480 466 B |
| base  | `openai/whisper-base`  | `e37978b90ca9030d5170a5c07aadb050351a65bb`             |       290 403 936 B |       1 983 B |      2 480 466 B |
| small | `openai/whisper-small` | `973afd24965f72e36ca33b3055d56a652f456b4d`             |       966 995 080 B |       1 967 B |      2 480 466 B |

Verifiziert am 2026-09-17 pro Katalog-Pin: `GET /api/models/<repo>/revision/<sha>` bestätigte die
jeweilige SHA, die drei Dateien und den `safetensors`-Tag; `HEAD`-Requests (mit Redirect) auf
`resolve/<sha>/<file>` bestätigten die folgenden Größen. Es wurde kein `main`-Branch als
Nachweis verwendet. Damit haben alle drei Repositories am jeweils gepinnten Stand dasselbe
Dateilayout, sodass `LocalWhisperAdapter::load_from_dir` ohne Änderung auch `base`/`small`
laden können sollte.

**Rationale**: Gleiches Pinning-Prinzip wie bei `tiny` — Config, Tokenizer und Gewichte dürfen nie
über einen sich bewegenden `main`-Branch auseinanderlaufen.

**Alternatives considered**: `main` unversioniert referenzieren (wie der LLM-Katalog es für
bartowski-GGUFs tut) — verworfen, das bewusste Pinning von `tiny` sollte konsistent auf die
anderen beiden Tiers ausgeweitet werden, nicht aufgeweicht werden.

**Offen für Implementierung**: `approx_size_bytes` im Katalog-JSON sollte die Summe aller drei
Dateien sein (nicht nur die Gewichte), damit der Hardware-Fit-Check den tatsächlichen
Speicherbedarf widerspiegelt — Detail für `data-model.md`.

## 7. Cache-Invalidierung des warm gehaltenen Whisper-Adapters

**Decision**: Ein neuer Tauri-Command (`invalidate_stt_model_cache`) leert
`VoiceState.whisper` (`AsyncMutex<Option<Arc<LocalWhisperAdapter>>>`, `voice.rs:58`), nachdem
Settings die aktive Preference erfolgreich gewechselt hat. Der Command bleibt in allen Builds
registriert; mit `voice` und `llm-cpu` leert er den Cache, mit `voice` ohne `llm-cpu` und ohne
`voice` ist er ein erfolgreicher No-op ohne Zugriff auf das nicht kompilierte Feld.

**Rationale**: `resolve_local_adapter` (`voice.rs:228`) lädt den Adapter einmalig und hält ihn für
die Prozesslaufzeit warm (Modell-Load kostet Sekunden). Ohne explizite Invalidierung würde ein
Wechsel der aktiven STT-Preference erst nach einem App-Neustart wirken — das widerspricht FR-006/
SC-004.

**Alternatives considered**: Bei jeder Transkription die Preference gegen den zuletzt geladenen
Slug vergleichen und bei Abweichung automatisch neu laden — würde denselben Effekt ohne
zusätzlichen Command erreichen, aber `resolve_local_adapter` müsste dafür den zuletzt geladenen
Slug zusätzlich im `VoiceState` mitführen. Verworfen zugunsten des expliziten, einfacheren
Commands, der zum bestehenden Cache-Muster (Option, einmal befüllt) passt.

## 8. Geteilte Frontend-Composables statt Datei-Duplikate

**Decision**: `useCatalog.ts`/`useModels.ts` (bzw. der Teil davon, den die STT-Flows brauchen)
werden zu generischen Factory-Funktionen umgebaut, aus denen sowohl die Chat- als auch die
STT-Variante erzeugt werden, statt zwei separate Dateien mit near-identischem Code zu pflegen.

**Rationale**: Die Backend-Commands müssen getrennt bleiben (unterschiedliche Datenformen:
`CatalogEntry` vs. `SttCatalogEntry`, unterschiedliche Speicherung), aber das Composable-Muster
(list + recommendTiers [+ listInstalled + downloadFromCatalog]) ist identisch. Vom Operator
explizit angefragt, um Copy-Paste zwischen zwei Dateien zu vermeiden.

**Alternatives considered**: Zwei vollständig separate Composable-Dateien (ursprünglicher
Entwurf) — verworfen zugunsten der Factory, siehe Feedback des Operators.
