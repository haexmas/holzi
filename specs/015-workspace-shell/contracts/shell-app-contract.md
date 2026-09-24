# Contract: Shell ↔ App (Frontend)

**Feature**: 015-workspace-shell | **Date**: 2026-09-21

Interner Vertrag zwischen der Shell und dem, was in einem Tab läuft. Er ist
absichtlich klein und hängt von keiner konkreten App ab, sodass spätere Apps
(auch Extensions, Specs 017/018) ihn unverändert nutzen. Apps importieren
**nichts** aus `components/shell/` oder `stores/shell.ts`; sie sehen nur
`useShellTab()`.

## 1. App-Definition (`src/lib/shell/apps.ts`, reine Daten)

```ts
export type ShellAppDefinition = {
  id: string // 'system.chat' | 'system.settings' | 'system.federation'
  titleKey: string // i18n-Schlüssel, z. B. 'shell.apps.chat'
  icon: string // Iconify-Name, z. B. 'lucide:message-square'
  defaultSize: { width: number; height: number } // neues Fenster mit dieser App als erstem Tab
  minSize: { width: number; height: number } // Mindestgröße des Fensters
  multiInstance: boolean // false = Einzelinstanz (FR-016)
}
```

- In dieser Spec: alle drei Apps mit `multiInstance: false`.
- Die Zuordnung `id → Komponente` liegt getrennt in
  `components/shell/appComponents.ts` (`defineAsyncComponent`); der reine Teil bleibt
  ohne Vue testbar.
- Eine `appId` außerhalb der Registry ist beim Wiederherstellen ungültig (Tab
  verworfen, FR-025).

## 2. `useShellTab()` — was eine App von der Shell bekommt

Bereitgestellt per `provide`/`inject` je **Tab**. Außerhalb einer Shell-Instanz
(z. B. im Test-Harness) liefert `useShellTab()` einen inaktiven Standardwert, damit
Apps ohne Shell mountbar bleiben.

```ts
export type CloseGuardResult = {
  /** i18n-Schlüssel des Grundes, den die Bestätigung nennt (z. B. 'shell.close.activeReply'). */
  reasonKey: string
  /** Wird nach der Bestätigung des Nutzers ausgeführt und beendet, was das Schließen blockiert
      (z. B. Antwort abbrechen). Resolves, sobald der Tab gefahrlos schließen kann. */
  confirmAsync: () => Promise<void>
}
export type CloseGuard = () => CloseGuardResult | null // null = kein Nachfragen nötig

export type ShellTabApi = {
  readonly tabId: string
  readonly windowId: string
  /** Markiert den Tab als „wartet auf den Nutzer“ (Badge an Tab, Chevron-Eintrag, Fensterübersicht, Launcher, Arbeitsbereich). */
  requestAttention(): void
  clearAttention(): void
  /** Ersetzt den Tab-Titel dynamisch; `null` = zurück zum Titel der App-Definition. Nicht persistiert. */
  setTitle(title: string | null): void
  /** Meldet einen Guard an; ein neuer ersetzt den alten. Gibt eine Abmeldefunktion zurück. */
  registerCloseGuard(guard: CloseGuard): () => void
  /** Schließt diesen Tab (bei letztem Tab das Fenster), ohne die Guards zu befragen. */
  closeSelf(): void
}
```

Garantien der Shell:

- Der Tab bleibt gemountet, solange er offen ist (research R8): Minimieren,
  Maximieren, Tab-, Fokus- und Arbeitsbereichswechsel unmounten ihn nie.
- Die Shell befragt den Guard, bevor sie einen Tab schließt (Tab-Schließen,
  Fenster-Schließen mit diesem Tab, Arbeitsbereich löschen). Liefert mindestens
  ein Guard ein Ergebnis, zeigt sie **eine** Bestätigung, die die betroffenen
  Tabs und Gründe nennt; nach Bestätigung ruft sie alle `confirmAsync`, dann
  schließt sie.
- Wird ein Tab durch das Sperren/Schließen der Vault geschlossen, befragt die
  Shell keine Guards (die Vault-Session endet, FR-027); Apps müssen beim
  Unmount aufräumen.

Erwartetes Verhalten der Apps:

- Die Chat-App meldet einen Guard an, der bei laufender Antwort oder ausstehender
  Freigabe ein Ergebnis liefert; `confirmAsync` ruft die bestehende
  Abbruchfunktion (Spec 003). Sie ruft `requestAttention()`, wenn
  `onToolPermissionRequest` feuert, und `clearAttention()`, wenn die Freigabe
  beantwortet wird oder der Turn endet.
- Einstellungs- und Föderations-App melden nichts an.

## 3. Route-Vertrag

| Route                               | Verhalten                                                                                                                                                   |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `/workspace/:instance`              | Shell-Host-Seite (Middleware `onboarded` unverändert, FR-001)                                                                                               |
| `/workspace/:instance?open=<appId>` | Wie oben; die Shell öffnet die App (`openApp`), entfernt `open` per `router.replace`, lässt keinen zweiten Tab entstehen, wenn der Parameter erneut ankommt |
| `/chat/:instance`                   | Redirect auf `/workspace/:instance?open=system.chat` (FR-004)                                                                                               |
| `/settings/:instance`               | Redirect auf `/workspace/:instance?open=system.settings`                                                                                                    |
| `/federation/:instance`             | Redirect auf `/workspace/:instance?open=system.federation`                                                                                                  |
| `/`, `/onboarding/:instance`        | unverändert                                                                                                                                                 |

Ein unbekannter `open`-Wert wird ignoriert (Shell erscheint ohne zusätzliches
Fenster).

## 4. Öffentliche Aktionen der Shell (für Seiten und Shell-Komponenten)

Nicht für Apps. Der Store `useShellStore()` bietet u. a.: `openApp(appId)`,
`addTab(windowId, appId)`, `switchTab`, `closeTab`, `closeWindow`, `focusWindow`,
`minimizeWindow`, `toggleMaximizeWindow`, `moveWindowToWorkspace`,
`createWorkspace`, `deleteWorkspace`, `switchWorkspace` und `flushAsync()`. `flushAsync()` MUSS vor
`useInstance().closeAsync()` aufgerufen werden (FR-027).
