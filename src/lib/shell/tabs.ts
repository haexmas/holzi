// Tab reducers for the Workspace-Shell (spec 015-workspace-shell, T033): insertion, singleton
// resolution, switching, and closing (neighbor activation / last-tab closes the window). Pure
// functions — no Nuxt auto-imports, relative sibling imports with an explicit .ts extension (plan
// research R6).
import type { ShellAppDefinition } from './apps.ts'
import { getAppDefinition } from './apps.ts'
import { activateExistingSingleton, closeWindow } from './layoutState.ts'
import type { ShellState } from './types.ts'

/**
 * Adds `appId` as a new tab in `windowId`, or — for a singleton app already open somewhere
 * (FR-016, same device-wide search `openApp` uses) — activates its existing tab instead (no
 * second tab is created; its window is restored/focused and its workspace activated). No-op for
 * an unresolvable `appId` (the "+" menu only ever offers known apps).
 */
export function addTab(
  state: ShellState,
  windowId: string,
  appId: string,
  apps: readonly ShellAppDefinition[] = [],
): void {
  const app = getAppDefinition(appId, apps)
  if (!app) return
  if (activateExistingSingleton(state, app, appId)) return
  const window = state.windows.find((w) => w.id === windowId)
  if (!window) return
  const tabId = crypto.randomUUID()
  window.tabs.push({ id: tabId, appId })
  window.activeTabId = tabId
}

/** Activates `tabId` within its window, if both exist and the tab belongs to that window. */
export function switchTab(
  state: ShellState,
  windowId: string,
  tabId: string,
): void {
  const window = state.windows.find((w) => w.id === windowId)
  if (!window) return
  if (!window.tabs.some((t) => t.id === tabId)) return
  window.activeTabId = tabId
}

/**
 * Removes `tabId` from its window. The last tab closes the window instead (FR-037). Otherwise, if
 * the closed tab was active, its right neighbor becomes active — or, if it was the last tab in
 * the bar, the left one (data-model.md's Tab transitions); an inactive tab's removal never changes
 * which tab is active.
 */
export function closeTab(
  state: ShellState,
  windowId: string,
  tabId: string,
): void {
  const window = state.windows.find((w) => w.id === windowId)
  if (!window) return
  const index = window.tabs.findIndex((t) => t.id === tabId)
  if (index === -1) return
  if (window.tabs.length === 1) {
    closeWindow(state, windowId)
    return
  }
  const wasActive = window.activeTabId === tabId
  window.tabs.splice(index, 1)
  if (!wasActive) return
  const neighborIndex = Math.min(index, window.tabs.length - 1)
  const neighbor = window.tabs.find((_, i) => i === neighborIndex)
  if (neighbor) window.activeTabId = neighbor.id
}
