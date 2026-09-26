// Part of `pnpm check:wm-state` (spec 022-session-restore): the saved session
// (src/lib/wm/session.ts, T014) and saving/restoring it (src/lib/wm/sessionSync.ts, T017,
// T024, T026). Pure modules, loaded with Node type-stripping like check-wm-state.ts.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { ALL_ACTIONS } from '../src/lib/actions/catalog.ts'
import { validate } from '../src/lib/actions/schema.ts'
import { openApp } from '../src/lib/wm/layoutState.ts'
import {
  createHistory,
  MAX_HISTORY_ENTRIES,
  push,
  type TabHistory,
} from '../src/lib/wm/navigation.ts'
import {
  parseWmSession,
  snapshotSession,
  splitSession,
  withoutHistories,
  type WmSession,
} from '../src/lib/wm/session.ts'
import {
  createSessionSync,
  fromRestoreResult,
  toRestoreResult,
  type RestoreState,
  type SessionPort,
} from '../src/lib/wm/sessionSync.ts'
import { ALPHA, APPS, BETA, emptyState } from './lib/wm-fixtures.ts'

const ON: RestoreState = { device: true, vault: null, effective: true }
const OFF: RestoreState = { device: null, vault: null, effective: false }

function chatHistory(): TabHistory {
  return push(
    push(createHistory('/'), '/thread/a', 'New chat'),
    '/thread/b',
    'Thread A',
  )
}

function savedSession(): WmSession {
  return {
    version: 1,
    workspaces: [
      { id: 'ws-1', position: 0 },
      { id: 'ws-2', position: 1 },
    ],
    windows: [
      {
        id: 'w1',
        workspaceId: 'ws-2',
        x: 40,
        y: 30,
        width: 700,
        height: 500,
        minimized: false,
        maximized: true,
        stack: 0,
        activeTabId: 't2',
        tabs: [
          { id: 't1', appId: ALPHA.id, history: createHistory('/') },
          { id: 't2', appId: BETA.id, history: chatHistory() },
        ],
      },
    ],
    activeWorkspaceId: 'ws-2',
  }
}

// ---------------------------------------------------------------------------
// session.ts
// ---------------------------------------------------------------------------

test('a valid session survives a JSON round trip through parseWmSession', () => {
  const session = savedSession()
  assert.deepEqual(parseWmSession(JSON.parse(JSON.stringify(session))), session)
})

test('parseWmSession rejects a wrong version, a missing field or a mistyped window', () => {
  const good = savedSession()
  assert.equal(parseWmSession({ ...good, version: 2 }), null)
  assert.equal(parseWmSession({ ...good, workspaces: undefined }), null)
  assert.equal(
    parseWmSession({ ...good, windows: [{ ...good.windows[0], x: 'left' }] }),
    null,
  )
  assert.equal(parseWmSession(null), null)
  assert.equal(parseWmSession('{"version":1}'), null)
})

test('a single invalid tab history is replaced by a fresh one, the session is kept', () => {
  const good = savedSession()
  const window = good.windows[0]!
  const badHistories: unknown[] = [
    { entries: [], index: 0 },
    {
      entries: [{ location: { path: '/', query: {} }, title: null }],
      index: 3,
    },
    {
      entries: [{ location: { path: 'no-slash', query: {} }, title: null }],
      index: 0,
    },
    {
      entries: Array.from({ length: MAX_HISTORY_ENTRIES + 1 }, () => ({
        location: { path: '/', query: {} },
        title: null,
      })),
      index: 0,
    },
    undefined,
  ]
  for (const history of badHistories) {
    const parsed = parseWmSession({
      ...good,
      windows: [{ ...window, tabs: [{ id: 't1', appId: ALPHA.id, history }] }],
    })
    assert.ok(
      parsed,
      `session kept for ${JSON.stringify(history)?.slice(0, 40)}`,
    )
    assert.deepEqual(parsed.windows[0]?.tabs[0]?.history, createHistory())
  }
})

test('splitSession gives hydrate plain tabs and returns the histories by tab id', () => {
  const { layout, histories } = splitSession(savedSession())
  assert.deepEqual(layout.windows[0]?.tabs, [
    { id: 't1', appId: ALPHA.id },
    { id: 't2', appId: BETA.id },
  ])
  assert.deepEqual(histories.get('t2'), chatHistory())
})

test('snapshotSession takes every tab with its history', () => {
  const state = emptyState()
  openApp(state, ALPHA.id, APPS)
  const tabId = state.windows[0]!.tabs[0]!.id
  const histories = new Map([[tabId, chatHistory()]])
  const snapshot = snapshotSession(state, (id) => histories.get(id))
  assert.equal(snapshot.version, 1)
  assert.deepEqual(snapshot.windows[0]?.tabs[0]?.history, chatHistory())
  assert.deepEqual(snapshot.workspaces, state.workspaces)
})

test('withoutHistories keeps only the current entry of each tab', () => {
  const stripped = withoutHistories(savedSession())
  const history = stripped.windows[0]?.tabs[1]?.history
  assert.equal(history?.entries.length, 1)
  assert.equal(history?.index, 0)
  assert.equal(history?.entries[0]?.location.path, '/thread/b')
})

// ---------------------------------------------------------------------------
// sessionSync.ts
// ---------------------------------------------------------------------------

type PortLog = {
  saveNow: WmSession[]
  saveSoon: WmSession[]
  setRestore: [string, boolean | null][]
}

