// Part of `pnpm check:extensions` (spec 017, T025): installed extensions as apps of the window
// manager (src/lib/extensions/apps.ts) and their tabs in a restored session
// (src/lib/wm/sessionSync.ts).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import type { ExtensionSummary } from '../src/types/bindings/ExtensionSummary.ts'
import {
  allApps,
  extensionAppId,
  extensionApps,
  extensionIdOf,
} from '../src/lib/extensions/apps.ts'
import { WM_APPS, type AppDefinition } from '../src/lib/wm/apps.ts'
import { openApp } from '../src/lib/wm/layoutState.ts'
import { snapshotSession } from '../src/lib/wm/session.ts'
import {
  createSessionSync,
  type SessionPort,
} from '../src/lib/wm/sessionSync.ts'
import type { TabHistory } from '../src/lib/wm/navigation.ts'
import { emptyState } from './lib/wm-fixtures.ts'

function summary(patch: Partial<ExtensionSummary>): ExtensionSummary {
  return {
    id: '00000000-0000-0000-0000-000000000001',
    name: 'notes',
    title: 'Notizen',
    publisherFingerprint: 'aaaa',
    enabled: true,
    state: 'installed',
    singleInstance: false,
    hasIcon: false,
    ...patch,
  }
}

test('every installed and enabled extension is an app with its own name and icon', () => {
  const notes = summary({})
  const single = summary({
    id: 'id-2',
    title: 'Kalender',
    singleInstance: true,
  })
  const apps = extensionApps(
    [
      notes,
      single,
      summary({ id: 'off', enabled: false }),
      summary({ id: 'gone', state: 'removed' }),
    ],
    { [notes.id]: 'data:image/svg+xml;base64,AA==' },
  )
  assert.deepEqual(
    apps.map((a) => a.id),
    [extensionAppId(notes.id), 'extension.id-2'],
  )
  assert.equal(apps[0]?.title, 'Notizen')
  assert.equal(apps[0]?.iconUrl, 'data:image/svg+xml;base64,AA==')
  assert.equal(apps[0]?.multiInstance, true)
  assert.equal(apps[1]?.iconUrl, undefined)
  assert.equal(apps[1]?.multiInstance, false)
  assert.equal(apps[1]?.tabTitle, 'app')
  assert.deepEqual(allApps(apps).slice(0, WM_APPS.length), [...WM_APPS])
  assert.equal(extensionIdOf('extension.abc'), 'abc')
  assert.equal(extensionIdOf('system.chat'), null)
})

function savedSessionWith(app: AppDefinition) {
  const state = emptyState()
  const histories = new Map<string, TabHistory>()
  openApp(state, app.id, [app])
  return snapshotSession(state, (tabId) => histories.get(tabId))
}

function restoring(
  session: ReturnType<typeof savedSessionWith>,
  apps: () => readonly AppDefinition[],
) {
  const state = emptyState()
  const port: SessionPort = {
    saveNow: () => {},
    saveSoon: () => {},
    load: async () => ({ restore: { enabled: true }, session }),
    setRestore: async (enabled) => ({ enabled }),
    getRestore: async () => ({ enabled: true }),
    flushAsync: async () => {},
  }
  const sync = createSessionSync({
    state,
    histories: new Map(),
    apps,
    port,
    onRestored: () => {},
  })
  return { state, sync }
}

test('an extension tab survives the restore when the list is loaded by then', async () => {
  const [notes] = extensionApps([summary({})], {})
  assert.ok(notes)
  const session = savedSessionWith(notes)

  let loaded: AppDefinition[] = []
  const { state, sync } = restoring(session, () => allApps(loaded))
  // The page loads the list first, then restores: the getter is read at restore time.
  loaded = extensionApps([summary({})], {})
  await sync.restoreAsync()
  assert.deepEqual(
    state.windows.flatMap((w) => w.tabs.map((t) => t.appId)),
    [notes.id],
  )
})

test('a tab of an extension that is unknown after loading the list is dropped', async () => {
  const [notes] = extensionApps([summary({})], {})
  assert.ok(notes)
  const { state, sync } = restoring(savedSessionWith(notes), () => allApps([]))
  await sync.restoreAsync()
  assert.equal(state.windows.length, 0)
})
