// Run with `node scripts/check-vault-lifecycle.ts`. Frontend cases for spec 013 (vault lifecycle
// isolation, specs/013-vault-lifecycle-isolation): the lock flows, the passphrase lifetime in the
// unlock and create sheets, and the reads that must never fail. They run on the shared replay
// harness in `scripts/lib/chat-state-harness.ts`. New frontend cases for this feature go here,
// never into the oversized `scripts/check-chat-state.ts`.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createChatState } from './lib/chat-state-harness.ts'

test('the shared harness boots the chat page against the IPC double', () => {
  const state = createChatState()
  assert.equal(state.busy.value, false)
  assert.equal(state.input.value, '')
})
