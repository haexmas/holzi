import { computed, reactive, toRefs } from 'vue'
import { defineStore } from 'pinia'
import { getAppDefinition, SHELL_APPS } from '~/lib/shell/apps'
import {
  closeWindow as closeWindowReducer,
  createWorkspace as createWorkspaceReducer,
  deleteWorkspace as deleteWorkspaceReducer,
  focusWindow as focusWindowReducer,
  hydrate,
  minimizeWindow as minimizeWindowReducer,
  moveWindowToWorkspace as moveWindowToWorkspaceReducer,
  openApp as openAppReducer,
  switchWorkspace as switchWorkspaceReducer,
  toggleMaximizeWindow as toggleMaximizeWindowReducer,
  updateWindowGeometry as updateWindowGeometryReducer,
} from '~/lib/shell/layoutState'
import {
  addTab as addTabReducer,
  closeTab as closeTabReducer,
  switchTab as switchTabReducer,
} from '~/lib/shell/tabs'
import type {
  CloseGuard,
  CloseGuardResult,
  ShellState,
  ShellTab,
  ShellWindow,
  TabRuntime,
} from '~/lib/shell/types'

/**
 * Shell state and its public actions (spec 015-workspace-shell, T018,
 * contracts/shell-app-contract.md §4). Wraps the pure reducers from
 * `lib/shell/layoutState.ts` around one `reactive` `ShellState`, so they can
 * keep mutating their `state` parameter in place while Vue tracks it.
 *
 * User Story 1-4's actions are implemented here: `openApp`, `addTab`,
 * `switchTab`, `focusWindow`, `closeWindow`, `closeTab`, `minimizeWindow`,
 * `toggleMaximizeWindow`, `updateWindowGeometry`, `createWorkspace`,
 * `deleteWorkspace`, `switchWorkspace`, `moveWindowToWorkspace`, and a
 * `flushAsync` placeholder (FR-027) `ChatApp.vue`'s `lock()` already
 * depends on.
 *
 * Persistence does not exist yet (Phase 7): `state` starts from an empty
 * layout, and `flushAsync` is a no-op until T047 wires the real write queue.
 */
