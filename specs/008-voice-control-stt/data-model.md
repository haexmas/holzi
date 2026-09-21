# Phase 1 Data Model: Voice Control (Local Speech-to-Text)

## Schemaänderung: `providers.capability`

Die bestehende `providers`-Tabelle (`src-tauri/src/storage/providers.rs`) bekommt eine additive
Spalte:

| Feld         | Typ                     |             Nullable | Beschreibung                                                                                              |
| ------------ | ----------------------- | -------------------: | --------------------------------------------------------------------------------------------------------- |
| `capability` | `chat \| transcription` | nein, Default `chat` | Unterscheidet, wofür eine Provider-Zeile gilt. Bestehende Zeilen werden per Migration auf `chat` befüllt. |

`ProviderKind` (`local`/`api_key`/`cli_delegate`) bleibt unverändert und orthogonal zu
`capability` — eine Zeile kann z. B. `kind = local, capability = transcription` sein (das
gebündelte Whisper-Modell) oder `kind = api_key, capability = transcription` (ein externer
Dienst).

**Notwendige Anpassung an bestehendem Code**: `find_local_provider`
(`src-tauri/src/providers/local.rs:34`) filtert aktuell nur nach `kind = 'local'`. Nach dieser
Änderung gibt es zwei `kind = local`-Zeilen (Chat und Transkription); die Query MUSS zusätzlich
nach `capability = 'chat'` filtern, sonst kann sie je nach `created_at`-Reihenfolge die falsche
Zeile zurückgeben. Das ist keine neue Anforderung, sondern eine Korrektheitsbedingung der
Migration selbst.

## Neue Runtime-/Storage-Entität: gebündelter lokaler Transkriptions-Provider

Mirrors `ensure_local_provider` (`src-tauri/src/providers/local.rs:17`): eine neue
`ensure_local_transcription_provider(conn)`-Funktion, idempotent, legt bei Bedarf genau eine Zeile
an:

| Feld          | Wert                                                            |
| ------------- | --------------------------------------------------------------- |
| `kind`        | `Local`                                                         |
| `capability`  | `Transcription`                                                 |
| `adapter`     | `"whisper-local"` (Diskriminator für den `LocalWhisperAdapter`) |
| `name`        | z. B. `"Gebündelt (offline)"`                                   |
| `base_url`    | `None`                                                          |
| `credentials` | `None`                                                          |

Es gibt genau eine solche Zeile pro Instanz, analog zur bestehenden Singleton-Semantik des
lokalen Chat-Providers (ein `mistralrs`-Prozess = eine Zeile).

## Neue Preferences

Nutzt die bestehende namespaced Key/Value-Preferences-Infrastruktur
(`src-tauri/src/storage/preferences.rs`), kein neues Schema:

| Key                         | Scope                            | Typ                        | Default                                             | Beschreibung                                                                                                                                                                                                                                                                                                                                              |
| --------------------------- | -------------------------------- | -------------------------- | --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `voice.auto_send`           | `PrefScope::Vault`               | `"true" \| "false"`        | `"true"`                                            | Steuert, ob ein Transkript automatisch abgeschickt wird (FR-005). Vault-weit, weil reine UX-Präferenz.                                                                                                                                                                                                                                                    |
| `voice.active_stt_provider` | `PrefScope::Device(device_uuid)` | Provider-`Uuid` als String | Id des gebündelten lokalen Transkriptions-Providers | Welche Transkriptionsquelle auf diesem Gerät aktiv ist (FR-012). Geräte-lokal, weil an Hardware/Netzwerksituation des jeweiligen Geräts gekoppelt (research.md §6). Vor dem Adapter-Aufbau muss die referenzierte Zeile existieren, `capability = transcription` besitzen und `kind` `local` oder `api_key` sein; andernfalls greift der lokale Fallback. |

Beide Keys folgen der bestehenden Namespacing-Regel (`UnnamespacedKey`-Validierung in
`preferences.rs`) und laufen über dieselben Helfer wie andere Preferences, inklusive
`haex_hlc_no_sync`-Handling für CRDT-Sichtbarkeit.

