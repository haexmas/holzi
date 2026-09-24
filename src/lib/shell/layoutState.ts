// Window/workspace reducers and hydration for the Workspace-Shell (spec 015-workspace-shell,
// T016). Pure functions over `ShellState` — no Nuxt auto-imports, relative sibling imports with
// an explicit .ts extension (plan research R6), so this loads standalone under
// `node scripts/check-shell-state.ts`.
import type { ShellAppDefinition } from './apps.ts'
import { getAppDefinition } from './apps.ts'
import {
  COMPACT_MAX_WIDTH,
  type PersistedLayout,
  type ShellState,
  type ShellTab,
  type ShellWindow,
  type Size,
  type Workspace,
} from './types.ts'

/** New windows cascade by this offset per step (research R12/haex-vault reference), wrapping once
 * they would run off the visible area. */
const CASCADE_STEP = 24
const CASCADE_WRAP_MARGIN = 160

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), Math.max(min, max))
}

/** Full-clamp for restored geometry (FR-026): the window must end up entirely inside `area`. This
 * is deliberately simpler than the drag-time "keep a 64x32 title-bar strip visible" rule
 * (geometry.ts, T027) — hydration has no previous on-screen position to preserve partially. */
function clampToArea(
  window: Pick<ShellWindow, 'x' | 'y' | 'width' | 'height'>,
  minSize: Size,
  area: Size,
): Pick<ShellWindow, 'x' | 'y' | 'width' | 'height'> {
  const width = clamp(
    window.width,
    minSize.width,
    Math.max(area.width, minSize.width),
  )
  const height = clamp(
    window.height,
    minSize.height,
    Math.max(area.height, minSize.height),
  )
  const x = clamp(window.x, 0, Math.max(area.width - width, 0))
  const y = clamp(window.y, 0, Math.max(area.height - height, 0))
  return { x, y, width, height }
}

/** Where the store keeps the new window if `openApp` does not activate an existing singleton. */
function cascadePosition(
  state: ShellState,
  defaultSize: Size,
): { x: number; y: number } {
  const openInWorkspace = state.windows.filter(
    (w) => w.workspaceId === state.activeWorkspaceId,
  ).length
  const offset = (openInWorkspace * CASCADE_STEP) % CASCADE_WRAP_MARGIN
  return {
    x: clamp(offset, 0, Math.max(state.area.width - defaultSize.width, 0)),
    y: clamp(offset, 0, Math.max(state.area.height - defaultSize.height, 0)),
  }
}

/** Every open tab of `appId` across every workspace (FR-016 singleton search is device-wide, not
 * just the active workspace). */
function findOpenTab(
  state: ShellState,
  appId: string,
): { window: ShellWindow; tab: ShellTab } | null {
  for (const window of state.windows) {
    const tab = window.tabs.find((t) => t.appId === appId)
    if (tab) return { window, tab }
  }
  return null
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
  if (!app.multiInstance) {
    const existing = findOpenTab(state, appId)
    if (existing) {
      existing.window.activeTabId = existing.tab.id
      focusWindow(state, existing.window.id)
      return
    }
  }
  const tabId = crypto.randomUUID()
  const { x, y } = cascadePosition(state, app.defaultSize)
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
  const nextInWorkspace = state.windows
    .filter((w) => w.workspaceId === closed.workspaceId)
    .reduce<ShellWindow | null>(
      (front, w) => (front === null || w.stack > front.stack ? w : front),
      null,
    )
  state.activeWindowId = nextInWorkspace?.id ?? null
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
    const geometry = clampToArea(
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
