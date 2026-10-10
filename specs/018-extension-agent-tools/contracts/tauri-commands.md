# Vertrag: Tauri-Commands und -Events

## Neu

| Name                          | Art     | Payload / Args                               | Zweck                                                                  |
| ----------------------------- | ------- | -------------------------------------------- | ---------------------------------------------------------------------- |
| `extension_mcp_send`          | Command | `{frame: string, message: JsonValue}`        | MCP-Nachricht der Erweiterung an Rust                                  |
| `extension-mcp-message`       | Event   | `{frame: string, message: JsonValue}`        | MCP-Nachricht von Rust an die Erweiterung                              |
| `extension-agent-frame`       | Event   | `{extensionId: string, requestId: string}`   | Rust braucht einen Rahmen: Frontend öffnet die App im Hintergrund (R9) |
| `extension_agent_frame_ready` | Command | `{requestId: string, frame: string \| null}` | Antwort darauf; `null` = konnte nicht öffnen                           |
| `extension_agent_tools_set`   | Command | `{extensionId: string, available: bool}`     | Schalter „Für den Agenten verfügbar“ (`*`-Zeile)                       |

`extension_mcp_send` prüft die Rahmen-Sitzung wie `extension_bridge_call`; eine unbekannte wird verworfen.
Keiner der neuen Commands erreicht eine Methode der Brücke (`METHODS`); der Brücken-Vertragstest bleibt
unverändert grün.

## Geändert

| Name                           | Änderung                                                |
| ------------------------------ | ------------------------------------------------------- |
| `tool-permission-request`      | optionales Feld `toolOrigin`                            |
| `chat-tool-call` / Verlauf     | `toolSource: 'haextension'`, optionales `toolOrigin`    |
| `extension-permission-request` | Feld `byAgent: boolean`                                 |
| `extension_install_preview`    | `declared` enthält Zeilen der Art `agentTool` mit Titel |
