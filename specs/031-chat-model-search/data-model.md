# Data Model: Modellsuche im Chat

Keine neue persistente Entität und kein neuer Datenbestand (siehe
Assumptions in [spec.md](./spec.md)). Diese Spec fügt nur eine reine,
abgeleitete Sicht auf bereits vorhandene Laufzeit-Daten hinzu.

## Bestehend, unverändert

- **ModelGroup** (`src/components/chat/ComposerSettingsPopover.vue`):
  `{ providerId: string; providerName: string; models: { id: string; name:
string }[] }`. Kommt weiterhin unverändert als Prop in die Komponente.

## Neu, abgeleitet (nicht gespeichert)

- **Suchbegriff** (`query: string`, Komponenten-lokaler `ref`): der aktuelle
  Inhalt des neuen Suchfelds. Beginnt bei jedem Öffnen der Auswahl leer
  (FR-006).
- **Gefilterte Anzeige** (`computed<ModelGroup[]>`): Ergebnis von
  `filterModelGroups(groups: ModelGroup[], query: string): ModelGroup[]`
  (neu, `src/lib/chat/modelSearch.ts`) — dieselbe Form wie `ModelGroup[]`,
  nur eingeschränkt auf Treffer; eine Gruppe ohne Treffer entfällt
  vollständig (FR-004). Bei leerem Suchbegriff identisch zur Eingabe
  (Identitätsfall, keine Filterung).

### `filterModelGroups`

- **Eingabe**: die vollständige `ModelGroup[]`-Liste, der aktuelle
  Suchbegriff.
- **Ausgabe**: `ModelGroup[]`, gleiche Struktur, nur gefilterte `models`;
  Gruppen mit leerem `models`-Ergebnis werden weggelassen. Die Reihenfolge
  der verbleibenden Modelle innerhalb einer Gruppe entspricht der
  Eingabe-Reihenfolge (FR-004 — keine Neusortierung nach Trefferqualität
  über Anbieter hinweg).
- **Abgleich**: `fuse.js` über `providerName` und `models[].name` je Eintrag
  (siehe [research.md](./research.md) R1); ein Eintrag gilt als Treffer,
  wenn sein Modellname **oder** der Anbietername der Gruppe zum Suchbegriff
  passt (FR-002).
- **Reinheit**: keine Seiteneffekte, kein Vue-Import — mit `node --test`
  prüfbar wie `src/lib/settings/search.ts` (siehe
  `scripts/check-chat-model-search.ts` in [plan.md](./plan.md)).
