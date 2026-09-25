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
  switchWorkspace as switchWorkspaceReducer,
  toggleMaximizeWindow as toggleMaximizeWindowReducer,
  updateArea as updateAreaReducer,
  updateWindowGeometry as updateWindowGeometryReducer,
} from '~/lib/shell/layoutState'
import {
  closeTab as closeTabReducer,
  switchTab as switchTabReducer,
} from '~/lib/shell/tabs'
import type { TabLocation } from '~/lib/shell/navigation'
import { addTabAt, openAppAt } from '~/lib/shell/tabNavigation'
import type {
  CloseGuard,
  PersistedLayout,
  ShellState,
  ShellTab,
  ShellWindow,
  Size,
  TabRuntime,
  Workspace,
} from '~/lib/shell/types'
import { useShellLayout, type WindowDto } from '~/composables/useShellLayout'
import { createShellGuards } from '~/stores/shellGuards'
import { createShellNavigation } from '~/stores/shellNavigation'

/**
 * Shell state and its public actions (spec 015-workspace-shell, T018,
 * contracts/shell-app-contract.md §4). Wraps the pure reducers from
 * `lib/shell/layoutState.ts` around one `reactive` `ShellState`, so they can
 * keep mutating their `state` parameter in place while Vue tracks it.
 *
 * User Story 1-4's actions are implemented here: `openApp`, `addTab`,
 * `switchTab`, `focusWindow`, `closeWindow`, `closeTab`, `minimizeWindow`,
 * `toggleMaximizeWindow`, `updateWindowGeometry`, `createWorkspace`,
 * `deleteWorkspace`, `switchWorkspace`, `moveWindowToWorkspace`.
 *
 * Persistence (T048): `state` starts from an empty local layout — same as
 * before any backend round trip existed — so the store is usable
 * synchronously at setup. `hydrateFromBackendAsync` (called once by
 * `pages/workspace/[instance].vue`, before anything else touches the
 * store) replaces it in place with the real persisted layout via
 * `Object.assign`, which keeps `state`'s identity so every `toRefs`/
 * `computed` consumer already holding a reference keeps tracking it.
 * Every mutating action below also tells `useShellLayout()` what to
 * persist: immediately for structural changes (open, add/switch/close tab,
 * close window, move to another workspace, minimize/maximize) and
 * debounced for geometry and stack (focus) changes — research.md R13.
 * Workspace actions use their own commands, not `shell_save_windows`.
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

  const shellLayout = useShellLayout()

  /** Converts the in-memory window and ordered tabs to the backend wire shape. */
  function windowToDto(window: ShellWindow): WindowDto {
    return {
      windowId: window.id,
      workspaceId: window.workspaceId,
      x: window.x,
      y: window.y,
      width: window.width,
      height: window.height,
      isMinimized: window.minimized,
      isMaximized: window.maximized,
      stackOrder: window.stack,
      activeTabId: window.activeTabId,
      tabs: window.tabs.map((tab) => ({ tabId: tab.id, appId: tab.appId })),
    }
  }

  /** Queues an immediate save when the window still exists. */
  function persistWindowNow(windowId: string) {
    const window = state.windows.find((w) => w.id === windowId)
    if (window) shellLayout.saveWindowNow(windowToDto(window))
  }

  /** Coalesces frequent changes to a window's geometry or stack position. */
  function persistWindowDebounced(windowId: string) {
    const window = state.windows.find((w) => w.id === windowId)
    if (window) shellLayout.saveWindowDebounced(windowToDto(window))
  }

  /** Loads the device's real layout and replaces `state` in place (research
   * data-model.md "Wiederherstellen"; unknown-app filtering and geometry
   * clamping already live in `hydrate` itself, so this only has to feed it
   * real data). Called once, before anything else touches the store —
   * `pages/workspace/[instance].vue`'s `onMounted`, ahead of its own
   * `?open=` handling, so a window never opens into the throwaway
   * pre-hydration workspace `hydrate({ workspaces: [], ... })` mints. */
  async function hydrateFromBackendAsync(): Promise<void> {
    const dto = await shellLayout.loadLayout()
    const layout: PersistedLayout = {
      workspaces: dto.workspaces.map((w) => ({
        id: w.workspaceId,
        position: w.position,
      })),
      windows: dto.windows.map((w) => ({
        id: w.windowId,
        workspaceId: w.workspaceId,
        x: w.x,
        y: w.y,
        width: w.width,
        height: w.height,
        minimized: w.isMinimized,
        maximized: w.isMaximized,
        stack: w.stackOrder,
        tabs: w.tabs.map((tab) => ({ id: tab.tabId, appId: tab.appId })),
        activeTabId: w.activeTabId,
      })),
      activeWorkspaceId: dto.activeWorkspaceId,
    }
    Object.assign(state, hydrate(layout, SHELL_APPS, state.area))
    navigation.histories.clear()
    syncTabRuntime()
  }

  /** Never persisted (data-model.md); per-tab bookkeeping keyed by tab id, kept in sync with
   * `state.windows[*].tabs` after every action that can add or remove a tab. A `Map` rather than
   * a plain object, so pruning a stale entry needs no dynamic-key `delete`. */
  const tabRuntime = reactive(new Map<string, TabRuntime>())

  // Per-tab history and the action runner (spec 020-tab-navigation, stores/shellNavigation.ts).
  const navigation = createShellNavigation({
    state,
    titleOverrideOf: (tabId) => tabRuntime.get(tabId)?.titleOverride ?? null,
    clearTitleOverride: (tabId) => setTabTitle(tabId, null),
    openApp: (appId) => openApp(appId),
  })

  /** Retains runtime state for live tabs and initializes newly opened tabs. */
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
    navigation.syncNavigation()
  }
  syncTabRuntime()

  const windowsInActiveWorkspace = computed(() =>
    state.windows.filter((w) => w.workspaceId === state.activeWorkspaceId),
  )

  /** Returns inert defaults if the tab has already closed. */
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

  /** Overrides a live tab's displayed title without changing its app definition. */
  function setTabTitle(tabId: string, title: string | null) {
    const runtime = tabRuntime.get(tabId)
    if (runtime) runtime.titleOverride = title
  }

  /** Registers or clears the close guard for a live tab. */
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

  /** Opens the app, or activates its existing singleton tab (`state.activeWindowId` either way) —
   * persisted immediately either way: even reactivating a singleton is a deliberate click, not a
   * continuous gesture (research.md R13's structural bucket is everything but geometry/stack). */
  function openApp(appId: string, at: string | TabLocation | null = null) {
    const { tabId } = openAppAt(
      state,
      navigation.histories,
      appId,
      at,
      SHELL_APPS,
      (id) => runtimeFor(id).titleOverride,
    )
    syncTabRuntime()
    if (state.activeWindowId) persistWindowNow(state.activeWindowId)
    return tabId
  }

  /** Adds a tab (at `at`, spec 020 FR-012) and persists both the target and any newly active
   * window. */
  function addTab(
    windowId: string,
    appId: string,
    at: string | TabLocation | null = null,
  ) {
    const { tabId } = addTabAt(
      state,
      navigation.histories,
      windowId,
      appId,
      at,
      SHELL_APPS,
      (id) => runtimeFor(id).titleOverride,
    )
    syncTabRuntime()
    persistWindowNow(windowId)
    if (state.activeWindowId && state.activeWindowId !== windowId)
      persistWindowNow(state.activeWindowId)
    return tabId
  }

  /** Activates a tab and saves its window's new active tab id. */
  function switchTab(windowId: string, tabId: string) {
    switchTabReducer(state, windowId, tabId)
    persistWindowNow(windowId)
  }

  /** Removes the tab without asking anything — guard confirmation (FR-014) runs at the caller,
   * same as `closeWindow`. Persists via `shell_close_windows` if the window closed with it (it was
   * the last tab), otherwise via a save carrying the window's remaining tabs. */
  function closeTab(windowId: string, tabId: string) {
    closeTabReducer(state, windowId, tabId)
    syncTabRuntime()
    if (state.windows.some((w) => w.id === windowId)) persistWindowNow(windowId)
    else shellLayout.closeWindowNow(windowId)
  }

  /** Restores and raises a window, debouncing its new stack position. */
  function focusWindow(windowId: string) {
    focusWindowReducer(state, windowId)
    persistWindowDebounced(windowId)
  }

  /** Minimizes a window and saves the structural change immediately. */
  function minimizeWindow(windowId: string) {
    minimizeWindowReducer(state, windowId)
    persistWindowNow(windowId)
  }

  /** Saves the window's maximized or restored state immediately. */
  function toggleMaximizeWindow(windowId: string) {
    toggleMaximizeWindowReducer(state, windowId)
    persistWindowNow(windowId)
  }

  /** Applies a drag or resize result and debounces the resulting save. */
  function updateWindowGeometry(
    windowId: string,
    geometry: { x: number; y: number; width: number; height: number },
  ) {
    updateWindowGeometryReducer(state, windowId, geometry)
    persistWindowDebounced(windowId)
  }

  /** Keeps `state.area`/`state.compact` in sync with the Shell's actual size (T049,
   * `ShellDesktop.vue`'s `useWindowSize` watcher) and re-clamps every window's stored geometry
   * into it — debounced like `updateWindowGeometry`, since a live resize can fire rapidly too. */
  function updateArea(area: Size) {
    const changedWindowIds = updateAreaReducer(state, area, SHELL_APPS)
    for (const windowId of changedWindowIds) persistWindowDebounced(windowId)
  }

  /** Removes the window without asking anything — guard confirmation (FR-014) runs at the caller
   * (`useShellTab.ts`'s `requestCloseWindow`, T031; `ShellCloseConfirm.vue`, T038) before this is
   * invoked. */
  function closeWindow(windowId: string) {
    closeWindowReducer(state, windowId)
    syncTabRuntime()
    shellLayout.closeWindowNow(windowId)
  }

  /** Creates the workspace on the backend first — `workspace_id` is backend-assigned
   * (data-model.md) — then applies the identical id locally, so the two never diverge. */
  async function createWorkspace(): Promise<Workspace> {
    const dto = await shellLayout.createWorkspace()
    return createWorkspaceReducer(state, dto.workspaceId)
  }

  /** Activates a workspace locally and queues its device preference write. */
  function switchWorkspace(workspaceId: string) {
    switchWorkspaceReducer(state, workspaceId)
    void shellLayout.setActiveWorkspace(workspaceId).catch((error: unknown) => {
      console.error('[shell] shell_set_active_workspace failed', error)
    })
  }

  /** Deletes the workspace and its windows/tabs locally for instant feedback — guard confirmation
   * for any running replies (FR-021, same shape as `closeWindow`'s) and the "close N windows"
   * confirmation both run at the caller (`ShellWorkspaceOverview.vue`, T040) before this is
   * invoked. A no-op for the last remaining workspace (I3). The backend call runs the identical
   * dense-renumber/neighbor-activation algorithm (`shell_commands.rs`'s `neighbor_after_delete`),
   * so it only needs to persist the outcome, not correct it. Explicitly closes each of its windows
   * first (redundant with the backend's own FK cascade, but idempotent) so none of them can be
   * left dirty in `useShellLayout`'s retry set forever — a workspace this deletes no longer exists
   * for `shell_save_windows` to validate a stale pending save against. */
  function deleteWorkspace(workspaceId: string) {
    const removedWindowIds = state.windows
      .filter((w) => w.workspaceId === workspaceId)
      .map((w) => w.id)
    deleteWorkspaceReducer(state, workspaceId)
    syncTabRuntime()
    for (const windowId of removedWindowIds)
      shellLayout.closeWindowNow(windowId)
    void shellLayout.deleteWorkspace(workspaceId).catch((error: unknown) => {
      console.error('[shell] shell_delete_workspace failed', error)
    })
  }

  /** Moves a window with its tabs and immediately saves its new workspace id. */
  function moveWindowToWorkspace(windowId: string, workspaceId: string) {
    moveWindowToWorkspaceReducer(state, windowId, workspaceId)
    persistWindowNow(windowId)
  }

  /** Whether *any* window in the workspace has attention (data-model.md's derived rule, mirroring
   * `windowDisplayInfo`'s own window-level derivation one layer up). */
  function workspaceHasAttention(workspaceId: string): boolean {
    return state.windows.some(
      (w) =>
        w.workspaceId === workspaceId && windowDisplayInfo(w)?.hasAttention,
    )
  }

  /** Whether any open tab of this app (any window, any workspace) wants attention (FR-015: the
   * Launcher is one of the surfaces a background window's attention must reach). */
  function appHasAttention(appId: string): boolean {
    return state.windows.some((w) =>
      w.tabs.some((tab) => tab.appId === appId && runtimeFor(tab.id).attention),
    )
  }

  // Close-guard queries for FR-014/FR-021 (stores/shellGuards.ts).
  const { guardResultsFor, guardResultForTab, guardResultsForWorkspace } =
    createShellGuards(state, (tabId) => tabRuntime.get(tabId)?.guard)

  /** Awaits the persistence queue before the vault locks or closes (FR-027) — `ChatApp.vue`'s and
   * `FederationApp.vue`'s `lock()` already call this before `useInstance().closeAsync()`. */
  function flushAsync(): Promise<void> {
    return shellLayout.flushAsync()
  }

  return {
    ...toRefs(state),
    windowsInActiveWorkspace,
    hydrateFromBackendAsync,
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
    updateArea,
    closeWindow,
    closeTab,
    createWorkspace,
    switchWorkspace,
    deleteWorkspace,
    moveWindowToWorkspace,
    workspaceHasAttention,
    appHasAttention,
    guardResultsFor,
    guardResultForTab,
    guardResultsForWorkspace,
    flushAsync,
    historyOf: navigation.historyOf,
    navigate: navigation.navigate,
    goTab: navigation.go,
    runAction: navigation.runAction,
    registerGlobalActionHandler: navigation.registerGlobalActionHandler,
    registerTabActionHandler: navigation.registerTabActionHandler,
  }
})
