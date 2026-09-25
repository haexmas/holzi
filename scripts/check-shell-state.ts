// Run with `pnpm check:shell-state` (runs this file together with check-shell-geometry.ts and
// check-shell-persistence.ts). Harness for `src/lib/shell/*.ts` (spec 015-workspace-shell) — the
// `lib/shell` modules deliberately avoid Nuxt auto-imports and use relative `.ts`-suffixed sibling
// imports (plan research R6) so they load standalone here.
//
// Sections: pure reducers (layoutState.ts), tabs (tabs.ts), and multi-instance apps (User Story 7,
// T051/T052). Geometry and hydration live in check-shell-geometry.ts, the persistence queue in
// check-shell-persistence.ts (split per plan.md Complexity Tracking, T059).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import type { ShellAppDefinition } from '../src/lib/shell/apps.ts'
import type { PersistedLayout } from '../src/lib/shell/types.ts'
import {
  closeWindow,
  createWorkspace,
  deleteWorkspace,
  focusWindow,
  hydrate,
  minimizeWindow,
  moveWindowToWorkspace,
  openApp,
  switchWorkspace,
  toggleMaximizeWindow,
  updateWindowGeometry,
} from '../src/lib/shell/layoutState.ts'
import { addTab, closeTab, switchTab } from '../src/lib/shell/tabs.ts'
import { AREA, ALPHA, BETA, APPS, emptyState } from './lib/shell-fixtures.ts'

// ---------------------------------------------------------------------------
// Pure reducers (layoutState.ts)
// ---------------------------------------------------------------------------

test('openApp creates a new focused window for a singleton app not open yet', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  assert.equal(state.windows.length, 1)
  assert.equal(state.activeWindowId, state.windows[0]?.id)
  assert.equal(state.windows[0]?.tabs[0]?.appId, ALPHA.id)
})

test('openApp reactivates a singleton already open instead of creating a second window', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  const windowId = state.windows[0]?.id
  assert.ok(windowId)
  minimizeWindow(state, windowId)
  openApp(state, ALPHA.id, APPS)
  assert.equal(state.windows.length, 1, 'no second window')
  assert.equal(state.activeWindowId, windowId)
  assert.equal(state.windows[0]?.minimized, false, 'restored')
})

test('openApp is a no-op for an unresolvable appId', () => {
  const state = emptyState()
  openApp(state, 'nonexistent.app', APPS)
  assert.equal(state.windows.length, 0)
})

test('closeWindow removes the window and activates the next front-most one in its workspace', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  openApp(state, BETA.id, APPS)
  const [first, second] = state.windows
  assert.ok(first && second)
  closeWindow(state, second.id)
  assert.equal(state.windows.length, 1)
  assert.equal(state.activeWindowId, first.id)
})

test('closeWindow clears activeWindowId when the last window closes', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  const windowId = state.windows[0]?.id
  assert.ok(windowId)
  closeWindow(state, windowId)
  assert.equal(state.windows.length, 0)
  assert.equal(state.activeWindowId, null)
})

test('focusWindow restores, bumps stack ahead of the others, and activates the window and its workspace', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  openApp(state, BETA.id, APPS)
  const [first, second] = state.windows
  assert.ok(first && second)
  minimizeWindow(state, first.id)
  focusWindow(state, first.id)
  assert.equal(first.minimized, false)
  assert.ok(first.stack > second.stack)
  assert.equal(state.activeWindowId, first.id)
})

test('minimizeWindow activates the next visible front-most window in the same workspace', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  openApp(state, BETA.id, APPS)
  const [first, second] = state.windows
  assert.ok(first && second)
  minimizeWindow(state, second.id)
  assert.equal(second.minimized, true)
  assert.equal(state.activeWindowId, first.id)
})

test('toggleMaximizeWindow flips the flag without touching stored geometry, and focuses the window', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  const window = state.windows[0]
  assert.ok(window)
  const { x, y, width, height } = window
  toggleMaximizeWindow(state, window.id)
  assert.equal(window.maximized, true)
  assert.deepEqual(
    { x: window.x, y: window.y, width: window.width, height: window.height },
    { x, y, width, height },
  )
  toggleMaximizeWindow(state, window.id)
  assert.equal(window.maximized, false)
})

test('updateWindowGeometry writes the given rect directly', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  const windowId = state.windows[0]?.id
  assert.ok(windowId)
  updateWindowGeometry(state, windowId, {
    x: 12,
    y: 34,
    width: 500,
    height: 400,
  })
  const window = state.windows.find((w) => w.id === windowId)
  assert.deepEqual(
    {
      x: window?.x,
      y: window?.y,
      width: window?.width,
      height: window?.height,
    },
    { x: 12, y: 34, width: 500, height: 400 },
  )
})

