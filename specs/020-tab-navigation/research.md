# Research: Navigation im Tab

**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Datum**: 2026-09-25

Grundlage ist der Implementierungsstand von Spec 015 auf dem Branch
`015-workspace-shell` (noch nicht auf `main`). Pfade unten beziehen sich auf diesen
Stand; die Umsetzung von 020 beginnt erst, wenn 015 gemerged ist (R15).

## R1 — Eigener Router je Tab statt vue-router

**Decision**: Ein kleiner eigener Tab-Router: reine Reducer für die Historie und ein
reiner Pfad-Matcher unter `src/lib/shell/`, dazu zwei Vue-Bausteine
(`ShellRouterView`, `ShellLink`) und das Composable `useTabRouter()`.

**Rationale**:

- Nuxt stellt einen globalen vue-router bereit. `useRoute()`, `<RouterLink>` und
  `<RouterView>` lösen über Injection-Keys genau diesen Router auf; eine zweite
  Instanz je Tab müsste diese Keys in jedem Tab-Teilbaum überschreiben und
  kollidiert mit Nuxts Auto-Imports und Middleware.
- Eine vue-router-Instanz lebt an einer Komponente. Die Historie soll aber zum
  Tab gehören, beim Verschieben des Tabs mitwandern (FR-010) und reine Daten sein
  (FR-001) — das leistet eine im Store gehaltene Datenstruktur, nicht ein
  Router-Objekt mit Closures.
- Der Bedarf ist klein: Pfadmuster mit `:param`, verschachtelte Einträge,
  Push/Replace/Back/Forward/Go. Geschätzt 200–300 Zeilen einschließlich Tests,
  prüfbar ohne Nuxt im Node-Harness (Vorbild `check-shell-state.ts`).

**Alternatives considered**:

- _vue-router mit `createMemoryHistory` je Tab_: siehe oben (Injection-Konflikte,
  Historie nicht serialisierbar, an Komponentenlebensdauer gebunden).
- _haex-vault-Modell (`navigation.ts`, `useDrillDownNavigation`)_: Einträge sind
  `undo`/`redo`-Closures, die beim Unmount verwaisen; alle Tabs teilen
  `window.history` samt `navIndex`-Trick; verschachtelte Navigatoren mischen sich
  in einem Stack; keine Adressen. Genau diese Punkte schließt die Spec aus.

## R2 — Historie lebt im Shell-Store, nicht in der Komponente

**Decision**: Die Historie jedes Tabs liegt im Pinia-Store `stores/shell.ts` in
der bereits vorhandenen, nie persistierten Laufzeitstruktur `TabRuntime`
(Feld `history`), geschlüsselt nach Tab-Id.

**Rationale**: Der Store überlebt Remounts, die Tab-Id bleibt beim Verschieben
zwischen Fenstern und Arbeitsbereichen gleich, und `TabRuntime` ist in Spec 015
genau für flüchtige Tab-Daten angelegt. Schließen eines Tabs räumt die
Laufzeitdaten über das vorhandene `syncTabRuntime()` ab (FR-011).

**Alternatives considered**: Historie in der `ShellTabPanel`-Instanz (geht beim
künftigen Tab-Drag zwischen Fenstern verloren); Historie im persistierten
`ShellTab` (widerspricht der Betreiberentscheidung, FR-011).

## R3 — Ort als Pfad plus Query

**Decision**: `TabLocation = { path: string; query: Record<string, string> }`.
Der Pfad ist app-relativ und beginnt mit `/` (Start-Ort `/`). Der Titel eines
Eintrags wird separat im Historien-Eintrag gespeichert (R9), nicht im Ort.

**Rationale**: Ein Pfad ist das bekannteste Adressformat, trägt Hierarchie für
verschachtelte Ansichten (Präfix = Eltern-Ansicht, R4) und lässt sich als String
in Deep-Links, Tests und später in nativen Fenstern übergeben. Query deckt
Filter/Suche/Sortierung ab (FR-005). Gleichheit (FR-006) = gleicher normierter
Pfad und gleiche Query unabhängig von der Schlüsselreihenfolge.

**Alternatives considered**: `{ view: string; params: object }` (weniger lesbar in
Links, keine natürliche Hierarchie); beliebiger `state`-Blob (verleitet zu
nicht serialisierbaren Inhalten).

## R4 — Routen je App, verschachtelt wie RouterView

