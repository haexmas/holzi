# Contract: Dateiaktionen der Agents (044)

Definitionen im TS-Katalog `src/lib/actions/filesActions.ts`, neue Scopes `files.read` und
`files.write` in `scopes.ts`. Alle außer `files.show` tragen `runner: 'native'` und laufen in Rust über
`NativeActionTool` (research R1, ADR 0011). Der Aufrufer ist `Caller::BuiltinAgent` (Chat) bzw. später
`Caller::ExternalAgent { id }` (Spec 021). Werkzeugname = Id mit `_` statt `.`.

| Id                    | Effekt        | Stufe  | Eingabe                                                     | Ergebnis (Text, JSON)                                                                   |
| --------------------- | ------------- | ------ | ----------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `files.sources`       | `read`        | Safe   | –                                                           | Gerät (Laufwerke, bekannte Orte) und erreichbare Speicher                               |
| `files.list`          | `read`        | Safe   | `source`, `path`, `limit?` (Vorgabe 500)                    | Einträge, `truncated`                                                                   |
| `files.stat`          | `read`        | Safe   | `source`, `path`                                            | Eintrag                                                                                 |
| `files.search`        | `read`        | Safe   | `source`, `path`, `query`, Filter                           | Treffer bis 500 / 30 s, `truncated`                                                     |
| `files.read`          | `read`        | Safe   | `source`, `path`                                            | Text bis 200 000 Zeichen, `truncated`; Bild als Bildblock (R2); sonst Angaben + Hinweis |
| `files.folder.create` | `write`       | Change | `source`, `path`, `name`                                    | Eintrag                                                                                 |
| `files.copy`          | `write`       | Change | `source`, `paths[]`, `to`, `onConflict: 'skip'\|'keepBoth'` | Zusammenfassung (kopiert, übersprungen, Fehler)                                         |
| `files.rename`        | `write`       | Change | `source`, `path`, `newName`                                 | Eintrag                                                                                 |
| `files.move`          | `destructive` | Risky  | wie `files.copy`, zusätzlich `'replace'` erlaubt            | Zusammenfassung                                                                         |
| `files.delete`        | `destructive` | Risky  | `source`, `paths[]`                                         | Zusammenfassung; Desktop: Papierkorb                                                    |
| `files.show`          | `write`       | Change | `source`, `path`                                            | geöffnet (Frontend-Aktion, braucht ein Fenster)                                         |

`source` ist `"device"` oder `"storage:<id>"`.

## Regeln

- **Sperre** (FR-033): Jeder Pfad wird nach Auflösen von `..` und Links geprüft; in den eigenen Orten
  von holzi → Fehler `files_blocked` mit Grund, ohne Inhalt. Listen und Suchtreffer lassen diese
  Einträge weg.
- **Gerät** (FR-031): ohne Berechtigung `device` → `files_not_granted`.
- **Speicher** (FR-031a): ohne Berechtigung → Rückfrage über `files-agent-permission-request`; abgelehnt
  oder ohne Fenster → `files_not_granted`. Schreibende Aktionen verlangen `readWrite`, unabhängig von der
  Freigabestufe. Speicher ohne Berechtigung erscheinen in keinem Ergebnis, auch nicht in
  `files.sources`.
- **Ersetzen** bei `files.copy` gibt es nicht: Ein Agent überschreibt nur mit `files.move` und der Stufe
  Risky.
- **Agenten-Transfers** laufen ohne Rückfrage zum Namen; die Entscheidung steht in `onConflict`.
- **Grenzen** (FR-035): Eingabedatei für Textauszug bis 50 MB; Bilder auf lange Kante 1568 px und
  höchstens 5 MB verkleinert.
- **Ohne Fenster** (FR-034): alle Aktionen außer `files.show` arbeiten, sobald die Definitionen in
  dieser Vault-Sitzung übergeben wurden.
