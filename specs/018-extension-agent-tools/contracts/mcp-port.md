# Vertrag: MCP über den Port einer Erweiterung

holzi ist MCP-Client (`rmcp`), die Erweiterung Server (vault-sdk `sdk.tools`). Protokollversion: die von
`rmcp` 3.5.0 angebotene; der Server antwortet in `initialize` mit derselben Version.

## Transport

| Richtung            | Weg                                                                                                                        |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| holzi → Erweiterung | Rust-Event `extension-mcp-message {frame, message}` → Frontend → `port.postMessage({type: "haexspace:mcp", message})`      |
| Erweiterung → holzi | `port.postMessage({type: "haexspace:mcp", message})` → `useExtensionFrame` → Command `extension_mcp_send {frame, message}` |
| Tauri-Modus (nativ) | Event-Name `haexspace:mcp` über `listen()` wie andere Ereignisse                                                           |

`message` ist ein JSON-RPC-2.0-Objekt, unverändert. Rust bestimmt die Erweiterung aus `frame`
(`FrameRegistry`); eine unbekannte Rahmen-Sitzung wird verworfen.

## Methoden

| Methode                     | Richtung        | Pflicht | Verhalten des Servers (vault-sdk)                                                                  |
| --------------------------- | --------------- | ------- | -------------------------------------------------------------------------------------------------- |
| `initialize`                | Client → Server | ja      | Antwort mit `capabilities: {tools: {}}`, `serverInfo` aus Manifest                                 |
| `notifications/initialized` | Client → Server | ja      | keine Antwort                                                                                      |
| `ping`                      | beide           | ja      | `{}`                                                                                               |
| `tools/list`                | Client → Server | ja      | alle registrierten Werkzeuge, die im Manifest stehen; Titel, Beschreibung, Schema aus dem Manifest |
| `tools/call`                | Client → Server | ja      | Eingabe gegen Schema prüfen; Fehler → `isError: true` mit Feld; sonst Handler mit `AbortSignal`    |
| `notifications/cancelled`   | Client → Server | ja      | `AbortSignal` des laufenden Aufrufs auslösen                                                       |
| jede andere Anfrage         | beide           | —       | Fehler `-32601` (method not found)                                                                 |

holzi meldet in `initialize` **keine** Client-Fähigkeiten und beantwortet jede Anfrage des Servers mit
`-32601` (R2).

## Ergebnis eines Aufrufs

Handler-Rückgabe `string` → ein Textblock; Objekt → ein Textblock mit JSON; Ausnahme → `isError: true` mit
der Meldung. holzi verbindet Textblöcke, kürzt auf 64 KiB, und reicht dem Modell
`{"extension": "<Anzeigename>", "data": "<Text>"}` weiter (R10).

## Mitschnitte

`contracts/transcripts/*.jsonl`: je Zeile eine JSON-RPC-Nachricht mit Richtung (`c2s`/`s2c`). Aufgezeichnet
von einem Rust-Test (rmcp-Client gegen In-Memory-Server), abgespielt in vault-sdk (vitest) gegen
`sdk.tools`. Pflicht-Mitschnitte: `handshake`, `list`, `call-ok`, `call-invalid-input`, `call-error`,
`call-cancelled`, `server-request-rejected`.
