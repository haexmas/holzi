// Window/workspace reducers and hydration for the Workspace-Shell (spec 015-workspace-shell,
// T016). Pure functions over `ShellState` — no Nuxt auto-imports, relative sibling imports with
// an explicit .ts extension (plan research R6), so this loads standalone under
// `node scripts/check-shell-state.ts`.
import type { ShellAppDefinition } from './apps.ts'
import { getAppDefinition } from './apps.ts'
import { cascadePosition, clampGeometry } from './geometry.ts'
import {
  COMPACT_MAX_WIDTH,
  type PersistedLayout,
  type ShellState,
  type ShellTab,
  type ShellWindow,
  type Size,
  type Workspace,
} from './types.ts'

/** Every open tab of `appId` across every workspace (FR-016 singleton search is device-wide, not
 * just the active workspace). Exported for `tabs.ts`'s `addTab` (T033), the same singleton search
 * `openApp` uses here. */
export function findOpenTab(
  state: ShellState,
  appId: string,
): { window: ShellWindow; tab: ShellTab } | null {
  for (const window of state.windows) {
    const tab = window.tabs.find((t) => t.appId === appId)
    if (tab) return { window, tab }
  }
  return null
}

/** The front-most window in `workspaceId` (optionally excluding minimized ones) — shared by
 * `closeWindow`/`minimizeWindow`'s "who becomes active next" step, and (T039) `deleteWorkspace`'s
 * and `moveWindowToWorkspace`'s. */
export function frontmostWindow(
  state: ShellState,
  workspaceId: string,
  visibleOnly = false,
): ShellWindow | null {
  return state.windows
    .filter(
      (w) => w.workspaceId === workspaceId && (!visibleOnly || !w.minimized),
    )
    .reduce<ShellWindow | null>(
      (front, w) => (front === null || w.stack > front.stack ? w : front),
      null,
    )
}

/** Restores (if minimized), brings to front, and activates `tab` within its window and workspace
 * — the shared "make this tab visible" step behind `openApp`'s singleton path and, later, tab
 * selection (tabs.ts, T033). */
export function focusWindow(state: ShellState, windowId: string): void {
  const window = state.windows.find((w) => w.id === windowId)
  if (!window) return
  window.minimized = false
  window.stack = ++state.nextStack
  state.activeWindowId = window.id
  state.activeWorkspaceId = window.workspaceId
}

/** Minimizes the window; if it was active, the next-front-most *visible* window in the same
 * workspace becomes active (data-model.md's Fenster transitions). */
export function minimizeWindow(state: ShellState, windowId: string): void {
  const window = state.windows.find((w) => w.id === windowId)
  if (!window) return
  window.minimized = true
  if (state.activeWindowId !== windowId) return
  state.activeWindowId =
    frontmostWindow(state, window.workspaceId, true)?.id ?? null
}

/** Toggles maximized state (FR-039) and focuses the window — the stored normal geometry is never
 * touched either way (research R7); `geometry.ts`'s `windowDisplayRect` resolves what actually
 * renders. */
export function toggleMaximizeWindow(
  state: ShellState,
  windowId: string,
): void {
  const window = state.windows.find((w) => w.id === windowId)
  if (!window) return
  window.maximized = !window.maximized
  focusWindow(state, windowId)
}

/** Applies a drag/resize result (already clamped by the caller, `useWindowPointerGesture`) to the
 * window's stored normal geometry. */
export function updateWindowGeometry(
  state: ShellState,
  windowId: string,
  geometry: { x: number; y: number; width: number; height: number },
): void {
  const window = state.windows.find((w) => w.id === windowId)
  if (!window) return
  window.x = geometry.x
  window.y = geometry.y
  window.width = geometry.width
  window.height = geometry.height
}

/** For a singleton app already open somewhere, activates its existing tab (device-wide search,
 * FR-016) and reports `true` — `openApp`'s and `tabs.ts`'s `addTab`'s shared first step. `false`
 * means the caller should proceed to create a new tab/window. */
export function activateExistingSingleton(
  state: ShellState,
  app: ShellAppDefinition,
  appId: string,
): boolean {
  if (app.multiInstance) return false
  const existing = findOpenTab(state, appId)
  if (!existing) return false
  existing.window.activeTabId = existing.tab.id
  focusWindow(state, existing.window.id)
  return true
}

/** Opens `appId` as a new window, or activates its existing tab if it is a singleton app already
 * open somewhere (FR-016). No-op for an `appId` the registry does not resolve — callers (Launcher,
 * legacy-route redirect) are expected to only ever pass a known id. */
export function openApp(
  state: ShellState,
  appId: string,
  apps: readonly ShellAppDefinition[] = [],
): void {
  const app = getAppDefinition(appId, apps)
  if (!app) return
  if (activateExistingSingleton(state, app, appId)) return
  const tabId = crypto.randomUUID()
  const openInWorkspace = state.windows.filter(
    (w) => w.workspaceId === state.activeWorkspaceId,
  ).length
  const { x, y } = cascadePosition(openInWorkspace, app.defaultSize, state.area)
  const window: ShellWindow = {
    id: crypto.randomUUID(),
    workspaceId: state.activeWorkspaceId,
    x,
    y,
    width: app.defaultSize.width,
    height: app.defaultSize.height,
    minimized: false,
    maximized: false,
    stack: ++state.nextStack,
    tabs: [{ id: tabId, appId }],
    activeTabId: tabId,
  }
  state.windows.push(window)
  state.activeWindowId = window.id
}

