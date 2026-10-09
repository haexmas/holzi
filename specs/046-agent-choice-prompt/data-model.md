# Data Model: Agent-Rückfrage mit Auswahl

Keine neue Tabelle, keine Migration. Rückfragen leben im Speicher, solange der Turn läuft; ihr Ergebnis
landet im `content` der bestehenden `tool_result`-Zeile (`storage/chat_messages.rs`).

## ChoiceRequest (Rust, `chat/tools/mod.rs`)

| Feld       | Typ                 | Bedeutung                                                                 |
| ---------- | ------------------- | ------------------------------------------------------------------------- |
| `question` | `Option<String>`    | Frage des Agenten (`ask_user`); `None` bei einer Aktion → UI-Standardtext |
| `field`    | `Option<String>`    | Eingabefeld der Aktion, das die Antwort bekommt; `None` bei `ask_user`    |
| `value`    | `String`            | Die ursprüngliche Angabe (z. B. `"haex"`), leer bei `ask_user`            |
| `options`  | `Vec<ChoiceOption>` | 0–5 Kandidaten, nach Passung geordnet                                     |

Invarianten: `field.is_some()` ⇔ Herkunft Aktion. `ask_user` hat 2–5 Optionen; eine Aktion 0–5.

## ChoiceOption

| Feld          | Typ              | Bedeutung                                                     |
| ------------- | ---------------- | ------------------------------------------------------------- |
| `value`       | `String`         | Wert für die erneute Ausführung (App-ID) bzw. Antworttext     |
| `label`       | `String`         | Anzeigename (App-Titel wie im Launcher)                       |
| `unavailable` | `Option<String>` | Übersetzter Grund, warum nicht wählbar (R8); dann deaktiviert |

## ChoiceAnswer

`Option { value }` | `Text { text }` | `Cancel`.

## Zustände einer Rückfrage

```text
offen ──Option/Text──▶ beantwortet ──(Aktion, wieder mehrdeutig)──▶ neue Rückfrage (offen)
  │
  ├──Cancel──────────▶ abgelehnt   → Ergebnis declined_by_user
  └──Turn-Abbruch────▶ verworfen   → Runde ohne Zeilen (bestehende Regel)
```

## Vermerk im gespeicherten Ergebnis (FR-010)

- Aktion: `{"result": <Ergebnis der Aktion>, "choice": {"value": "haex", "answer": "extension.<uuid>"}}`
- `ask_user`: `{"answer": "<gewählte Option oder Text>", "freeText": true?}`
- Abgelehnt: Marker `declined_by_user`, `tool_is_error = true`.

## Frontend

`PendingPrompt = { kind: 'approval', requestId, toolName, toolInput, riskClass, toolSource? }
| { kind: 'choice', requestId, toolName, question?, field?, value, options }` — ersetzt `PendingApproval`
in der Warteschlange (R9).

## AppMatch (`src/lib/wm/appMatch.ts`)

`matchApp(input, apps, titleOf) → { kind: 'exact', appId } | { kind: 'choice', candidates: { appId, title, unavailableKey? }[] }`
