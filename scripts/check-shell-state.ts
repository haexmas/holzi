// Run with `node scripts/check-shell-state.ts`. Harness for `src/lib/shell/*.ts` (spec
// 015-workspace-shell) — those modules deliberately avoid Nuxt auto-imports and use relative
// `.ts`-suffixed sibling imports (plan research R6) so they load standalone here, the same way
// `check-chat-state.ts` replays `ChatApp.vue`'s script block.
//
// T052 (User Story 7 — multi-instance apps) is the first real scenario; T053 adds the rest (pure
// reducer, geometry, tabs, hydration, queue, and unknown-app checks) on top of this file.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import type { ShellAppDefinition } from '../src/lib/shell/apps.ts'
import { hydrate, openApp } from '../src/lib/shell/layoutState.ts'
import { addTab } from '../src/lib/shell/tabs.ts'
import type { PersistedLayout, Size } from '../src/lib/shell/types.ts'

const AREA: Size = { width: 1920, height: 1080 }

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
const APPS: readonly ShellAppDefinition[] = [MULTI_APP]

function freshState() {
  return hydrate(
    { workspaces: [], windows: [], activeWorkspaceId: '' },
    APPS,
    AREA,
  )
}

test('openApp opens a new window every time for a multi-instance app', () => {
  const state = freshState()
  openApp(state, MULTI_APP.id, APPS)
  openApp(state, MULTI_APP.id, APPS)
  assert.equal(state.windows.length, 2)
  assert.notEqual(state.windows[0]?.id, state.windows[1]?.id)
})

test('addTab appends a second tab in the same window instead of activating a singleton elsewhere', () => {
  const state = freshState()
  openApp(state, MULTI_APP.id, APPS)
  const windowId = state.windows[0]?.id
  assert.ok(windowId)
  addTab(state, windowId, MULTI_APP.id, APPS)
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
  const state = hydrate(layout, APPS, AREA)
  assert.equal(state.windows.length, 2)
  assert.deepEqual(
    state.windows.map((w) => w.tabs.map((t) => t.appId)),
    [[MULTI_APP.id], [MULTI_APP.id]],
  )
})
