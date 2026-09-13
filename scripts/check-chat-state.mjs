// Run with `node scripts/check-chat-state.mjs`. Replays real page handlers
// with Tauri replaced at its IPC boundary; no browser or GPU is required.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import ts from 'typescript'
import { computed, nextTick, ref } from 'vue'

const source = await readFile(new URL('../src/pages/chat/[instance].vue', import.meta.url), 'utf8')
const setup = source.match(/<script setup lang="ts">([\s\S]*?)<\/script>/)[1]
const compiled = ts.transpileModule(setup.replace(/^import[\s\S]*?from '[^']+'\n/gm, ''), {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None },
}).outputText

function createChatState(overrides = {}, preferenceOverrides = {}, dependencyOverrides = {}) {
  let mount
  let unmount
  const chat = {
    ...Object.fromEntries(['onToken', 'onMessageComplete', 'onMessageError', 'onToolCall',
      'onToolResult', 'onRetry', 'onTurnComplete', 'onToolPermissionRequest',
      'onModelLoadProgress'].map((name) => [name, async () => () => {}])),
    activeModelInfoAsync: async () => ({ modelId: 'model' }),
    sendMessageAsync: async () => ({ threadId: 'a', userMessageId: 'u', assistantMessageId: 'answer' }),
    listThreadsAsync: async () => [{ id: 'a', title: 'New conversation' }],
    listMessagesAsync: async () => [],
    ...overrides,
  }
  const globals = {
    computed, nextTick, ref,
    onMounted: (hook) => { mount = hook },
    onBeforeUnmount: (hook) => { unmount = hook },
    definePageMeta: () => {},
    useRoute: () => ({ params: { instance: 'vault' } }),
    useI18n: () => ({ t: (key) => key }),
    useChat: () => chat,
    useModels: () => ({ listInstalledAsync: async () => [], onDownloadProgress: async () => () => {} }),
    useCatalog: () => ({ listAsync: async () => [] }),
    useProviders: () => ({ listAsync: async () => [] }),
    useInstance: () => ({}),
    usePreferences: () => ({ getPrefAsync: async () => null, ...preferenceOverrides }),
    useDevice: () => ({ currentDeviceInfoAsync: async () => ({ vaultDeviceUuid: 'device' }) }),
    useInstancesStore: () => ({}),
    document: { querySelector: () => null },
    ...dependencyOverrides,
  }
  const state = new Function(...Object.keys(globals), `${compiled}
    return { send, selectThread, handleToken, handleRetry, handleTurnComplete,
      handleToolPermissionRequest, threads, messagesByThread, activeThreadId,
      input, busy, pendingApprovals, streamingMessageId, lastError,
      updatePermissionMode, permissionMode, permissionModeSaving };
  `)(...Object.values(globals))
  return { ...state, mount: () => mount(), unmount: () => unmount() }
}

test('an accepted first send appears in the conversation list immediately', async () => {
  const state = createChatState()
  state.input.value = 'Hello'
  await state.send()
  assert.deepEqual(state.threads.value.map((thread) => thread.id), ['a'])
  assert.equal(state.busy.value, true)
})

test('tokens and retries stay with the generating thread after switching conversations', async () => {
  const state = createChatState()
  state.input.value = 'Hello'
  await state.send()
  await state.selectThread('b')
  state.handleToken({ messageId: 'answer', delta: 'Failed attempt', reasoning: null })
  assert.equal(state.messagesByThread.value.a[1].content, 'Failed attempt')
  state.handleRetry({ threadId: 'a', assistantMessageId: 'answer', attempt: 1 })
  assert.equal(state.messagesByThread.value.a[1].content, '')
  state.handleToken({ messageId: 'answer', delta: 'Successful answer', reasoning: null })
  assert.equal(state.messagesByThread.value.a[1].content, 'Successful answer')
  assert.deepEqual(state.messagesByThread.value.b, [])
})

test('turn completion restores persisted step order and removes cancelled approvals', async () => {
  const persisted = [
    { id: 'u', role: 'user', content: 'Inspect' },
    { id: 'interim', role: 'assistant', content: 'Inspecting' },
    { id: 'call', role: 'tool_call', content: '' },
    { id: 'result', role: 'tool_result', content: 'Found' },
    { id: 'answer', role: 'assistant', content: 'Done', finishReason: 'cancelled' },
  ]
  const state = createChatState({ listMessagesAsync: async () => persisted })
  state.input.value = 'Inspect'
  await state.send()
  state.handleToken({ messageId: 'answer', delta: 'InspectingDone', reasoning: null })
  state.handleToolPermissionRequest({ threadId: 'a', requestId: 'approval', toolName: 'run_command' })
  await state.handleTurnComplete({ threadId: 'a', assistantMessageId: 'answer', finishReason: 'cancelled' })
  assert.deepEqual(state.messagesByThread.value.a, persisted)
  assert.deepEqual(state.pendingApprovals.value, [])
  assert.equal(state.busy.value, false)
  assert.equal(state.streamingMessageId.value, null)
})

