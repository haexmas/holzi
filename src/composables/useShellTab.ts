import { inject, provide, type InjectionKey } from 'vue'
import type { useShellStore } from '~/stores/shell'
import { requestConfirmation } from '~/composables/useShellCloseConfirm'
import type { ActionHandler } from '~/lib/actions/handlers'
import type { TabLocation } from '~/lib/shell/navigation'
import type { CloseGuard } from '~/lib/shell/types'

/**
 * The Shell↔App contract (spec 015-workspace-shell, T019,
 * contracts/shell-app-contract.md). Apps import nothing from
 * `components/shell/` or `stores/shell.ts` — they only ever see this.
 */
export type ShellTabApi = {
  readonly tabId: string
  readonly windowId: string
  /** The app this tab runs (spec 020: selects its route table). */
  readonly appId: string
  /** Marks the tab as "waiting on the user" (badge on the tab, Chevron entry, window overview,
   * Launcher, workspace switcher). */
  requestAttention(): void
  clearAttention(): void
  /** Overrides the tab's title dynamically; `null` reverts to the app definition's title. Never
   * persisted. */
  setTitle(title: string | null): void
  /** Registers a guard; a new one replaces the previous. Returns an unregister function. */
  registerCloseGuard(guard: CloseGuard): () => void
  /** Closes this tab (the window too, if it is the last tab) without asking any guard. */
  closeSelf(): void
  /** Opens another app, optionally at a location (spec 020 FR-012): a singleton already open is
   * activated and navigated there. */
  openApp(appId: string, at?: string | TabLocation): void
  /** Registers this tab's handler for one of its app's tab-bound actions (spec 020 research R19);
   * returns the unregister function. */
  registerActionHandler(actionId: string, handler: ActionHandler): () => void
}

const SHELL_TAB_KEY: InjectionKey<ShellTabApi> = Symbol('shellTab')

/** Inert default so an app mounts outside a Shell too (e.g. this project's Node test harnesses,
 * which never provide a real Shell instance). */
const INERT_SHELL_TAB: ShellTabApi = {
  tabId: '',
  windowId: '',
  appId: '',
  requestAttention() {},
  clearAttention() {},
  setTitle() {},
  registerCloseGuard() {
    return () => {}
  },
  closeSelf() {},
  openApp() {},
  registerActionHandler() {
    return () => {}
  },
}

/** Called by the Shell (one instance per rendered tab) to make the contract available to
 * whichever app mounts inside it. */
export function provideShellTab(api: ShellTabApi): void {
  provide(SHELL_TAB_KEY, api)
}

/** Called by an app. Outside a Shell instance, returns `INERT_SHELL_TAB` rather than throwing. */
export function useShellTab(): ShellTabApi {
  return inject(SHELL_TAB_KEY, INERT_SHELL_TAB)
}

/**
 * Shared "ask, then close" step behind every user-initiated window close (title bar, window
 * overview) — never `closeSelf()`, which the contract defines as skipping guards on purpose
 * (spec 015-workspace-shell, T031/T038, FR-014). `requestConfirmation` shows one
 * `ShellCloseConfirm.vue` dialog naming every guard's reason; declining leaves the window open.
 */
export async function requestCloseWindow(
  shell: ReturnType<typeof useShellStore>,
  windowId: string,
): Promise<boolean> {
  const results = shell.guardResultsFor(windowId)
  if (results.length > 0) {
    if (!(await requestConfirmation(results))) return false
    await Promise.all(results.map((result) => result.confirmAsync()))
  }
  shell.closeWindow(windowId)
  return true
}

/** The same "ask, then close" step for a single tab (T038) — used by `ShellTabBar.vue`'s and
 * `ShellTabListMenu.vue`'s close controls. */
export async function requestCloseTab(
  shell: ReturnType<typeof useShellStore>,
  windowId: string,
  tabId: string,
): Promise<boolean> {
  const result = shell.guardResultForTab(tabId)
  if (result) {
    if (!(await requestConfirmation([result]))) return false
    await result.confirmAsync()
  }
  shell.closeTab(windowId, tabId)
  return true
}

/** The same "ask, then delete" step for a workspace (015 FR-021, moved here from
 * `ShellWorkspaceOverview.vue` for spec 020's `shell.workspace.delete` action): deleting one with
 * open windows always confirms, naming the window count next to any guard reasons. Resolves
 * `false` when the user declines. */
export async function requestDeleteWorkspace(
  shell: ReturnType<typeof useShellStore>,
  workspaceId: string,
): Promise<boolean> {
  const windowCount = shell.windows.filter(
    (w) => w.workspaceId === workspaceId,
  ).length
  const guardResults = shell.guardResultsForWorkspace(workspaceId)
  const reasons = [
    ...(windowCount > 0
      ? [
          {
            reasonKey: 'shell.workspaces.deleteWindows',
            params: { count: windowCount },
          },
        ]
      : []),
    ...guardResults,
  ]
  if (reasons.length > 0) {
    if (!(await requestConfirmation(reasons))) return false
    await Promise.all(guardResults.map((result) => result.confirmAsync()))
  }
  shell.deleteWorkspace(workspaceId)
  return true
}
