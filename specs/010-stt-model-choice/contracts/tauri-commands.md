# Tauri-Vertrag: STT Model Choice

Alle Rückgaben verwenden `camelCase` (wie überall sonst im Projekt). Das Setzen/Lesen der aktiven
Preference läuft über die bereits bestehenden generischen `get_pref`/`set_pref`-Commands
(`src-tauri/src/storage/preferences_commands.rs`) mit dem in [data-model.md](../data-model.md)
definierten Key `voice.stt_model_id` — dafür entsteht **kein** neuer Command. Fünf neue Commands
kommen dazu, alle in `src-tauri/src/stt/commands.rs`.

## `list_stt_catalog`

**Args**: keine.

**Returns**:

```typescript
Array<{
  id: string
  name: string
  hfRepo: string
  hfRevision: string
  approxSizeBytes: number
  license: string
  fit: 'fits' | 'tight' | 'too_big' | 'unknown'
}>
```

Listet den eingebauten STT-Katalog (`stt/stt_catalog.json`), jeder Eintrag annotiert mit dem
Hardware-Fit-Verdict gegen die aktuelle Geräte-Hardware (`hardware::classify`, `context_window:
None`). Analog zu `list_catalog` (`catalog/commands.rs`), aber für den STT-Katalog.

## `stt_recommend_tiers`

**Args**: keine.

**Returns**:

```typescript
Array<{
  tier: 'easy' | 'sweet' | 'max'
  entry: { id: string; name: string; hfRepo: string; hfRevision: string; approxSizeBytes: number; license: string }
  fit: 'fits' | 'tight' | 'too_big' | 'unknown'
}>
```

(genau drei Einträge)

Exakt drei Tier-Empfehlungen für den Onboarding-Schritt, nach demselben generalisierten
Sortier-/Fallback-Algorithmus wie `catalog_recommend_tiers` (siehe
[research.md](../research.md#3-generische-tier-auswahl-logik-statt-duplikation)). Fehlt der
Katalog (kompilierte JSON leer/kaputt), Fehler `CatalogEntryNotFound` — kann in der Praxis nicht
auftreten.

## `list_installed_stt_models`

**Args**: keine.

**Returns**:

```typescript
Array<{ id: string; name: string }>
```

Katalog-Einträge, deren benötigte Dateien bereits vollständig im jeweiligen Slug-Verzeichnis
liegen (Existenz-Check, siehe [data-model.md](../data-model.md#dateisystem-layout-kein-neues-konzept-nur-neue-nutzung)).
Kein DB-Table dahinter — reiner Dateisystem-Scan über die Katalog-IDs.

## `download_stt_model`

**Args**:

```typescript
{ catalogId: string }
```

**Returns**: `{ id: string; name: string }` (das heruntergeladene/bereits vorhandene Modell).

Lädt die drei benötigten Dateien für `catalogId` in `models::paths::slug_dir(app, catalogId)`,
sofern nicht schon vorhanden (idempotent — mirrored `ensure_model_files`, jetzt parametrisiert
über den Katalog-Eintrag statt der bisherigen `WHISPER_REPO`/`WHISPER_REVISION`-Konstanten). Setzt
**nicht** automatisch die aktive Preference — das übernimmt das Frontend nach erfolgreichem
Download über `set_pref`, genau wie beim bestehenden `download_model_from_catalog`-Fluss für
Chat-Modelle (`ModelChoiceStep.vue` ruft `downloadFromCatalogAsync` und danach separat
`setPrefAsync`).

**Fehler**: `CatalogEntryNotFound` (unbekannte `catalogId`); `TranscriptionFailed { reason }` bei
Download-Fehlern (Netzwerk, HTTP-Fehler) — gleiche Fehler-Unification wie bei jedem anderen
`SttError` (siehe `stt/mod.rs`), da hier dieselbe Downloadfunktion (`models::download::download_to_file`)
zum Einsatz kommt wie im bisherigen `ensure_model_files`.

## `invalidate_stt_model_cache`

**Args**: keine.

**Returns**: `void`.

Setzt `VoiceState.whisper` auf `None` zurück. Das Frontend ruft diesen Command unmittelbar nach
einem erfolgreichen `set_pref(voice.stt_model_id, …)`-Wechsel in Settings auf, damit die nächste
Aufnahme den neu gewählten Slug lädt, ohne dass die App neu gestartet werden muss (FR-006/SC-004).
Kein Effekt, falls aktuell kein Adapter geladen ist (z. B. vor der ersten Transkription).

## Änderung an bestehendem Verhalten: `resolve_local_adapter`

`resolve_local_adapter` (`voice.rs:228`) liest vor dem Laden zusätzlich die Preference
`voice.stt_model_id` (device-scoped). Fehlt sie, ist sie leer/ungültig, oder zeigt sie auf eine ID,
die nicht im Katalog steht, fällt sie auf `"whisper-tiny"` zurück — identisch zum heutigen
Verhalten für jeden Nutzer, der diese Funktion nie anfasst (FR-007).
