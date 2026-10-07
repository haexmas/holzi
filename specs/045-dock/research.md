# Research: Dock

Alle Pfadangaben beziehen sich auf den Stand von `main` @ `927b4849`.

## R1 — Fläche und Kompaktmodus entkoppeln

**Decision**: `wm.area` misst die tatsächliche Fensterfläche (das Element, in dem die Fenster liegen,
per `useElementSize`), `wm.compact` richtet sich weiter nach der Breite des App-Fensters
(`useWindowSize`). Dafür bekommt `updateArea(state, area, apps)` (`src/lib/wm/layoutState.ts:314`)
einen vierten Parameter `viewportWidth` und setzt `state.compact = viewportWidth <= COMPACT_MAX_WIDTH`.
`hydrate(layout, apps, area)` (`layoutState.ts:294`) bekommt einen optionalen Parameter `compact`
(Default wie heute aus `area.width`); `sessionSync.replaceState` (`src/lib/wm/sessionSync.ts:72`) gibt
`state.compact` mit, damit eine wiederhergestellte Sitzung den Modus nicht aus der verkleinerten Fläche
ableitet. Der Watcher in `Desktop.vue:22-26` beobachtet beide Größen und ruft `wm.updateArea(area,
viewportWidth)`.

**Rationale**: FR-033. Mit nur einer Größe schwingt das Dock (Leiste links → Fläche schmal → kompakt →
Leiste unten → Fläche breit → nicht kompakt). Alle Verbraucher von `wm.area` (`Window.vue:87,101`,
`wmLayoutHandlers.ts:88`, `sessionSync.ts:72`) wollen ohnehin die Fläche, in die Fenster passen; alle
Verbraucher von `wm.compact` (`Window.vue`, Overviews, `Launcher.vue:37`, `tabNavigation.ts:148`)
wollen die Gerätebreite. Der Kommentar in `types.ts:62` („derived from `area.width`“) wird angepasst.

**Alternatives considered**: Fläche = Viewport minus berechnete Dock-Größe — verworfen, doppelte
Geometrie-Wahrheit neben dem CSS-Layout. Kompakt-Schwelle mit Hysterese — verworfen, verschiebt das
Problem nur.

## R2 — Speichern: zwei Präferenzen, ein Composable

**Decision**: Vault-Präferenz `dock.items` (JSON-Liste) und Geräte-Präferenz `dock.placement` (JSON-Objekt)
über `usePreferences` (`src/composables/usePreferences.ts:55-70`; Werte sind Strings, JSON wie
`useAppearance.ts:91`). Neues Composable `src/composables/useDock.ts` nach dem Muster
`useWorkspaceBackground.ts`: modulweite `ref`s (ein Prozess, eine Vault), `loadAsync`/`refreshAsync`,
eingehängt in `pages/workspace/[instance].vue:37-46` (Refresh) und `:96-106` (Load). Die Geräte-UUID kommt
aus `useDevice().currentDeviceInfoAsync()` wie in `settingsActionHandlers.ts:53-56`.
Schreibvorgänge serialisiert eine einfache Promise-Kette; die zuletzt angestoßene Änderung gewinnt.

**Rationale**: Laziness-Ladder, vorhandene Infrastruktur. Die `preferences`-Tabelle synchronisiert den
Vault-Scope bereits per CRDT und hält den Geräte-Scope lokal (FR-035, FR-036).

**Alternatives considered**: Pinia-Store — verworfen, die übrigen Präferenz-Zustände sind Composables mit
Modul-`ref`s. Eigene CRDT-Tabelle pro Eintrag — verworfen (Design, ~10 Einträge, Last-Writer-Wins
akzeptiert).

## R3 — Reine Logik in `src/lib/wm/dock.ts`

**Decision**: Ohne Vue, ohne Nuxt-Auto-Imports, relative `.ts`-Imports (Regel `src/lib/wm/types.ts:1-5`),
damit `node --test` sie direkt lädt:

- `parseDockItems(raw: string | null): DockItem[] | null` und `parseDockPlacement(raw): DockPlacement`
  (Validierung an der Vertrauensgrenze, `null` = unlesbar → Default, FR-038).
- `normalizeDockItems(items, apps)`: Alias auflösen (`resolveAppAlias`, `src/lib/wm/apps.ts:76`),
  Duplikate entfernen, fehlenden Launcher vorn einfügen, unbekannte Apps als `available: false` markieren
  statt löschen (FR-037, FR-039).
