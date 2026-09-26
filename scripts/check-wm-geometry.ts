// Part of `pnpm check:wm-state` (spec 015-workspace-shell): geometry (geometry.ts) and
// hydration (layoutState.ts's `hydrate`). Split from check-wm-state.ts per plan.md Complexity
// Tracking (T059).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import type { PersistedLayout } from '../src/lib/wm/types.ts'
import { hydrate } from '../src/lib/wm/layoutState.ts'
import {
  cascadePosition,
  clampDragPosition,
  clampGeometry,
  clampResizeSize,
  MIN_VISIBLE_TITLEBAR,
  windowDisplayRect,
} from '../src/lib/wm/geometry.ts'
import { AREA, ALPHA, BETA, APPS } from './lib/wm-fixtures.ts'

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

test('hydrate keeps singleton apps unique and selects a visible window in the active workspace', () => {
  const layout: PersistedLayout = {
    workspaces: [
      { id: 'ws-active', position: 0 },
      { id: 'ws-other', position: 1 },
    ],
    windows: [
      {
        id: 'w-active-visible',
        workspaceId: 'ws-active',
        x: 0,
        y: 0,
        width: 400,
        height: 300,
        minimized: false,
        maximized: false,
        stack: 1,
        tabs: [{ id: 't-alpha-1', appId: ALPHA.id }],
        activeTabId: 't-alpha-1',
      },
      {
        id: 'w-active-duplicate',
        workspaceId: 'ws-active',
        x: 0,
        y: 0,
        width: 400,
        height: 300,
        minimized: false,
        maximized: false,
        stack: 2,
        tabs: [{ id: 't-alpha-2', appId: ALPHA.id }],
        activeTabId: 't-alpha-2',
      },
      {
        id: 'w-other-front',
        workspaceId: 'ws-other',
        x: 0,
        y: 0,
        width: 400,
        height: 300,
        minimized: false,
        maximized: false,
        stack: 3,
        tabs: [{ id: 't-beta', appId: BETA.id }],
        activeTabId: 't-beta',
      },
    ],
    activeWorkspaceId: 'ws-active',
  }

  const state = hydrate(layout, APPS, AREA)

  assert.deepEqual(
    state.windows.flatMap((window) =>
      window.tabs.map((tab) => `${window.id}:${tab.appId}`),
    ),
    ['w-active-visible:test.alpha', 'w-other-front:test.beta'],
  )
  assert.equal(state.activeWindowId, 'w-active-visible')
})
