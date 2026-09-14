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
    Idle {
        vault_generation: u64,
    },
    Loading {
        vault_generation: u64,
        load_id: u64,
        model_id: String,
        model_name: String,
        phase: LoadPhase,
        provider_name: Option<String>,
    },
    Ready {
        vault_generation: u64,
        load_id: u64,
        model_id: String,
        model_name: String,
    },
    Error {
        vault_generation: u64,
        load_id: u64,
        model_id: Option<String>,
        model_name: Option<String>,
        code: String,
    },
}
```

`load_id` steigt bei jedem neuen Preload oder manuellen Modellwechsel und
ordnet Loads innerhalb derselben `vault_generation`. `vault_generation` steigt
unabhängig davon bei jedem Vault-Open oder -Wechsel und grenzt eine
Vault-Instanz gegen ihre Vorgänger ab. Nur ein Snapshot oder Event mit der
aktuell gültigen `vault_generation` und — innerhalb dieser Generation — der
aktuellen `load_id` darf `ChatState.session` und den sichtbaren Status auf
`Ready` oder `Error` setzen; ein Event mit veralteter `vault_generation` wird
verworfen, selbst wenn seine `load_id` numerisch neuer wäre als der zuletzt für
die aktuelle Vault bekannte Wert. Der Status wird beim Vault-Wechsel auf `Idle`
mit der neuen `vault_generation` bzw. auf den neuen Load gesetzt.

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

## Reasoning-Capability

Reasoning ist keine Composer-Einstellung, sondern eine aus dem gewählten
Modell abgeleitete Fähigkeit:

```rust
pub struct ChatRequest {
    // …bestehende Felder…
    pub reasoning_requested: bool,
}
```

`reasoning_requested` wird serverseitig in `commands.rs` aus der Capability des
aufgelösten Modells befüllt, bevor `adapter.stream_chat(request)` aufgerufen
wird — das Frontend setzt dieses Feld nicht. Ein Adapter, der Reasoning nur
nach explizitem Request-Flag liefert (z. B. Anthropic `thinking`), aktiviert es
ausschließlich über dieses Feld; ein Adapter ohne ein solches Flag ignoriert
es. Ist die Capability für das gewählte Modell unbekannt oder `false`, MUSS
kein Reasoning angefordert werden, und `StreamChunk::Delta.reasoning` bleibt
`None` — dann wird auch kein leeres Accordion gerendert (FR-031).

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