test("moveWindowToWorkspace changes only the window's workspace, keeping its tabs", () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  const window = state.windows[0]
  assert.ok(window)
  const target = createWorkspace(state, 'ws-target')
  const tabsBefore = window.tabs
  moveWindowToWorkspace(state, window.id, target.id)
  assert.equal(window.workspaceId, target.id)
  assert.equal(window.tabs, tabsBefore)
})

test('moveWindowToWorkspace is a no-op for an unknown target workspace', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  const window = state.windows[0]
  assert.ok(window)
  const originalWorkspaceId = window.workspaceId
  moveWindowToWorkspace(state, window.id, 'nonexistent-workspace')
  assert.equal(window.workspaceId, originalWorkspaceId)
})

test('createWorkspace appends at a dense position without switching to it', () => {
  const state = emptyState()
  const activeBefore = state.activeWorkspaceId
  const created = createWorkspace(state, 'ws-2')
  assert.equal(created.position, 1)
  assert.equal(state.workspaces.length, 2)
  assert.equal(state.activeWorkspaceId, activeBefore)
})

test('switchWorkspace is a no-op for an unknown id', () => {
  const state = emptyState()
  const before = state.activeWorkspaceId
  switchWorkspace(state, 'nonexistent-workspace')
  assert.equal(state.activeWorkspaceId, before)
})

test('deleteWorkspace refuses to delete the last remaining workspace', () => {
  const state = emptyState()
  const onlyId = state.activeWorkspaceId
  deleteWorkspace(state, onlyId)
  assert.equal(state.workspaces.length, 1)
  assert.equal(state.workspaces[0]?.id, onlyId)
})

test('deleteWorkspace densely renumbers survivors and removes their windows', () => {
  const state = emptyState()
  const first = state.activeWorkspaceId
  const middle = createWorkspace(state, 'ws-middle')
  const last = createWorkspace(state, 'ws-last')
  switchWorkspace(state, middle.id)
  openApp(state, ALPHA.id, APPS) // a window that must disappear with `middle`
  deleteWorkspace(state, middle.id)
  assert.deepEqual(
    state.workspaces.map((w) => ({ id: w.id, position: w.position })),
    [
      { id: first, position: 0 },
      { id: last.id, position: 1 },
    ],
  )
  assert.equal(state.windows.length, 0)
})

test('deleting the active workspace activates its previous neighbor, or the next one if it was first', () => {
  // Previous-neighbor case: delete the middle (currently active) of three.
  {
    const state = emptyState()
    const first = state.activeWorkspaceId
    const middle = createWorkspace(state, 'ws-middle')
    createWorkspace(state, 'ws-last')
    switchWorkspace(state, middle.id)
    deleteWorkspace(state, middle.id)
    assert.equal(state.activeWorkspaceId, first)
  }
  // Next-neighbor case: delete the first (currently active) of two.
  {
    const state = emptyState()
    const first = state.activeWorkspaceId
    const second = createWorkspace(state, 'ws-second')
    deleteWorkspace(state, first)
    assert.equal(state.activeWorkspaceId, second.id)
  }
})

// ---------------------------------------------------------------------------
// Tabs (tabs.ts)
// ---------------------------------------------------------------------------

test('addTab appends a new tab and activates it', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  const windowId = state.windows[0]?.id
  assert.ok(windowId)
  addTab(state, windowId, BETA.id, APPS)
  const window = state.windows.find((w) => w.id === windowId)
  assert.equal(window?.tabs.length, 2)
  assert.equal(window?.activeTabId, window?.tabs[1]?.id)
})

test('addTab activates a singleton open in a different window instead of creating a new tab', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS) // window A, tab alpha
  openApp(state, BETA.id, APPS) // window B, tab beta (also becomes the active window)
  const windowA = state.windows[0]
  const windowB = state.windows[1]
  assert.ok(windowA && windowB)
  addTab(state, windowB.id, ALPHA.id, APPS)
  assert.equal(windowB.tabs.length, 1, 'no second tab added to window B')
  assert.equal(
    state.activeWindowId,
    windowA.id,
    'window A (holding the singleton) is activated instead',
  )
})

test('switchTab activates an existing tab and is a no-op for an unknown one', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  const windowId = state.windows[0]?.id
  assert.ok(windowId)
  addTab(state, windowId, BETA.id, APPS)
  const window = state.windows.find((w) => w.id === windowId)
  const firstTabId = window?.tabs[0]?.id
  assert.ok(firstTabId)
  switchTab(state, windowId, firstTabId)
  assert.equal(window?.activeTabId, firstTabId)
  switchTab(state, windowId, 'nonexistent-tab')
  assert.equal(window?.activeTabId, firstTabId, 'unchanged')
})

