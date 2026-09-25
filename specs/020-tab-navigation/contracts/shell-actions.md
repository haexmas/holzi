# Vertrag: Shell-Aktionen und Eingaben

Die „Befehle“ der Spec (FR-024–026) heißen im Code **Aktionen**, weil „Command“ in
holzi Tauri-Commands bezeichnet (research R8).

## 1. Aktionsliste

| Id                            | Titel-Schlüssel                      | Ziel                                 | Standardbelegung                                   |
| ----------------------------- | ------------------------------------ | ------------------------------------ | -------------------------------------------------- |
| `shell.tab.back`              | `shell.actions.tabBack`              | `focusedTab`                         | Alt+ArrowLeft; macOS zusätzlich Meta+BracketLeft   |
| `shell.tab.forward`           | `shell.actions.tabForward`           | `focusedTab`                         | Alt+ArrowRight; macOS zusätzlich Meta+BracketRight |
| `shell.tab.new`               | `shell.actions.tabNew`               | `focusedWindow` („+“-Liste öffnen)   | –                                                  |
| `shell.tab.close`             | `shell.actions.tabClose`             | `focusedTab` (mit Guard, 015 FR-014) | –                                                  |
| `shell.tab.list`              | `shell.actions.tabList`              | `focusedWindow` (Chevron)            | –                                                  |
| `shell.window.minimize`       | `shell.actions.windowMinimize`       | `focusedWindow`                      | –                                                  |
| `shell.window.toggleMaximize` | `shell.actions.windowToggleMaximize` | `focusedWindow`                      | –                                                  |
| `shell.window.close`          | `shell.actions.windowClose`          | `focusedWindow` (mit Guard)          | –                                                  |
| `shell.windows.overview`      | `shell.actions.windowsOverview`      | `shell`                              | –                                                  |
| `shell.workspace.create`      | `shell.actions.workspaceCreate`      | `shell`                              | –                                                  |
| `shell.workspace.switch`      | `shell.actions.workspaceSwitch`      | `shell` (Argument `workspaceId`)     | –                                                  |
| `shell.launcher.open`         | `shell.actions.launcherOpen`         | `shell`                              | –                                                  |

Neue Aktionen dürfen hinzukommen; Ids sind stabil, sobald ausgeliefert (die
Folge-Spec speichert Nutzerbelegungen unter diesen Ids).

## 2. Aufruf

```ts
shell.runAction(id: string, context?: { windowId?: string; tabId?: string; workspaceId?: string; steps?: number })
```

- Ohne `context` löst der Store das Ziel nach `target` auf
  (`focusedTab` = aktiver Tab von `state.activeWindowId`).
- Schaltflächen, Menüs und Tastenkürzel aus Spec 015 und 020 rufen
  ausschließlich `runAction` (FR-024). Die Maus-Seitentasten übergeben
  `windowId` des Fensters unter dem Zeiger; die Verlaufsliste übergibt
  `steps` (±n).
- Unbekannte Id oder nicht auflösbares Ziel: No-op, Warnung nur im Dev-Build.

## 3. Tastatur

- Ein einziger `keydown`-Listener (Capture-Phase) auf der Shell-Host-Seite.
- Normierung: `Ctrl+Alt+Shift+Meta+<KeyboardEvent.code>`; Plattform
  `mac` = `navigator.userAgentData?.platform ?? navigator.platform` beginnt mit
  „Mac“.
- Treffer → `preventDefault()` + `runAction`. Ausnahme: `yieldToTextInput` der
  Aktion für die Plattform ist gesetzt und `event.target` ist editierbar
  (`input`, `textarea`, `[contenteditable]`) → nicht abfangen (FR-017; betrifft
  Alt+Pfeil unter macOS).
- Die Pfeiltasten-Navigation der Tab-Leiste (015 FR-038) und alle
  komponentenlokalen Tasten (Escape in Popovern, Enter im Composer) bleiben
  lokal und sind keine globalen Belegungen (FR-026).

## 4. Maus

- Am Fenster-Wurzelelement (`ShellWindow.vue`, `data-shell-window-id`):
  `mouseup` mit `button === 3` → `shell.tab.back`, `button === 4` →
  `shell.tab.forward`, jeweils mit `context.windowId` dieses Fensters; kein
  `focusWindow` (FR-018). `mousedown`/`auxclick` dieser Tasten: `preventDefault`.
- Außerhalb von Fenstern: nichts.
- Kommen die Tasten nicht als DOM-Ereignis an, greift der System-Zurück-Pfad
  (Abschnitt 5) mit dem Fenster unter der zuletzt bekannten Zeigerposition
  (research R6).

## 5. System-Zurück (Webview/Android)

- `pages/workspace/[instance].vue` bricht jede Router-Navigation weg von der
  Seite ab (`onBeforeRouteLeave`) und ruft stattdessen `shell.systemBack()`.
- `systemBack()`:
  1. Ist ein Shell-Overlay offen (Launcher, Fensterübersicht,
     Arbeitsbereichs-Übersicht, „+“-Liste, Chevron, Verlaufsliste): schließen.
  2. Sonst Ziel = Fenster unter der letzten Zeigerposition, falls die Eingabe
     eine Maustaste war, sonst oberstes sichtbares Fenster; dessen aktiver Tab
     `back()`.
  3. Hat das Ziel keinen Zurück-Eintrag: in der Kompaktdarstellung
     Fensterübersicht öffnen; sonst nichts.
- Nach jedem abgefangenen Zurück stellt die Host-Seite den Sperr-Eintrag der
  Router-Historie wieder her (research R7). Die App wird nie verlassen.

## 6. Schaltflächen und Verlaufsliste

- `ShellNavButtons.vue` links vor `ShellTabBar` in der Titelleiste; zwei
  Icon-Schaltflächen mit `aria-label` (`shell.nav.back`, `shell.nav.forward`),
  `disabled`, wenn keine Einträge in der Richtung existieren; Tooltip mit
  Tastenkürzel.
- Langer Druck (≥ 500 ms, Zeiger bleibt auf der Schaltfläche) oder
  `contextmenu` öffnet die Verlaufsliste (Dropdown, höchstens 15 Einträge,
  nächster zuerst, Titel je Eintrag). Auswahl → `runAction('shell.tab.back' |
'shell.tab.forward', { windowId, steps })`.
- `pointerdown` auf den Schaltflächen startet keinen Fenster-Drag (wie die
  übrigen Titelleisten-Schaltflächen in 015).