/** Removes the window (and its tabs) without asking anything — guard confirmation (FR-014) runs
 * at the store/UI layer before this is called (data-model.md). */
export function closeWindow(state: ShellState, windowId: string): void {
  const closed = state.windows.find((w) => w.id === windowId)
  if (!closed) return
  const wasActive = state.activeWindowId === windowId
  state.windows = state.windows.filter((w) => w.id !== windowId)
  if (!wasActive) return
  state.activeWindowId = frontmostWindow(state, closed.workspaceId)?.id ?? null
}

/** Appends a new, empty workspace at the end (FR-019). Its id is provided by the caller —
 * workspace ids are backend-assigned (research R5); until Phase 7 wires `shell_create_workspace`,
 * the store generates a temporary local one. Does not switch to it (contracts/tauri-commands.md:
 * the caller follows up with `switchWorkspace`). */
export function createWorkspace(state: ShellState, id: string): Workspace {
  const workspace: Workspace = { id, position: state.workspaces.length }
  state.workspaces.push(workspace)
  return workspace
}

/** Activates `workspaceId`, if it exists. */
export function switchWorkspace(state: ShellState, workspaceId: string): void {
  if (!state.workspaces.some((w) => w.id === workspaceId)) return
  state.activeWorkspaceId = workspaceId
}

/** Deletes a workspace and its windows/tabs (FR-021 — confirmation runs at the store/UI layer
 * first); a no-op for the last remaining workspace (I3). Positions are re-densified afterward
 * (I2). If the deleted workspace was active, its previous neighbor becomes active, or the next one
 * if it was first (data-model.md's Workspace transitions). */
export function deleteWorkspace(state: ShellState, workspaceId: string): void {
  if (state.workspaces.length <= 1) return
  const index = state.workspaces.findIndex((w) => w.id === workspaceId)
  if (index === -1) return
  state.windows = state.windows.filter((w) => w.workspaceId !== workspaceId)
  state.workspaces.splice(index, 1)
  state.workspaces.forEach((w, i) => {
    w.position = i
  })
  if (state.activeWorkspaceId !== workspaceId) return
  const neighborIndex = Math.max(0, index - 1)
  const neighbor = state.workspaces.find((_, i) => i === neighborIndex)
  state.activeWorkspaceId = neighbor?.id ?? ''
}

/** Moves a window (with all its tabs) to another workspace without touching its content (FR-020).
 * If it was the active window and it just left the active workspace, the next front-most window
 * remaining there becomes active instead (same rule as `closeWindow`/`minimizeWindow`). */
export function moveWindowToWorkspace(
  state: ShellState,
  windowId: string,
  workspaceId: string,
): void {
  const window = state.windows.find((w) => w.id === windowId)
  if (!window) return
  if (!state.workspaces.some((w) => w.id === workspaceId)) return
  if (window.workspaceId === workspaceId) return
  const wasActive = state.activeWindowId === windowId
  window.workspaceId = workspaceId
  if (!wasActive) return
  state.activeWindowId =
    frontmostWindow(state, state.activeWorkspaceId)?.id ?? null
}

/**
 * Builds a fresh `ShellState` from a persisted layout (or an empty one, before Phase 7 wires the
 * real backend call). Steps match data-model.md's "Wiederherstellen": drop tabs with an
 * unresolvable `appId` and any window left without one (FR-025); repair a dangling
 * `activeTabId`/`workspaceId`; clamp geometry into `area` (FR-026); re-derive dense `stack` from
 * the persisted order; fall back to one default workspace if none survive (FR-018, FR-025).
 */
export function hydrate(
  layout: PersistedLayout,
  apps: readonly ShellAppDefinition[],
  area: Size,
): ShellState {
  const defaultWorkspace: Workspace =
    layout.workspaces.reduce<Workspace | null>(
      (min, w) => (min === null || w.position < min.position ? w : min),
      null,
    ) ?? { id: crypto.randomUUID(), position: 0 }
  const workspaces: Workspace[] =
    layout.workspaces.length > 0
      ? [...layout.workspaces].sort((a, b) => a.position - b.position)
      : [defaultWorkspace]
  const workspaceIds = new Set(workspaces.map((w) => w.id))

  const windows: ShellWindow[] = []
  let lastWindow: ShellWindow | null = null
  let stack = 0
  for (const source of [...layout.windows].sort((a, b) => a.stack - b.stack)) {
    const tabs = source.tabs.filter(
      (tab) => getAppDefinition(tab.appId, apps) !== undefined,
    )
    const firstTab = tabs.find(() => true)
    if (!firstTab) continue
    const activeTabId = tabs.some((tab) => tab.id === source.activeTabId)
      ? source.activeTabId
      : firstTab.id
    const workspaceId = workspaceIds.has(source.workspaceId)
      ? source.workspaceId
      : defaultWorkspace.id
    const firstApp = getAppDefinition(firstTab.appId, apps)
    const geometry = clampGeometry(
      source,
      firstApp?.minSize ?? { width: 0, height: 0 },
      area,
    )
    const window: ShellWindow = {
      ...source,
      ...geometry,
      workspaceId,
      tabs,
      activeTabId,
      stack: stack++,
    }
    windows.push(window)
    lastWindow = window
  }

  const activeWorkspaceId = workspaceIds.has(layout.activeWorkspaceId)
    ? layout.activeWorkspaceId
    : defaultWorkspace.id

  return {
    workspaces,
    windows,
    activeWorkspaceId,
    activeWindowId: lastWindow?.id ?? null,
    nextStack: stack,
    area,
    compact: area.width <= COMPACT_MAX_WIDTH,
  }
}