Beim Dispatch wird die Preference als untrusted UUID geparst und gegen die Providerzeile
validiert, bevor ein `SttAdapter` konstruiert wird. Eine fehlende, gelöschte, ungültige oder
inkompatible Zeile (insbesondere ein Chat-Provider oder `cli_delegate`) fällt auf den gebündelten
lokalen Transkriptions-Provider zurück. Dieselbe Validierung bestimmt das externe Signal der
Mic-Kontrolle; eine bloße UUID-Übereinstimmung reicht nicht.

## Laufzeit-only-Typen (nicht persistiert)

### `VoiceRecordingState` (Frontend und Backend-Pipeline)

Reiner UI-Zustand der Mic-Kontrolle, nicht persistiert (konsistent mit FR-020 — nichts über die
Aufnahme selbst wird gespeichert):

| Zustand        | Bedeutung                                                                                                                |
| -------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `idle`         | Bereit, keine Aufnahme läuft.                                                                                            |
| `recording`    | Aufnahme aktiv (per Mic-Kontrolle gestartet); sichtbar sind nur noch Abbrechen- und Senden-Icon.                         |
| `transcribing` | Aufnahme beendet, Transkription läuft.                                                                                   |
| `error`        | Mikrofon-Berechtigung fehlt, Transkription fehlgeschlagen, oder externer Dienst nicht erreichbar/ungültige Zugangsdaten. |

Backend-seitig wird der Puffer beim ersten Stop atomar aus `recording` in
`pending-transcription` überführt. Weitere Stop-Aufrufe erhalten dasselbe laufende Ergebnis; sie
starten weder eine zweite Transkription noch liefern sie `NotRecording`. Das verhindert die Race
zwischen `voice-recording-capped` und manuellem Senden.

Ein zusätzliches sichtbares Merkmal (nicht ein eigener State) markiert, wenn die aktuell aktive
Quelle extern ist — sichtbar in jedem Nicht-`idle`-Zustand, solange `voice.active_stt_provider`
auf eine `capability = transcription, kind = api_key`-Zeile zeigt (FR-021).

### `InterruptCommand` (Backend, `stt::interrupt`)

Kein Datenmodell im eigentlichen Sinn — eine feste, nicht persistierte Aufzählung:

```rust
#[serde(rename_all = "lowercase")]
enum InterruptCommand { Stop, Halt, Abbrechen }
```

Die drei Varianten haben damit die explizite Wire-Repräsentation `"stop"`, `"halt"` und
`"abbrechen"`. `TranscriptionResult.interrupt` wird über diese serialisierte Repräsentation aus
`match_interrupt` befüllt; es werden keine Rust-Variantennamen wie `Stop` über die Tauri-Grenze
gegeben. Der lokale Fast-Path kann denselben Wert vor Abschluss der vollständigen Transkription
als `voice-interrupt-detected`-Event liefern.

Das Matching (`stt::interrupt::match_interrupt(transcript: &str) -> Option<InterruptCommand>`)
normalisiert (trim, lowercase) und vergleicht das **gesamte** Transkript gegen die drei Wörter
(case-insensitive). Kein Teilstring-Match — "abbrechen" darf nur zünden, wenn es die gesamte
Äußerung ist, nicht Teil eines längeren Satzes (FR-009, SC-006).

### `TranscriptionResult` (Backend↔Frontend-Grenze)

```typescript
{
  text: string,
  interrupt: "stop" | "halt" | "abbrechen" | null
}
```

`interrupt` ist gesetzt, wenn `match_interrupt` einen Treffer hatte; in dem Fall bleibt `text`
informativ (z. B. für Logging), das Frontend schreibt es **nicht** ins Eingabefeld (FR-007 vs.
FR-004 sind exklusiv für dieselbe Transkription). Tests prüfen für alle drei Treffer die exakten
lowercase-Werte sowie `null` für Nicht-Treffer.
