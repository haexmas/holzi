# Tauri-Vertrag: Voice Control (Local Speech-to-Text)

Alle Rückgaben verwenden `camelCase`. Auto-Send und aktive Transkriptionsquelle laufen über die
bereits bestehenden generischen `get_pref`/`set_pref`-Commands (`src-tauri/src/storage/preferences_commands.rs`)
mit den in [data-model.md](../data-model.md) definierten Keys — dafür entstehen **keine** neuen
Commands.

## `start_voice_recording`

**Args**: keine.

**Returns**: `void`.

Startet die Mikrofon-Aufnahme (cpal) in einen In-Memory-PCM-Puffer. Schlägt fehl (`PermissionDenied`),
wenn keine Mikrofon-Berechtigung vorliegt. Schlägt fehl (`AlreadyRecording`), wenn bereits eine
Aufnahme läuft — es gibt pro Instanz höchstens eine aktive Aufnahme gleichzeitig.

**Fehler**: `PermissionDenied`, `AlreadyRecording`, `DeviceUnavailable`.

## `stop_voice_recording`

**Args**: keine.

**Returns**:

```typescript
{
  text: string,
  interrupt: "stop" | "halt" | "abbrechen" | null
}
```

Beendet die laufende Aufnahme und transkribiert den aufgenommenen Puffer über die aktuell aktive
Transkriptionsquelle dieses Geräts (`voice.active_stt_provider`-Preference; fällt auf den
gebündelten lokalen Provider zurück, falls die Preference fehlt oder auf eine gelöschte Zeile
zeigt). Der Audio-Puffer wird nach der Transkription verworfen — unabhängig von Erfolg oder
Fehlschlag (FR-020).

Wenn das Transkript exakt (nach Trim/Lowercase) `"stop"`, `"halt"` oder `"abbrechen"` ist, ist
`interrupt` gesetzt und der aufrufende Code MUSS den laufenden Turn sofort abbrechen (bestehendes
`CancellationToken`, `chat/session.rs`) statt `text` ins Eingabefeld zu schreiben. Ein leeres
Transkript (keine erkannte Sprache) liefert `text: ""`, `interrupt: null`; das Frontend ändert in
diesem Fall das Eingabefeld nicht (FR-014).

**Fehler**: `NotRecording` (kein vorheriger `start_voice_recording`-Aufruf),
`TranscriptionFailed { reason }` (lokales Modell-Fehler oder externer Dienst nicht
erreichbar/ungültige Zugangsdaten — Fehler-Taxonomie identisch zum bestehenden `AdapterError`,
inkl. `InvalidCredentials` für 401/403 beim externen Adapter).

## `cancel_voice_recording`

**Args**: keine.

**Returns**: `void`.

Verwirft eine laufende Aufnahme ohne zu transkribieren — für den expliziten Abbruch (Nutzer lässt
den Button außerhalb der Kontrolle los, o. ä.) und für den Fall, dass die App den Vordergrund
verlässt, während eine Aufnahme läuft (FR-016). Kein Fehler, wenn gerade keine Aufnahme läuft
(idempotent).

## Event: `voice-recording-capped`

Kein Command, sondern ein Tauri-Event, emittiert genau einmal pro Aufnahme, wenn die maximale
Aufnahmedauer (FR-017) erreicht wird, bevor der Nutzer selbst `stop_voice_recording` aufgerufen
hat. Payload: `{}`.

Das Frontend reagiert darauf, indem es selbst `stop_voice_recording` aufruft (die Aufnahme läuft
Backend-seitig zu diesem Zeitpunkt bereits nicht mehr weiter, aber der Puffer ist noch vorhanden
und wird bei diesem Aufruf transkribiert). **Race-Hinweis**: wie bei anderen Event+Command-Paaren
in dieser Codebase kann das Event vor Abschluss eines noch laufenden Frontend-Awaits eintreffen —
der Listener muss unabhängig vom Promise-Status auf das Event reagieren können, nicht sich auf
eine Reihenfolge verlassen.

## Erweiterung: `add_provider`

`AddProviderArgs` (`src-tauri/src/providers/mod.rs`) bekommt ein neues, additives Feld:

```typescript
{
  kind: ProviderKind,
  name: string,
  adapter?: string,
  baseUrl?: string,
  apiKey?: string,
  capability?: "chat" | "transcription"   // NEU, Default: "chat"
}
```

Für `capability: "transcription"` und `kind: "api_key"` ist `adapter` der Vendor-Diskriminator
für den externen Transkriptions-Adapter (`ExternalSttAdapter`); `baseUrl`/`apiKey` wie bei
Chat-`api_key`-Providern. `kind: "local"` in Kombination mit `capability: "transcription"` wird
vom Frontend nicht angeboten — die lokale Transkriptionszeile entsteht ausschließlich über
`ensure_local_transcription_provider` beim Start, nie über diesen Command.

## Erweiterung: `list_providers` / `ProviderPayload`

`ProviderPayload` bekommt das gleiche additive `capability`-Feld, damit das Frontend Chat- und
Transkriptions-Provider in der "Modelle verwalten"-Fläche getrennt darstellen kann.
