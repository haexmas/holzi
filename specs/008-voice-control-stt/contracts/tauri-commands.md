# Tauri-Vertrag: Voice Control (Local Speech-to-Text)

Alle Rückgaben verwenden `camelCase`. Auto-Send und aktive Transkriptionsquelle laufen über die
bereits bestehenden generischen `get_pref`/`set_pref`-Commands (`src-tauri/src/storage/preferences_commands.rs`)
mit den in [data-model.md](../data-model.md) definierten Keys — dafür entstehen **keine** neuen
Commands.

## Kanonischer Audio-Vertrag

Der Backend-Puffer ist ein `CanonicalPcm` mit exakt 16.000 Hz, einem Kanal und normalisierten
`f32`-Samples im Bereich `[-1.0, 1.0]`. `audio`/`cpal` konvertiert native Sample-Typen,
Kanalzahlen und Abtastraten an der Aufnahmegrenze in dieses Format. `SttAdapter::transcribe`
akzeptiert ausschließlich `&CanonicalPcm`; der lokale Adapter verarbeitet die Samples direkt und
der externe Adapter kodiert sie an der HTTP-Grenze als 16-kHz-Mono-signed-16-bit-PCM-WAV. Dieses
Objekt bleibt backend-lokal im Speicher und wird nie über Tauri-IPC oder in Persistenz übertragen.

## `start_voice_recording`

**Args**: keine.

**Returns**: `void`.

Startet die Mikrofon-Aufnahme (cpal) in einen In-Memory-`CanonicalPcm`-Puffer. Parallel startet
eine lokale, begrenzte Interrupt-Erkennung auf denselben Frames. Schlägt fehl
(`PermissionDenied`), wenn keine Mikrofon-Berechtigung vorliegt. Schlägt fehl
(`AlreadyRecording`), wenn bereits eine Aufnahme oder Transkription läuft — es gibt pro Instanz
höchstens eine aktive Pipeline gleichzeitig.

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
gebündelten lokalen Provider zurück, falls die Preference fehlt, ungültig ist, auf eine gelöschte
Zeile zeigt oder auf keinen Provider mit `capability: transcription` und unterstütztem `kind`
zeigt). Der Audio-Puffer wird nach der Transkription verworfen — unabhängig von Erfolg oder
Fehlschlag (FR-020).

Der Übergang von `recording` nach `pending-transcription` ist atomar. Der erste Stop-Aufruf
übernimmt den Puffer und startet genau eine Transkription; gleichzeitige Aufrufe (einschließlich
des Cap-Listeners und eines manuellen Loslassens) warten auf dasselbe Ergebnis. Sie liefern weder
`NotRecording` noch transkribieren oder senden den Puffer ein zweites Mal. Das gilt auch, wenn der
Cap-Listener eintrifft, während ein vorheriger Frontend-`stop_voice_recording`-Await noch läuft.

Die lokale Interrupt-Erkennung läuft während der Aufnahme unabhängig von der gewählten
Transkriptionsquelle. Nach einem bestätigten End-of-Utterance für ein exakt (nach Trim/Lowercase)
passendes `"stop"`, `"halt"` oder `"abbrechen"` löst der Backend-Pfad sofort das bestehende
`CancellationToken` (`chat/session.rs`) aus und emittiert `voice-interrupt-detected`. Dieser
Pfad wartet weder auf die externe Transkription noch auf den Zustand des Assistenten und darf
durch Provider-Fehler nicht blockiert werden. Das vollständige Ergebnis übernimmt den Treffer in
`interrupt`; der Frontend-Code unterdrückt den `text`-Wert und schreibt ihn nicht ins Eingabefeld.
Ein leeres Transkript liefert `text: ""`, `interrupt: null`; das Frontend ändert in diesem Fall
das Eingabefeld nicht (FR-014). Die Cancellation gehört ausschließlich dem Backend-Handler bzw.
seinem lokalen Fast-Path; T019 besitzt sie, während T020 nur Interrupt-Text unterdrückt.

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
und wird bei diesem Aufruf transkribiert). Der Backend-Zustand ist dabei bereits
`pending-transcription`; der Cap-Aufruf und ein gleichzeitiger manueller Aufruf werden zu einem
einzigen Stop-Vorgang koalesziert. **Race-Hinweis**: wie bei anderen Event+Command-Paaren in dieser
Codebase kann das Event vor Abschluss eines noch laufenden Frontend-Awaits eintreffen — der
Listener muss unabhängig vom Promise-Status auf das Event reagieren können.

## Event: `voice-interrupt-detected`

Das Backend emittiert dieses Event höchstens einmal pro Aufnahme, sobald der lokale Fast-Path nach
der End-of-Utterance-Grenze exakt `stop`, `halt` oder `abbrechen` erkennt und das bestehende
`CancellationToken` bereits ausgelöst hat. Payload: `{ command: "stop" | "halt" | "abbrechen" }`.
Die Frontend-Reaktion ist rein visuell; sie schreibt keinen Text und sendet keine Nachricht.

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

Die Rust-Struktur verwendet für dieses Feld einen expliziten Serde-Default (funktional äquivalent
zu `#[serde(default = "default_chat_capability")]`). Fehlt `capability` in einem Legacy-Payload,
wird vor der Persistenz `chat` gesetzt; `None` darf nicht bis in die Datenbank gelangen. Ein
Vertragstest des Legacy-Payloads ohne Feld prüft den gespeicherten Wert `chat`.

Für `capability: "transcription"` und `kind: "api_key"` ist `adapter` der Vendor-Diskriminator
für den externen Transkriptions-Adapter (`ExternalSttAdapter`); `baseUrl`/`apiKey` wie bei
Chat-`api_key`-Providern. `kind: "local"` in Kombination mit `capability: "transcription"` wird
vom Frontend nicht angeboten — die lokale Transkriptionszeile entsteht ausschließlich über
`ensure_local_transcription_provider` beim Start, nie über diesen Command.

## Erweiterung: `list_providers` / `ProviderPayload`

`ProviderPayload` bekommt das gleiche additive `capability`-Feld, damit das Frontend Chat- und
Transkriptions-Provider in der "Modelle verwalten"-Fläche getrennt darstellen kann.
