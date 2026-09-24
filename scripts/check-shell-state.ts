// Run with `node scripts/check-shell-state.ts`. Harness for `src/lib/shell/*.ts` and
// `useShellLayout.ts` (spec 015-workspace-shell) — the `lib/shell` modules deliberately avoid
// Nuxt auto-imports and use relative `.ts`-suffixed sibling imports (plan research R6) so they
// load standalone here, the same way `check-chat-state.ts` replays `ChatApp.vue`'s script block;
// `useShellLayout.ts` takes an injectable `invokeFn` (defaulting to the real Tauri `invoke`)
// specifically so its queue/debounce/dirty-retry logic can run here too, without a Tauri runtime
// or module mocking.
//
// Sections: pure reducers (layoutState.ts), geometry (geometry.ts), tabs (tabs.ts), hydration
// (layoutState.ts's `hydrate`), the persistence queue (useShellLayout.ts), and multi-instance
// apps (User Story 7, T051/T052 — the first scenario written, kept last here).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import type { ShellAppDefinition } from '../src/lib/shell/apps.ts'
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
import {
  cascadePosition,
  clampDragPosition,
  clampGeometry,
  clampResizeSize,
  MIN_VISIBLE_TITLEBAR,
  windowDisplayRect,
} from '../src/lib/shell/geometry.ts'
import { addTab, closeTab, switchTab } from '../src/lib/shell/tabs.ts'
import type {
  PersistedLayout,
  ShellState,
  Size,
} from '../src/lib/shell/types.ts'
import {
  useShellLayout,
  type ShellLayoutDto,
  type WindowDto,
} from '../src/composables/useShellLayout.ts'

const AREA: Size = { width: 1920, height: 1080 }

// Two singletons, distinct minSizes, for the general reducer/geometry/tabs/hydration sections.
const ALPHA: ShellAppDefinition = {
  id: 'test.alpha',
  titleKey: 'test.alpha',
  icon: 'lucide:circle',
  defaultSize: { width: 600, height: 400 },
  minSize: { width: 300, height: 200 },
  multiInstance: false,
}
const BETA: ShellAppDefinition = {
  id: 'test.beta',
  titleKey: 'test.beta',
  icon: 'lucide:triangle',
  defaultSize: { width: 500, height: 350 },
  minSize: { width: 250, height: 180 },
  multiInstance: false,
}
const APPS: readonly ShellAppDefinition[] = [ALPHA, BETA]

function emptyState(): ShellState {
  return hydrate(
    { workspaces: [], windows: [], activeWorkspaceId: '' },
    APPS,
    AREA,
  )
}

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
// Geometry (geometry.ts)
// ---------------------------------------------------------------------------

test('clampGeometry keeps a rect that already fits untouched', () => {
  const rect = { x: 100, y: 100, width: 400, height: 300 }
  assert.deepEqual(clampGeometry(rect, { width: 200, height: 150 }, AREA), rect)
})

test('clampGeometry grows a size below minSize and repositions to stay fully inside area', () => {
  const clamped = clampGeometry(
    { x: 1800, y: 1000, width: 100, height: 80 },
    { width: 300, height: 200 },
    AREA,
  )
  assert.equal(clamped.width, 300)
  assert.equal(clamped.height, 200)
  assert.ok(clamped.x + clamped.width <= AREA.width)
  assert.ok(clamped.y + clamped.height <= AREA.height)
})

test('clampResizeSize only enforces the minimum, never touching position', () => {
  assert.deepEqual(
    clampResizeSize({ width: 100, height: 80 }, { width: 300, height: 200 }),
    { width: 300, height: 200 },
  )
  assert.deepEqual(
    clampResizeSize({ width: 400, height: 300 }, { width: 300, height: 200 }),
    { width: 400, height: 300 },
  )
})

test('clampDragPosition allows most of the window off-screen, keeping only the title bar reachable', () => {
  const size = { width: 500, height: 400 }
  const farLeft = clampDragPosition({ x: -10000, y: 0 }, size, AREA)
  assert.equal(farLeft.x, MIN_VISIBLE_TITLEBAR.width - size.width)
  const farRight = clampDragPosition({ x: 10000, y: 0 }, size, AREA)
  assert.equal(farRight.x, AREA.width - MIN_VISIBLE_TITLEBAR.width)
})

test('cascadePosition offsets each successive window and wraps before running off the area', () => {
  const size = { width: 400, height: 300 }
  const first = cascadePosition(0, size, AREA)
  const second = cascadePosition(1, size, AREA)
  assert.deepEqual(first, { x: 0, y: 0 })
  assert.ok(second.x > first.x)
})

