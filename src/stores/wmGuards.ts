import type { CloseGuard, CloseGuardResult, WmState } from '~/lib/wm/types'

/**
 * Close-guard queries of the window manager store (spec 015-workspace-shell, FR-014,
 * FR-021), moved out of `stores/windowManager.ts` unchanged to keep it below the
 * 500-line limit when spec 020 added navigation there.
 */
export function createWmGuards(
  state: WmState,
  guardOf: (tabId: string) => CloseGuard | null | undefined,
) {
  /** Every non-null close-guard result across the window's tabs (FR-014, for closing the whole
   * window) — a tab without a registered guard, or whose guard currently allows closing,
   * contributes nothing. */
  function guardResultsFor(windowId: string): CloseGuardResult[] {
    const window = state.windows.find((w) => w.id === windowId)
    if (!window) return []
    const results: CloseGuardResult[] = []
    for (const tab of window.tabs) {
      const result = guardOf(tab.id)?.()
      if (result) results.push(result)
    }
    return results
  }

  /** One tab's own close-guard result (FR-014, for closing just that tab) — `null` if it has none
   * registered or its guard currently allows closing. */
  function guardResultForTab(tabId: string): CloseGuardResult | null {
    return guardOf(tabId)?.() ?? null
  }

  /** Every non-null close-guard result across every window in the workspace (FR-021, for deleting
   * it) — `guardResultsFor` extended to the whole workspace. */
  function guardResultsForWorkspace(workspaceId: string): CloseGuardResult[] {
    const results: CloseGuardResult[] = []
    for (const window of state.windows) {
      if (window.workspaceId === workspaceId)
        results.push(...guardResultsFor(window.id))
    }
    return results
  }

  return { guardResultsFor, guardResultForTab, guardResultsForWorkspace }
}
