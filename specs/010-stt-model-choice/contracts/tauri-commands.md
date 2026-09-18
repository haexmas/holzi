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
  entry: {
    id: string
    name: string
    hfRepo: string
    hfRevision: string
    approxSizeBytes: number
    license: string
  }
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
liegen. Vollständig bedeutet für jede erwartete Datei `metadata().is_file()` und `len() > 0`; der
Command verwendet dafür dieselbe `is_complete_file`-/`is_complete_model`-Prüfung wie
`ensure_model_files` (siehe
[data-model.md](../data-model.md#dateisystem-layout-kein-neues-konzept-nur-neue-nutzung)). Kein
DB-Table dahinter — reiner Dateisystem-Scan über die Katalog-IDs.

## `download_stt_model`

**Args**:

```typescript
{
  catalogId: string
}
```

**Returns**: `{ id: string; name: string }` (das heruntergeladene/bereits vorhandene Modell).

Lädt die drei benötigten Dateien für `catalogId` in `models::paths::slug_dir(app, catalogId)`. Eine
Datei wird nur dann übersprungen, wenn die gemeinsame Vollständigkeitsprüfung sie als reguläre
Datei mit Größe > 0 erkennt; leere, abgeschnittene oder fehlende Dateien werden erneut geladen
(idempotent — mirrored `ensure_model_files`, jetzt parametrisiert über den Katalog-Eintrag statt
der bisherigen `WHISPER_REPO`/`WHISPER_REVISION`-Konstanten). Setzt
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

Der Command bleibt wie die übrigen Voice-Commands in allen Feature-Sets registriert, damit die
Frontend-Aufrufliste stabil bleibt. Im Build mit `voice` und `llm-cpu` setzt er `VoiceState.whisper`
auf `None`; falls aktuell kein Adapter geladen ist, ist das ein No-op. Im Build mit `voice`, aber
ohne `llm-cpu`, liefert er erfolgreich `void` zurück und darf `VoiceState.whisper` nicht
referenzieren. Ohne `voice` stellt der Stub ebenfalls einen erfolgreichen No-op bereit. Das
Frontend ruft den Command unmittelbar nach einem erfolgreichen
`set_pref(voice.stt_model_id, …)`-Wechsel in Settings auf, damit die nächste Aufnahme den neu
gewählten Slug lädt, ohne dass die App neu gestartet werden muss (FR-006/SC-004).

## Änderung an bestehendem Verhalten: `resolve_local_adapter`

`resolve_local_adapter` (`voice.rs:228`) liest vor dem Laden zusätzlich die Preference
`voice.stt_model_id` (device-scoped). Fehlt sie, ist sie leer/ungültig, oder zeigt sie auf eine ID,
die nicht im Katalog steht, fällt sie auf `"whisper-tiny"` zurück — identisch zum heutigen
Verhalten für jeden Nutzer, der diese Funktion nie anfasst (FR-007).
