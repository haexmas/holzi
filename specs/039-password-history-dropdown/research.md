# Research: Kompakter Passwortverlauf

## Bestehende Oberfläche

- `src/components/passwords/HistoryTab.vue` lädt die Header und den ausgewählten Snapshot.
  Das Grid `@2xl:grid-cols-[15rem_minmax(0,1fr)]` erzeugt die bisherige zweispaltige Ansicht.
- `src/components/passwords/HistoryTimeline.vue` enthält bereits die zeitliche Sortierung,
  Zeitformatierung, Auswahl-Emission und Tastaturbedienung.
- `src/components/passwords/HistorySnapshot.vue` enthält die unveränderte Snapshot-Ansicht
  und den Wiederherstellungsbutton. Nur die Typografie des Zeittexts muss angepasst werden.
- `src/components/settings/Select.vue` und die globale `UiSelect`-Komponente sind im Projekt
  bereits etablierte Dropdown-Primitiven und unterstützen Beschriftung, Tastatur und
  `data-testid`-Attribute.

## Entscheidung

Den vorhandenen `HistoryTimeline`-Baustein direkt auf `UiSelect` umstellen. Dadurch bleiben
die Daten- und Ereignisverträge stabil, und es entsteht keine zweite, parallele Auswahl-Logik.
Die Snapshot-Komponente bleibt fachlich unverändert.

## Verworfen

- Ein eigener neuer History-Dropdown-Baustein: unnötige Duplizierung vorhandener
  Auswahl-/Formatierungslogik.
- Eine Änderung an Snapshot-Daten: die Verlaufsänderung ist rein präsentational. (Der
  app-scoped Befehl `active_instance_name` gehört zu US3/FR-009, nicht zum Verlauf.)
