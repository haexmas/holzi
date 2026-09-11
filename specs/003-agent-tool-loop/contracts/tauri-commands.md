# Contracts: Tauri Commands & Events (Agent Tool Loop)

Alle Kommandos sind async, laufen im Tauri-`invoke_handler` und geben `Result<T, HolziError>`
zurück. Rust-`snake_case` wird beim JSON-Payload zu `camelCase` per
`#[serde(rename_all = "camelCase")]` — bestehende Konvention, unverändert.

## Geänderte Commands

### `send_message(args) -> SendMessageResult`

**Args/Result-Typen unverändert** (`thread_id`, `content`, `system_prompt`, `max_new_tokens`,
`idempotency_key` → `{ thread_id, user_message_id, assistant_message_id }`).

**Verhalten geändert**: löst statt eines einzelnen LLM-Calls die Turn/Step-Loop aus (spec.md User
Story 1+2). `assistant_message_id` bleibt die ID der *finalen* Antwort-Zeile; dazwischenliegende
`tool_call`/`tool_result`-Zeilen (siehe data-model.md) bekommen eigene, neu geminted IDs, die über
die neuen Events unten bekannt gegeben werden — kein Args/Result-Contract-Bruch, nur mehr Events pro
Aufruf.

### `abort_current_generation() -> ()`

**Args/Result unverändert.**

**Verhalten geändert**: bricht nicht mehr nur den LLM-Stream ab, sondern den gesamten laufenden Turn
— inklusive eines aktuell laufenden Tool-Prozesses und eines offenen `tool-permission-request`
(spec.md FR-009/FR-010). Weiterhin idempotent.

## Neue Commands

### `respond_tool_permission(request_id, decision) -> ()`

Beantwortet eine offene Freigabe-Anfrage (spec.md FR-005). Zweiter Teil des Request/Response-Paars
zum `tool-permission-request`-Event unten.

**Args**:

```typescript
{ requestId: string /* uuid */, decision: 'allow' | 'deny' }
```

**Returns**: `void`.

**Fehler**:
- `HolziError::InvalidInput` wenn `requestId` keiner offenen Anfrage entspricht (bereits
  beantwortet, oder der Turn wurde inzwischen abgebrochen — beides idempotent behandelt: kein
  Fehler bei bereits durch Cancellation aufgelöster ID, `InvalidInput` nur bei nie existierter ID).

**Verhalten**: löst den in `ChatState.pending_tool_approvals` wartenden `oneshot::Sender` auf. Kein
Timeout — die Anfrage bleibt offen, bis beantwortet oder der Turn abgebrochen wird (spec.md FR-005,
Acceptance Scenario 5).

## Neue Events (Backend → Frontend)

Bestehende Events (`chat-token`, `chat-message-complete`, `chat-message-error`,
`model-load-progress`) bleiben unverändert in Form und Bedeutung; sie feuern weiterhin pro Step statt
nur einmal pro `send_message`-Aufruf (ein Turn kann mehrere Steps haben).

### `tool-permission-request`

Emittiert, wenn das Freigabe-Gate laut aktivem `chat.permission_mode` eine Bestätigung braucht
(spec.md FR-005). Beantwortet über `respond_tool_permission`.

```typescript
{
  requestId: string,      // uuid, Korrelation zur Antwort
  threadId: string,
  toolName: string,
  toolInput: unknown,     // geparstes JSON, wie es dem Modell/Tool vorliegt
  riskClass: 'safe' | 'risky',
}
```

### `chat-tool-call`

Eine `tool_call`-Zeile (data-model.md) wurde persistiert — Modell hat ein Tool aufgerufen, Freigabe
(falls nötig) wurde bereits erteilt.

```typescript
{ messageId: string, threadId: string, toolName: string, toolInput: unknown, toolSource: 'built_in' | 'mcp' | 'cli' }
```

### `chat-tool-result`

Eine `tool_result`-Zeile wurde persistiert — Ausführung abgeschlossen (erfolgreich, fehlgeschlagen,
oder als "blocked" laut Plan-Modus, spec.md FR-006).

```typescript
{ messageId: string, threadId: string, toolCallId: string, content: string, isError: boolean }
```

### `chat-retry`

Transient, **nicht persistiert** (design doc §3/§6) — informiert die UI über einen laufenden
automatischen Retry-Versuch (spec.md User Story 4), ohne dass ein fehlgeschlagener Versuch je in der
Konversation sichtbar wird.

```typescript
{ threadId: string, assistantMessageId: string, attempt: number /* 1-basiert */ }
```

## Geänderte Datentypen

### `FinishReason` (bestehend: `Complete` | `Cancelled` | `Error`)

**Neue Variante**: `ToolLimitReached` — Turn hat die feste Rundenobergrenze (spec.md FR-016)
erreicht, ohne eine finale Antwort zu liefern. Muss von `Error` unterscheidbar bleiben (spec.md
Edge Cases: "distinguishable from a user-initiated stop and from retry exhaustion"), damit Frontend
eine andere Meldung zeigen kann als bei einem echten Fehler.

```typescript
type FinishReason = 'complete' | 'cancelled' | 'error' | 'tool_limit_reached'
```

## Nicht geänderte Contracts (zur Klarstellung)

- `chat.permission_mode` hat **keine eigenen Commands** — wird über die bestehenden generischen
  `get_pref` / `set_pref` (Scope `Device`) gelesen/geschrieben, siehe data-model.md.
- `list_messages` (bestehend, `thread_commands.rs`) bekommt keinen neuen Parameter — `tool_call`/
  `tool_result`-Zeilen laufen einfach als zusätzliche Rollenwerte durch den bestehenden Rückgabetyp;
  Frontend muss neue `role`-Werte rendern können, das ist eine reine Erweiterung, kein Bruch.
