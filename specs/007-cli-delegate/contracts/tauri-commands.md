# Contracts: Tauri Commands & Events (CLI Delegate Backend)

Alle Kommandos sind async, laufen im Tauri-`invoke_handler` und geben `Result<T, HolziError>`
zurück. `snake_case` ↔ `camelCase` per `#[serde(rename_all = "camelCase")]`, unverändert.

## Neue Commands

### `connect_cli_delegate(args) -> Provider`

Startet und schließt den einmaligen Verbindungs-Flow ab (spec.md FR-001, User Story 2). Läuft in
einem frischen, temporären Verzeichnis (`CLAUDE_CONFIG_DIR`/`CODEX_HOME` + `cwd`, research.md §3):
für `claude` ein `claude setup-token`-Kindprozess, für `codex` ein `codex login`-Kindprozess — beides
Browser-basierte OAuth-Flows, die eine URL ausgeben, die der Nutzer öffnen muss.

**Args**:

```typescript
{ vendor: 'claude' | 'codex', name: string }
```

**Returns**: die neu angelegte oder aktualisierte `Provider`-Zeile (`kind: 'cli_delegate'`,
`adapter: vendor`). **Upsert-Semantik**: genau eine `cli_delegate`-Provider-Zeile pro Vendor,
analog zum bestehenden `local`-Provider-Singleton-Muster (`providers/local.rs`) — ein erneuter Aufruf
für denselben Vendor aktualisiert `credentials` an Ort und Stelle statt eine zweite Zeile anzulegen
(das bedient auch FR-014: reconnect ist derselbe Command, keine separate "reconnect"-Route).

**Fehler**:

- `HolziError::InvalidInput` wenn der Kindprozess (`claude setup-token`/`codex login`) nicht startet
  (Binary fehlt) — spec.md FR-008-artige Behandlung, aber am Setup- statt am Chat-Pfad.
- Ein Fehlschlag _während_ des Browser-Flows (Nutzer bricht ab, Timeout) liefert `InvalidInput` mit
  einer erklärenden `reason`; es wird keine Provider-Zeile angelegt oder verändert.

**Verhalten**: emittiert `delegate-connect-progress` (unten) während des Flows; der Command selbst
löst erst nach Erfolg oder endgültigem Fehlschlag auf. Das temporäre Verzeichnis wird garantiert
entfernt (RAII/`finally`), auch bei Fehschlag (research.md §3, spec.md FR-003).

## Geänderte Commands

### `add_provider(args) -> Provider`

**Verhalten geändert**: die bestehende Ablehnung von `adapter` für `kind: 'cli_delegate'`
(`providers/mod.rs:112-123`) entfällt zugunsten einer Prüfung `adapter ∈ {'claude', 'codex'}`
(data-model.md). In der Praxis ruft die Frontend-UI für `cli_delegate` aber `connect_cli_delegate`
auf, nicht `add_provider` direkt — `add_provider` bleibt der generische, von `connect_cli_delegate`
intern genutzte Insert-Pfad, kein zweiter öffentlich beworbener Weg, dieselbe Provider-Art anzulegen.

### `delete_provider(providerId) -> ()`

**Unverändert, wiederverwendet für "disconnect"** (spec.md FR-013): eine `cli_delegate`-Provider-Zeile
zu löschen _ist_ Disconnect. `storage::delete_provider` löscht nur die `providers`-Zeile, ohne
Cascade auf `models` — die eine gecachte Modell-Zeile pro Vendor (korrigiert während der
Implementierung, data-model.md) bleibt als verwaiste Zeile zurück, exakt wie es heute schon für
gelöschte `api_key`-Provider der Fall ist (bereits bestehendes, nicht `cli_delegate`-spezifisches
Verhalten). Harmlos in der Praxis: `modelGroups` (`stores/models.ts`) iteriert immer über die
aktuelle `providerList`, sodass eine verwaiste Zeile ohne zugehörigen Provider nie im Picker
auftaucht. Kein neuer `disconnect_cli_delegate`-Command nötig.

### `abort_current_generation() -> ()`

**Verhalten erweitert**: bricht — wie schon für den Host-CLI-Tool-Prozess in 003 — auch einen
laufenden Delegate-Subprozess (`claude -p`/`codex app-server`) hart ab, inklusive Prozessgruppen-Kill
analog zu `chat/tools/cli.rs`s bestehendem Muster (spec.md FR-011, User Story 5). Ein offener
`tool-permission-request` aus einer laufenden Delegate-Freigabe wird wie jeder andere beim Abbruch
aufgelöst (bestehendes Verhalten aus 003, unverändert).

## Neue Events (Backend → Frontend)

### `delegate-connect-progress`

Fortschritt während `connect_cli_delegate` läuft.

```typescript
{
  vendor: 'claude' | 'codex',
  status: 'awaiting_browser' | 'success' | 'error',
  url?: string,      // nur bei 'awaiting_browser' — vom Kindprozess ausgegebene OAuth-URL
  message?: string,  // nur bei 'error'
}
```

## Nicht geänderte Contracts (zur Klarstellung)

- **Freigabe-Gate**: `tool-permission-request` / `respond_tool_permission` sind **unverändert** und
  werden von beiden Delegate-Backends genauso genutzt wie vom eingebauten Tool-Loop (research.md §1,
  data-model.md `approval_bridge.rs`) — keine separate Delegate-Freigabe-UI, kein neues Event-Paar.
- **Welches Backend geantwortet hat** (spec.md FR-005): läuft über die bestehende
  `provider_id`/`model_id`-Verknüpfung auf `chat_messages`, die für `api_key`-Provider bereits existiert
  — keine neue Spalte, kein neues Feld in den Message-Events. Ob die aktuelle Frontend-Darstellung
  daraus schon einen Backend-Namen rendert oder das noch verdrahtet werden muss, ist eine
  tasks.md-Detailfrage, kein Contract-Neuentwurf.
- **`chat-tool-call`/`chat-tool-result`** (aus 003): bleiben in Form unverändert; ein Delegate-Backend
  erzeugt dieselben Zeilenarten mit neuem `toolSource`-Wert (`cli_delegate:claude`/`cli_delegate:codex`,
  data-model.md) statt `mcp`/`cli` — reine Werterweiterung, kein Schema- oder Event-Bruch.
- **`list_providers`/`list_provider_models`/`refresh_provider_models`**: unverändert in Form.
  `refresh_provider_models` bleibt für `cli_delegate` bedeutungslos (liefert `Ok(vec![])`, wie
  `local` heute schon) statt eines Fehlers — kleine Verhaltensänderung gegenüber dem heutigen
  `build_adapter`-Fehlerfall, aber keine Signatur-/Contract-Änderung.
