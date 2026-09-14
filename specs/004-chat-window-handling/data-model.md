# Data Model: Chatfenster und Session-Handling

Dieses Feature führt keine neue persistierte Entität ein. Es ergänzt den
ephemeren Runtime-State und beschreibt die Frontend-Zustände für Session,
Composer und Reasoning.

## Chat-Session-Entwurf

```typescript
type ChatDraft = {
  kind: 'new'
  threadId: null
}
```

Der Entwurf wird beim Chat-Einstieg erzeugt und enthält keine Nachrichten. Beim
ersten Senden wird er über `send_message({ threadId: null, ... })` zu einem
normalen `chat_threads`-Eintrag. Beim Verlassen ohne Nachricht wird er verworfen.

Ein explizit aus der Historie ausgewählter Thread bleibt dagegen ein
persistierter Kontext mit einer konkreten `threadId`.

## Model-Load-Status

Der Status liegt in `ChatState` und wird nicht in der Vault gespeichert.

```rust
pub enum ModelLoadStatus {
    Idle,
    Loading {
        load_id: u64,
        model_id: String,
        model_name: String,
        phase: LoadPhase,
        provider_name: Option<String>,
    },
    Ready {
        load_id: u64,
        model_id: String,
        model_name: String,
    },
    Error {
        load_id: u64,
        model_id: Option<String>,
        model_name: Option<String>,
        code: String,
    },
}
```

`load_id` steigt bei jedem neuen Preload oder manuellen Modellwechsel. Nur der
aktuelle Load darf `ChatState.session` und den sichtbaren Status auf `Ready`
oder `Error` setzen. Der Status wird beim Vault-Wechsel auf `Idle` bzw. auf den
neuen Load gesetzt.

Die bestehenden Phasen `connecting`, `loading`, `cuda-jit-warmup` und `ready`
bleiben Bestandteil des Event-Vertrags aus Spec 002.

## Reasoning-State

```typescript
type ReasoningState = {
  byMessageId: Record<string, string>
  expandedMessageIds: Set<string>
}
```

`byMessageId` wird aus eingehenden `TokenEvent.reasoning`-Deltas aufgebaut.
`expandedMessageIds` enthält nur vom Nutzer geöffnete Accordions. Beide Werte
werden beim neuen Chat-Einstieg bzw. beim Mount der Chat-Ansicht neu initialisiert
und nicht in `chat_messages` geschrieben.

## Composer-State

Der Composer nutzt die bestehenden Werte:

```typescript
type ComposerState = {
  input: string
  effortLevel: 'low' | 'medium' | 'high'
}
```

Die Modellwahl und der Freigabemodus bleiben an ihre bestehenden Runtime- bzw.
Preference-Verträge gebunden. Unterstützt das gewählte Modell Reasoning, wird
es automatisch aktiviert; dafür gibt es keinen Composer-State. Die neue Spec
verändert keine Datenbank-Persistenz für diese Werte.

## Zustandsübergänge

```text
Vault closed
  → Vault published + preload idle/loading
  → Workspace available
  → Chat opened + new ChatDraft
  → send_message(threadId=null)
  → persisted Thread + streaming Turn
```

Ein Modell-Load kann unabhängig davon `loading → ready/error` wechseln. Ein
neuer Vault-Wechsel oder ein neuer Load invalidiert alle älteren `load_id`s.
