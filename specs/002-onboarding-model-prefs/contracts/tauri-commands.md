# Contracts: Tauri Commands (Onboarding + Preferences)

Alle Kommandos sind async, laufen im Tauri-`invoke_handler` und geben `Result<T, HolziError>` zurück. Rust-`snake_case` wird beim JSON-Payload zu `camelCase` per `#[serde(rename_all = "camelCase")]`.

## Neue Commands

### `get_pref(scope, key) -> Option<String>`

Liest genau einen Preference-Wert für den angegebenen Scope+Key.

**Args**:

```typescript
{ scope: { kind: 'vault' } | { kind: 'device'; uuid: string }, key: string }
```

**Returns**: `string | null` (Frontend), `Option<String>` (Rust). `null` bedeutet "row nicht vorhanden ODER value ist NULL" — der Wrapper faltet beide zusammen.

**Fehler**: `HolziError::InvalidInput` wenn `key` leer oder ohne `.`-Namespace.

**Beispiel**:

```typescript
const value = await invoke<string | null>('get_pref', {
  args: { scope: { kind: 'device', uuid: '3f7c-...' }, key: 'chat.default_model_id' }
})
```

### `set_pref(scope, key, value) -> ()`

Setzt oder aktualisiert einen Preference-Eintrag.

**Args**:

```typescript
{ scope: PrefScope, key: string, value: string }
```

**Returns**: `void`.

**Fehler**: 
- `HolziError::InvalidInput` wenn Key-Format ungültig.
- `HolziError::CrdtInit` wenn DB-Zugriff scheitert.

**Verhalten**: 
- Sync-tracked via `haex_hlc_no_sync = current_hlc()`.
- FK-Prüfung: wenn `scope.kind === 'device'` und diese UUID nicht in `known_devices` existiert → SQLite-FK-Fehler → propagiert als `HolziError::CrdtInit` (theoretisch nie erreichbar, weil Bootstrap den Sentinel + eigenes Device sichergestellt hat).

### `clear_pref(scope, key) -> ()`

Löscht einen Preference-Eintrag (nicht Update-auf-NULL).

**Args**: `{ scope: PrefScope, key: string }`

**Returns**: `void`. Idempotent — löschen einer nicht-existierenden Row ist kein Fehler.

### `resolve_default_model() -> ResolveDefaultModelResult`

Führt die Session-Resolver-Kette (FR-014) aus und gibt zurück, welches Modell zu laden ist plus warum.

**Args**: (keine — nutzt intern die aktive Device-UUID).

**Returns**:

```typescript
{
  modelId: string | null,             // null = Onboarding-Panel zeigen (FR-014 Pos. 5)
  source: 'last_active' | 'default_device' | 'default_vault' | 'first_available' | 'none'
}
```

**Fehler**: `HolziError::CrdtInit` bei DB-Zugriffsfehlern.

**Verhalten**: 
- Reine Read-Operation, KEIN Preference-Write.
- Wenn `source === 'first_available'`, ist das eine passive Wahl — der Aufrufer (Frontend) schreibt kein `last_active_model_id` auf Basis dieses Ergebnisses. Erst wenn der Nutzer intentional handelt (send_message oder manueller Picker-Wechsel), erfolgt der Write.

### `current_device_info() -> DeviceInfoPayload`

Liefert Metadaten über das aktuelle Gerät im Kontext des aktiven Vaults.

**Args**: (keine).

**Returns**:

```typescript
{
  installationUuid: string,     // aus <AppLocalData>/installation-id
  vaultDeviceUuid: string,      // aus known_devices[installation_uuid]
  alias: string | null,         // null = Onboarding fällig
  hostname: string | null       // OS-Hostname wenn ermittelbar, sonst null
}
```

**Fehler**: `HolziError::CrdtInit` wenn kein aktiver Vault geöffnet oder Bootstrap noch nicht gelaufen.

**Verwendung**:
- Chat-/Workspace-/Settings-Route-Guard-Middleware ruft dies und prüft `alias === null`.
- Onboarding-Wizard nutzt `hostname` als initial-Wert des Alias-Inputs; wenn `null`, greift Frontend auf `$t('onboarding.alias.defaultPlaceholder')` zurück.
- Settings-Screen zeigt `alias` als Kontext-Titel (`$t('settings.header.forDevice', { alias })`).

**Grund für nullable `hostname`**: Konsistent mit FR-020 (i18n-Boundary) — Backend liefert die rohe OS-Info, Frontend entscheidet über den lokalisierten Fallback-Text. Kein lokalisierter String im Backend.

### `update_device_alias(alias) -> ()`

Setzt oder ändert den Alias des aktuellen Geräts.

**Args**: `{ alias: string }`

**Returns**: `void`.

**Fehler**: `HolziError::InvalidInput` wenn `alias` leer oder nur Whitespace.

**Verhalten**: Ruft intern `storage::known_devices::update_alias`, das bereits existiert und `haex_hlc_no_sync` korrekt injiziert.

### `catalog_recommend_tiers() -> [TierRecommendation; 3]`

Liefert drei Hardware-passende Modell-Vorschläge (Easy/Sweet/Max) für den Onboarding-Wizard.

