# Phase 1 Data Model: Agent Tool Loop

## Geänderte Entitäten

### `chat_messages` (sync-tracked, bestehend)

**Migration 0014** (nach `0013_chat_messages_add_idempotency_key`):

```sql
ALTER TABLE chat_messages ADD COLUMN tool_name TEXT;
--> statement-breakpoint
ALTER TABLE chat_messages ADD COLUMN tool_call_id TEXT;
--> statement-breakpoint
ALTER TABLE chat_messages ADD COLUMN tool_input TEXT;
--> statement-breakpoint
ALTER TABLE chat_messages ADD COLUMN tool_is_error INTEGER;
--> statement-breakpoint
ALTER TABLE chat_messages ADD COLUMN tool_source TEXT;
```

`role` ist bereits eine ungeprüfte `TEXT`-Spalte (keine CHECK-Constraint, siehe Migration
`0006_chat_messages`) — die neuen Rollenwerte brauchen keine Schema-Änderung an `role` selbst, nur
am Rust-seitigen `MessageRole`-Enum.

**Neue/geänderte Spalten**:

| Name | Typ | Nullable | Beschreibung |
|---|---|---|---|
| `role` | TEXT | NO | Bestehend. Neue Werte `tool_call` und `tool_result` neben `user`/`assistant`/`system`. |
| `tool_name` | TEXT | YES | Nur bei `role = tool_call`. Name des aufgerufenen Tools. |
| `tool_call_id` | TEXT | YES | Bei `role = tool_call`: die vom Adapter/Provider vergebene Call-ID (z.B. Anthropics `toolu_...`). Bei `role = tool_result`: dieselbe ID, verknüpft Ergebnis mit Aufruf — kein Hard-FK (gleiche Begründung wie bei `parent_id`: Verzweigung/Historie bleibt über Anwendungslogik, nicht SQL-FK, konsistent). |
| `tool_input` | TEXT | YES | Nur bei `role = tool_call`. JSON-Text der Tool-Eingabe, wie vom Modell geliefert (nach Parsing aus `partial_json`/`arguments`-String). |
| `tool_is_error` | INTEGER | YES | Nur bei `role = tool_result`. `0`/`1`/`NULL` (SQLite hat kein natives Bool); `NULL` bedeutet "kein Fehler" für vor dieser Migration angelegte Zeilen — Anwendungscode behandelt `NULL` und `0` gleich. |
| `tool_source` | TEXT | YES | Nur bei `role = tool_call`. Einer von `mcp` / `cli`. (`built_in` und `acp_delegate` aus der Design-Doku sind für dieses Feature nicht erreichbar — `cli_delegate` ist out of scope, siehe spec.md.) |

**Validierungsregeln** (Rust-Wrapper, `storage/chat_messages.rs`):

- Eine `tool_call`-Zeile hat immer `tool_name`, `tool_call_id`, `tool_input` und `tool_source` gesetzt;
  `content` und `tool_is_error` bleiben leer/`NULL` (kein Doppel-Feld für dieselbe Information).
- Eine `tool_result`-Zeile hat immer `tool_call_id` und `tool_is_error` gesetzt (die ID ist dieselbe
  wie bei ihrer `tool_call`-Zeile) und trägt das Ergebnis in `content` (JSON- oder Klartext, je nach
  Tool); `tool_name`, `tool_input` und `tool_source` sind `NULL`.
- Bei `user`, `assistant` und `system` sind alle fünf Tool-Spalten `NULL`. Die Validierung akzeptiert
  weiterhin vor der Migration angelegte Nicht-Tool-Zeilen, deren neue Spalten automatisch `NULL` sind.
- `parent_id`-Kette bleibt wie heute strikt linear pro Turn: `user` → `assistant`(*) → `tool_call` →
  `tool_result` → `assistant`(*) → ... → finale `assistant`-Zeile ohne weiteren `tool_call`.

**State-Transitions**: keine neuen — jede Zeile ist ab dem Insert final (bestehende
Turn/Step-Loop-Semantik: nichts wird nachträglich verändert, ein Retry vor Erfolg erzeugt gar keine
Zeile, siehe `docs/plans/2026-09-11-agent-tool-loop-design.md` §3).

**Beziehungen**: unverändert (`thread_id` → `chat_threads`, `parent_id` intern, `provider_id`/
`model_id` wie bisher).

---

## Neue Entitäten (Preference, kein Schema-Change)

### `chat.permission_mode` (device-scoped, via bestehende `preferences`-Tabelle)

| Key | Scope | Werte | Default |
|---|---|---|---|
| `chat.permission_mode` | Device | `manual` \| `auto` \| `plan` | `manual` (siehe spec.md Assumptions) |

Kein neues Storage-Konzept — nutzt `storage/preferences.rs`/`PrefScope::Device` exakt wie
`chat.last_active_model_id`.

---

## Neue Rust-Typen (nicht persistiert, laufzeitintern)

### `Tool` (Trait, `chat/tools/mod.rs`)