test('windowDisplayRect fills the area when compact or maximized, without touching stored geometry', () => {
  const window = { x: 50, y: 60, width: 400, height: 300, maximized: false }
  assert.deepEqual(windowDisplayRect(window, false, AREA), {
    x: 50,
    y: 60,
    width: 400,
    height: 300,
  })
  assert.deepEqual(windowDisplayRect(window, true, AREA), {
    x: 0,
    y: 0,
    width: AREA.width,
    height: AREA.height,
  })
  assert.deepEqual(
    windowDisplayRect({ ...window, maximized: true }, false, AREA),
    {
      x: 0,
      y: 0,
      width: AREA.width,
      height: AREA.height,
    },
  )
  // Neither call above mutated the window it was given.
  assert.deepEqual(window, {
    x: 50,
    y: 60,
    width: 400,
    height: 300,
    maximized: false,
  })
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
// Hydration (layoutState.ts's `hydrate`)
// ---------------------------------------------------------------------------

test('hydrate mints a default workspace when none are persisted (I1)', () => {
  const state = hydrate(
    { workspaces: [], windows: [], activeWorkspaceId: '' },
    APPS,
    AREA,
  )
  assert.equal(state.workspaces.length, 1)
  assert.equal(state.activeWorkspaceId, state.workspaces[0]?.id)
})

test('hydrate drops a tab with an unresolvable appId, and the window entirely if none survive (FR-025)', () => {
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
        tabs: [
          { id: 't1', appId: 'unknown.app' },
          { id: 't2', appId: ALPHA.id },
        ],
        activeTabId: 't1',
      },
      {
        id: 'w2',
        workspaceId: 'ws-1',
        x: 0,
        y: 0,
        width: 400,
        height: 300,
        minimized: false,
        maximized: false,
        stack: 1,
        tabs: [{ id: 't3', appId: 'unknown.app' }],
        activeTabId: 't3',
      },
    ],
    activeWorkspaceId: 'ws-1',
  }
  const state = hydrate(layout, APPS, AREA)
  assert.equal(
    state.windows.length,
    1,
    'w2 dropped entirely (no surviving tab)',
  )
  assert.equal(state.windows[0]?.tabs.length, 1)
  assert.equal(state.windows[0]?.tabs[0]?.appId, ALPHA.id)
})

test("hydrate repairs a dangling activeTabId to the window's first surviving tab", () => {
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
        tabs: [{ id: 't1', appId: ALPHA.id }],
        activeTabId: 'nonexistent-tab',
      },
    ],
    activeWorkspaceId: 'ws-1',
  }
  const state = hydrate(layout, APPS, AREA)
  assert.equal(state.windows[0]?.activeTabId, 't1')
})

test('hydrate reassigns a window with an unknown workspaceId to the default workspace', () => {
  const layout: PersistedLayout = {
    workspaces: [{ id: 'ws-1', position: 0 }],
    windows: [
      {
        id: 'w1',
        workspaceId: 'nonexistent-workspace',
        x: 0,
        y: 0,
        width: 400,
        height: 300,
        minimized: false,
        maximized: false,
        stack: 0,
        tabs: [{ id: 't1', appId: ALPHA.id }],
        activeTabId: 't1',
      },
    ],
    activeWorkspaceId: 'ws-1',
  }
  const state = hydrate(layout, APPS, AREA)
  assert.equal(state.windows[0]?.workspaceId, 'ws-1')
})

test('hydrate clamps geometry that no longer fits the current area (FR-026)', () => {
  const layout: PersistedLayout = {
    workspaces: [{ id: 'ws-1', position: 0 }],
    windows: [
      {
        id: 'w1',
        workspaceId: 'ws-1',
        x: 5000,
        y: 5000,
        width: 3000,
        height: 2000,
        minimized: false,
        maximized: false,
        stack: 0,
        tabs: [{ id: 't1', appId: ALPHA.id }],
        activeTabId: 't1',
      },
    ],
    activeWorkspaceId: 'ws-1',
  }
  const state = hydrate(layout, APPS, { width: 800, height: 600 })
  const window = state.windows[0]
  assert.ok(window)
  assert.ok(window.x + window.width <= 800)
  assert.ok(window.y + window.height <= 600)
})

test('hydrate re-derives a dense stack from the persisted order', () => {
  const layout: PersistedLayout = {
    workspaces: [{ id: 'ws-1', position: 0 }],
    windows: [
      {
        id: 'w-back',
        workspaceId: 'ws-1',
        x: 0,
        y: 0,
        width: 400,
        height: 300,
        minimized: false,
        maximized: false,
        stack: 40,
        tabs: [{ id: 't1', appId: ALPHA.id }],
        activeTabId: 't1',
      },
      {
        id: 'w-front',
        workspaceId: 'ws-1',
        x: 0,
        y: 0,
        width: 400,
        height: 300,
        minimized: false,
        maximized: false,
        stack: 41,
        tabs: [{ id: 't2', appId: BETA.id }],
        activeTabId: 't2',
      },
    ],
    activeWorkspaceId: 'ws-1',
  }
  const state = hydrate(layout, APPS, AREA)
  const back = state.windows.find((w) => w.id === 'w-back')
  const front = state.windows.find((w) => w.id === 'w-front')
  assert.equal(back?.stack, 0)
  assert.equal(front?.stack, 1)
  assert.equal(
    state.activeWindowId,
    'w-front',
    'the front-most (highest original stack) window is active',
  )
})

