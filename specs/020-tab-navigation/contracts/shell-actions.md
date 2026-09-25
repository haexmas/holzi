# Vertrag: Aktionen und Eingaben

Die Aktionen der Spec (FR-024–FR-033) heißen im Code **Aktionen**, weil „Command“
in holzi Tauri-Commands bezeichnet (research R8). Datenformen:
[data-model.md](../data-model.md#shellactiondefinition-aktion).

## 1. Katalog

**Umsetzungsstand:** Der ausgelieferte Katalog steht in `src/lib/actions/` (58
Aktionen; `shell.actions.list` liefert ihn zur Laufzeit). Gegenüber der Liste unten
kamen `shell.window.setGeometry`, `shell.workspaces.overview`,
`chat.model.retryLoad`, `chat.model.downloadRecommended`,
`chat.modelIntegrity.decide` (Leitplanke), `chat.voice.setAutoSend` sowie weitere
Einstellungsaktionen hinzu. Der Sperren-Knopf bleibt bewusst direkt
(`action-exempt:`, Spec 013 prüft ihn); eine Aktion `shell.vault.lock` folgt mit
der Spec für Tastenkürzel.

Die folgende Liste ist der Mindestumfang. Die endgültige Liste der
Einstellungs- und Chat-Aktionen entsteht in tasks aus einer Bestandsaufnahme
aller Bedienelemente (SC-007); jede dort gefundene zustandsändernde Bedienung
bekommt eine Aktion oder eine begründete `action-exempt:`-Ausnahme (R20).

### Shell (`binding: global`)

| Id                                             | Ziel      | Bereich            | Wirkung     | Agent                     | Kürzel                                  |
| ---------------------------------------------- | --------- | ------------------ | ----------- | ------------------------- | --------------------------------------- |
| `shell.tab.back`                               | tab       | `shell.navigation` | write       | ja                        | Alt+ArrowLeft; mac + Meta+BracketLeft   |
| `shell.tab.forward`                            | tab       | `shell.navigation` | write       | ja                        | Alt+ArrowRight; mac + Meta+BracketRight |
| `shell.tab.go` (`steps`)                       | tab       | `shell.navigation` | write       | ja                        | –                                       |
| `shell.tab.navigate` (`to`, `replace?`)        | tab       | `shell.navigation` | write       | ja                        | –                                       |
| `shell.app.open` (`appId`, `at?`)              | none      | `shell.navigation` | write       | ja                        | –                                       |
| `shell.tab.new` (`appId`, `at?`)               | window    | `shell.layout`     | write       | ja                        | –                                       |
| `shell.tab.activate`                           | tab       | `shell.layout`     | write       | ja                        | –                                       |
| `shell.tab.close`                              | tab       | `shell.layout`     | destructive | ja                        | –                                       |
| `shell.window.focus`                           | window    | `shell.layout`     | write       | ja                        | –                                       |
| `shell.window.minimize`                        | window    | `shell.layout`     | write       | ja                        | –                                       |
| `shell.window.toggleMaximize`                  | window    | `shell.layout`     | write       | ja                        | –                                       |
| `shell.window.close`                           | window    | `shell.layout`     | destructive | ja                        | –                                       |
| `shell.window.moveToWorkspace` (`workspaceId`) | window    | `shell.layout`     | write       | ja                        | –                                       |
| `shell.workspace.create`                       | none      | `shell.layout`     | write       | ja                        | –                                       |
| `shell.workspace.switch`                       | workspace | `shell.layout`     | write       | ja                        | –                                       |
| `shell.workspace.delete`                       | workspace | `shell.layout`     | destructive | ja                        | –                                       |
| `shell.windows.overview`                       | none      | `shell.layout`     | write       | ja                        | –                                       |
| `shell.launcher.open`                          | none      | `shell.layout`     | write       | ja                        | –                                       |
| `shell.system.back`                            | none      | `shell.navigation` | write       | nein (nur Plattform-Hook) | –                                       |
| `shell.state.get`                              | none      | `shell.read`       | read        | ja                        | –                                       |
| `shell.tab.history`                            | tab       | `shell.read`       | read        | ja                        | –                                       |
| `shell.apps.list`                              | none      | `shell.read`       | read        | ja                        | –                                       |
| `shell.actions.list`                           | none      | `shell.read`       | read        | ja                        | –                                       |

Fenster- und Tab-Schließen durch einen Agenten laufen durch dieselben Guards wie
beim Nutzer (015 FR-014): Die Bestätigung erscheint beim Nutzer; lehnt er ab,
liefert die Aktion `failed` mit `message: "declined by user"`.

### Chat (`binding: tab`, `appId: system.chat`)

| Id                                                 | Bereich      | Wirkung     | Agent    |
| -------------------------------------------------- | ------------ | ----------- | -------- |
| `chat.conversation.new`                            | `chat.write` | write       | ja       |
| `chat.conversation.open` (`threadId`)              | `chat.read`  | write       | ja       |
| `chat.conversation.rename` (`threadId`, `title`)   | `chat.write` | write       | ja       |
| `chat.conversation.delete` (`threadId`)            | `chat.write` | destructive | ja       |
| `chat.conversations.list`                          | `chat.read`  | read        | ja       |
| `chat.messages.list` (`threadId`)                  | `chat.read`  | read        | ja       |
| `chat.message.send` (`text`)                       | `chat.write` | write       | ja       |
| `chat.reply.cancel`                                | `chat.write` | write       | ja       |
| `chat.model.select` (`modelId`)                    | `chat.write` | write       | ja       |
| `chat.approval.decide` (`requestId`, `decision`)   | `guardrails` | write       | **nein** |
| `chat.permissionMode.set` (`manual`/`auto`/`plan`) | `guardrails` | write       | **nein** |

### Einstellungen (`binding: global`)

| Id                                                         | Bereich           | Wirkung             | Agent    |
| ---------------------------------------------------------- | ----------------- | ------------------- | -------- |
| `settings.get`                                             | `settings.read`   | read                | ja       |
| `settings.device.setAlias` (`alias`)                       | `settings.device` | write               | ja       |
| `settings.models.setDefault` (`modelId`)                   | `settings.models` | write               | ja       |
| `settings.models.setStt` (`modelId`)                       | `settings.models` | write               | ja       |
| `settings.models.download` / `.cancelDownload` / `.delete` | `settings.models` | write / destructive | ja       |
| `settings.delegate.connectProvider`                        | `guardrails`      | write               | **nein** |
| `settings.autonomy.setMode`                                | `guardrails`      | write               | **nein** |
| `settings.delegate.setDenyRules`                           | `guardrails`      | write               | **nein** |

`settings.get` gibt Werte der Leitplanken lesbar zurück, aber **nie**
Zugangsdaten oder Geheimnisse von Anbietern.

## 2. Aufruf

```ts
shell.runAction(id: string, input?: Record<string, unknown>, caller?: ActionCaller): Promise<ActionOutcome>
// caller Standard: { kind: 'user' }
```

- Ablauf und Fehlercodes: [data-model.md](../data-model.md#actionoutcome-ergebnis).
- Schaltflächen, Menüs und Tastenkürzel rufen ausschließlich `runAction`
  (FR-024); in Vue-Komponenten über `useAction(id)` (liefert eine
  Aufruf-Funktion mit `caller: user`).
- Tab-gebundene Handler registriert die App mit
  `useShellTab().registerActionHandler(id, handler)`; ein Handler je Aktion und
  Tab-Instanz, Abmeldung beim Unmount.

## 3. Tastatur

- Ein einziger `keydown`-Listener (Capture-Phase) im obersten Dokument auf der
  Shell-Host-Seite.
- Normierung `Ctrl+Alt+Shift+Meta+<KeyboardEvent.code>`; Plattform `mac`, wenn
  `navigator.userAgentData?.platform ?? navigator.platform` mit „Mac“ beginnt.
- Treffer → `preventDefault()` + `runAction(id, {}, user)`. Ausnahme:
  `yieldToTextInput` für die Plattform gesetzt und `event.target` editierbar
  (`input`, `textarea`, `[contenteditable]`) → nicht abfangen (FR-017).
- Komponentenlokale Tasten (Pfeiltasten der Tab-Leiste, Escape in Popovern,
  Enter im Composer) bleiben lokal (FR-026).
- Browser-eigene Kürzel des Webviews werden abgeschaltet, wo die Plattform es
  anbietet (research R17).

## 4. Maus

- Am Fenster-Wurzelelement (`ShellWindow.vue`, `data-shell-window-id`):
  `mouseup` mit `button === 3` → `shell.tab.back`, `button === 4` →
  `shell.tab.forward`, mit dem aktiven Tab dieses Fensters als `tabId`; kein
  `focusWindow` (FR-018). `mousedown`/`auxclick` dieser Tasten:
  `preventDefault`.
- Außerhalb von Fenstern: nichts. Über eingebetteten Dokumenten: siehe research
  R17.

## 5. System-Zurück

- Nur der native Zurück-Hook der Plattform ruft `shell.system.back` auf
  (research R7). Die Browser-Historie des Webviews löst nie eine Aktion aus.
- `pages/workspace/[instance].vue` bricht jede Router-Navigation weg von der
  Seite ab und ignoriert sie (`onBeforeRouteLeave`), ohne sie zu deuten.
- `shell.system.back`:
  1. Offenes Shell-Overlay (Launcher, Fensterübersicht,
     Arbeitsbereichs-Übersicht, „+“-Liste, Chevron, Verlaufsliste) → schließen.
  2. Sonst aktiver Tab des obersten sichtbaren Fensters → `back()`.
  3. Kein Zurück-Eintrag: Kompaktdarstellung → Fensterübersicht öffnen; sonst
     nichts. Die App wird nie verlassen.

## 6. Schaltflächen und Verlaufsliste

- `ShellNavButtons.vue` links vor `ShellTabBar`; zwei Icon-Schaltflächen mit
  `aria-label` (`shell.nav.back`, `shell.nav.forward`), `disabled`, wenn keine
  Einträge in der Richtung existieren; Tooltip mit Tastenkürzel.
- Langer Druck (≥ 500 ms) oder `contextmenu` öffnet die Verlaufsliste
  (höchstens 15 Einträge, nächster zuerst, Titel je Eintrag). Auswahl →
  `runAction('shell.tab.go', { tabId, steps })`.
- `pointerdown` auf den Schaltflächen startet keinen Fenster-Drag.
