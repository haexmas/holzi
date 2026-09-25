# Vertrag: Navigation im Tab (App-Seite)

Ergänzt [`015-workspace-shell/contracts/shell-app-contract.md`](../../015-workspace-shell/contracts/shell-app-contract.md).
Apps sehen weiterhin nur `useShellTab()` und die hier beschriebenen Bausteine;
sie importieren nichts aus `stores/shell.ts` oder `components/shell/` außer
`ShellRouterView` und `ShellLink`.

## 1. Routen anmelden

In `src/components/shell/appRoutes.ts` (ersetzt `appComponents.ts`):

```ts
// Form, kein Implementierungscode
'system.chat': {
  routes: [
    { path: '/', component: ChatApp, children: [
      { path: '', component: ChatNewConversation },          // Start-Ort
      { path: 'thread/:id', component: ChatThreadView, titleKey: 'shell.chat.thread' },
    ]},
  ],
},
'system.federation': { component: FederationApp },           // ohne Routen = nur '/'
```

- Pfade der Kinder sind relativ zum Eltern-Eintrag; `''` ist das Index-Kind.
- Jede App hat einen Start-Ort `/`.
- Eine App ohne `routes` hat implizit genau `/` → ihre Komponente.
- Ob die Chat-Ansichten als eigene Kind-Komponenten oder als eine Komponente mit
  Watcher auf `route.params` umgesetzt werden, entscheidet die Umsetzung
  (research R10); der Vertrag ist nur die Routentabelle.

## 2. `useTabRouter()`

```ts
type TabRouter = {
  readonly route: {
    // reaktiv
    path: string
    query: Record<string, string>
    params: Record<string, string> // aus dem Muster, z. B. { id }
    matched: readonly { path: string; titleKey?: string }[]
  }
  readonly canGoBack: boolean
  readonly canGoForward: boolean
  push(to: string | TabLocation): void // neue Ansicht (FR-004), No-op bei gleichem Ort (FR-006)
  replace(to: string | TabLocation): void // gleiche Ansicht anders (FR-005)
  setQuery(
    patch: Record<string, string | null>,
    opts?: { push?: boolean },
  ): void // Standard: replace
  back(): void
  forward(): void
}
```

- `to` als String: `'/models/hf/abc?sort=size'` (Query wird geparst); relative
  Pfade sind nicht erlaubt.
- Alle Aufrufe wirken ausschließlich auf den eigenen Tab (FR-008).
- Außerhalb einer Shell liefert `useTabRouter()` einen inerten Router mit Ort
  `/` (wie `useShellTab()`), damit Apps in Test-Harnesses montierbar bleiben.

**Regel für Apps (FR-005)**: `push` für alles, was der Nutzer als „andere Seite“
wahrnimmt (andere Ansicht, anderes Element); `replace`/`setQuery` für Filter,
Suche, Sortierung, Aufklappzustand, Tabs innerhalb einer Ansicht.

## 3. `ShellRouterView` und `ShellLink`

- `<ShellRouterView />` rendert den gematchten Eintrag der eigenen Tiefe. Die
  Wurzel rendert die Shell selbst in `ShellTabPanel`; Apps setzen
  `<ShellRouterView />` dort ein, wo Kinder erscheinen (z. B. rechts neben der
  Seitenleiste).
- `<ShellLink to="/models" [replace] [active-class]>` rendert ein `<a>` mit
  `href="#"` (kein Webview-Navigationsziel), löst `push`/`replace` aus und setzt
  `aria-current="page"`, wenn der aktuelle Pfad gleich oder (mit `prefix`) ein
  Unterpfad ist — Grundlage für die Seitenleisten-Markierung (US1 AS7).
- Unbekannter Pfad: `ShellRouterView` der Wurzel ersetzt per `replace('/')` und
  zeigt den Hinweis `shell.nav.unknownLocation` (FR-014).

## 4. Erweiterung von `useShellTab()`

```ts
openApp(appId: string, at?: string | TabLocation): void   // FR-012
```

- Neue Instanz: Tab mit Historie `[at ?? '/']`.
- Offene Einzelinstanz: Tab aktivieren, Fenster wiederherstellen und
  fokussieren, Arbeitsbereich aktivieren (015 FR-016), dann `push(at)`.
- `setTitle(title)` bleibt; ein gesetzter Titel gilt bis zur nächsten Navigation
  des Tabs (research R9).

## 5. Legacy-Adressen

`/<seite>/<instance>` → `/workspace/<instance>?open=<appId>[&at=<pfad>]`. Die
Host-Seite ruft `openApp(appId, at)` einmal auf und entfernt beide Parameter per
`router.replace` (wie bisher `open`).

## 6. Was die Shell garantiert

- Historie überlebt Tab-Wechsel, Minimieren, Maximieren, Arbeitsbereichswechsel,
  Verschieben (FR-010), nicht aber Neustart, Sperren oder Schließen des Tabs
  (FR-011).
- Zurück/Vor schließen nie etwas (FR-007).
- Beim Navigieren wird die verlassene Ansicht unmontiert; Apps, die Eingaben
  retten wollen, tun das selbst (spec Annahmen).
