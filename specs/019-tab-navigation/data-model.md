# Data Model: Navigation im Tab

**Spec**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

Alle Strukturen sind reine Frontend-Daten im Speicher. Es gibt **keine** neuen
Tabellen, Migrationen, DTOs oder Tauri-Commands (research R12).

## TabLocation (Ort)

| Feld    | Typ                      | Regeln                                                                                               |
| ------- | ------------------------ | ---------------------------------------------------------------------------------------------------- |
| `path`  | `string`                 | app-relativ, beginnt mit `/`, ohne abschließenden `/` außer beim Start-Ort `/`; Segmente URL-kodiert |
| `query` | `Record<string, string>` | optional, Standard `{}`; nur Darstellungszustand der Ansicht (Filter, Suche, Sortierung)             |

- **Gleichheit** (FR-006): normierter `path` gleich und `query` mit gleichen
  Schlüssel-Wert-Paaren, unabhängig von der Reihenfolge.
- **Serialisierbarkeit** (FR-001): nur Strings; `JSON.parse(JSON.stringify(x))`
  ergibt einen gleichen Ort.

## HistoryEntry

| Feld       | Typ              | Regeln                                                                                                               |
| ---------- | ---------------- | -------------------------------------------------------------------------------------------------------------------- |
| `location` | `TabLocation`    |                                                                                                                      |
| `title`    | `string \| null` | angezeigter Titel beim Verlassen des Eintrags (R9); `null`, solange der Eintrag aktuell ist oder nie verlassen wurde |

## TabHistory (Tab-Historie)

| Feld      | Typ              | Regeln                                 |
| --------- | ---------------- | -------------------------------------- |
| `entries` | `HistoryEntry[]` | 1 ≤ Länge ≤ 50 (`MAX_HISTORY_ENTRIES`) |
| `index`   | `number`         | 0 ≤ `index` < `entries.length`         |

Abgeleitet: `current = entries[index]`, `canGoBack = index > 0`,
`canGoForward = index < entries.length - 1`, Verlaufsliste zurück =
`entries[index-1 … max(0, index-15)]`, vor = `entries[index+1 … index+15]`.

Ablage: `TabRuntime.history` im Shell-Store (research R2), nie persistiert.

### Zustandsübergänge

| Operation                    | Vorbedingung                            | Wirkung                                                                                                                                                |
| ---------------------------- | --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `createHistory(loc = '/')`   | –                                       | `entries = [{loc, null}]`, `index = 0`                                                                                                                 |
| `push(h, loc, leavingTitle)` | `loc ≠ current`                         | Einträge nach `index` entfernen; `current.title = leavingTitle`; `{loc, null}` anhängen; `index++`; bei Länge > 50 ersten Eintrag entfernen, `index--` |
| `push(h, loc, …)`            | `loc = current`                         | keine Änderung (FR-006)                                                                                                                                |
| `replace(h, loc)`            | –                                       | `entries[index].location = loc`                                                                                                                        |
| `go(h, delta, leavingTitle)` | `0 ≤ index+delta < length`, `delta ≠ 0` | `current.title = leavingTitle`; `index += delta`; Zieleintrag `title = null`                                                                           |
| `go(h, delta, …)`            | außerhalb                               | keine Änderung (FR-007)                                                                                                                                |
| `removeEntry(h, i)`          | `length > 1`                            | Eintrag `i` entfernen; `index` nachführen (`i < index` → `index--`; `i = index` → Nachbar in Laufrichtung)                                             |

`back` = `go(-1)`, `forward` = `go(+1)`; Sprung aus der Verlaufsliste = `go(±n)`.

## RouteRecord (Ansicht)

| Feld        | Typ              | Regeln                                                                                                  |
| ----------- | ---------------- | ------------------------------------------------------------------------------------------------------- |
| `path`      | `string`         | Muster relativ zum Eltern-Eintrag; Segmente literal oder `:name`; `''` = Index-Kind des Eltern-Eintrags |
| `component` | Vue-Komponente   | nur in `components/shell/appRoutes.ts`, nicht in reinen Modulen                                         |
| `titleKey`  | `string?`        | i18n-Schlüssel; darf `{param}`-Platzhalter nutzen                                                       |
| `children`  | `RouteRecord[]?` | verschachtelte Ansichten, gerendert vom `ShellRouterView` der nächsten Tiefe                            |

Match-Ergebnis (`RouteMatch`): `chain: RouteRecord[]` (außen → innen), `params:
Record<string, string>`; kein Match → `null` (FR-014). Eine App ohne
Routentabelle hat implizit `[{ path: '/', component: <App-Wurzel> }]`.

## TabRuntime (Erweiterung aus Spec 015)

| Feld                                             | neu?      | Zweck                 |
| ------------------------------------------------ | --------- | --------------------- |
| `attention`, `titleOverride`, `guard`, `mounted` | bestehend | unverändert           |
| `history`                                        | **neu**   | `TabHistory` des Tabs |

Lebensdauer: entsteht mit dem Tab (`openApp`, `addTab`, `hydrate` → Start-Ort),
bleibt beim Tab-Wechsel, Minimieren, Maximieren, Arbeitsbereichswechsel und
Verschieben erhalten (Tab-Id unverändert, FR-010), verschwindet beim Schließen
des Tabs (`syncTabRuntime`, FR-011).

## ShellActionDefinition (Aktion / „Befehl“ der Spec)

| Feld               | Typ                                                                | Regeln                                                                                  |
| ------------------ | ------------------------------------------------------------------ | --------------------------------------------------------------------------------------- |
| `id`               | `string`                                                           | Namensraum `shell.*`, stabil (Grundlage der Folge-Spec Tastenkürzel)                    |
| `titleKey`         | `string`                                                           | i18n-Schlüssel `shell.actions.<…>`                                                      |
| `target`           | `'focusedTab' \| 'focusedWindow' \| 'pointerWindowTab' \| 'shell'` | worauf die Aktion ohne expliziten Kontext wirkt                                         |
| `defaultKeys`      | `{ default?: KeyChord[]; mac?: KeyChord[] }`                       | nur `shell.tab.back`/`forward` belegt (FR-025)                                          |
| `yieldToTextInput` | `{ mac?: boolean; default?: boolean }`                             | ob die Belegung in editierbaren Elementen nicht abgefangen wird (Alt+Pfeil unter macOS) |

`KeyChord`: String in fester Reihenfolge `Ctrl+Alt+Shift+Meta+<code>`, `code`
nach `KeyboardEvent.code` (layoutunabhängig, z. B. `ArrowLeft`, `BracketLeft`).

Aktionsliste und Kontext siehe
[contracts/shell-actions.md](./contracts/shell-actions.md).

## Invarianten (Prüfung in `check:shell-navigation`)

1. Jeder offene Tab hat genau eine `TabHistory` mit gültigem `index`.
2. Keine Operation auf Tab A ändert `TabHistory` von Tab B.
3. `moveWindowToWorkspace`, `switchTab`, `minimizeWindow`, `toggleMaximizeWindow`
   ändern keine Historie.
4. Nach `hydrate` hat jeder Tab genau einen Eintrag `/`.
5. Länge ≤ 50 nach jeder Operation.
