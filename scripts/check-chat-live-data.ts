// `pnpm check:chat-state` (spec 024 FR-032): the chat page follows changes to the vault's data
// that it did not make itself — another device's threads and messages, a preference changed in
// another window — without a reload. Replays the real page like check-chat-state.ts; the
// harness's `emitVaultChange` stands in for the backend's `vault-data-changed` event.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { createChatState } from './lib/chat-state-harness.ts'

const NOW = 1_700_000_000_000

function thread(id: string, title: string) {
  return { id, title, lastModelId: null, createdAt: NOW, updatedAt: NOW }
}

function message(id: string, content: string) {
  return {
    id,
    threadId: 'a',
    role: 'user',
    content,
    createdAt: NOW,
    finishReason: null,
  }
}

test('a thread another device created appears in the sidebar', async () => {
  const state = createChatState({
    listThreadsAsync: async () => [
      thread('a', 'Mine'),
      thread('b', 'From the other device'),
    ],
  })
  assert.deepEqual(state.threads.value, [])

  await state.emitVaultChange(['chat_threads'])

  assert.deepEqual(
    state.threads.value.map((t: { id: string }) => t.id),
    ['a', 'b'],
  )
})

test('a message another device added shows in an open thread', async () => {
  let remote = [message('m1', 'first')]
  const state = createChatState({ listMessagesAsync: async () => remote })
  await state.selectThread('a')
  assert.equal(state.messagesByThread.value.a.length, 1)

  remote = [message('m1', 'first'), message('m2', 'second')]
  await state.emitVaultChange(['chat_messages'])

  assert.deepEqual(
    state.messagesByThread.value.a.map((m: { id: string }) => m.id),
    ['m1', 'm2'],
  )
})

test('changes to other tables leave the chat alone', async () => {
  let reads = 0
  const state = createChatState({
    listMessagesAsync: async () => {
      reads++
      return []
    },
    listThreadsAsync: async () => {
      reads++
      return []
    },
  })
  await state.selectThread('a')
  reads = 0

  await state.emitVaultChange(['providers', 'known_devices'])

  assert.equal(reads, 0)
})

test('messages are not reloaded while a turn runs here', async () => {
  let remote = [message('m1', 'first')]
  const state = createChatState({ listMessagesAsync: async () => remote })
  await state.selectThread('a')

  state.busy.value = true
  remote = [message('m1', 'first'), message('m2', 'second')]
  await state.emitVaultChange(['chat_messages'])

  assert.equal(state.messagesByThread.value.a.length, 1)
})

test('the thread a reply is streaming into is left alone', async () => {
  let remote = [message('m1', 'first')]
  const state = createChatState({ listMessagesAsync: async () => remote })
  await state.selectThread('a')

  state.streamingThreadId.value = 'a'
  remote = [message('m1', 'first'), message('m2', 'second')]
  await state.emitVaultChange(['chat_messages'])

  assert.equal(state.messagesByThread.value.a.length, 1)
})

test('a failing reload is shown as the chat error and later changes still reload', async () => {
  let fail = true
  const state = createChatState({
    listThreadsAsync: async () => {
      if (fail) throw new Error('database busy')
      return [thread('a', 'Back')]
    },
  })

  await state.emitVaultChange(['chat_threads'])
  assert.match(String(state.lastError.value), /database busy/)

  fail = false
  await state.emitVaultChange(['chat_threads'])
  assert.equal(state.threads.value[0].title, 'Back')
})
