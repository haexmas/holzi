# Contracts: Chat Runtime und Tauri-Schnittstelle

Alle neuen bzw. erweiterten Kommandos liefern `Result<T, HolziError>`. Rust-
Felder werden wie bisher per `camelCase` serialisiert.

## Bestehende Kommandos mit geänderter Nutzung

### `send_message(args)`

Die Chat-Seite sendet für einen neuen Einstieg weiterhin:

```typescript
{
  threadId: null,
  content: string,
  systemPrompt?: string,
  maxNewTokens?: number,
  idempotencyKey: string
}
```

`threadId: null` MUSS einen neuen Thread erzeugen. Ein explizit ausgewählter
Historien-Thread sendet dagegen seine konkrete ID. Idempotenz- und
Retry-Semantik aus Spec 002/003 bleibt unverändert.

### `resolve_default_model()`

Bleibt unverändert und bleibt die autoritative Fallback-Kette aus Spec 002. Der
neue Preload verwendet dieselbe Funktion intern nach dem Vault-Open. Die
passive Auswahl schreibt weiterhin kein `last_active_model_id`.

### `load_model(modelId)`

Bleibt für manuelle Modellwechsel verfügbar. Seine gemeinsame interne
Load-Funktion wird zusätzlich vom Preload verwendet. Ein manueller Wechsel
invalidiert einen älteren Preload über dessen `loadId`.

## Neuer Snapshot-Command

### `model_load_status() -> ModelLoadStatusPayload`

Liefert den aktuell bekannten Load-Zustand für den aktiven Vault.

```typescript
type ModelLoadStatusPayload =
  | { status: 'idle' }
  | {
      status: 'loading'
      loadId: number
      modelId: string
      modelName: string
      phase: 'connecting' | 'loading' | 'cuda-jit-warmup'
      providerName?: string
    }
  | {
      status: 'ready'
      loadId: number
      modelId: string
      modelName: string
    }
  | {
      status: 'error'
      loadId: number
      modelId?: string
      modelName?: string
      code: string
    }
```

Der Command ist read-only und liefert `idle`, wenn keine aktive Vault oder kein
aktiver Load vorhanden ist.

## Bestehendes Event mit Korrelations-ID

### `model-load-progress`

Der bestehende Payload wird um `loadId` ergänzt:

```typescript
{
  loadId: number,
  modelId: string,
  modelName: string,
  phase: 'connecting' | 'loading' | 'cuda-jit-warmup' | 'ready',
  providerName?: string
}
```

Das Frontend verwirft Events mit einer älteren `loadId` als dem zuletzt
bekannten Load.

## Neuer Fehler-Event

### `model-load-error`

```typescript
{
  loadId: number,
  modelId?: string,
  modelName?: string,
  code: string
}
```

`code` ist ein stabiler Backend-Code, kein lokalisierter UI-Text. Das Frontend
übersetzt ihn via `@nuxtjs/i18n`. Der Fehler beendet nicht den aktiven Vault-
Runtime und erlaubt eine erneute Modellwahl.

## Interner Preload

Nach erfolgreicher Veröffentlichung einer neuen aktiven Vault starten
`create_instance` und `open_instance` intern:

```rust
start_default_model_preload(app_handle, app_state, chat_state)
```

Der Helper:

1. erzeugt eine neue `loadId`;
2. führt `resolve_default_model` aus;
3. lädt nur den ersten ladbaren lokalen Kandidaten über die gemeinsame
   Load-Funktion; ein Anbieter-Kandidat wird nicht proaktiv verbunden;
4. veröffentlicht nur bei weiterhin gültiger `loadId` das Ergebnis;
5. setzt bei Fehlern den strukturierten Error-Status und lässt die Vault offen.

Der Helper ist kein zusätzlicher Frontend-Command. Dadurch können Workspace und
Chat nicht versehentlich mehrere konkurrierende Preloads anfordern.

## Frontend-Event-Wrapper

`useChat` ergänzt:

```typescript
async function modelLoadStatusAsync(): Promise<ModelLoadStatusPayload>
async function onModelLoadProgress(
  handler: (event: ModelLoadProgressEvent) => void,
): Promise<UnlistenFn>
async function onModelLoadError(
  handler: (event: ModelLoadErrorEvent) => void,
): Promise<UnlistenFn>
```

Die Chat-Seite registriert Listener vor dem Snapshot-Aufruf, damit zwischen
Listener-Registrierung und Statusabfrage kein Übergang verloren geht.
