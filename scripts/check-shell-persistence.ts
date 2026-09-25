// Part of `pnpm check:shell-state` (spec 015-workspace-shell): the persistence queue
// (useShellLayout.ts). `useShellLayout` takes an injectable `invokeFn` (defaulting to the real
// Tauri `invoke`) specifically so its queue/debounce/dirty-retry logic runs here without a Tauri
// runtime or module mocking. Split from check-shell-state.ts per plan.md Complexity Tracking
// (T059).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  useShellLayout,
  type ShellLayoutDto,
  type WindowDto,
} from '../src/composables/useShellLayout.ts'

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
