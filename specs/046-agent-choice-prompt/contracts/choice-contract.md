# Contract: Rückfrage

## Aktion → Rust (`respond_action_call`, `ActionOutcomeWire`)

Neu ist der Fehlercode `needs_choice` mit `options`:

```json
{
  "ok": false,
  "code": "needs_choice",
  "field": "appId",
  "message": "no app matches haex unambiguously",
  "options": [
    { "value": "extension.3103c350-…", "label": "haex-mail" },
    { "value": "extension.2b40421d-…", "label": "haex-notes" },
    {
      "value": "extension.b5101ab7-…",
      "label": "haex-files",
      "unavailable": "Wird übertragen"
    }
  ]
}
```

Frontend: `ActionErrorCode` + `'needs_choice'`; Handler werfen `ActionChoiceError(message, field,
options)`, der Runner bildet sie ab wie `ActionInputError`; `toOutcomeWire` gibt `options` weiter.
Rust maskiert `needs_choice` nicht.

## Rust → Chat: Event `chat-choice-request`

```json
{
  "requestId": "uuid",
  "threadId": "uuid",
  "toolName": "wm_app_open",
  "question": null,
  "field": "appId",
  "value": "haex",
  "options": [{ "value": "…", "label": "haex-mail", "unavailable": null }]
}
```

`question` ist bei `ask_user` gesetzt; dann sind `field` `null` und `value` leer.

## Chat → Rust: Command `respond_choice`

```json
{ "args": { "requestId": "uuid", "answer": { "kind": "option", "value": "extension.…" } } }
{ "args": { "requestId": "uuid", "answer": { "kind": "text", "text": "Einstellungen" } } }
{ "args": { "requestId": "uuid", "answer": { "kind": "cancel" } } }
```

Unbekannte ID → Fehler `InvalidInput`; ID einer abgebrochenen Rückfrage → `Ok(())` (wie
`respond_tool_permission`). Braucht eine offene Vault (nicht in `APP_SCOPED_COMMANDS`).

## Werkzeug `ask_user`

```json
{
  "name": "ask_user",
  "description": "Ask the user a question and offer 2 to 5 possible answers. Use it when you cannot carry out an instruction unambiguously, instead of refusing. The user can also answer in their own words or decline.",
  "input_schema": {
    "type": "object",
    "properties": {
      "question": { "type": "string" },
      "options": {
        "type": "array",
        "items": { "type": "string" },
        "minItems": 2,
        "maxItems": 5
      }
    },
    "required": ["question", "options"]
  }
}
```

Ergebnis an das Modell: `{"answer": "…"}`, `{"answer": "…", "freeText": true}` oder Fehler
`declined_by_user`.

## Aktion `chat.choice.answer`

Scope `guardrails` (nur Nutzer), Eingabe `{ requestId, answer }` wie oben; ruft `respond_choice`.
