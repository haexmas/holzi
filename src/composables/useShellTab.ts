import { inject, provide, type InjectionKey } from 'vue'
import type { useShellStore } from '~/stores/shell'
import type { CloseGuard } from '~/lib/shell/types'

/**
 * The Shell↔App contract (spec 015-workspace-shell, T019,
 * contracts/shell-app-contract.md). Apps import nothing from
 * `components/shell/` or `stores/shell.ts` — they only ever see this.
 */
export type ShellTabApi = {
  readonly tabId: string
  readonly windowId: string
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
}

const SHELL_TAB_KEY: InjectionKey<ShellTabApi> = Symbol('shellTab')

/** Inert default so an app mounts outside a Shell too (e.g. this project's Node test harnesses,
 * which never provide a real Shell instance). */
const INERT_SHELL_TAB: ShellTabApi = {
  tabId: '',
  windowId: '',
  requestAttention() {},
  clearAttention() {},
  setTitle() {},
  registerCloseGuard() {
    return () => {}
  },
  closeSelf() {},
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
 * (spec 015-workspace-shell, T031, FR-014). Placeholder confirmation until `ShellCloseConfirm.vue`
 * (T038) aggregates multiple guard reasons into one real dialog; a single `window.confirm` already
 * gives the required behavior (ask once, run every `confirmAsync`, then close) for today's
 * one-tab-per-window case.
 */
export async function requestCloseWindow(
  shell: ReturnType<typeof useShellStore>,
  t: (key: string) => string,
  windowId: string,
): Promise<void> {
  const results = shell.guardResultsFor(windowId)
  if (results.length > 0) {
    const reasons = results.map((result) => t(result.reasonKey)).join('\n')
    // Placeholder until ShellCloseConfirm.vue (T038) aggregates these into a real dialog.
    if (!confirm(reasons)) return
    await Promise.all(results.map((result) => result.confirmAsync()))
  }
  shell.closeWindow(windowId)
}