function fakePort(load: SessionPort['load']): {
  port: SessionPort
  log: PortLog
} {
  const log: PortLog = { saveNow: [], saveSoon: [], setRestore: [] }
  return {
    log,
    port: {
      saveNow: (s) => log.saveNow.push(s),
      saveSoon: (s) => log.saveSoon.push(s),
      load,
      setRestore: async (scope, enabled) => {
        log.setRestore.push([scope, enabled])
        return enabled === null
          ? OFF
          : { device: enabled, vault: null, effective: enabled }
      },
      getRestore: async () => OFF,
      flushAsync: async () => {},
    },
  }
}

function makeSync(load: SessionPort['load']) {
  const state = emptyState()
  const histories = new Map<string, TabHistory>()
  let restoredCalls = 0
  const { port, log } = fakePort(load)
  const sync = createSessionSync({
    state,
    histories,
    apps: APPS,
    port,
    onRestored: () => restoredCalls++,
  })
  return { state, histories, sync, log, restored: () => restoredCalls }
}

test('with the setting off nothing is ever saved (US1)', async () => {
  const { state, sync, log } = makeSync(async () => ({
    restore: OFF,
    session: null,
  }))
  await sync.restoreAsync()
  openApp(state, ALPHA.id, APPS)
  sync.saveNow()
  sync.saveSoon()
  assert.deepEqual([log.saveNow, log.saveSoon], [[], []])
  assert.equal(sync.isEnabled(), false)
})

test('without a saved session the start is one workspace and no window', async () => {
  const { state, sync, restored } = makeSync(async () => ({
    restore: ON,
    session: null,
  }))
  await sync.restoreAsync()
  assert.equal(state.workspaces.length, 1)
  assert.equal(state.windows.length, 0)
  assert.equal(restored(), 1)
})

test('a failed load starts empty and keeps saving off (FR-012)', async () => {
  const { state, sync, log } = makeSync(async () => {
    throw new Error('vault locked')
  })
  await sync.restoreAsync()
  assert.equal(state.workspaces.length, 1)
  assert.equal(state.windows.length, 0)
  sync.saveNow()
  assert.equal(log.saveNow.length, 0)
})

test('an invalid saved session starts empty', async () => {
  const { state, sync } = makeSync(async () => ({
    restore: ON,
    session: { version: 99 },
  }))
  await sync.restoreAsync()
  assert.equal(state.windows.length, 0)
})

test('a restored session brings back windows, the active workspace and every tab history (US2)', async () => {
  const { state, histories, sync } = makeSync(async () => ({
    restore: ON,
    session: JSON.parse(JSON.stringify(savedSession())),
  }))
  await sync.restoreAsync()
  assert.equal(state.activeWorkspaceId, 'ws-2')
  assert.equal(state.windows.length, 1)
  assert.equal(state.windows[0]?.maximized, true)
  assert.equal(state.windows[0]?.activeTabId, 't2')
  assert.deepEqual(histories.get('t2'), chatHistory())
  assert.equal(histories.get('t2')?.index, 2)
})

test('a session is ignored when the setting does not apply, even if the backend sent one', async () => {
  const { state, sync } = makeSync(async () => ({
    restore: OFF,
    session: savedSession(),
  }))
  await sync.restoreAsync()
  assert.equal(state.windows.length, 0)
})

test('turning the setting on saves the current session right away (FR-005)', async () => {
  const { state, sync, log } = makeSync(async () => ({
    restore: OFF,
    session: null,
  }))
  await sync.restoreAsync()
  openApp(state, ALPHA.id, APPS)
  sync.applyRestore(ON)
  assert.equal(log.saveNow.length, 1)
  assert.equal(log.saveNow[0]?.windows.length, 1)
  sync.saveSoon()
  assert.equal(log.saveSoon.length, 1)
  sync.applyRestore(ON)
  assert.equal(log.saveNow.length, 1, 'staying on does not save again')
})

test('turning the setting off keeps every window and stops saving (US4)', async () => {
  const { state, sync, log } = makeSync(async () => ({
    restore: ON,
    session: savedSession(),
  }))
  await sync.restoreAsync()
  sync.applyRestore(OFF)
  assert.equal(state.windows.length, 1)
  assert.equal(state.workspaces.length, 2)
  sync.saveNow()
  sync.saveSoon()
  assert.deepEqual([log.saveNow, log.saveSoon], [[], []])
})

test('setRestoreAsync goes through the port and takes over the new setting', async () => {
  const { state, sync, log } = makeSync(async () => ({
    restore: OFF,
    session: null,
  }))
  await sync.restoreAsync()
  openApp(state, ALPHA.id, APPS)
  const on = await sync.setRestoreAsync('device', true)
  assert.equal(on.effective, true)
  assert.deepEqual(log.setRestore, [['device', true]])
  assert.equal(log.saveNow.length, 1, 'turning on saves at once')
  const off = await sync.setRestoreAsync('device', null)
  assert.equal(off.effective, false)
  sync.saveNow()
  assert.equal(log.saveNow.length, 1, 'no save after turning off')
})

test('session restore action results fit their schema, with unset values left out', () => {
  const values = [true, false, null]
  for (const id of [
    'settings.sessionRestore.set',
    'settings.sessionRestore.clear',
  ]) {
    const action = ALL_ACTIONS.find((candidate) => candidate.id === id)
    assert.ok(action, id)
    for (const device of values) {
      for (const vault of values) {
        const state: RestoreState = {
          device,
          vault,
          effective: device ?? vault ?? false,
        }
        const result = toRestoreResult(state)
        assert.deepEqual(validate(action.result, result), { ok: true })
        assert.deepEqual(fromRestoreResult(result), state)
      }
    }
  }
})
