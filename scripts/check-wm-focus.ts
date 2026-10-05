// Run with `pnpm check:wm-state`. The focus rule of `src/lib/wm/layoutState.ts` (spec 015,
// data-model.md: `activeWindowId` is the focused window of the active workspace): every reducer
// that moves the focus away from a window hands it to a visible window of the workspace on
// screen, never to a minimized one or one of another workspace, so the focused window's shortcuts
// (Alt+ArrowLeft, closing a tab) never act on a window the user cannot see.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  closeWindow,
  createWorkspace,
  deleteWorkspace,
  focusWindow,
  minimizeWindow,
  moveWindowToWorkspace,
  openApp,
  switchWorkspace,
} from '../src/lib/wm/layoutState.ts'
import { ALPHA, BETA, APPS, emptyState } from './lib/wm-fixtures.ts'

test('closeWindow never activates a minimized window', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  openApp(state, BETA.id, APPS)
  const [first, second] = state.windows
  assert.ok(first && second)
  minimizeWindow(state, first.id)
  focusWindow(state, second.id)
  closeWindow(state, second.id)
  assert.equal(state.activeWindowId, null)
})

test('moving the active window away never activates a minimized window', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  openApp(state, BETA.id, APPS)
  const [first, second] = state.windows
  assert.ok(first && second)
  minimizeWindow(state, first.id)
  const target = createWorkspace(state, 'ws-target')
  moveWindowToWorkspace(state, second.id, target.id)
  assert.equal(state.activeWindowId, null)
})

test('switchWorkspace activates the front-most visible window of the new workspace', () => {
  const state = emptyState()
  const home = state.activeWorkspaceId
  openApp(state, ALPHA.id, APPS)
  const other = createWorkspace(state, 'ws-other')
  switchWorkspace(state, other.id)
  assert.equal(state.activeWindowId, null, 'an empty workspace has none')
  openApp(state, BETA.id, APPS)
  const beta = state.windows.find((w) => w.workspaceId === other.id)
  assert.ok(beta)
  switchWorkspace(state, home)
  assert.equal(state.activeWindowId, state.windows[0]?.id)
  switchWorkspace(state, home)
  assert.equal(
    state.activeWindowId,
    state.windows[0]?.id,
    'switching to the active workspace keeps the active window',
  )
  switchWorkspace(state, other.id)
  assert.equal(state.activeWindowId, beta.id)
})

test('deleting the active workspace activates a window of the workspace that takes over', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  const kept = state.windows[0]
  assert.ok(kept)
  const doomed = createWorkspace(state, 'ws-doomed')
  switchWorkspace(state, doomed.id)
  openApp(state, BETA.id, APPS)
  deleteWorkspace(state, doomed.id)
  assert.equal(state.activeWindowId, kept.id)
})
