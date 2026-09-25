// Part of `pnpm check:wm-state` (spec 015-workspace-shell): the persistence queue
// (useWmLayout.ts). `useWmLayout` takes an injectable `invokeFn` (defaulting to the real
// Tauri `invoke`) specifically so its queue/debounce/dirty-retry logic runs here without a Tauri
// runtime or module mocking. Split from check-wm-state.ts per plan.md Complexity Tracking
// (T059).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  useWmLayout,
  type WmLayoutDto,
  type WindowDto,
} from '../src/composables/useWmLayout.ts'

// ---------------------------------------------------------------------------
// Persistence queue (useWmLayout.ts)
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

test('useWmLayout debounces rapid geometry saves into one call with the latest value', async () => {
  const calls: FakeCall[] = []
  const layout = useWmLayout(async (cmd, args) => {
    calls.push({ cmd, args })
    return null
  })
  layout.saveWindowDebounced(makeWindowDto('w1', 1))
  layout.saveWindowDebounced(makeWindowDto('w1', 2))
  layout.saveWindowDebounced(makeWindowDto('w1', 3))
  await sleep(500)
  await layout.flushAsync()
  const saveCalls = calls.filter((c) => c.cmd === 'wm_save_windows')
  assert.equal(saveCalls.length, 1)
  const windows = (saveCalls[0]?.args as { args: { windows: WindowDto[] } })
    .args.windows
  assert.equal(windows.length, 1)
  assert.equal(windows[0]?.x, 3)
})

test('useWmLayout persists a structural save immediately, without waiting for the debounce', async () => {
  const calls: FakeCall[] = []
  const layout = useWmLayout(async (cmd, args) => {
    calls.push({ cmd, args })
    return null
  })
  layout.saveWindowNow(makeWindowDto('w2', 10))
  await layout.flushAsync()
  assert.equal(calls.filter((c) => c.cmd === 'wm_save_windows').length, 1)
})

test('useWmLayout keeps a failed save dirty and retries it on the next save', async () => {
  const calls: FakeCall[] = []
  let failNext = true
  const layout = useWmLayout(async (cmd, args) => {
    calls.push({ cmd, args })
    if (cmd === 'wm_save_windows' && failNext) {
      failNext = false
      throw new Error('simulated failure')
    }
    return null
  })
  layout.saveWindowNow(makeWindowDto('w3', 5))
  await layout.flushAsync()
  layout.saveWindowNow(makeWindowDto('w4', 6)) // unrelated window, later save
  await layout.flushAsync()
  const saveCalls = calls.filter((c) => c.cmd === 'wm_save_windows')
  assert.equal(saveCalls.length, 2)
  const secondBatchIds = (
    saveCalls[1]?.args as { args: { windows: WindowDto[] } }
  ).args.windows
    .map((w) => w.windowId)
    .sort()
  assert.deepEqual(secondBatchIds, ['w3', 'w4'])
})

test('useWmLayout cancels a pending save when the same window is closed', async () => {
  const calls: FakeCall[] = []
  const layout = useWmLayout(async (cmd, args) => {
    calls.push({ cmd, args })
    return null
  })
  layout.saveWindowDebounced(makeWindowDto('w5', 1))
  layout.closeWindowNow('w5')
  await layout.flushAsync()
  assert.equal(calls.filter((c) => c.cmd === 'wm_save_windows').length, 0)
  assert.equal(calls.filter((c) => c.cmd === 'wm_close_windows').length, 1)
})

test('useWmLayout never runs two wm_* calls at once', async () => {
  const order: string[] = []
  const layout = useWmLayout(async (cmd) => {
    order.push(`${cmd}:start`)
    await sleep(20)
    order.push(`${cmd}:end`)
    return null
  })
  layout.saveWindowNow(makeWindowDto('w6', 1))
  layout.closeWindowNow('w6')
  await layout.flushAsync()
  assert.ok(
    order.indexOf('wm_save_windows:end') <
      order.indexOf('wm_close_windows:start'),
    `expected the save to fully finish before the close started, got ${JSON.stringify(order)}`,
  )
})

test('useWmLayout.loadLayout returns whatever the backend sends', async () => {
  const fakeLayout: WmLayoutDto = {
    workspaces: [{ workspaceId: 'ws-1', position: 0 }],
    windows: [],
    activeWorkspaceId: 'ws-1',
  }
  const layout = useWmLayout(async () => fakeLayout)
  const result = await layout.loadLayout()
  assert.deepEqual(result, fakeLayout)
})
