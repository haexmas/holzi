import { computed, reactive, toRefs } from 'vue'
import { defineStore } from 'pinia'
import { getAppDefinition, SHELL_APPS } from '~/lib/shell/apps'
import {
  closeWindow as closeWindowReducer,
  focusWindow as focusWindowReducer,
  hydrate,
  minimizeWindow as minimizeWindowReducer,
  openApp as openAppReducer,
  toggleMaximizeWindow as toggleMaximizeWindowReducer,
  updateWindowGeometry as updateWindowGeometryReducer,
} from '~/lib/shell/layoutState'
import type {
  CloseGuard,
  CloseGuardResult,
  ShellState,
  ShellWindow,
  TabRuntime,
} from '~/lib/shell/types'

/**
 * Shell state and its public actions (spec 015-workspace-shell, T018,
 * contracts/shell-app-contract.md §4). Wraps the pure reducers from
 * `lib/shell/layoutState.ts` around one `reactive` `ShellState`, so they can
 * keep mutating their `state` parameter in place while Vue tracks it.
 *
 * User Story 1 and 2's actions are implemented here: `openApp`,
 * `focusWindow`, `closeWindow`, `minimizeWindow`, `toggleMaximizeWindow`,
 * `updateWindowGeometry`, and a `flushAsync` placeholder (FR-027)
 * `ChatApp.vue`'s `lock()` already depends on. Tab actions (`addTab`,
 * `switchTab`, `closeTab`) and workspace actions (`createWorkspace`,
 * `deleteWorkspace`, `switchWorkspace`, `moveWindowToWorkspace`) land in
 * this same store as their own user stories (Phase 5-6) build them.
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

  /** Raw display data for a window — icon/titleKey from its active tab's app, any title override,
   * and whether *any* of its tabs wants attention (data-model.md's derived "window has attention"
   * rule). Callers translate `titleKey` themselves; this store does not depend on `useI18n()`.
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
    const app = getAppDefinition(tab.appId, SHELL_APPS)
    return {
      titleKey: app?.titleKey,
      icon: app?.icon,
      titleOverride: runtimeFor(tab.id).titleOverride,
      tabCount: window.tabs.length,
      hasAttention: window.tabs.some((t) => runtimeFor(t.id).attention),
    }
  }

  function openApp(appId: string) {
    openAppReducer(state, appId, SHELL_APPS)
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

  /** Every non-null close-guard result across the window's tabs (FR-014) — a tab without a
   * registered guard, or whose guard currently allows closing, contributes nothing. */
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

  /** Placeholder until Phase 7 (T047) wires the real serialized write queue; `ChatApp.vue`'s
   * `lock()` already depends on awaiting it before `useInstance().closeAsync()` (FR-027). */
  async function flushAsync(): Promise<void> {}

  return {
    ...toRefs(state),
    windowsInActiveWorkspace,
    runtimeFor,
    windowDisplayInfo,
    markTabMounted,
    setTabAttention,
    setTabTitle,
    setTabCloseGuard,
    openApp,
    focusWindow,
    minimizeWindow,
    toggleMaximizeWindow,
    updateWindowGeometry,
    closeWindow,
    guardResultsFor,
    flushAsync,
  }
})