test('closeTab on the last tab closes the whole window', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  const window = state.windows[0]
  assert.ok(window)
  closeTab(state, window.id, window.tabs[0]!.id)
  assert.equal(state.windows.length, 0)
})

test('closing the active tab activates its right neighbor, or the left one if it was last', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  const windowId = state.windows[0]?.id
  assert.ok(windowId)
  addTab(state, windowId, BETA.id, APPS) // now [alpha, beta], beta active
  const window = state.windows.find((w) => w.id === windowId)
  const [alphaTab, betaTab] = window!.tabs
  assert.ok(alphaTab && betaTab)
  assert.equal(window?.activeTabId, betaTab.id)
  closeTab(state, windowId, betaTab.id) // closing the last (and active) tab -> falls back left
  assert.equal(window?.tabs.length, 1)
  assert.equal(window?.activeTabId, alphaTab.id)
})

test('closing an inactive tab never changes which tab is active', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  const windowId = state.windows[0]?.id
  assert.ok(windowId)
  addTab(state, windowId, BETA.id, APPS) // [alpha, beta], beta active
  const window = state.windows.find((w) => w.id === windowId)
  const [alphaTab, betaTab] = window!.tabs
  assert.ok(alphaTab && betaTab)
  closeTab(state, windowId, alphaTab.id)
  assert.equal(window?.tabs.length, 1)
  assert.equal(window?.activeTabId, betaTab.id)
})

// ---------------------------------------------------------------------------
// Multi-instance apps (User Story 7, T051/T052)
// ---------------------------------------------------------------------------

// A registry the shipped SHELL_APPS never contains (all three are singletons, FR-017) — the
// `apps` parameter every reducer takes exists so a test can substitute exactly this (apps.ts's own
// doc comment names this task).
const MULTI_APP: ShellAppDefinition = {
  id: 'extension.multi-instance-example',
  titleKey: 'test.multiInstanceExample',
  icon: 'lucide:square',
  defaultSize: { width: 400, height: 300 },
  minSize: { width: 200, height: 150 },
  multiInstance: true,
}
const MULTI_APPS: readonly ShellAppDefinition[] = [MULTI_APP]

function freshMultiState() {
  return hydrate(
    { workspaces: [], windows: [], activeWorkspaceId: '' },
    MULTI_APPS,
    AREA,
  )
}

test('openApp opens a new window every time for a multi-instance app', () => {
  const state = freshMultiState()
  openApp(state, MULTI_APP.id, MULTI_APPS)
  openApp(state, MULTI_APP.id, MULTI_APPS)
  assert.equal(state.windows.length, 2)
  assert.notEqual(state.windows[0]?.id, state.windows[1]?.id)
})

test('addTab appends a second tab in the same window instead of activating a singleton elsewhere', () => {
  const state = freshMultiState()
  openApp(state, MULTI_APP.id, MULTI_APPS)
  const windowId = state.windows[0]?.id
  assert.ok(windowId)
  addTab(state, windowId, MULTI_APP.id, MULTI_APPS)
  assert.equal(state.windows.length, 1, 'still one window, not a second one')
  const window = state.windows.find((w) => w.id === windowId)
  assert.equal(window?.tabs.length, 2)
  assert.notEqual(window?.tabs[0]?.id, window?.tabs[1]?.id)
})

test('hydrate restores multiple windows sharing the same multi-instance appId without collapsing them', () => {
  const layout: PersistedLayout = {
    workspaces: [{ id: 'ws-1', position: 0 }],
    windows: [
      {
        id: 'w1',
        workspaceId: 'ws-1',
        x: 0,
        y: 0,
        width: 400,
        height: 300,
        minimized: false,
        maximized: false,
        stack: 0,
        tabs: [{ id: 't1', appId: MULTI_APP.id }],
        activeTabId: 't1',
      },
      {
        id: 'w2',
        workspaceId: 'ws-1',
        x: 50,
        y: 50,
        width: 400,
        height: 300,
        minimized: false,
        maximized: false,
        stack: 1,
        tabs: [{ id: 't2', appId: MULTI_APP.id }],
        activeTabId: 't2',
      },
    ],
    activeWorkspaceId: 'ws-1',
  }
  const state = hydrate(layout, MULTI_APPS, AREA)
  assert.equal(state.windows.length, 2)
  assert.deepEqual(
    state.windows.map((w) => w.tabs.map((t) => t.appId)),
    [[MULTI_APP.id], [MULTI_APP.id]],
  )
})
