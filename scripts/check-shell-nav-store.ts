// Part of `pnpm check:shell-navigation` (spec 020-tab-navigation): store-level navigation — tab
// histories kept in sync with the layout (tabNavigation.ts) and the handler registries the action
// runner uses (actions/handlers.ts). Pure modules, no Pinia/Nuxt; the store only wires them.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  createHandlerRegistry,
  type ActionHandlerContext,
} from '../src/lib/actions/handlers.ts'
import {
  closeWindow,
  hydrate,
  minimizeWindow,
  moveWindowToWorkspace,
  switchWorkspace,
  toggleMaximizeWindow,
  createWorkspace,
} from '../src/lib/shell/layoutState.ts'
import {
  currentLocation,
  createHistory,
  type TabHistory,
} from '../src/lib/shell/navigation.ts'
import {
  addTabAt,
  goTab,
  navigateTab,
  openAppAt,
  syncHistories,
} from '../src/lib/shell/tabNavigation.ts'
import { closeTab, switchTab } from '../src/lib/shell/tabs.ts'
import type { ShellState } from '../src/lib/shell/types.ts'
import { ALPHA, APPS, AREA, BETA, emptyState } from './lib/shell-fixtures.ts'

type Histories = Map<string, TabHistory>

function pathOf(histories: Histories, tabId: string | null): string | null {
  const history = tabId ? histories.get(tabId) : undefined
  return history ? currentLocation(history).path : null
}

function fresh(): { state: ShellState; histories: Histories } {
  return { state: emptyState(), histories: new Map() }
}

// ---------------------------------------------------------------------------
// Tab histories (tabNavigation.ts)
// ---------------------------------------------------------------------------

test('a newly opened tab starts with one `/` entry', () => {
  const { state, histories } = fresh()
  const { tabId, created } = openAppAt(state, histories, ALPHA.id, null, APPS)
  assert.equal(created, true)
  assert.equal(histories.get(tabId ?? '')?.entries.length, 1)
  assert.equal(pathOf(histories, tabId), '/')
})

test('openAppAt on a closed app starts a single-entry history at the location', () => {
  const { state, histories } = fresh()
  const { tabId } = openAppAt(state, histories, ALPHA.id, '/models', APPS)
  assert.equal(pathOf(histories, tabId), '/models')
  assert.equal(histories.get(tabId ?? '')?.entries.length, 1)
})

test('openAppAt on an open singleton activates it and pushes the location', () => {
  const { state, histories } = fresh()
  const first = openAppAt(state, histories, ALPHA.id, null, APPS)
  openAppAt(state, histories, BETA.id, null, APPS)
  const again = openAppAt(state, histories, ALPHA.id, '/models', APPS)
  assert.equal(again.created, false)
  assert.equal(again.tabId, first.tabId)
  assert.equal(state.windows.length, 2)
  const history = histories.get(first.tabId ?? '')
  assert.equal(history?.entries.length, 2)
  assert.equal(pathOf(histories, first.tabId), '/models')
})

test('openAppAt with the current location of an open singleton adds no entry', () => {
  const { state, histories } = fresh()
  const first = openAppAt(state, histories, ALPHA.id, '/models', APPS)
  openAppAt(state, histories, ALPHA.id, '/models', APPS)
  assert.equal(histories.get(first.tabId ?? '')?.entries.length, 1)
})

test('addTabAt adds a tab at the location, or navigates an open singleton', () => {
  const { state, histories } = fresh()
  const alpha = openAppAt(state, histories, ALPHA.id, null, APPS)
  const windowId = state.windows[0]?.id ?? ''
  const beta = addTabAt(state, histories, windowId, BETA.id, '/x', APPS)
  assert.equal(beta.created, true)
  assert.equal(pathOf(histories, beta.tabId), '/x')
  const reopened = addTabAt(state, histories, windowId, ALPHA.id, '/y', APPS)
  assert.equal(reopened.tabId, alpha.tabId)
  assert.equal(pathOf(histories, alpha.tabId), '/y')
})

test('navigating one tab never changes another (FR-008)', () => {
  const { state, histories } = fresh()
  const a = openAppAt(state, histories, ALPHA.id, null, APPS).tabId ?? ''
  const b = openAppAt(state, histories, BETA.id, null, APPS).tabId ?? ''
  const before = histories.get(b)
  navigateTab(histories, a, '/one')
  navigateTab(histories, a, '/two')
  goTab(histories, a, -1)
  assert.equal(histories.get(b), before)
  assert.equal(pathOf(histories, a), '/one')
})

