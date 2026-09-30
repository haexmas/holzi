# Vertrag: Commands und Ereignisse (Rust ↔ Frontend)

**Spec**: [../spec.md](../spec.md) | **Datenmodell**: [../data-model.md](../data-model.md)

Alle Nutzlasten camelCase. „Command“ meint hier ausschließlich Tauri-Commands
(Begriff aus `CONTEXT.md`); fachlich heißt es „Aktion“. Die Commands laufen
durch das Tresor-Gate (`vault_gate`) und sind nur bei offener Sitzung
aufrufbar.

## Commands

### `set_agent_actions`

Ersetzt alle Werkzeuge der Quelle `action` im Register durch die übergebenen.
Wird vom Frontend nach dem Start der Tresor-Sitzung und nach jedem
Sprachwechsel aufgerufen. Idempotent.

```text
args:   { actions: AgentActionDef[] }         // Form: data-model.md §2
result: { registered: number }
errors: InvalidInput  // doppelter toolName, toolName verletzt ^[A-Za-z0-9_-]{1,64}$,
                      // Schema außerhalb der Teilmenge
```

Nebenwirkung: `ToolRegistry` behält `run_command` und MCP-Werkzeuge, ersetzt nur
Werkzeuge mit Quelle `action`. Das Meta-Werkzeug `find_actions` ist immer
vorhanden, sobald mindestens eine Aktion registriert ist.

### `respond_action_call`

Antwort des Frontends auf `action-call-request`.

```text
args:   { requestId: Uuid, outcome: ActionOutcomeWire }
        ActionOutcomeWire = { ok: true, result: any }
                          | { ok: false, code: string, field?: string, message: string }
result: ()
errors: nie für eine unbekannte oder späte requestId (stilles Nichts)
```

Das Frontend sendet nur `code`, `field` und `message` des Runners. Das Feld
`error` (roher Fehler) wird nicht übertragen (FR-006). Bei `code == "failed"`
ersetzt Rust die `message` durch den festen Text `"The action failed."`.

## Ereignisse (Rust → Frontend)

### `action-call-request`

```text
{ requestId: Uuid, threadId: Uuid, actionId: string, input: object }
```

Der globale Listener (`src/plugins/agentActions.client.ts`) führt
`wm.runAction(actionId, input, { kind: 'builtinAgent' })` aus und antwortet mit
`respond_action_call`. Bleibt die Antwort 60 s aus, bricht Rust mit dem
Werkzeugfehler `action_timeout` ab.

### `chat-tool-availability`

```text
{ threadId: Uuid, state: "offered" | "offeredUnverified" | "unsupported" | "delegate" }
```

Einmal je Zug zu Beginn gesendet. Das Frontend zeigt für die drei Zustände
außer `offered` einmal je Unterhaltung und Zustand pro App-Sitzung einen
schließbaren Hinweis (`chat.toolNotice.*`).

### `model-tool-use-updated`

```text
{ modelId: string }
```

Nach Probe oder Selbsttest. Das Frontend lädt die Modelllisten neu
(`list_installed_models` / `list_provider_models`), die `toolUse` in
`capabilities` tragen.

## Geänderte bestehende Verträge

| Vertrag                                          | Änderung                                                                                                                |
| ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------- |
| `tool-permission-request.riskClass`              | neuer Wert `"change"` (vorher `"safe" \| "risky"`)                                                                      |
| `chat-tool-call.toolSource`                      | neuer Wert `"action"` (vorher `"mcp" \| "cli"`); TS-Typ `ToolCallEvent.toolSource`                                      |
| `list_installed_models` / `list_provider_models` | `capabilities.toolUse` (`{ support, basis } \| null`)                                                                   |
| Werkzeug-Fehlertexte                             | neu: `action_timeout`, `action_unavailable`; bestehend: `denied_by_user`, `blocked_by_plan_mode`, `tool_call_cancelled` |

## Suchwerkzeug `find_actions` (für das Modell)

```text
name:        find_actions
description: Search the actions holzi offers by keywords (English works best)
             and get up to 5 matches with id, description and input schema.
             Matches become callable tools in the next step.
input:       { query: string }
risk:        Safe, source "action"
result:      { actions: [{ tool: string, description: string, inputSchema: object }] }
```

- Höchstens 5 Treffer, wortbasiert über camelCase-zerlegte ID-Segmente,
  Beschreibung und die deutschen und englischen Titel; Gleichstand nach
  stabiler ID-Reihenfolge. Ohne Treffer: `{ actions: [] }` und der Hinweis
  `"no matching action"`.
- Die Treffer stammen aus dem Register, nicht aus einem Frontend-Umlauf.
  Nach einer Runde mit diesem Werkzeug hängt der Zug die Treffer an
  `request.tools` an; eine spätere Suche ersetzt die früheren (Angebot
  insgesamt höchstens 15: Kernangebot ≤ 10 plus ≤ 5 Treffer).
- Das Kernangebot (`core: true` in `AgentActionDef`) ist in jedem Schritt für
  jedes Modell gleich.