**Args**: (keine — ruft intern `hardware::detect` und wendet den Fit-Klassifikator auf den Katalog an).

**Returns**:

```typescript
[
  { tier: 'easy', entry: CatalogEntry, fit: 'fits' | 'tight' | 'too_big' | 'unknown' },
  { tier: 'sweet', entry: CatalogEntry, fit: ... },
  { tier: 'max', entry: CatalogEntry, fit: ... }
]
```

**Fehler**: `HolziError::CatalogEntryNotFound` in dem Randfall, dass der Katalog nur zwei Einträge hat (dann wird Max = Sweet-Fallback verwendet und ein Fehler-Signal beibehalten). Frontend behandelt bei Empfangs-Fehler den Wizard mit "Katalog leer" — sollte in Prod nie eintreten weil `model_catalog.json` fünf Einträge shippt.

**Algorithmus**:
1. Katalog-Einträge laden, per Fit klassifizieren.
2. Easy = kleinster `Fits`-Kandidat. Fallback: kleinster `Tight`, dann kleinster `Unknown`.
3. Sweet = größter `Fits`-Kandidat. Fallback: Median-Kandidat.
4. Max = größter `Fits`- ODER `Tight`-Kandidat. Fallback: größter overall.

## Geänderte Commands

### `list_installed_models` (bestehend)

**Verhaltens-Änderung**: statt DB-Query gegen `device_downloaded_models_no_sync` × `models`, wird jetzt `<AppLocalData>/models/` per `tokio::fs::read_dir` gescannt. Jedes Sub-Dir wird gegen `models::get_model(&slug)` gelookupt; nur Slugs mit passender `models`-Row werden zurückgegeben.

**Return-Signatur** (bleibt gleich):

```typescript
{
  id: string,
  name: string,
  providerId: string,
  contextWindow: number | null,
  relativePath: string,       // rekonstruiert aus <slug>/<first-gguf-in-dir>
  sizeBytes: number           // aus fs::metadata
}[]
```

**Fehler**: `HolziError::Io` wenn `AppLocalData/models/` nicht gelesen werden kann; sollte nur bei Filesystem-Corruption auftreten.

### `send_message` (bestehend)

**Verhaltens-Änderung**: nach erfolgreichem Message-Persistieren wird `preferences[('<my_device_uuid>', 'chat.last_active_model_id')] = <current_model_id>` gesetzt (FR-009 Bedingung "erfolgreiches Senden einer Nachricht"). Idempotent bei wiederholten Sends mit demselben Modell.

**Kein Return-Change**.

### `load_model` (bestehend)

**Verhaltens-Änderung**: KEINE Contract-Änderung. `load_model` schreibt WEDER bei Auto-Load NOCH bei manuellem Picker-Wechsel in `chat.last_active_model_id`. Der Preference-Write erfolgt ausschließlich in `send_message` (siehe unten).

**Grund** (Post-Clarify/Analyze-Korrektur FR-009): Ausprobier-Klicks im Dropdown sollen keine Persistenz-Wirkung haben. Der Nutzer kann bedenkenlos verschiedene Modelle laden und wieder wechseln; erst wenn er tatsächlich mit einem Modell chattet (via `send_message`) wird der Wechsel als "letzte aktive Nutzung" gemerkt.

## Neue Events

### `model-load-progress`

Emittiert vom Backend während `load_model`-Ausführung. **Strukturierter Payload — keine lokalisierten Strings** (FR-020):

```typescript
{
  modelId: string,
  modelName: string,               // display name aus models-Tabelle
  phase: 'connecting' | 'loading' | 'cuda-jit-warmup' | 'ready',
  providerName?: string            // nur bei phase === 'connecting' gesetzt
}
```

**Frontend-Übersetzung** (Beispielhaft für deutsche Locale):

| Phase | i18n-Key | Beispiel (de) |
|---|---|---|
| `connecting` | `chat.loading.connecting` | "Verbinde mit {providerName}…" |
| `loading` | `chat.loading.loading` | "Lade {modelName}…" |
| `cuda-jit-warmup` | `chat.loading.cudaJitWarmup` | "Optimiere GPU für erste Nutzung von {modelName}, dauert einmalig etwa 30 Sekunden…" |
| `ready` | `chat.loading.ready` | "Bereit" (Signal: Ladepanel ausblenden) |

Englische Locale-Einträge in derselben Struktur (`src/i18n/en/chat.json`).

**CUDA-Erkennung**: Backend prüft `~/.nv/ComputeCache/` für Modell-Signatur; wenn kein Cache-Eintrag existiert UND der Build `cfg(feature = "llm-cuda")` hat, wird `cuda-jit-warmup` emittiert. Auf CPU/Metal-Builds wird stattdessen `loading` gesendet.

## Nicht-Regression

Alle bestehenden Commands außer den drei oben aufgeführten (`list_installed_models`, `send_message`, `load_model`) bleiben unverändert. Alle bestehenden Events (`chat-token`, `chat-message-complete`, `chat-message-error`, `model-download-progress`, `model-download-complete`) bleiben strukturell unverändert.
