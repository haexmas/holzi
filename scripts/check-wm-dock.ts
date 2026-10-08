// Part of `pnpm check:wm-state` (spec 045-dock): the dock's pure logic (src/lib/wm/dock.ts) —
// reading its preferences and normalizing the stored entries (data-model.md).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  DEFAULT_DOCK_PLACEMENT,
  normalizeDockItems,
  parseDockItems,
  parseDockPlacement,
  resolveDockEntries,
  serializeDockItems,
  type DockItem,
} from '../src/lib/wm/dock.ts'
import type { WmWindow } from '../src/lib/wm/types.ts'
import { ALPHA, APPS, BETA } from './lib/wm-fixtures.ts'

const launcher: DockItem = { kind: 'control', id: 'launcher' }
const windows: DockItem = { kind: 'control', id: 'windows' }
const alpha: DockItem = { kind: 'app', appId: ALPHA.id }
const beta: DockItem = { kind: 'app', appId: BETA.id }

// ---------------------------------------------------------------------------
// Reading the preferences
// ---------------------------------------------------------------------------

test('parseDockItems reports a missing or non-list value as unreadable', () => {
  assert.equal(parseDockItems(null), null)
  assert.equal(parseDockItems('x'), null)
  assert.equal(parseDockItems('{}'), null)
})

test('parseDockItems keeps valid elements and drops malformed ones', () => {
  const raw = JSON.stringify([
    launcher,
    { kind: 'control', id: 'taskbar' },
    { kind: 'app' },
    { kind: 'app', appId: '' },
    'alpha',
    alpha,
  ])
  assert.deepEqual(parseDockItems(raw), [launcher, alpha])
})

test('parseDockPlacement falls back field by field', () => {
  assert.deepEqual(parseDockPlacement(null), DEFAULT_DOCK_PLACEMENT)
  assert.deepEqual(parseDockPlacement('nope'), DEFAULT_DOCK_PLACEMENT)
  assert.deepEqual(
    parseDockPlacement(
      JSON.stringify({
        style: 'wheel',
        edge: 'diagonal',
        align: 'end',
        mode: 7,
      }),
    ),
    { style: 'wheel', edge: 'bottom', align: 'end', mode: 'reserved' },
  )
})

// ---------------------------------------------------------------------------
// Normalizing (data-model.md, "Regeln beim Lesen")
// ---------------------------------------------------------------------------

test('normalizeDockItems keeps the stored order and marks known items available', () => {
  assert.deepEqual(normalizeDockItems([launcher, beta, windows, alpha], APPS), [
    { ...launcher, available: true },
    { ...beta, available: true },
    { ...windows, available: true },
    { ...alpha, available: true },
  ])
})

test('normalizeDockItems keeps the first of duplicate controls and apps', () => {
  assert.deepEqual(
    normalizeDockItems([launcher, alpha, windows, alpha, windows], APPS),
    [
      { ...launcher, available: true },
      { ...alpha, available: true },
      { ...windows, available: true },
    ],
  )
})

test('normalizeDockItems puts a missing launcher first', () => {
  assert.deepEqual(normalizeDockItems([alpha], APPS), [
    { ...launcher, available: true },
    { ...alpha, available: true },
  ])
})

test('normalizeDockItems keeps an app unknown on this device, marked unavailable', () => {
  const gone: DockItem = { kind: 'app', appId: 'extension.gone' }
  assert.deepEqual(normalizeDockItems([launcher, gone], APPS), [
    { ...launcher, available: true },
    { ...gone, available: false },
  ])
})

test('normalizeDockItems maps a legacy app id to its app and dedupes against it', () => {
  const settings: DockItem = { kind: 'app', appId: 'system.settings' }
  const federation: DockItem = { kind: 'app', appId: 'system.federation' }
  const apps = [
    ...APPS,
    { ...ALPHA, id: 'system.settings', titleKey: 'settings' },
  ]
  assert.deepEqual(normalizeDockItems([launcher, federation, settings], apps), [
    { ...launcher, available: true },
    { ...settings, available: true },
  ])
})

test('serializeDockItems writes unavailable items back, without the device-local flag', () => {
  const gone: DockItem = { kind: 'app', appId: 'extension.gone' }
  const written = serializeDockItems(normalizeDockItems([gone, alpha], APPS))
  assert.deepEqual(JSON.parse(written), [launcher, gone, alpha])
})

// ---------------------------------------------------------------------------
// Entries to show (resolveDockEntries)
// ---------------------------------------------------------------------------

function windowWith(
  id: string,
  workspaceId: string,
  tabs: { id: string; appId: string }[],
): WmWindow {
  return {
    id,
    workspaceId,
    x: 0,
    y: 0,
    width: 400,
    height: 300,
    minimized: false,
    maximized: false,
    stack: 0,
    tabs,
    activeTabId: tabs[0]?.id ?? '',
  }
}

test('resolveDockEntries shows available items in order and hides unavailable apps', () => {
  const gone: DockItem = { kind: 'app', appId: 'extension.gone' }
  const items = normalizeDockItems([launcher, gone, beta, windows], APPS)
  assert.deepEqual(resolveDockEntries(items, []), [
    { kind: 'control', id: 'launcher' },
    { kind: 'app', appId: BETA.id, pinned: true, instances: [] },
    { kind: 'control', id: 'windows' },
  ])
})

test("resolveDockEntries collects an app's tabs across windows and workspaces", () => {
  const items = normalizeDockItems([launcher, alpha], APPS)
  const open = [
    windowWith('w1', 'ws-1', [
      { id: 't1', appId: ALPHA.id },
      { id: 't2', appId: BETA.id },
    ]),
    windowWith('w2', 'ws-2', [{ id: 't3', appId: ALPHA.id }]),
  ]
  const [, entry] = resolveDockEntries(items, open)
  assert.deepEqual(entry, {
    kind: 'app',
    appId: ALPHA.id,
    pinned: true,
    instances: [
      { tabId: 't1', windowId: 'w1', workspaceId: 'ws-1' },
      { tabId: 't3', windowId: 'w2', workspaceId: 'ws-2' },
    ],
  })
})