**Decision**: Jede App meldet eine Routentabelle an:
`{ path, component, titleKey?, children? }`. Die reine Matcher-Funktion liefert
für einen Pfad die Kette der passenden Einträge von außen nach innen plus die
Parameter. `ShellRouterView` rendert je Verschachtelungstiefe den Eintrag dieser
Tiefe (Tiefe über provide/inject, wie `RouterView`). Eine App ohne eigene
Routentabelle hat implizit genau die Route `/` auf ihre Wurzelkomponente — damit
bleibt `components/shell/appComponents.ts` rückwärtskompatibel und wird zur
Routentabelle erweitert (`appRoutes.ts`).

**Rationale**: Die Seitenleiste der künftigen Einstellungs-App ist dann eine
Eltern-Route (`/` mit Seitenleiste), die Kategorien sind Kinder
(`/models`, `/federation`), ihre Unteransichten Enkel (`/models/hf/:repo`). Alle
Wechsel schreiben in dieselbe Tab-Historie (FR-003); die aktive Kategorie ergibt
sich aus dem gematchten Kind — die in haex-vault getrennten Navigatoren entfallen.

**Alternatives considered**: flache Routen mit manueller Layout-Wahl in jeder App
(Wiederholung, keine gemeinsame Seitenleisten-Logik).

## R5 — Push, Replace und Grenzen

**Decision**:

- `push(to)`: legt einen Eintrag hinter der Position an, verwirft Vor-Einträge
  (FR-004); bei identischem Ort No-op (FR-006).
- `replace(to)`: ersetzt den aktuellen Eintrag (FR-005).
- `setQuery(patch)`: Komfortfunktion, ersetzt standardmäßig (Filter, Suche).
- `back()`, `forward()`, `go(delta)`: verschieben die Position; außerhalb der
  Grenzen wirkungslos (FR-007).
- Höchstens 50 Einträge, der älteste entfällt, die Position wird nachgeführt
  (FR-009). Die Verlaufsliste zeigt höchstens 15 Einträge je Richtung (FR-016).
- `removeEntry(index)` für verwaiste Ziele (R10, FR-014).

**Rationale**: Deckt sich mit Browser-Semantik; alle Operationen sind reine
Funktionen `(history, …) → history` und damit ohne Vue testbar.

## R6 — Eingaben und ihr Ziel

**Decision**:

| Eingabe                                       | Ziel                                         | Umsetzung                                                                                          |
| --------------------------------------------- | -------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Schaltflächen Zurück/Vor                      | aktiver Tab dieses Fensters                  | `ShellNavButtons.vue` links vor `ShellTabBar` in `ShellWindow.vue`                                 |
| Langer Druck (≥ 500 ms) / Rechtsklick         | Verlaufsliste                                | Dropdown aus dem haex-ui-Layer (`ShadcnDropdownMenu`), per Tastatur bedienbar                      |
| Alt+←/→ (alle), Cmd+[/] (macOS)               | aktiver Tab des fokussierten Fensters        | ein globaler `keydown`-Listener auf der Shell-Host-Seite, Auflösung über die Aktions-Registry (R8) |
| Maustasten Zurück/Vor                         | aktiver Tab des Fensters unter dem Zeiger    | `mouseup`/`auxclick` mit `button` 3/4 am Fenster-Wurzelelement, Default unterdrückt; Fallback R7   |
| System-Zurück (Android-Geste, Webview-Zurück) | aktiver Tab des obersten sichtbaren Fensters | R7                                                                                                 |

Für Alt+Pfeil gilt: Liegt der Fokus in einem editierbaren Element und läuft holzi
unter macOS, wird die Taste nicht abgefangen (Wortsprung hat Vorrang, FR-017).
Unter Linux und Windows hat Alt+Pfeil in Textfeldern keine Textbedeutung und
navigiert.

**Rationale**: Jede Eingabe hat genau eine Zielregel (FR-018, SC-002). Das Ziel
wird bei der Eingabe aufgelöst, nicht aus einem globalen „aktiven Tab“ (Fehler
in haex-vault).

**Offener Punkt (Spike in tasks)**: Ob die Maus-Seitentasten unter WebKitGTK
(Linux), WKWebView (macOS) und WebView2 (Windows) als DOM-Ereignis ankommen oder
vom Webview selbst als History-Navigation verarbeitet werden. Kommen sie nicht
an, greift R7 und löst das Ziel über die zuletzt bekannte Zeigerposition
(`pointermove` → `document.elementFromPoint` → nächstes Fenster-Wurzelelement)
auf. Das Verhalten ist damit in beiden Fällen spezifikationsgemäß.