test('completion clears approvals queued on a background conversation', async () => {
  const state = createChatState()
  state.input.value = 'Inspect'
  await state.send()
  state.handleToolPermissionRequest({ threadId: 'a', requestId: 'approval', toolName: 'run_command' })
  await state.selectThread('b')
  await state.handleTurnComplete({ threadId: 'a', assistantMessageId: 'answer', finishReason: 'cancelled' })
  await state.selectThread('a')
  assert.deepEqual(state.pendingApprovals.value, [])
})

test('a terminal event before the invoke response cannot leave the composer busy', async () => {
  let acceptSend
  const state = createChatState({
    sendMessageAsync: () => new Promise((resolve) => { acceptSend = resolve }),
  })
  state.input.value = 'Inspect'
  const sending = state.send()
  await state.handleTurnComplete({ threadId: 'a', assistantMessageId: null, finishReason: 'error' })
  acceptSend({ threadId: 'a', userMessageId: 'u', assistantMessageId: 'answer' })
  await sending
  assert.equal(state.busy.value, false)
  assert.deepEqual(state.messagesByThread.value.a, [])
})

test('history refresh failure surfaces the error and still releases the composer', async () => {
  const state = createChatState({ listMessagesAsync: async () => { throw new Error('storage unavailable') } })
  state.input.value = 'Inspect'
  await state.send()
  await state.handleTurnComplete({ threadId: 'a', assistantMessageId: 'answer', finishReason: 'cancelled' })
  assert.equal(state.busy.value, false)
  assert.match(state.lastError.value, /storage unavailable/)
  assert.equal(state.messagesByThread.value.a[1].finishReason, 'cancelled')
})

test('replaying a lost invoke response recovers a completed turn without new events or duplicate rows', async () => {
  let attempts = 0
  const requests = []
  const persisted = [
    { id: 'u', role: 'user', content: 'Hello' },
    { id: 'answer', role: 'assistant', content: 'Already answered', finishReason: 'complete' },
  ]
  const state = createChatState({
    sendMessageAsync: async (request) => {
      requests.push(request)
      if (attempts++ === 0) throw new Error('invoke response lost')
      return { threadId: 'a', userMessageId: 'u', assistantMessageId: 'answer' }
    },
    listMessagesAsync: async () => persisted,
  })
  state.input.value = 'Hello'
  await state.send()
  assert.equal(state.busy.value, false)
  await state.send(true)
  assert.equal(requests[0].idempotencyKey, requests[1].idempotencyKey)
  assert.deepEqual(state.messagesByThread.value.a, persisted)
  assert.equal(state.busy.value, false)
  assert.equal(state.streamingMessageId.value, null)
})


test('leaving a chat with an approval pending aborts its active turn', async () => {
  let aborted = 0
  const state = createChatState({ abortAsync: async () => { aborted++ } })
  state.input.value = 'Inspect'
  await state.send()
  state.handleToolPermissionRequest({ threadId: 'a', requestId: 'approval', toolName: 'run_command' })
  await state.unmount()
  assert.equal(aborted, 1)
})

test('leaving while the initial request starts aborts before response IDs exist', async () => {
  let aborted = 0
  let rejectSend
  const state = createChatState({
    abortAsync: async () => { aborted++; rejectSend(new Error('cancelled')) },
    sendMessageAsync: () => new Promise((_, reject) => { rejectSend = reject }),
  })
  state.input.value = 'Inspect'
  const sending = state.send()
  await state.unmount()
  await sending
  assert.equal(aborted, 1)
  assert.equal(state.busy.value, false)
})

test('listener registrations finishing after unmount are disposed without continuing initialization', async () => {
  let finishRegistration
  let deviceReads = 0
  const released = []
  const subscriptions = ['onToken', 'onMessageComplete', 'onMessageError', 'onToolCall',
    'onToolResult', 'onRetry', 'onTurnComplete', 'onToolPermissionRequest', 'onModelLoadProgress']
  const state = createChatState({
    ...Object.fromEntries(subscriptions.map((name) => [name, async () => () => released.push(name)])),
    onToken: () => new Promise((resolve) => { finishRegistration = resolve }),
  }, {}, {
    useDevice: () => ({ currentDeviceInfoAsync: async () => { deviceReads++; return {} } }),
    useModels: () => ({ onDownloadProgress: async () => () => released.push('download') }),
  })
  const mounting = state.mount()
  await nextTick()
  await state.unmount()
  finishRegistration(() => released.push('onToken'))
  await mounting
  assert.deepEqual(released.sort(), [...subscriptions, 'download'].sort())
  assert.equal(deviceReads, 0)
})

test('a failed permission save restores the persisted mode and prevents overlapping changes', async () => {
  let rejectSave
  let writes = 0
  const state = createChatState({}, {
    setPrefAsync: () => { writes++; return new Promise((_, reject) => { rejectSave = reject }) },
  })
  await state.mount()
  const saving = state.updatePermissionMode('plan')
  assert.equal(state.permissionModeSaving.value, true)
  await state.updatePermissionMode('auto')
  assert.equal(writes, 1)
  rejectSave(new Error('preference write failed'))
  await saving
  assert.equal(state.permissionMode.value, 'manual')
  assert.equal(state.permissionModeSaving.value, false)
  assert.match(state.lastError.value, /preference write failed/)
})
