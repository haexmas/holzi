# Quickstart: Mehrfachinstanzen für Apps

Validiert [spec.md](./spec.md) Ende-zu-Ende. Siehe [research.md](./research.md)
für die Begründung, warum der zugrunde liegende Mechanismus schon vor
dieser Spec bestand und getestet war.

## Voraussetzungen

- `nix develop --command scripts/with-nix-host-bridge.sh pnpm test:e2e`
  lauffähig (siehe `scripts/e2e/README.md`).

## Reine Logik (schnell, ohne gebaute App)

```sh
nix develop --command scripts/with-nix-host-bridge.sh node --test scripts/check-wm-state.ts
```

Erwartung: Die bestehenden T051/T052-Fälle (synthetische Mehrfachinstanz-App)
bleiben grün; die neuen Fälle mit der echten `WM_APPS`-Registry (`system.chat`)
zeigen dasselbe Verhalten wie die synthetische Test-App.

## Manuell / Ende-zu-Ende

1. Vault öffnen, Chat über den Launcher öffnen. **Erwartung**: ein Fenster
   mit einem Chat-Tab.
2. Im selben Fenster über „+" erneut Chat wählen. **Erwartung**: ein
   zweiter Tab mit einer neuen, leeren Unterhaltung entsteht; der erste Tab
   und seine Unterhaltung bleiben unverändert (spec.md User Story 1,
   Szenario 1). Die „+"-Liste zeigt bei Chat nie „bereits geöffnet".
3. Chat stattdessen über den Launcher erneut öffnen. **Erwartung**: ein
   neues Fenster mit einer weiteren neuen Unterhaltung, unabhängig vom
   bereits offenen Tab (Szenario 2).
4. In einer der beiden Chat-Instanzen eine Nachricht senden. **Erwartung**:
   die andere Instanz bleibt unverändert (Szenario 3).
5. Einstellungen öffnen, dann über den Launcher erneut öffnen. **Erwartung**:
   der vorhandene Einstellungs-Tab wird aktiviert und fokussiert, kein
   zweiter entsteht (User Story 2, unverändert gegenüber heute).
6. Bei eingeschalteter Einstellung „Sitzung wiederherstellen" (Spec 022):
   mit zwei offenen Chat-Instanzen die Vault schließen und erneut öffnen.
   **Erwartung**: beide Chat-Tabs kehren als getrennte, je leere
   Unterhaltungen zurück (Edge Case „Sitzungswiederherstellung").

## CI-Parität

`pnpm lint`, `pnpm typecheck`, `npx tsc --project tsconfig.scripts.json
--noEmit`, `pnpm format:check`, `pnpm check:wm-state` — plus das neue
e2e-Szenario `scripts/e2e/scenarios/chat-multi-instance.test.ts` über
`pnpm test:e2e`.