## R7 — Webview-Historie: Sperre und System-Zurück

**Decision**: Die Shell-Host-Seite `pages/workspace/[instance].vue` registriert
`onBeforeRouteLeave`. Jede vom Router ausgelöste Navigation weg von der
Workspace-Seite (praktisch nur durch System-Zurück, denn Sperren und Schließen
beenden den Prozess, Spec 013) wird abgebrochen und als **System-Zurück**
behandelt: Ziel ist der aktive Tab des obersten sichtbaren Fensters bzw. — für
Maustasten, die nicht als DOM-Ereignis ankommen — das Fenster unter der zuletzt
bekannten Zeigerposition. Hat das Ziel keinen Zurück-Eintrag, öffnet sich in der
Kompaktdarstellung die Fensterübersicht; ist ein Shell-Overlay offen, schließt es
sich (FR-019). Damit System-Zurück überhaupt ein Ereignis auslöst statt die App
zu beenden, hält die Host-Seite einen Sperr-Eintrag in der Router-Historie
(ein `router.push` auf dieselbe Seite mit einem Marker in `history.state`,
nach jedem abgefangenen Zurück erneut).

**Rationale**: Die Webview-Historie dient nie als Tab-Historie (FR-020); der
Router bleibt Herr seiner eigenen `history.state`-Struktur (direktes
`pushState` an vue-router vorbei bricht dessen Zustandsschlüssel).

**Alternatives considered**: roher `popstate`-Listener mit eigenem `pushState`
(haex-vault) — kollidiert mit vue-router, der denselben `popstate` verarbeitet.

**Einschränkung**: holzi hat heute kein Android-Target. Die Android-Geste wird
über denselben Pfad bedient (Tauri leitet sie an die Webview-Historie weiter),
kann aber erst mit einem Android-Build end-to-end geprüft werden. Auf dem Desktop
wird der Pfad mit `history.back()` in den DevTools geprüft (quickstart.md).

## R8 — Aktions-Registry (Name „Aktion“, nicht „Command“)

**Decision**: `src/lib/shell/actions.ts` definiert Shell-Aktionen
(`ShellActionDefinition`: `id`, `titleKey`, `target`, `defaultKeys`),
`src/lib/shell/keybindings.ts` normiert `KeyboardEvent` zu Chords
(`Alt+ArrowLeft`, `Meta+BracketLeft`) und löst Chords je Plattform zu Aktionen
auf. Der Store bekommt `runAction(id, context)`. Schaltflächen und Menüs der
Spec-015-Komponenten rufen künftig `runAction` statt Store-Methoden direkt
(FR-024). Nur `shell.tab.back` und `shell.tab.forward` haben eine
Standardbelegung (FR-025). Die Pfeiltasten der Tab-Leiste bleiben lokales
ARIA-Verhalten (FR-026).

**Rationale**: In holzi bezeichnet „Command“ durchgehend Tauri-Commands
(`#[tauri::command]`, `contracts/tauri-commands.md`, die sechs
Shell-Layout-Commands aus 015). Graphify-Abfrage (Graph vom 2026-09-21) zu
`router`, `navigation`, `keybinding`, `shortcut`, `hotkey`, `keydown`: keine
Treffer im Frontend; `command` trifft nur Tauri-Commands. Spec 015 spricht
bereits von „Shell-Aktionen“ — der Begriff wird übernommen. Die Befehle der Spec
heißen im Code daher Aktionen.

**Alternatives considered**: `commands.ts` (Namenskollision), Tastenkürzel direkt
in Komponenten (widerspricht FR-024 und macht die Folge-Spec teuer).

## R9 — Titel

**Decision**: Angezeigter Tab-Titel = dynamischer Titel der App
(`useShellTab().setTitle`, Spec 015) → sonst `titleKey` der tiefsten gematchten
Route mit `titleKey` → sonst App-Name. Beim Verlassen eines Eintrags speichert
der Store den zu diesem Zeitpunkt angezeigten Titel im Eintrag (`title`), sodass
die Verlaufsliste Titel „zum Zeitpunkt des Besuchs“ zeigt (FR-021). Ein
dynamischer Titel wird beim Navigieren zurückgesetzt; die App setzt ihn für die
neue Ansicht bei Bedarf neu.

## R10 — Chat

