# Data Model: Chat-Historie verwalten

## Persistierte Entitäten

### ChatThread / Verlaufseintrag

Der bestehende Thread bleibt die persistierte Unterhaltung und wird im Verlauf
als Eintrag dargestellt.

| Feld                                | Bedeutung                                           | Änderbarkeit in diesem Feature           |
| ----------------------------------- | --------------------------------------------------- | ---------------------------------------- |
| `id`                                | Stabile Identität des Threads                       | unverändert                              |
| `title`                             | Nutzerlesbarer Titel                                | über Umbenennen änderbar                 |
| `created_at`                        | Eröffnungszeitpunkt als Unix-Epoch in Millisekunden | unverändert; Basis der Duration          |
| `updated_at`                        | Bestehende Änderungszeit für Verlaufssortierung     | folgt der bestehenden Persistenzsemantik |
| `last_provider_id`, `last_model_id` | Bestehende technische Metadaten                     | unverändert durch Historienverwaltung    |

### ChatMessage

Jede Nachricht gehört über `thread_id` zu einem Thread. Eine bestätigte
Thread-Löschung entfernt den Thread und seine zugehörigen Nachrichten als eine
fachliche Aktion. Einzelne Nachrichten werden durch dieses Feature nicht
bearbeitet oder gelöscht.

## Flüchtige UI-Projektionen

### HistoryDuration

```typescript
type HistoryDuration = {
  value: number
  unit: 'min' | 'h' | 'd'
}
```

Berechnung aus dem aktuellen Zeitpunkt und `created_at` in Unix-Epoch-
Millisekunden:

| Vergangene Zeit                 | Einheit | Beispiel                |
| ------------------------------- | ------- | ----------------------- |
| `0` bis `< 60` Minuten          | `min`   | `0min`, `1min`, `59min` |
| `60` Minuten bis `< 24` Stunden | `h`     | `1h`, `2h`, `23h`       |
| `>= 24` Stunden                 | `d`     | `1d`, `5d`              |

Die Zahl wird immer auf die volle Einheit abgerundet. Ein negativer Abstand
wird als `0min` dargestellt. Ein fehlender, ungültiger oder sonst unbrauchbarer
Zeitstempel wird ebenfalls als `{ value: 0, unit: 'min' }` dargestellt. Die UI
aktualisiert die Projektion spätestens an der nächsten relevanten
Einheiten-Grenze. `min`, `h` und `d` sind feste kompakte
Darstellungseinheiten; ergänzende Screenreader-Informationen werden
lokalisiert.

### HistoryEditState

```typescript
type HistoryEditState = {
  threadId: string
  draftTitle: string
  error: string | null
}
```

Es gibt höchstens einen aktiven Editierzustand. Der gespeicherte Titel bleibt
bis zum erfolgreichen Speichern unverändert. `Escape` verwirft den Entwurf;
ein leerer bereinigter Titel oder mehr als 120 sichtbare Zeichen wird nicht
gespeichert.

### HistoryDeleteState

```typescript
type HistoryDeleteState = {
  threadId: string
  title: string
}
```

Der Zustand hält den konkreten Löschkandidaten bis zur Bestätigung oder zum
Abbruch. Nach erfolgreicher Löschung wird der Eintrag aus der Projektion
entfernt. Bei Löschung des aktiven Threads wird `activeThreadId` auf `null`
gesetzt und der neue Chat-Entwurf aus Spec 004 aktiviert.

Wenn für den Kandidaten ein laufender Turn oder ein offener Tool-Approval
existiert, durchläuft der Zustand nach der Bestätigung zuerst
`cancelling → terminal`. Nur nach einem erfolgreichen terminalen Zustand darf
er in `deleting → deleted` wechseln. Bei einem Abbruchfehler endet er in
`cancel-failed`; der Thread bleibt unverändert.

## Invarianten

- Die Duration wird niemals persistiert und darf keine Thread-Daten verändern.
- Eine Titeländerung verändert weder `created_at` noch Nachrichten.
- Ein abgebrochener oder fehlgeschlagener Löschvorgang verändert keine
  sichtbare oder persistierte Thread-/Nachrichtenbeziehung.
- Ein erfolgreicher Löschvorgang hinterlässt keine Nachricht mit der
  gelöschten `thread_id`.
- Ein gelöschter aktiver Thread wählt nicht automatisch einen anderen Thread.
- Ein Thread wird nie gelöscht, solange sein laufender Turn nicht erfolgreich
  in einem terminalen Zustand beendet wurde.
