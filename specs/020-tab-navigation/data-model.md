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

Ablage: eigene reaktive `Map<tabId, TabHistory>` neben `TabRuntime` im Shell-Store
(`stores/shellNavigation.ts`, research R2), nie persistiert.

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

## Tab-Historien im Store (Umsetzung)

Statt eines Felds `TabRuntime.history` führt der Store eine eigene reaktive Map
`Map<tabId, TabHistory>` plus je Tab die letzte Laufrichtung (für das
Überspringen verwaister Einträge, FR-027). Die reinen Layout-Reducer aus 015
bleiben unverändert; `lib/shell/tabNavigation.ts` legt nach `openApp`/`addTab`
die Historie eines neuen Tabs an bzw. pusht den Ort auf eine aktivierte
Einzelinstanz.

Lebensdauer: entsteht mit dem Tab (`openApp`, `addTab`, `hydrate` → Start-Ort),
bleibt beim Tab-Wechsel, Minimieren, Maximieren, Arbeitsbereichswechsel und
Verschieben erhalten (Tab-Id unverändert, FR-010), verschwindet beim Schließen
des Tabs (Abgleich in `syncTabRuntime`, FR-011).

## ShellActionDefinition (Aktion)

Reine Daten unter `src/lib/actions/` (research R8); Handler werden getrennt
registriert (R19).

| Feld               | Typ                                          | Regeln                                                                                                                                                 |
| ------------------ | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `id`               | `string`                                     | Namensraum `shell.*`, `chat.*`, `settings.*`; stabil, sobald ausgeliefert                                                                              |
| `titleKey`         | `string`                                     | i18n-Schlüssel `actions.<id>`                                                                                                                          |
| `description`      | `string`                                     | englisch, für maschinelle Aufrufer (Tool-Beschreibung)                                                                                                 |
| `input`            | JSON Schema (Teilmenge)                      | Wurzel `type: object`; erlaubt `properties`, `required`, `string`, `number`, `integer`, `boolean`, `array`, `enum`, `description`                      |
| `result`           | JSON Schema (Teilmenge)                      | Form des Ergebnisses                                                                                                                                   |
| `target`           | `'none' \| 'tab' \| 'window' \| 'workspace'` | worauf die Aktion wirkt; bei ≠ `none` enthält `input` das Feld `tabId` / `windowId` / `workspaceId` (optional für `user`, Pflicht für Agenten, FR-030) |
| `scope`            | `ActionScope`                                | genau ein Bereich (FR-031)                                                                                                                             |
| `effect`           | `'read' \| 'write' \| 'destructive'`         | Abbildung später: `read` → `Safe`, sonst `Risky`                                                                                                       |
| `agentCallable`    | `boolean`                                    | `false` für jeden Eintrag mit `scope = 'guardrails'` (Invariante)                                                                                      |
| `binding`          | `'global' \| 'tab'`                          | global beim Start registriert oder von der App-Instanz (R19)                                                                                           |
| `appId`            | `string?`                                    | Pflicht bei `binding = 'tab'`: welche App der Runner öffnet                                                                                            |
| `defaultKeys`      | `{ default?: KeyChord[]; mac?: KeyChord[] }` | nur `shell.tab.back`/`forward` belegt (FR-025)                                                                                                         |
| `yieldToTextInput` | `{ mac?: KeyChord[]; default?: KeyChord[] }` | Kombinationen, die in editierbaren Elementen nicht abgefangen werden (unter macOS nur Alt+Pfeil, nicht Cmd+[/])                                        |

`KeyChord`: `Ctrl+Alt+Shift+Meta+<KeyboardEvent.code>` in dieser Reihenfolge.

## ActionScope (Berechtigungsbereich)

`shell.layout` · `shell.navigation` · `shell.read` · `chat.read` · `chat.write` ·
`settings.read` · `settings.device` · `settings.models` · `guardrails`

Jeder Bereich hat `titleKey` (`actions.scopes.<name>`) und eine Beschreibung;
`guardrails` ist für Agenten gesperrt (FR-032). Neue Bereiche (etwa für
haextensions, Passwörter, Dateien, Shell) kommen mit den Specs 017–021 hinzu.

## ActionCaller (Aufrufer)

`{ kind: 'user' }` · `{ kind: 'builtinAgent' }` · `{ kind: 'externalAgent';
agentId: string }` (FR-029). In dieser Spec ruft nur `user` auf; die anderen
Formen existieren für Runner und Tests.

## ActionOutcome (Ergebnis)

| Form   | Felder                                                                                                                                                 |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Erfolg | `{ ok: true, result: unknown }` (entspricht `result`-Schema)                                                                                           |
| Fehler | `{ ok: false, code, message, field?, error? }` — `error` ist bei `failed` der Rohfehler des Handlers, nur für In-Process-Aufrufer (`useActionOrThrow`) |

Fehlercodes: `unknown_action`, `invalid_input` (mit `field`), `target_required`,
`target_not_found`, `forbidden_for_agents`, `app_unavailable`, `failed`
(Handler-Fehler; `message` aus `Error.message` bzw. dem `reason`/`message` eines
Backend-Fehlers).

### Ablauf `runAction(id, input, caller)`

1. Aktion nachschlagen → sonst `unknown_action`.
2. `caller.kind ≠ 'user'` und `agentCallable = false` → `forbidden_for_agents`.
3. Eingabe gegen `input` prüfen → sonst `invalid_input`.
4. Ziel: ausdrücklich angegeben → prüfen (`target_not_found`); fehlt es →
   `user`: aus dem Fokus (FR-025), Agent: `target_required`.
5. `binding = 'tab'`: App öffnen/aktivieren, auf Handler warten (≤ 5 s) →
   sonst `app_unavailable`.
6. Handler ausführen → Erfolg oder `failed`.

## Invarianten (Prüfung in `check:shell-navigation`)

1. Jeder offene Tab hat genau eine `TabHistory` mit gültigem `index`.
2. Keine Operation auf Tab A ändert `TabHistory` von Tab B.
3. `moveWindowToWorkspace`, `switchTab`, `minimizeWindow`, `toggleMaximizeWindow`
   ändern keine Historie.
4. Nach `hydrate` hat jeder Tab genau einen Eintrag `/`.
5. Länge ≤ 50 nach jeder Operation.
6. Aktions-Ids eindeutig; jedes Schema in der erlaubten Teilmenge; jeder
   `scope` existiert; `scope = 'guardrails'` ⇒ `agentCallable = false`.
7. Kein Aufruf mit Agenten-Aufrufer erreicht den Handler einer Aktion mit
   `agentCallable = false`.