test('window and workspace operations keep every history (FR-010)', () => {
  const { state, histories } = fresh()
  const a = openAppAt(state, histories, ALPHA.id, null, APPS).tabId ?? ''
  navigateTab(histories, a, '/deep')
  const windowId = state.windows[0]?.id ?? ''
  const snapshot = histories.get(a)
  const other = createWorkspace(state, 'ws-2')
  minimizeWindow(state, windowId)
  toggleMaximizeWindow(state, windowId)
  moveWindowToWorkspace(state, windowId, other.id)
  switchWorkspace(state, other.id)
  switchTab(state, windowId, a)
  syncHistories(histories, state)
  assert.equal(histories.get(a), snapshot)
})

test('closing a tab or window drops its history on the next sync (FR-011)', () => {
  const { state, histories } = fresh()
  const alpha = openAppAt(state, histories, ALPHA.id, null, APPS).tabId ?? ''
  const windowId = state.windows[0]?.id ?? ''
  const beta =
    addTabAt(state, histories, windowId, BETA.id, null, APPS).tabId ?? ''
  closeTab(state, windowId, beta)
  syncHistories(histories, state)
  assert.equal(histories.has(beta), false)
  closeWindow(state, windowId)
  syncHistories(histories, state)
  assert.equal(histories.has(alpha), false)
})

test('hydrate plus sync gives every restored tab exactly `[/]`', () => {
  const state = hydrate(
    {
      workspaces: [{ id: 'ws', position: 0 }],
      windows: [
        {
          id: 'w',
          workspaceId: 'ws',
          x: 0,
          y: 0,
          width: 800,
          height: 600,
          minimized: false,
          maximized: false,
          stack: 1,
          tabs: [
            { id: 't1', appId: ALPHA.id },
            { id: 't2', appId: BETA.id },
          ],
          activeTabId: 't1',
        },
      ],
      activeWorkspaceId: 'ws',
    },
    APPS,
    AREA,
  )
  const histories: Histories = new Map([['t1', createHistory('/stale')]])
  histories.clear()
  syncHistories(histories, state)
  for (const id of ['t1', 't2']) {
    assert.equal(histories.get(id)?.entries.length, 1)
    assert.equal(pathOf(histories, id), '/')
  }
})

test('navigateTab and goTab report whether anything changed', () => {
  const { state, histories } = fresh()
  const a = openAppAt(state, histories, ALPHA.id, null, APPS).tabId ?? ''
  assert.equal(goTab(histories, a, -1), false)
  assert.equal(navigateTab(histories, a, '/'), false)
  assert.equal(navigateTab(histories, a, '/x'), true)
  assert.equal(navigateTab(histories, a, '/x?f=1', { replace: true }), true)
  assert.equal(histories.get(a)?.entries.length, 2)
  assert.equal(navigateTab(histories, 'missing', '/x'), false)
})

// ---------------------------------------------------------------------------
// Handler registries (actions/handlers.ts)
// ---------------------------------------------------------------------------

const CONTEXT: ActionHandlerContext = {
  input: {},
  caller: { kind: 'user' },
  target: {},
}

test('global handlers register and resolve by action id', () => {
  const registry = createHandlerRegistry()
  registry.registerGlobal('a', () => 'ok')
  assert.equal(registry.globalHandler('a')?.(CONTEXT), 'ok')
  assert.equal(registry.globalHandler('b'), undefined)
})

test('a tab handler already registered resolves immediately', async () => {
  const registry = createHandlerRegistry()
  registry.registerTab('tab-1', 'chat.send', () => 'sent')
  const handler = await registry.awaitTab(['tab-1'], 'chat.send', 50)
  assert.equal(handler?.(CONTEXT), 'sent')
})

test('awaitTab resolves once the tab registers its handler', async () => {
  const registry = createHandlerRegistry()
  const pending = registry.awaitTab(['tab-1'], 'chat.send', 500)
  setTimeout(() => registry.registerTab('tab-1', 'chat.send', () => 'late'), 10)
  assert.equal((await pending)?.(CONTEXT), 'late')
})

test('awaitTab gives up after the timeout; unregister removes the handler', async () => {
  const registry = createHandlerRegistry()
  assert.equal(await registry.awaitTab(['tab-1'], 'chat.send', 20), null)
  const unregister = registry.registerTab('tab-1', 'chat.send', () => 'x')
  unregister()
  assert.equal(await registry.awaitTab(['tab-1'], 'chat.send', 20), null)
})

test('dropTab removes every handler of a closed tab', async () => {
  const registry = createHandlerRegistry()
  registry.registerTab('tab-1', 'chat.send', () => 'x')
  registry.registerTab('tab-1', 'chat.cancel', () => 'y')
  registry.dropTab('tab-1')
  assert.equal(await registry.awaitTab(['tab-1'], 'chat.cancel', 20), null)
})
