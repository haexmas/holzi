# Data Model: Kompakter Passwortverlauf

Diese Änderung führt keine neuen Datenmodelle ein und verändert keine gespeicherten Daten.

## Bestehende Projektionen

- `SnapshotHeader[]`: bestehende Liste der Stände mit `id`, `modifiedAt` und den geänderten
  Feldern. Sie wird ausschließlich in Dropdown-Optionen projiziert.
- `SnapshotView`: bestehender, geladener Stand. Er bleibt die Datenquelle der unterhalb des
  Dropdowns angezeigten Snapshot-Ansicht.

## UI-Zustand

- `selectedId`: bestehende Auswahl-ID; bleibt der Identifikator für den Snapshot-Ladevorgang.
- `selected`: bestehende geladene Snapshot-Projektion; keine Persistenz und keine neue
  Geheimnisablage.

Die Reihenfolge der bestehenden `SnapshotHeader[]` (neuester Stand zuerst) bleibt erhalten.