- `resolveDockEntries(items, apps, windows, workspaces)`: sichtbare Einträge plus laufende nicht
  angeheftete Apps, jeweils mit Instanzen (`{ tabId, windowId, workspaceId }`) (FR-005, FR-007).
- `effectivePlacement(placement, compact)` (FR-031, FR-032).
- `dockActivation(entry)`: `'open' | { focusTab } | 'choose'` (FR-010–FR-012).
- `wheelLayout(count, anchor)`: Winkelbereich und Ringe (R6).

**Rationale**: Testbar ohne DOM (spaex: Seiteneffekte an sichtbaren Grenzen). Geschätzt ~250 Zeilen,
unter der 500er-Grenze.

## R4 — Keine neuen Actions

**Decision**: Das Dock ruft vorhandene Actions über `useAction`:
`wm.app.open` (öffnen, FR-010, FR-013), `wm.tab.activate` (`wmLayoutHandlers.ts:45`: `switchTab` +
`focusWindow`, das laut `layoutState.ts:53` den Arbeitsbereich wechselt und die Minimierung aufhebt;
FR-011), `wm.tab.close` (`requestCloseTab` mit Rückfragen; FR-016, je Instanz), `wm.launcher.open`,
`wm.windows.overview`, `wm.workspaces.overview` (FR-014). Anheften ist eine Präferenzänderung im
Composable, keine Action.

**Rationale**: Abweichung vom Entwurf (dort `wm.dock.activate`): Alle Wirkungen existieren schon als
Actions; eine neue Action würde nur den Snapshot `src-tauri/src/chat/eval/tools.json`
(`check-agent-actions.ts:334`) und den Katalog vergrößern. Agenten brauchen laut Spec kein Anheften.

**Alternatives considered**: `wm.dock.activate` / `wm.dock.pin` — verworfen (YAGNI).

## R5 — Instanz statt Fenster

**Decision**: Eine Instanz ist ein Tab (`WmTab.appId`, `src/lib/wm/types.ts:28`); ein Fenster kann Tabs
verschiedener Apps halten. Zähler, Auswahlfeld und „Alle schließen“ arbeiten auf Tabs. Titel und Symbol
kommen aus `tabDisplayInfo` (`windowManager.ts:199`), übersetzt mit demselben Ausdruck wie in
`TabBar.vue:50`; Arbeitsbereiche heißen wie in der Arbeitsbereichs-Übersicht (`wm.workspaces.numbered`).
Aufmerksamkeit: `appHasAttention` (`windowManager.ts:408`).

**Rationale**: Die Spec sprach zunächst von „Fenstern einer App“; beim Planen korrigiert (Spec-Begriff
„Instanz“, Abschnitt Begriffe). „Alle schließen“ über Fenster hätte Tabs anderer Apps mitgeschlossen.

**Note (graphify-first)**: Der Übersetzungsausdruck `titleOverride ?? t(titleKey, titleParams)` steht
schon an sieben Stellen. Ein gemeinsamer Helper wäre eine eigene Aufräumänderung; diese Spec fügt eine
achte Stelle hinzu und schlägt die Extraktion separat vor.

## R6 — Rad-Geometrie

**Decision**: Ausrichtung „Anfang“/„Ende“ an einer Kante ist eine Ecke (z. B. `bottom/end` = unten
rechts, `right/end` = ebenfalls unten rechts); „Mitte“ ist die Kantenmitte. Ecke → Bogen 90°, Kante →
180°, jeweils zur Bildschirmmitte hin geöffnet. Ring `k` hat Radius `r0 + k·gap`; Kapazität
`max(1, floor(bogenlänge / abstand) + 1)` bei Eintragsgröße 48 px und Mindestabstand 56 px. Einträge
füllen innen nach außen. `wheelLayout` liefert Offsets `{ x, y }` relativ zum FAB; die Komponente
positioniert absolut und animiert per CSS-Transition (`transform`), respektiert
`prefers-reduced-motion`.

**Rationale**: FR-027, FR-028; reine Funktion, testbar auf Überlappung (Abstand je Paar ≥ 48 px).

**Alternatives considered**: Bibliothek für Radialmenüs — verworfen (keine neue Abhängigkeit).

## R7 — Kontextmenü und Langdruck

