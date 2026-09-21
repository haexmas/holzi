// Run with `node scripts/check-vault-lifecycle.ts`. Frontend cases for spec 013 (vault lifecycle
// isolation, specs/013-vault-lifecycle-isolation): the lock flows, the passphrase lifetime in the
// unlock and create sheets, and the reads that must never fail. They run on the shared replay
// harness in `scripts/lib/chat-state-harness.ts`. New frontend cases for this feature go here,
// never into the oversized `scripts/check-chat-state.ts`.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createChatState } from './lib/chat-state-harness.ts'
import { loadScriptSetup } from './lib/script-setup-sandbox.ts'

test('the shared harness boots the chat page against the IPC double', () => {
  const state = createChatState()
  assert.equal(state.busy.value, false)
  assert.equal(state.input.value, '')
})

// The lock flows (US1, FR-002): the lock control only asks the backend to close. The backend
// replaces the page with a spinner and ends the process, so the page keeps no closing state,
// navigates nowhere and does not clear the active instance itself.

/** Records what a lock flow does besides asking for the close. */
function watchedPage() {
  const effects: string[] = []
  return {
    effects,
    instancesStore: {
      setActiveInstance: () => effects.push('cleared the instance'),
    },
    navigateTo: (to: string) => effects.push(`navigated to ${to}`),
  }
}

test('the chat page lock() asks for the close once and does nothing else', async () => {
  const invokes: string[] = []
  const page = watchedPage()
  const state = createChatState({}, {}, {}, invokes, page)
  invokes.length = 0

  await state.lock()

  assert.deepEqual(invokes, ['close_instance'])
  assert.deepEqual(page.effects, [])
})

test('the chat page lock() swallows a rejected close and shows no error', async () => {
  const page = watchedPage()
  const state = createChatState(
    {},
    {},
    {
      useInstance: () => ({
        closeAsync: async () => {
          throw new Error('the page is already gone')
        },
      }),
    },
    undefined,
    page,
  )
  const before = state.lastError.value

  await state.lock()

  assert.equal(state.lastError.value, before)
  assert.deepEqual(page.effects, [])
})

test('the federation page onLock() asks for the close once and does nothing else', async () => {
  const page = watchedPage()
  let closes = 0
  const { onLock } = loadScriptSetup<{ onLock: () => Promise<void> }>(
    'src/pages/federation/[instance].vue',
    ['onLock'],
    {
      useInstance: () => ({
        closeAsync: async () => {
          closes += 1
        },
      }),
      useInstancesStore: () => page.instancesStore,
      navigateTo: page.navigateTo,
    },
  )

  await onLock()

  assert.equal(closes, 1)
  assert.deepEqual(page.effects, [])
})

test('the federation page onLock() swallows a rejected close', async () => {
  const page = watchedPage()
  const { onLock } = loadScriptSetup<{ onLock: () => Promise<void> }>(
    'src/pages/federation/[instance].vue',
    ['onLock'],
    {
      useInstance: () => ({
        closeAsync: async () => {
          throw new Error('the page is already gone')
        },
      }),
      useInstancesStore: () => page.instancesStore,
      navigateTo: page.navigateTo,
    },
  )

  await onLock()

  assert.deepEqual(page.effects, [])
})
