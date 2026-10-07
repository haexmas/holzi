# Contract: Dock

## Präferenzen

| Schlüssel        | Scope | Wert                 | Schreiber                               |
| ---------------- | ----- | -------------------- | --------------------------------------- |
| `dock.items`     | Vault | JSON `DockItem[]`    | `useDock` (Dock, Launcher, Einstellung) |
| `dock.placement` | Gerät | JSON `DockPlacement` | `useDock` (Einstellung, Dock-Menü)      |

Typen: [data-model.md](../data-model.md). Andere Geräte und ältere Stände dürfen beliebigen Inhalt
schreiben; Leser normalisieren (FR-038).

## `useDock()` (`src/composables/useDock.ts`)

```ts
function useDock(): {
  items: Readonly<Ref<readonly DockItemState[]>> // normalisiert, inkl. available-Flag
  placement: Readonly<Ref<DockPlacement>>
  loadAsync(): Promise<void>
  refreshAsync(): Promise<void>
  pinAsync(appId: string): Promise<void> // ans Ende; schon angeheftet → nichts
  unpinAsync(appId: string): Promise<void>
  addControlAsync(id: DockControlId): Promise<void>
  removeAsync(index: number): Promise<void> // Launcher → Fehler, UI bietet es nicht an
  moveAsync(from: number, to: number): Promise<void>
  setPlacementAsync(patch: Partial<DockPlacement>): Promise<void>
}
```

## Actions (bestehend, unverändert)

| Dock-Handlung              | Action                   | Eingabe          |
| -------------------------- | ------------------------ | ---------------- |
| App öffnen / neues Fenster | `wm.app.open`            | `{ appId }`      |
| Instanz fokussieren        | `wm.tab.activate`        | Ziel `{ tabId }` |
| Instanz schließen (je Tab) | `wm.tab.close`           | Ziel `{ tabId }` |
| Launcher                   | `wm.launcher.open`       | —                |
| Fensterübersicht           | `wm.windows.overview`    | —                |
| Arbeitsbereichs-Übersicht  | `wm.workspaces.overview` | —                |

## Window Manager (geänderte Signaturen)

```ts
// src/lib/wm/layoutState.ts
function updateArea(
  state: WmState,
  area: Size,
  apps: readonly AppDefinition[],
  viewportWidth: number,
): string[]
function hydrate(layout, apps, area: Size, compact?: boolean): WmState

// src/stores/windowManager.ts
function updateArea(area: Size, viewportWidth: number): void
```

## UI-Testkennungen (E2E)

| `data-testid`       | Element                                                            |
| ------------------- | ------------------------------------------------------------------ |
| `open-launcher`     | Launcher-Eintrag (bleibt, ≈31 Szenarien)                           |
| `dock`              | Leiste bzw. Rad-Container, `data-style`, `data-edge`, `data-align` |
| `dock-item-<appId>` | App-Eintrag, `data-running`, `data-count`                          |
| `dock-wheel-toggle` | runde Schaltfläche des Rads                                        |
| `dock-instances`    | Auswahlfeld mit Instanzen                                          |