**Decision**: Der Chat bekommt die Routen `/` (neue Unterhaltung, Start-Ort) und
`/thread/:id`. Ein neues Composable `useChatNavigation.ts` verbindet Router und
bestehende Logik, weil `ChatApp.vue` mit 499 Zeilen an der 500-Zeilen-Grenze
steht:

- Verlaufseintrag öffnen → `push('/thread/:id')`; ein Watcher auf den Ort ruft
  das vorhandene `selectThread(id)` auf — damit gelten die bestehenden Regeln für
  laufende Antworten und ausstehende Freigaben unverändert (FR-027).
- Neue Unterhaltung → `push('/')`.
- Die erste Nachricht einer neuen Unterhaltung legt den Thread an
  (`useComposer.ts`, `activeThreadId.value = result.threadId`) → `replace`
  auf `/thread/:id`, damit Zurück nicht auf eine leere neue Unterhaltung führt.
- Aktiver Thread gelöscht (Spec 006, FR-015) → `replace('/')`.
- Ort zeigt auf einen nicht existierenden Thread → Eintrag entfernen und in
  dieselbe Richtung weiter (Zurück bzw. Vor); gibt es keinen, `replace('/')`.
- Spec 004 bleibt erfüllt: jeder Chat-Einstieg (neuer Tab, Neustart) beginnt am
  Start-Ort `/`.

## R11 — Öffnen an einem Ort

**Decision**: `shell.openApp(appId, location?)` und `addTab(windowId, appId,
location?)`; `useShellTab()` bekommt `openApp(appId, location?)` für Sprünge
zwischen Apps. Neue Instanz: Historie `[location ?? '/']`. Vorhandene
Einzelinstanz: aktivieren wie bisher (`activateExistingSingleton`), dann `push`
(No-op bei gleichem Ort). Die Legacy-Weiterleitung `?open=<appId>` bleibt und
nimmt optional `&at=<pfad>` an. Unbekannter Pfad → `ShellRouterView` ersetzt
per `replace('/')` und zeigt einen Hinweis (Toast aus dem haex-ui-Layer)
(FR-014).

## R12 — Aufbewahrung, Sperren, Neustart

**Decision**: Keine Persistenz; `TabDto`/Migrationen bleiben unverändert, es gibt
**keine** neuen Tauri-Commands. `hydrate` legt für jeden wiederhergestellten Tab
eine Historie `[{ path: '/' }]` an. Sperren/Schließen beendet den Prozess
(Spec 013), damit ist alles verworfen.

## R13 — Prüfungen

**Decision**: Neues Skript `scripts/check-shell-navigation.ts` (Node mit
Type-Stripping, Vorbild `check-shell-state.ts`) für Historien-Reducer, Matcher,
Chord-Normierung und Aktionsauflösung sowie Store-Integration (openApp mit Ort,
Singleton-Push, Verschieben erhält Historie, hydrate ohne Historie). Neues
`pnpm check:shell-navigation` und ein CI-Schritt.

**Rationale**: `scripts/check-shell-state.ts` hat bereits 829 Zeilen und liegt
über der 500-Zeilen-Grenze (in Spec 015 als Complexity Tracking mit
„aufteilen, sobald erreicht“ vermerkt). Neue Prüfungen kommen deshalb in eine
eigene Datei; die Aufteilung des bestehenden Skripts ist ein Befund für 015,
nicht Teil dieser Spec.

## R14 — Oberfläche

**Decision**: Nur der vorhandene Stack: haex-ui-Layer (`ShadcnDropdownMenu`,
`ShadcnTooltip`, Toast), `@lucide/vue` (`arrow-left`, `arrow-right`),
Tailwind. Keine neue Abhängigkeit. Beide Schaltflächen bleiben in der
Kompaktdarstellung sichtbar (FR-015) und erhalten dort die Touch-Größe aus
015 (T037/`326b0ed`).

## R15 — Abhängigkeit von 015 und Phasen-Disziplin

**Decision**: Spec und Plan liegen auf `020-tab-navigation` (Basis `main`). Die
Umsetzung beginnt erst nach dem Merge von 015; dann wird der Branch auf `main`
rebased. `plans/README.md` bekommt einen Roadmap-Eintrag (Aufgabe in tasks).

**Rationale**: Constitution, Phasen-Disziplin: Folgefunktionen dürfen vorab
spezifiziert, aber erst umgesetzt werden, wenn ihre Voraussetzungen im Einsatz
sind.

## R16 — ADR

**Decision**: Kein ADR nötig. Die Entscheidungen berühren kein Prinzip der
Constitution; sie sind in dieser Datei begründet.