| Feld/Methode | Typ | Beschreibung |
|---|---|---|
| `name()` | `&str` | Eindeutig innerhalb der Registry für einen Turn (Kollisionsfall: siehe Edge Case unten). |
| `description()` | `&str` | Geht unverändert in `ToolSpec.description`. |
| `input_schema()` | `serde_json::Value` | JSON-Schema-Objekt, direkt in Anthropics `input_schema` bzw. mistralrs' `Function.parameters` wiederverwendbar (beide sind JSON-Schema-förmig, siehe research.md §1/§2). |
| `risk_class()` | `RiskClass::{Safe, Risky}` | Bestimmt Freigabe-Verhalten (spec.md FR-003–FR-006). Das Host-CLI-Tool liefert immer `Risky` (FR-015). |
| `execute(input)` | `async fn(Value) -> ToolResult` (via `async-trait`) | `ToolResult { content: String, is_error: bool }`; the trait remains object-safe for `Box<dyn Tool>`. |

**Edge Case Namenskollision** (aus spec.md Edge Cases nicht explizit behandelt, hier ergänzt): zwei
Quellen (z.B. der Host-CLI und ein MCP-Server) liefern denselben `name()`. Registry-Aufbau MUST das
später hinzugefügte MCP-Tool mit einem Quellen-Präfix disambiguieren, statt eines der beiden
stillschweigend zu verdecken — sonst wird FR-007 ("jede Tool-Nutzung sichtbar") für den verdeckten
Fall irreführend.

Für jedes disambiguierte MCP-Tool hält der Registry-Eintrag getrennt fest:

| Registry-Wert | Bedeutung |
|---|---|
| `registry_name` | Präfixierter Name für ToolSpec, Registry-Lookup und Persistenz (z.B. `mcp:<server-id>:<tool-name>`). |
| `mcp_tool_name` | Unveränderter Originalname aus `tools/list`, ausschließlich für `tools/call`. |
| `mcp_connection` | Die konkrete Serververbindung, die `tools/list` geliefert hat und für `tools/call` wiederverwendet wird. |

`Tool::name()`/die Persistenz verwenden `registry_name`; `execute()` verwendet dagegen immer
`mcp_tool_name` zusammen mit `mcp_connection`. So bleibt die globale Registry eindeutig, ohne den
vom MCP-Server erwarteten Namen oder seine Verbindung zu verlieren.

### `ToolRegistry` (`chat/tools/mod.rs`, in `ChatState`)

Hält `Vec<Box<dyn Tool>>` aus zwei Quellen (das Host-CLI-Tool statisch und MCP dynamisch bei
Server-Verbindung). Individuelle vault-scoped Built-in-Lese-/Schreibtools sind in diesem Feature
nicht enthalten. Baut pro Step die `tools: Vec<ToolSpec>` für `ChatRequest`.

### `PendingToolApproval` (`chat/tools/permission.rs`, in `ChatState`)

`HashMap<Uuid, oneshot::Sender<ApprovalDecision>>`, `ApprovalDecision::{Allow, Deny}`. Lebensdauer:
von Erstellung der Freigabe-Anfrage bis zur Antwort oder bis der Turn abgebrochen wird (Cancellation
nimmt den Sender vorzeitig heraus und droppt ihn, was den wartenden `oneshot::Receiver` mit einem Err
auflöst — der Loop interpretiert das als Cancellation, nicht als Deny, siehe Edge Case in spec.md).

### `ChatRequest`/`StreamChunk`-Erweiterung (`adapters/types.rs`)

| Typ | Neues Feld/Variante |
|---|---|
| `ChatRequest` | `tools: Vec<ToolSpec>` |
| `ChatMessage::role` (`ChatRole`) | + `ToolCall { id, name, input }`, `ToolResult { call_id, content, is_error }` neben `User`/`Assistant` |
| `StreamChunk` | + `ToolCalls(Vec<ToolCall>)`, vor `Done` |
| `ToolSpec` (neu) | `{ name: String, description: String, input_schema: serde_json::Value }` |
| `ToolCall` (neu) | `{ id: String, name: String, input: serde_json::Value }` |

Bei `StreamChunk::ToolCalls(calls)` bleibt die Reihenfolge von `calls` verbindlich. Der Adapter
rekonstruiert daraus für Anthropic genau zwei aufeinanderfolgende Nachrichten:

1. eine `assistant`-Nachricht mit einem `tool_use`-Content-Block je Call, in derselben Reihenfolge,
   jeweils mit `id`, dem vom Modell verwendeten Tool-Namen und `input`;
2. nach der Ausführung eine `user`-Nachricht mit einem passenden `tool_result`-Block je Call, wieder
   in derselben Reihenfolge und mit exakt der jeweiligen `tool_use_id`.

Die persistierten `tool_call`/`tool_result`-Zeilen bleiben einzeln und linear verkettet; nur die
Provider-Rekonstruktion gruppiert die Calls eines Steps. Das Wire-Format für zwei Calls ist damit
schematisch:

```json
[
  {"role":"assistant","content":[
    {"type":"tool_use","id":"call-a","name":"first","input":{}},
    {"type":"tool_use","id":"call-b","name":"second","input":{}}
  ]},
  {"role":"user","content":[
    {"type":"tool_result","tool_use_id":"call-a","content":"result-a"},
    {"type":"tool_result","tool_use_id":"call-b","content":"result-b"}
  ]}
]
```

Ein Adaptertest mit genau zwei Calls prüft diese Assistant-dann-User-Sequenz, die Blockreihenfolge
und die ID-Zuordnung (tasks.md T011A). Wire-Format-Mapping pro Adapter ist zusätzlich in research.md
§1 (Anthropic) und §2 (mistralrs) fixiert.
