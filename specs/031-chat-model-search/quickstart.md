# Quickstart: Modellsuche im Chat

Validiert [spec.md](./spec.md) Ende-zu-Ende, mit Verweisen auf
[data-model.md](./data-model.md) und [research.md](./research.md) statt
Details zu duplizieren.

## Voraussetzungen

- Mindestens zwei Anbieter mit zusammen mindestens zehn installierten bzw.
  verbundenen Modellen (für einen aussagekräftigen Test von User Story 1).
  Der Stand-in-Provider der e2e-Suite (`scripts/e2e/lib/provider.ts`) reicht
  für die automatisierten Szenarien; für den manuellen Test genügt auch eine
  kleinere, von Hand zusammengestellte Liste.
- `nix develop --command scripts/with-nix-host-bridge.sh pnpm test:e2e`
  lauffähig (siehe `scripts/e2e/README.md`).

## Reine Logik (schnell, ohne gebaute App)

```sh
nix develop --command scripts/with-nix-host-bridge.sh node --test scripts/check-chat-model-search.ts
```

Erwartung: `filterModelGroups` liefert für die in R1/R2 der Spec genannten
Beispiele (nicht-zusammenhängender Treffer, ein Tippfehler, kein Treffer,
Gruppierung bleibt erhalten) die erwarteten Ergebnisse — siehe
[data-model.md](./data-model.md) für die genaue Signatur.

## Manuell / Ende-zu-Ende

1. Chat öffnen, mit mehreren installierten/verbundenen Modellen über
   mindestens zwei Anbieter (Settings → Modelle, oder Stand-in-Provider im
   e2e-Rig).
2. Die Modell-Auswahl im Composer öffnen (Zahnrad-/Einstellungen-Popover).
   **Erwartung**: Ein Suchfeld ist sofort sichtbar, alle Modelle in ihren
   Anbieter-Gruppen sind aufgelistet (Spec FR-001).
3. Einen Teil eines Modellnamens tippen, der nicht am Anfang steht oder
   nicht zusammenhängend im Namen vorkommt (z. B. „4o" bei „GPT-4o", oder
   „gpt4o" ganz ohne Bindestrich). **Erwartung**: Nur passende Modelle
   bleiben sichtbar, Anbieter-Gruppen ohne Treffer verschwinden (FR-002–004).
4. Einen Suchbegriff mit einem bewussten Tippfehler eingeben (z. B. einen
   Buchstaben des Modellnamens vertauschen). **Erwartung**: Das gemeinte
   Modell bleibt Treffer (FR-003, SC-002).
5. Einen Suchbegriff eingeben, zu dem kein Modell passt (z. B.
   „zzzzzzzz"). **Erwartung**: Ein kurzer Hinweis statt einer leeren Fläche
   (FR-005).
6. Suchbegriff löschen. **Erwartung**: Alle Modelle erscheinen wieder in der
   ursprünglichen Gruppierung und Reihenfolge (FR-004, FR-006).
7. Einen Suchbegriff eingeben, mit den Pfeiltasten einen Treffer markieren
   und mit Eingabe übernehmen. **Erwartung**: Das markierte Modell wird
   aktives Modell, die Auswahl schließt (FR-007, User Story 2).
8. Auswahl mit Suchbegriff erneut öffnen. **Erwartung**: Suchfeld ist leer,
   alle Modelle sichtbar (FR-006).
9. Ein Suchbegriff, der das aktuell gewählte Modell herausfiltert; Auswahl
   mit Escape schließen, ohne ein anderes Modell zu wählen. **Erwartung**:
   Das zuvor gewählte Modell bleibt aktiv (FR-008).

## CI-Parität

`pnpm lint`, `pnpm typecheck`, `npx tsc --project tsconfig.scripts.json
--noEmit`, `pnpm format:check`, `pnpm check:chat-model-search` (neu, siehe
oben) — plus das neue e2e-Szenario
`scripts/e2e/scenarios/chat-model-search.test.ts` über `pnpm test:e2e`.
