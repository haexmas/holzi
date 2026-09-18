# Phase 1 Data Model: STT Model Choice

Keine Schemaänderung an einer SQLite-Tabelle — weder `models`/`device_downloaded_models_no_sync`
noch `providers` werden angefasst (siehe [research.md](research.md) §5). Die einzige neue
persistente Größe ist eine zusätzliche Preference; alles andere ist statischer Katalog-Inhalt oder
Dateisystem-Zustand.

## Neue Entität: `SttCatalogEntry` (statischer Katalog, kein DB-Table)

Analog zu `catalog::CatalogEntry` (`src-tauri/src/catalog/mod.rs:27`), aber ohne die
GGUF-spezifischen Felder (`hf_filename`, `tokenizer_repo`, `context_window`), die für den
mehrdateiigen Whisper-Ladepfad nicht zutreffen:

| Feld                | Typ      | Beschreibung                                                                                                                                                                               |
| ------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `id`                | `String` | Stabile ID, zugleich der `models::paths`-Slug (z. B. `"whisper-tiny"`).                                                                                                                    |
| `name`              | `String` | Anzeigename (z. B. "Whisper Tiny (mehrsprachig)").                                                                                                                                         |
| `hf_repo`           | `String` | HuggingFace-Repo (z. B. `"openai/whisper-tiny"`).                                                                                                                                          |
| `hf_revision`       | `String` | Gepinnter Commit-SHA (siehe research.md §6) — nie `"main"`.                                                                                                                                |
| `approx_size_bytes` | `u64`    | Summe aller drei benötigten Dateien (`config.json` + `tokenizer.json` + `model.safetensors`), nicht nur der Gewichte — das ist der tatsächliche Speicherbedarf für den Hardware-Fit-Check. |
| `license`           | `String` | Lizenz des HuggingFace-Repositories (für die drei `openai/whisper-*`-Repos: Apache-2.0).                                                                                                   |

Persistiert als `src-tauri/src/stt/stt_catalog.json`, geparst genau wie `model_catalog.json`
(`OnceLock`, `include_str!`). Die drei benötigten Dateinamen (`config.json`/`tokenizer.json`/
`model.safetensors`) stehen **nicht** im Katalog — das ist ein Implementierungsdetail des
Whisper-Adapters (`stt/local.rs`), kein Katalog-Attribut, damit ein späterer Backend-Wechsel den
Katalog-Eintrag nicht anfassen muss (FR-008).

## Geteilte Typen (generalisiert aus dem LLM-Katalog)

`catalog::Tier` (`Easy`/`Sweet`/`Max`) und die Sortier-/Auswahl-Logik aus `recommend_tiers`
(research.md §3) werden generisch, z. B.:

```rust
// src-tauri/src/hardware/tiers.rs (neu)
pub fn pick_three<T: Clone>(candidates: Vec<(T, Fit)>, size_of: impl Fn(&T) -> u64, id_of: impl Fn(&T) -> &str) -> Option<[(Tier, T, Fit); 3]>
```

`catalog::recommend_tiers` (LLM) und `stt::catalog::recommend_tiers` (neu) rufen beide diese
Funktion auf und verpacken das Ergebnis in ihren jeweils eigenen, serialisierbaren
`TierRecommendation`/`SttTierRecommendation`-Wire-Typ (getrennte Typen, da `#[derive(Serialize)]`
über den konkreten Entry-Typ generisch nicht praktikabel ist und die beiden Kataloge ohnehin
unterschiedliche TS-Interfaces auf Frontend-Seite haben).

## Neue Preference: `voice.stt_model_id`

| Scope    | Key                  | Wert                                              | Default bei Fehlen                                      |
| -------- | -------------------- | ------------------------------------------------- | ------------------------------------------------------- |
| `device` | `voice.stt_model_id` | Ein `SttCatalogEntry.id` (z. B. `"whisper-base"`) | `"whisper-tiny"` (kleinster Tier, bisheriges Verhalten) |

Läuft über die bestehenden generischen `get_pref`/`set_pref`-Commands
(`src-tauri/src/storage/preferences_commands.rs`) — keine neue Pref-Infrastruktur, analog zu
`chat.default_model_id` (device- und vault-scoped) und `voice.auto_send` (device-scoped). Anders
als `chat.default_model_id` ist `voice.stt_model_id` ausschließlich device-scoped (kein
vault-weiter Default) — Konsistent mit `voice.auto_send`, das ebenfalls nur device-scoped
existiert; die Transkriptions-Hardware ist an das Gerät gebunden, ein Vault-Default ergäbe hier
keinen zusätzlichen Nutzen gegenüber dem ohnehin geräteweiten Fallback auf `whisper-tiny`.

## Dateisystem-Layout (kein neues Konzept, nur neue Nutzung)

`<AppLocalData>/models/<entry.id>/` über `models::paths::slug_dir` — dieselbe Wurzel, dieselbe
Slug-Validierung (`validate_slug`), die Chat-Modelle bereits nutzen. Ein STT-Slug enthält
`config.json`, `tokenizer.json`, `model.safetensors`; ein Chat-Slug enthält eine `.gguf`-Datei.
Beide Sorten koexistieren nebeneinander im selben Wurzelverzeichnis, ohne dass eine Seite von der
anderen weiß — `canonical_model_file` (Chat-spezifisch, filtert auf `.gguf`) ignoriert
STT-Slug-Verzeichnisse automatisch, da dort keine `.gguf`-Datei liegt.

**Installations-Check** (Ersatz für eine DB-Zeile): Beide Flows verwenden dieselbe
Vollständigkeitsprüfung. Die elementare Predicate-Funktion `is_complete_file(path)` liefert nur
dann `true`, wenn `metadata()` erfolgreich ist, der Pfad eine reguläre Datei bezeichnet und
`len() > 0` gilt. `is_complete_model(dir)` wendet dieses Predicate auf
`config.json`/`tokenizer.json`/`model.safetensors` an und liefert nur dann `true`, wenn alle drei
Dateien vollständig sind. `list_installed_stt_models` nutzt `is_complete_model`; `ensure_model_files`
überspringt einzelne Dateien nur bei `is_complete_file == true` und lädt jede andere Datei neu.
Damit bleiben leere oder abgeschnittene Dateien nicht fälschlich als installiert bestehen. Es
findet weiterhin keine Content-Validierung statt.

## Laufzeit-Zustand: `VoiceState`-Cache-Invalidierung

Kein neues Datenmodell, aber eine neue Übergangsregel: `VoiceState.whisper`
(`AsyncMutex<Option<Arc<LocalWhisperAdapter>>>`) wird von `None` → `Some(adapter für Slug X)` beim
ersten `resolve_local_adapter`-Aufruf nach Start oder nach einer Invalidierung. Ein erfolgreicher
`invalidate_stt_model_cache`-Aufruf setzt ihn zurück auf `None` (im `llm-cpu`-losen Build ist der
Command ein erfolgreicher No-op); der nächste
`resolve_local_adapter`-Aufruf lädt dann den zu diesem Zeitpunkt in `voice.stt_model_id`
eingetragenen Slug.
