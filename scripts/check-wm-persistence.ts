// Part of `pnpm check:wm-state` (spec 022-session-restore, T012): the session save queue
// (useWmSession.ts). `useWmSession` takes an injectable `invokeFn` (defaulting to the real
// Tauri `invoke`) so its queue, debounce, retry and size fallback run here without a Tauri
// runtime or module mocking.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { useWmSession } from '../src/composables/useWmSession.ts'
import { createHistory, push } from '../src/lib/wm/navigation.ts'
import type { WmSession } from '../src/lib/wm/session.ts'

type FakeCall = { cmd: string; args: unknown }

function session(marker: string): WmSession {
  const history = push(createHistory('/'), '/thread/a', 'Chat')
  return {
    version: 1,
    workspaces: [{ id: 'ws-1', position: 0 }],
    windows: [
      {
        id: 'w1',
        workspaceId: 'ws-1',
        x: 0,
        y: 0,
        width: 800,
        height: 600,
        minimized: false,
        maximized: false,
        stack: 0,
        activeTabId: 'tab-1',
        tabs: [{ id: 'tab-1', appId: 'system.chat', history }],
      },
    ],
    activeWorkspaceId: marker,
  }
}

function savedSessions(calls: FakeCall[]): WmSession[] {
  return calls
    .filter((c) => c.cmd === 'wm_session_save')
    .map((c) => (c.args as { args: { session: WmSession } }).args.session)
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

test('saveSoon debounces a burst into one save with the latest snapshot', async () => {
  const calls: FakeCall[] = []
  const store = useWmSession(async (cmd, args) => {
    calls.push({ cmd, args })
    return { saved: true }
  })
  store.saveSoon(session('a'))
  store.saveSoon(session('b'))
  store.saveSoon(session('c'))
  await sleep(500)
  await store.flushAsync()
  const saved = savedSessions(calls)
  assert.equal(saved.length, 1)
  assert.equal(saved[0]?.activeWorkspaceId, 'c')
})

test('saveNow saves without waiting for the debounce', async () => {
  const calls: FakeCall[] = []
  const store = useWmSession(async (cmd, args) => {
    calls.push({ cmd, args })
    return { saved: true }
  })
  store.saveNow(session('now'))
  await store.flushAsync()
  assert.equal(savedSessions(calls).length, 1)
})

test('a failed save stays pending and goes out with the next save', async () => {
  const calls: FakeCall[] = []
  let fail = true
  const store = useWmSession(async (cmd, args) => {
    calls.push({ cmd, args })
    if (fail) throw new Error('backend down')
    return { saved: true }
  })
  store.saveNow(session('first'))
  await store.flushAsync()
  fail = false
  store.saveNow(session('second'))
  await store.flushAsync()
  const saved = savedSessions(calls)
  assert.equal(saved.length, 2)
  assert.equal(saved[1]?.activeWorkspaceId, 'second')
})

test('flushAsync sends a pending debounced save before resolving', async () => {
  const calls: FakeCall[] = []
  const store = useWmSession(async (cmd, args) => {
    calls.push({ cmd, args })
    return { saved: true }
  })
  store.saveSoon(session('pending'))
  await store.flushAsync()
  assert.equal(savedSessions(calls).length, 1)
})

test('never runs two backend calls at once', async () => {
  let inFlight = 0
  let maxInFlight = 0
  const store = useWmSession(async () => {
    inFlight++
    maxInFlight = Math.max(maxInFlight, inFlight)
    await sleep(20)
    inFlight--
    return { saved: true }
  })
  store.saveNow(session('a'))
  void store.load()
  store.saveNow(session('b'))
  void store.setRestore('device', true)
  await store.flushAsync()
  assert.equal(maxInFlight, 1)
})

test('load and setRestore pass the backend result through', async () => {
  const calls: FakeCall[] = []
  const loaded = {
    restore: { device: true, vault: null, effective: true },
    session: null,
  }
  const state = { device: false, vault: null, effective: false }
  const store = useWmSession(async (cmd, args) => {
    calls.push({ cmd, args })
    return cmd === 'wm_session_load' ? loaded : state
  })
  assert.deepEqual(await store.load(), loaded)
  assert.deepEqual(await store.setRestore('device', false), state)
  assert.deepEqual(calls[1], {
    cmd: 'wm_session_restore_set',
    args: { args: { scope: 'device', enabled: false } },
  })
})

test('a session that is too large is retried once with histories reduced to their current entry', async () => {
  const calls: FakeCall[] = []
  const store = useWmSession(async (cmd, args) => {
    calls.push({ cmd, args })
    const sent = (args as { args: { session: WmSession } }).args.session
    const entries = sent.windows[0]?.tabs[0]?.history.entries.length ?? 0
    if (entries > 1) throw { kind: 'SessionTooLarge', bytes: 5_000_000 }
    return { saved: true }
  })
  store.saveNow(session('big'))
  await store.flushAsync()
  const saved = savedSessions(calls)
  assert.equal(saved.length, 2)
  const retried = saved[1]?.windows[0]?.tabs[0]?.history
  assert.equal(retried?.entries.length, 1)
  assert.equal(retried?.entries[0]?.location.path, '/thread/a')
})

test('a session still too large without histories is dropped, not retried forever', async () => {
  const calls: FakeCall[] = []
  const store = useWmSession(async (cmd, args) => {
    calls.push({ cmd, args })
    throw { kind: 'SessionTooLarge', bytes: 9_000_000 }
  })
  store.saveNow(session('huge'))
  await store.flushAsync()
  store.saveSoon(session('later'))
  await store.flushAsync()
  // Two attempts for 'huge' (full, stripped), two for 'later'; nothing re-sends 'huge'.
  const markers = savedSessions(calls).map((s) => s.activeWorkspaceId)
  assert.deepEqual(markers, ['huge', 'huge', 'later', 'later'])
})
