# Agent-Rückfrage mit Auswahl: Design

Stand: 2026-10-08, abgestimmt im Brainstorming. Spec: [`specs/046-agent-choice-prompt`](../../specs/046-agent-choice-prompt/spec.md).

## Ausgangslage

Der eingebaute Agent sollte die installierte Erweiterung „haex-notes“ öffnen und riet `system.notes`.
Erweiterungen haben die App-ID `extension.<uuid>` (UUIDv5 aus Public Key und Name,
`src-tauri/src/extensions/ids.rs`), die kein Modell aus dem Titel ableiten kann. Der Fehler kam als
maskiertes „The action failed.“ an, der Agent antwortete „nicht möglich“.

Der Fix-Branch `fix/agent-unknown-app` lässt eine unbekannte App als `invalid_input` mit allen
gültigen IDs durch (`ActionInputError` im Runner). Der Test mit Qwen3-4B zeigte: Das Modell sieht die
richtige ID in der Meldung und korrigiert trotzdem nicht. Es verkettet nach einem Tool-Fehler keine
weiteren Schritte. Eine Lösung, die auf diese Verkettung setzt, trägt mit kleinen lokalen Modellen
nicht.

Gewünscht ist allgemein: Kann der Agent eine Anweisung nicht umsetzen (Tippfehler im App-Namen,
mehrdeutige Angabe), fragt er nach und bietet Alternativen an. Der Nutzer wählt per Radio-Group, die
Wahl wird ausgeführt.

## Entscheidungen

| Frage                    | Entscheidung                                                                                                                                |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Wer löst Tippfehler auf? | Die Aktion selbst: exakte ID, Alias, dann Fuzzy-Treffer auf Titel und Name. Ein klarer Treffer öffnet direkt                                |
| Wer stellt die Rückfrage | Deterministisch das `ActionTool` in Rust, wenn eine Aktion Kandidaten liefert. Das Modell muss nichts verketten                             |
| Allgemeine Unklarheit    | Zusätzlich ein eingebautes Tool `ask_user` plus eine Zeile im Systemprompt: nachfragen statt ablehnen                                       |
| Form der Rückfrage       | Blockierend im laufenden Turn (Muster der Tool-Freigabe), keine eigene Chat-Nachricht, die den Turn beendet                                 |
| Antwortmöglichkeiten     | Radio-Group der Kandidaten, „Etwas anderes …“ mit Textfeld, Bestätigen, Abbrechen                                                           |
| Verworfen                | Nur `ask_user` ohne deterministischen Pfad (scheitert an fehlender Verkettung). Rückfrage als turn-beendende Nachricht (braucht neuen Turn) |

## Architektur

**1. Auflösung in der Aktion (Frontend)**

- `wm.app.open` und `wm.tab.new` akzeptieren in `appId` neben der ID auch Freitext („haex-mial“,
  „Notizen“).
- Reihenfolge: exakte ID → Alias (`resolveAppAlias`) → Fuzzy-Suche über Titel und Name mit `fuse.js`
  (wie `src/lib/passwords/search.ts`, `src/lib/models/search.ts`).
- Ein klarer Treffer öffnet direkt. Sonst wirft der Handler `ActionChoiceError` mit `field: 'appId'`
  und bis zu 5 Kandidaten `{ value: <App-ID>, label: <Titel> }`.
- Der Runner bildet `ActionChoiceError` auf den neuen Code `needs_choice` ab. Rust maskiert ihn nicht.

**2. Rückfrage im Turn (Rust)**

- Erhält das `ActionTool` (`src-tauri/src/chat/tools/action_tool.rs`) `needs_choice`, parkt es auf
  einem oneshot wie die Tool-Freigabe (`pending_tool_approvals`) und sendet `choice-request` mit
  Frage, Optionen und Thread an den Chat.
- Antwort:
  - Option gewählt → dieselbe Aktion erneut mit `field = value`.
  - Freitext → dieselbe Aktion erneut mit dem Text als Feldwert; ist er wieder mehrdeutig, folgt eine
    neue Rückfrage.
  - Abbrechen → Tool-Ergebnis „vom Nutzer abgelehnt“.
- Das Modell sieht nur das Endergebnis (geöffnet oder abgelehnt).

**3. `ask_user` (Rust, eingebautes Tool)**

- Wie `find_actions` immer angeboten. Eingabe: Frage, 2–5 Optionen. Läuft über denselben
  `choice-request`-Weg.
- Ergebnis an das Modell: gewählte Option, Freitext oder „abgelehnt“.
- Systemprompt (`src-tauri/src/chat/tools/prompt.rs`): Kann der Agent eine Anweisung nicht umsetzen,
  fragt er mit `ask_user` und Alternativen nach, statt abzulehnen.

**4. UI**

- `src/components/chat/ChoicePrompt.vue` neben `PermissionPrompt.vue`: Frage, Radio-Group,
  „Etwas anderes …“ mit Textfeld, Bestätigen, Abbrechen. Beantwortet über ein Command analog zur
  Tool-Freigabe.

## Offen für den Plan

- Schwelle für „klarer Treffer“ (Fuse-Score und Abstand zum Zweitplatzierten).
- Verhalten, wenn der Chat-Tab der Rückfrage geschlossen wird oder die App neu startet (vermutlich wie
  eine offene Tool-Freigabe: der Turn bricht ab).
- Timeout: Die Action-Bridge wartet 60 s auf das Frontend; eine Rückfrage an den Nutzer darf daran
  nicht scheitern.
- Externe Agenten (MCP): bekommen `needs_choice` mit Kandidaten als Ergebnis, ohne Rückfrage-UI.