// ---------------------------------------------------------------------------
// Persistence queue (useShellLayout.ts)
// ---------------------------------------------------------------------------

type FakeCall = { cmd: string; args: unknown }

function makeWindowDto(windowId: string, x: number): WindowDto {
  return {
    windowId,
    workspaceId: 'ws-1',
    x,
    y: 0,
    width: 800,
    height: 600,
    isMinimized: false,
    isMaximized: false,
    stackOrder: 1,
    activeTabId: 'tab-1',
    tabs: [{ tabId: 'tab-1', appId: 'system.chat' }],
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

test('useShellLayout debounces rapid geometry saves into one call with the latest value', async () => {
  const calls: FakeCall[] = []
  const layout = useShellLayout(async (cmd, args) => {
    calls.push({ cmd, args })
    return null
  })
  layout.saveWindowDebounced(makeWindowDto('w1', 1))
  layout.saveWindowDebounced(makeWindowDto('w1', 2))
  layout.saveWindowDebounced(makeWindowDto('w1', 3))
  await sleep(500)
  await layout.flushAsync()
  const saveCalls = calls.filter((c) => c.cmd === 'shell_save_windows')
  assert.equal(saveCalls.length, 1)
  const windows = (saveCalls[0]?.args as { args: { windows: WindowDto[] } })
    .args.windows
  assert.equal(windows.length, 1)
  assert.equal(windows[0]?.x, 3)
})

test('useShellLayout persists a structural save immediately, without waiting for the debounce', async () => {
  const calls: FakeCall[] = []
  const layout = useShellLayout(async (cmd, args) => {
    calls.push({ cmd, args })
    return null
  })
  layout.saveWindowNow(makeWindowDto('w2', 10))
  await layout.flushAsync()
  assert.equal(calls.filter((c) => c.cmd === 'shell_save_windows').length, 1)
})

test('useShellLayout keeps a failed save dirty and retries it on the next save', async () => {
  const calls: FakeCall[] = []
  let failNext = true
  const layout = useShellLayout(async (cmd, args) => {
    calls.push({ cmd, args })
    if (cmd === 'shell_save_windows' && failNext) {
      failNext = false
      throw new Error('simulated failure')
    }
    return null
  })
  layout.saveWindowNow(makeWindowDto('w3', 5))
  await layout.flushAsync()
  layout.saveWindowNow(makeWindowDto('w4', 6)) // unrelated window, later save
  await layout.flushAsync()
  const saveCalls = calls.filter((c) => c.cmd === 'shell_save_windows')
  assert.equal(saveCalls.length, 2)
  const secondBatchIds = (
    saveCalls[1]?.args as { args: { windows: WindowDto[] } }
  ).args.windows
    .map((w) => w.windowId)
    .sort()
  assert.deepEqual(secondBatchIds, ['w3', 'w4'])
})

test('useShellLayout cancels a pending save when the same window is closed', async () => {
  const calls: FakeCall[] = []
  const layout = useShellLayout(async (cmd, args) => {
    calls.push({ cmd, args })
    return null
  })
  layout.saveWindowDebounced(makeWindowDto('w5', 1))
  layout.closeWindowNow('w5')
  await layout.flushAsync()
  assert.equal(calls.filter((c) => c.cmd === 'shell_save_windows').length, 0)
  assert.equal(calls.filter((c) => c.cmd === 'shell_close_windows').length, 1)
})

test('useShellLayout never runs two shell_* calls at once', async () => {
  const order: string[] = []
  const layout = useShellLayout(async (cmd) => {
    order.push(`${cmd}:start`)
    await sleep(20)
    order.push(`${cmd}:end`)
    return null
  })
  layout.saveWindowNow(makeWindowDto('w6', 1))
  layout.closeWindowNow('w6')
  await layout.flushAsync()
  assert.ok(
    order.indexOf('shell_save_windows:end') <
      order.indexOf('shell_close_windows:start'),
    `expected the save to fully finish before the close started, got ${JSON.stringify(order)}`,
  )
})

test('useShellLayout.loadLayout returns whatever the backend sends', async () => {
  const fakeLayout: ShellLayoutDto = {
    workspaces: [{ workspaceId: 'ws-1', position: 0 }],
    windows: [],
    activeWorkspaceId: 'ws-1',
  }
  const layout = useShellLayout(async () => fakeLayout)
  const result = await layout.loadLayout()
  assert.deepEqual(result, fakeLayout)
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