export const useShellStore = defineStore('shell', () => {
  const initialArea =
    typeof window === 'undefined'
      ? { width: 1280, height: 800 }
      : { width: window.innerWidth, height: window.innerHeight }
  const state = reactive<ShellState>(
    hydrate(
      { workspaces: [], windows: [], activeWorkspaceId: '' },
      SHELL_APPS,
      initialArea,
    ),
  )

  /** Never persisted (data-model.md); per-tab bookkeeping keyed by tab id, kept in sync with
   * `state.windows[*].tabs` after every action that can add or remove a tab. A `Map` rather than
   * a plain object, so pruning a stale entry needs no dynamic-key `delete`. */
  const tabRuntime = reactive(new Map<string, TabRuntime>())

  function syncTabRuntime() {
    const liveIds = new Set(
      state.windows.flatMap((w) => w.tabs.map((t) => t.id)),
    )
    for (const id of tabRuntime.keys()) {
      if (!liveIds.has(id)) tabRuntime.delete(id)
    }
    for (const id of liveIds) {
      if (!tabRuntime.has(id)) {
        tabRuntime.set(id, {
          attention: false,
          titleOverride: null,
          guard: null,
          mounted: false,
        })
      }
    }
  }
  syncTabRuntime()

  const windowsInActiveWorkspace = computed(() =>
    state.windows.filter((w) => w.workspaceId === state.activeWorkspaceId),
  )

  function runtimeFor(tabId: string): TabRuntime {
    return (
      tabRuntime.get(tabId) ?? {
        attention: false,
        titleOverride: null,
        guard: null,
        mounted: false,
      }
    )
  }

  /** Marks a tab's content mounted for this session (research R8: it then stays mounted, `v-show`
   * hides it instead of unmounting). Called by the Shell window the first time a tab becomes
   * visible (T031). */
  function markTabMounted(tabId: string) {
    const runtime = tabRuntime.get(tabId)
    if (runtime) runtime.mounted = true
  }

  /** The three `useShellTab()` setters (contracts/shell-app-contract.md) — a no-op for a tab id
   * `syncTabRuntime` has already pruned (the tab closed while an app's own async work was still
   * settling). */
  function setTabAttention(tabId: string, attention: boolean) {
    const runtime = tabRuntime.get(tabId)
    if (runtime) runtime.attention = attention
  }

  function setTabTitle(tabId: string, title: string | null) {
    const runtime = tabRuntime.get(tabId)
    if (runtime) runtime.titleOverride = title
  }

  function setTabCloseGuard(tabId: string, guard: CloseGuard | null) {
    const runtime = tabRuntime.get(tabId)
    if (runtime) runtime.guard = guard
  }

  /** The active tab of a window, falling back to its first tab if `activeTabId` is somehow stale
   * (should not happen post-hydrate, but callers should not assume it never will). */
  function activeTabOf(window: ShellWindow) {
    return (
      window.tabs.find((t) => t.id === window.activeTabId) ?? window.tabs[0]
    )
  }

  /** Raw display data for one tab — icon/titleKey from its app, any title override, and its own
   * attention flag. Callers translate `titleKey` themselves; this store does not depend on
   * `useI18n()`. Shared by `ShellTabBar.vue`/`ShellTabListMenu.vue` (T034/T036) and
   * `windowDisplayInfo` below. */
  function tabDisplayInfo(tab: ShellTab): {
    titleKey: string | undefined
    icon: string | undefined
    titleOverride: string | null
    hasAttention: boolean
  } {
    const app = getAppDefinition(tab.appId, SHELL_APPS)
    const runtime = runtimeFor(tab.id)
    return {
      titleKey: app?.titleKey,
      icon: app?.icon,
      titleOverride: runtime.titleOverride,
      hasAttention: runtime.attention,
    }
  }

  /** Raw display data for a window — its active tab's `tabDisplayInfo`, its tab count, and
   * whether *any* of its tabs wants attention (data-model.md's derived "window has attention"
   * rule — a window can have attention from a background tab even while its active tab does not).
   * Shared by `ShellWindow.vue` and `ShellWindowOverview.vue` (T030). */
  function windowDisplayInfo(window: ShellWindow): {
    titleKey: string | undefined
    icon: string | undefined
    titleOverride: string | null
    tabCount: number
    hasAttention: boolean
  } | null {
    const tab = activeTabOf(window)
    if (!tab) return null
    return {
      ...tabDisplayInfo(tab),
      tabCount: window.tabs.length,
      hasAttention: window.tabs.some((t) => runtimeFor(t.id).attention),
    }
  }

  function openApp(appId: string) {
    openAppReducer(state, appId, SHELL_APPS)
    syncTabRuntime()
  }

  function addTab(windowId: string, appId: string) {
    addTabReducer(state, windowId, appId, SHELL_APPS)
    syncTabRuntime()
  }

  function switchTab(windowId: string, tabId: string) {
    switchTabReducer(state, windowId, tabId)
  }

  /** Removes the tab without asking anything — guard confirmation (FR-014) runs at the caller,
   * same as `closeWindow`. */
  function closeTab(windowId: string, tabId: string) {
    closeTabReducer(state, windowId, tabId)
    syncTabRuntime()
  }

  function focusWindow(windowId: string) {
    focusWindowReducer(state, windowId)
  }

  function minimizeWindow(windowId: string) {
    minimizeWindowReducer(state, windowId)
  }

  function toggleMaximizeWindow(windowId: string) {
    toggleMaximizeWindowReducer(state, windowId)
  }

  function updateWindowGeometry(
    windowId: string,
    geometry: { x: number; y: number; width: number; height: number },
  ) {
    updateWindowGeometryReducer(state, windowId, geometry)
  }

  /** Removes the window without asking anything — guard confirmation (FR-014) runs at the caller
   * (`useShellTab.ts`'s `requestCloseWindow`, T031; `ShellCloseConfirm.vue`, T038) before this is
   * invoked. */
  function closeWindow(windowId: string) {
    closeWindowReducer(state, windowId)
    syncTabRuntime()
  }

  function createWorkspace() {
    return createWorkspaceReducer(state, crypto.randomUUID())
  }

  function switchWorkspace(workspaceId: string) {
    switchWorkspaceReducer(state, workspaceId)
  }

  /** Deletes the workspace and its windows/tabs — guard confirmation for any running replies
   * (FR-021, same shape as `closeWindow`'s) and the "close N windows" confirmation both run at the
   * caller (`ShellWorkspaceOverview.vue`, T040) before this is invoked. A no-op for the last
   * remaining workspace (I3). */
  function deleteWorkspace(workspaceId: string) {
    deleteWorkspaceReducer(state, workspaceId)
    syncTabRuntime()
  }

  function moveWindowToWorkspace(windowId: string, workspaceId: string) {
    moveWindowToWorkspaceReducer(state, windowId, workspaceId)
  }

  /** Whether *any* window in the workspace has attention (data-model.md's derived rule, mirroring
   * `windowDisplayInfo`'s own window-level derivation one layer up). */
  function workspaceHasAttention(workspaceId: string): boolean {
    return state.windows.some(
      (w) =>
        w.workspaceId === workspaceId && windowDisplayInfo(w)?.hasAttention,
    )
  }

  /** Every non-null close-guard result across the window's tabs (FR-014, for closing the whole
   * window) — a tab without a registered guard, or whose guard currently allows closing,
   * contributes nothing. */
  function guardResultsFor(windowId: string): CloseGuardResult[] {
    const window = state.windows.find((w) => w.id === windowId)
    if (!window) return []
    const results: CloseGuardResult[] = []
    for (const tab of window.tabs) {
      const result = tabRuntime.get(tab.id)?.guard?.()
      if (result) results.push(result)
    }
    return results
  }

  /** One tab's own close-guard result (FR-014, for closing just that tab) — `null` if it has none
   * registered or its guard currently allows closing. */
  function guardResultForTab(tabId: string): CloseGuardResult | null {
    return tabRuntime.get(tabId)?.guard?.() ?? null
  }

  /** Placeholder until Phase 7 (T047) wires the real serialized write queue; `ChatApp.vue`'s
   * `lock()` already depends on awaiting it before `useInstance().closeAsync()` (FR-027). */
  async function flushAsync(): Promise<void> {}

  return {
    ...toRefs(state),
    windowsInActiveWorkspace,
    runtimeFor,
    tabDisplayInfo,
    windowDisplayInfo,
    markTabMounted,
    setTabAttention,
    setTabTitle,
    setTabCloseGuard,
    openApp,
    addTab,
    switchTab,
    focusWindow,
    minimizeWindow,
    toggleMaximizeWindow,
    updateWindowGeometry,
    closeWindow,
    closeTab,
    createWorkspace,
    switchWorkspace,
    deleteWorkspace,
    moveWindowToWorkspace,
    workspaceHasAttention,
    guardResultsFor,
    guardResultForTab,
    flushAsync,
  }
})