**Decision**: `ShadcnContextMenu` (haex-ui, reka) um jeden Dock-Eintrag, um jede Launcher-Kachel und um
die freie Dock-Fläche; reka öffnet es bei Rechtsklick und bei Langdruck auf Touch (FR-015, FR-017,
FR-018, FR-043). Vorbild: `src/components/passwords/EntryMenu.vue:30-53`. Mittelklick über `@auxclick`
mit `button === 1`.

**Rationale**: Kandidat `usePasswordsRowPress` (`src/composables/usePasswordsRowPress.ts`) geprüft: Er
unterdrückt gerade das Kontextmenü bei Langdruck (Langdruck = Auswahl). Gegenteiliges Verhalten, daher
nicht wiederverwendet.

## R8 — Sortieren

**Decision**: Leiste: natives HTML5-Drag-and-Drop wie in `passwords/List.vue`/`TreeItem.vue` (Maus).
Einstellungen „Dock“: Liste mit „Nach oben“/„Nach unten“, „Entfernen“, „Hinzufügen“ (Picker aus
`allApps()` plus entfernten Steuer-Einträgen). Auf Touch sortiert man in den Einstellungen.

**Rationale**: FR-019 verlangt Ziehen in der Leiste, nicht auf Touch; auf Touch belegt der Langdruck das
Kontextmenü. Keine neue Bibliothek.

**Alternatives considered**: Pointer-Events-Sortierung (auch Touch) — verworfen, Konflikt mit Langdruck
und mehr eigener Code.

## R9 — Automatisch ausblenden

**Decision**: Ein 4 px breiter, unsichtbarer Streifen an der Dock-Kante (`pointerenter`) blendet die
Leiste ein; `pointerleave` der Leiste startet 400 ms bis zum Ausblenden. Sichtbar bleibt sie, solange
`:focus-within`, ein Auswahlfeld oder ein Kontextmenü des Docks offen ist (FR-025). Im Modus „Schweben“
und „Automatisch ausblenden“ liegt die Leiste absolut über der Fensterfläche, im Modus „Platz
reservieren“ ist sie ein Flex-Geschwister der Fensterfläche (R1 misst dann automatisch die Restfläche).

## R10 — Einstellungen

**Decision**: `subView('general.dock', 'general/dock', 'general', { icon: 'lucide:panel-bottom',
overviewRow: true, settingKeys: [...] })` in `src/lib/settings/registry.ts` nach `general.appearance`;
Komponente `src/components/settings/DockView.vue` in `SETTINGS_VIEWS` (`src/components/wm/appRoutes.ts:34-46`).
Texte unter `settings.locations.general.dock.*` und `settings.dock.*` in `de.json`/`en.json`;
`check-settings.ts:192` prüft sie.

**Rationale**: Spec 042 hat „Allgemein“ als Übersicht angelegt; das Dock gehört zur Bedienung des
Arbeitsbereichs, nicht zum Erscheinungsbild allein (Einträge sind Inhalte).

## R11 — Tests

**Decision**:

- `scripts/check-wm-dock.ts` (neu, in `check:wm-state`): Parsen, Normalisierung, Einträge auflösen,
  effektive Platzierung, Aktivierung, Rad-Geometrie.
- `scripts/check-wm-geometry.ts` bzw. `check-wm-state.ts`: `updateArea` mit `viewportWidth` (Fläche
  schmal, Viewport breit → nicht kompakt) und `hydrate` mit übergebenem `compact`.
- `scripts/check-settings.ts`: läuft unverändert, deckt den neuen Unterpunkt über die Registry ab.
- E2E `scripts/e2e/scenarios/dock.test.ts`: anheften, öffnen, erneut klicken fokussiert, Stil Rad.
  `data-testid="open-launcher"` bleibt am Launcher-Eintrag (≈31 Szenarien hängen daran).

## Graphify-Abfrage

`graphify query` gegen `graphify-out/graph.json` (Hauptcheckout) mit „dock taskbar pinned apps“, „long
press context menu row“, „window display title helper“, „device scoped preference load refresh“, „radial
fan menu arc“: Der Graph enthält fast nur Rust- und Spec-Knoten, die Treffer waren fachfremd. Die
Kandidaten oben (`usePasswordsRowPress`, `tabDisplayInfo`, `useWorkspaceBackground`, `wm.tab.activate`)
stammen aus gezielter Code-Suche. Nachprüfung mit einem aktualisierten Graphen steht aus.
