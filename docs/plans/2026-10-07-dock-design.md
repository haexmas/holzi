# Dock: Design

Stand: 2026-10-07, abgestimmt im Brainstorming. Spec: [`specs/045-dock`](../../specs/045-dock/spec.md).

## Ausgangslage

Unten rechts in `src/components/wm/Desktop.vue` stehen drei runde Schaltflächen
(Arbeitsbereichs-Übersicht, Fensterübersicht, Launcher). Sie sind nicht konfigurierbar, und es gibt
keine Taskleiste. Spec 030 hat „Klick auf ein App-Symbol fokussiert die laufende Instanz“ als spätere
Spec ausgelagert. Apps sind bereits einheitlich: `allApps()` (`src/lib/extensions/apps.ts`) liefert
System-Apps und haextensions als `AppDefinition`, geöffnet wird alles über `wm.app.open({ appId })`.

## Entscheidungen

| Frage                           | Entscheidung                                                                                                                                                             |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Umfang                          | Volle Taskleiste: angeheftete und laufende Apps, Instanz-Zähler, Attention                                                                                               |
| Laufende Apps                   | Aus allen Arbeitsbereichen; ein Klick wechselt in den Arbeitsbereich der Instanz                                                                                         |
| Speicherort                     | Platzierung pro Gerät, Einträge in der Vault (synchronisiert)                                                                                                            |
| Platzierung                     | Kante (oben/unten/links/rechts) × Ausrichtung (Anfang/Mitte/Ende) = 12 Positionen                                                                                        |
| Stil                            | Leiste oder Rad (FAB, der als Viertelkreis in der Ecke, als Halbkreis an der Kante auffächert)                                                                           |
| Verhältnis zu Fenstern (Leiste) | Einstellbar: reserviert Platz, schwebt, blendet automatisch aus                                                                                                          |
| Rad und Fenster                 | Das Rad schwebt immer                                                                                                                                                    |
| Steuer-Einträge                 | Arbeitsbereiche, Fenster, Launcher sind normale Einträge, frei sortier- und entfernbar; nur der Launcher ist nicht entfernbar                                            |
| Bedienung                       | Kontextmenü (Rechtsklick/Langdruck) im Dock und im Launcher, Ziehen zum Sortieren in der Leiste, Settings-Sektion „Dock“ mit sortierbarer Liste                          |
| Kompaktmodus                    | Leiste: immer unten, mittig, reserviert. Rad: bleibt Rad, untere Ecke                                                                                                    |
| Klick                           | 0 Instanzen: öffnen. 1: Arbeitsbereich wechseln, wiederherstellen, fokussieren. n: Popover nach Arbeitsbereich gruppiert plus „Neues Fenster“. Mittelklick: neue Instanz |

## Architektur

**Daten**

- Vault-Pref `dock.items`: geordnete Liste aus
  `{ kind: 'control', id: 'launcher' | 'workspaces' | 'windows' }` und `{ kind: 'app', appId }`.
  Ein JSON-Wert, Last-Writer-Wins bei gleichzeitigem Umsortieren auf zwei Geräten (akzeptiert;
  eine eigene CRDT-Tabelle pro Eintrag wäre für ~10 Einträge Overkill).
- Device-Pref `dock.placement`: `{ style: 'bar' | 'wheel', edge, align, mode }`; `mode`
  (`reserved | floating | autohide`) gilt nur für `bar`.
- Laufende, nicht angeheftete Apps sind Laufzeit-Zustand, abgeleitet aus `wm.windows`, nie gespeichert.

**Module**

- `src/lib/wm/dock.ts` (pure): Typen, Default-Liste, Normalisierung, `resolveDockEntries(items, apps,
windows)`, effektive Platzierung, Rad-Geometrie (Winkelbereich, Aufteilung auf Ringe),
  Aktivierungsentscheidung (öffnen / fokussieren / Popover).
- `useDockStore`: lädt beide Prefs über `usePreferences`, hört auf
  `onVaultTablesChanged(['preferences'])`, bietet `pin`, `unpin`, `move`, `setPlacement`.
- `WmDock.vue` wählt `WmDockBar.vue` oder `WmDockWheel.vue`; beide rendern über `WmDockItem.vue`
  (Symbol, Instanz-Badge, Laufend-Punkt, Attention).
- Aktivierung über die bestehenden Actions `wm.app.open`, `wm.tab.activate`, `wm.tab.close` (Plan: keine neue Action, research R4).

**Geometrie-Falle**

Heute speist `useWindowSize()` (Viewport) sowohl `wm.area` als auch `wm.compact`. Reserviert die
Leiste Platz, muss `wm.area` die tatsächliche Fensterfläche messen (`useElementSize`), `wm.compact`
aber weiter den Viewport. Sonst schwingt es: Leiste links → Fläche schmal → kompakt → Leiste unten →
Fläche breit → nicht kompakt → Leiste links.

## Fehlerfälle

- Pref fehlt oder ist ungültig: Default-Liste; erst bei der nächsten Nutzeränderung schreiben.
- Unbekannte `appId` (Erweiterung deinstalliert/deaktiviert, `dev`-App im Release): ausblenden, in der
  Pref behalten; in der Settings-Liste grau als „nicht verfügbar“, dort entfernbar.
- Duplikate: erster gewinnt; angeheftet und laufend erscheint einmal an der Pin-Position.
- Launcher fehlt: vorne einfügen.
- Legacy-Alias `system.federation` → `system.settings`.
- Wechsel kompakt ↔ normal: effektive Platzierung neu berechnen, offenes Rad/Popover schließen.

## Tests

- `scripts/check-wm-dock.ts` (in `check:wm-state`): Normalisierung, `resolveDockEntries`, effektive
  Platzierung, Rad-Geometrie, Aktivierungsentscheidung.
- `check-settings.ts`: Registry-Sektion „Dock“.
- E2E: anheften → Klick öffnet → erneuter Klick fokussiert; Stil auf Rad, auffächern, aktivieren.
  `data-testid="open-launcher"` bleibt am Launcher-Eintrag.
