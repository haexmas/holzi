// Run with `node scripts/check-chat-state.ts`. Replays real page handlers
// with Tauri replaced at its IPC boundary; no browser or GPU is required.
//
// Maintainability exception (spaex 500-LoC rule): 19 replay tests plus the
// `createChatState` scaffold that boots the page's real `<script setup>`
// against injected globals. The scaffold's shape is dictated by the page
// it replays, so it must move together with the page's own split; see the
// plan in `src/pages/chat/[instance].vue`. Splitting these tests across
// files before that would duplicate the scaffold.
//
// Concrete split plan: once the harness imports the page's composables
// instead of stripping imports (step 1 of the page's plan), move
// `createChatState` into `scripts/lib/chat-state-harness.ts` and split
// the cases by the composable they exercise — transcript/event ordering,
// thread sidebar, and composer/permission state.
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import ts from 'typescript'
import { computed, nextTick, ref } from 'vue'

const source = await readFile(
  new URL('../src/pages/chat/[instance].vue', import.meta.url),
  'utf8',
)
const setupBlock = source.match(/<script setup lang="ts">([\s\S]*?)<\/script>/)
if (!setupBlock) {
  throw new Error(
    'No <script setup lang="ts"> block found in the chat page. These tests ' +
      'replay that block verbatim, so a renamed or reformatted tag silently ' +
      'removes all coverage — fix the pattern above rather than this message.',
  )
}
const setup = setupBlock[1]
const compiled = ts.transpileModule(
  setup.replace(/^import[\s\S]*?from '[^']+'\n/gm, ''),
  {
    compilerOptions: {
      target: ts.ScriptTarget.ES2022,
      module: ts.ModuleKind.None,
    },
  },
).outputText

function createChatState(
  overrides = {},
  preferenceOverrides = {},
  dependencyOverrides = {},
) {
  let mount: (() => unknown) | undefined
  let unmount: (() => unknown) | undefined
  const chat = {
    ...Object.fromEntries(
      [
        'onToken',
        'onMessageComplete',
        'onMessageError',
        'onToolCall',
        'onToolResult',
        'onRetry',
        'onTurnComplete',
        'onToolPermissionRequest',
        'onModelLoadProgress',
        'onModelLoadStatus',
        'onModelLoadError',
      ].map((name) => [name, async () => () => {}]),
    ),
    modelLoadStatusAsync: async () => ({ status: 'idle', vaultGeneration: 0 }),
    activeModelInfoAsync: async () => ({ modelId: 'model' }),
    sendMessageAsync: async () => ({
      threadId: 'a',
      userMessageId: 'u',
      assistantMessageId: 'answer',
    }),
    listThreadsAsync: async () => [
      {
        id: 'a',
        title: 'New conversation',
        lastModelId: null,
        createdAt: Date.now(),
        updatedAt: Date.now(),
      },
    ],
    renameThreadAsync: async (threadId, title) => ({
      id: threadId,
      title,
      lastModelId: null,
      createdAt: Date.now(),
      updatedAt: Date.now(),
    }),
    deleteThreadAsync: async () => {},
    listMessagesAsync: async () => [],
    ...overrides,
  }
  const globals = {
    computed,
    nextTick,
    ref,
    onMounted: (hook) => {
      mount = hook
    },
    onBeforeUnmount: (hook) => {
      unmount = hook
    },
    definePageMeta: () => {},
    useRoute: () => ({ params: { instance: 'vault' } }),
    useI18n: () => ({ t: (key) => key }),
    useChat: () => chat,
    useModels: () => ({
      listInstalledAsync: async () => [],
      onDownloadProgress: async () => () => {},
    }),
    useCatalog: () => ({ listAsync: async () => [] }),
    useProviders: () => ({ listAsync: async () => [] }),
    useInstance: () => ({}),
    usePreferences: () => ({
      getPrefAsync: async () => null,
      ...preferenceOverrides,
    }),
    useDevice: () => ({
      currentDeviceInfoAsync: async () => ({ vaultDeviceUuid: 'device' }),
    }),
    useInstancesStore: () => ({}),
    useAutoResizeTextarea: () => ({
      textareaRef: ref(null),
      isOverflowing: ref(false),
      resize: () => {},
      reset: async () => {},
    }),
    document: { querySelector: () => null },
    ...dependencyOverrides,
  }
  const state = new Function(
    ...Object.keys(globals),
    `${compiled}
    return { send, selectThread, handleToken, handleRetry, handleTurnComplete,
      historyDuration, startEditing, saveThreadTitle, cancelEditing,
      requestDelete, confirmDelete,
      handleToolPermissionRequest, threads, messagesByThread, activeThreadId,
      input, busy, activeModel, loadingPhase, modelLoadPending, draftTitle,
      editingThreadId, editTitleError, deleteCandidate, deleteError,
      composerInputDisabled, sendDisabled, pendingApprovals, streamingMessageId,
      streamingThreadId, lastError,
      updatePermissionMode, permissionMode, permissionModeSaving };
  `,
  )(...Object.values(globals))
  // Existing send-flow tests model a ready chat session unless they override it.
  state.activeModel.value = {
    modelId: 'model',
    name: 'Model',
    tokenizerRepo: 'tokenizer',
    contextWindow: null,
  }
  // The page registers both hooks in its `setup`. If a refactor ever drops
  // one, fail with that sentence instead of `mount is not a function`.
  return {
    ...state,
    mount: () => {
      if (!mount) throw new Error('setup never registered onMounted')
      return mount()
    },
    unmount: () => {
      if (!unmount) throw new Error('setup never registered onBeforeUnmount')
      return unmount()
    },
  }
}

test('history durations use Unix milliseconds and compact thresholds', () => {
  const state = createChatState()
  const now = 10 * 86_400_000
  assert.deepEqual(state.historyDuration(now - 30_000, now), {
    value: 0,
    unit: 'min',
  })
  assert.deepEqual(state.historyDuration(now - 60_000, now), {
    value: 1,
    unit: 'min',
  })
  assert.deepEqual(state.historyDuration(now - 2 * 3_600_000, now), {
    value: 2,
    unit: 'h',
  })
  assert.deepEqual(state.historyDuration(now - 5 * 86_400_000, now), {
    value: 5,
    unit: 'd',
  })
})

test('history durations clamp future and unusable timestamps to zero minutes', () => {
  const state = createChatState()
  assert.deepEqual(state.historyDuration(Date.now() + 60_000), {
    value: 0,
    unit: 'min',
  })
  assert.deepEqual(state.historyDuration(-1), { value: 0, unit: 'min' })
  assert.deepEqual(state.historyDuration(Number.NaN), {
    value: 0,
    unit: 'min',
  })
})

test('renaming a history entry trims the title and updates only that row', async () => {
  const calls: { threadId: string; title: string }[] = []
  const state = createChatState({
    renameThreadAsync: async (threadId, title) => {
      calls.push({ threadId, title })
      return {
        id: threadId,
        title,
        lastModelId: null,
        createdAt: 1_000,
        updatedAt: 2_000,
      }
    },
  })
  state.threads.value = [
    {
      id: 'a',
      title: 'Old title',
      lastModelId: null,
      createdAt: 1_000,
      updatedAt: 2_000,
    },
    {
      id: 'b',
      title: 'Keep title',
      lastModelId: null,
      createdAt: 1_000,
      updatedAt: 2_000,
    },
  ]
  state.startEditing(state.threads.value[0])
  state.draftTitle.value = '  New title  '
  await state.saveThreadTitle()
  assert.deepEqual(calls, [{ threadId: 'a', title: 'New title' }])
  assert.equal(state.threads.value[0].title, 'New title')
  assert.equal(state.threads.value[1].title, 'Keep title')
  assert.equal(state.editingThreadId.value, null)
})

test('deleting the active history entry clears its cached session without selecting another', async () => {
  let deletedId = null
  const state = createChatState({
    deleteThreadAsync: async (threadId) => {
      deletedId = threadId
    },
  })
  state.threads.value = [
    {
      id: 'a',
      title: 'Active',
      lastModelId: null,
      createdAt: 1_000,
      updatedAt: 2_000,
    },
    {
      id: 'b',
      title: 'Other',
      lastModelId: null,
      createdAt: 1_000,
      updatedAt: 2_000,
    },
  ]
  state.activeThreadId.value = 'a'
  state.messagesByThread.value = { a: [{ id: 'message' }] }
  state.requestDelete(state.threads.value[0])
  await state.confirmDelete()
  assert.equal(deletedId, 'a')
  assert.deepEqual(
    state.threads.value.map((thread) => thread.id),
    ['b'],
  )
  assert.equal(state.activeThreadId.value, null)
  assert.equal(state.messagesByThread.value.a, undefined)
})

test('deleting a running active thread aborts and waits before persistence', async () => {
  const events: string[] = []
  const chat = {
    abortAsync: async () => {
      events.push('abort')
      await state.handleTurnComplete({
        threadId: 'a',
        assistantMessageId: 'answer',
        finishReason: 'cancelled',
      })
    },
    deleteThreadAsync: async () => {
      events.push('delete')
    },
  }
  const state = createChatState(chat)
  state.threads.value = [
    {
      id: 'a',
      title: 'Active',
      lastModelId: null,
      createdAt: 1_000,
      updatedAt: 2_000,
    },
  ]
  state.activeThreadId.value = 'a'
  state.streamingThreadId.value = 'a'
  state.streamingMessageId.value = 'answer'
  state.requestDelete(state.threads.value[0])
  await state.confirmDelete()
  assert.deepEqual(events, ['abort', 'delete'])
  assert.equal(state.threads.value.length, 0)
})

test('rename failures keep the previous title and edit mode available', async () => {
  const state = createChatState({
    renameThreadAsync: async () => {
      throw new Error('persist failed')
    },
  })
  state.threads.value = [
    {
      id: 'a',
      title: 'Keep me',
      lastModelId: null,
      createdAt: 1_000,
      updatedAt: 2_000,
    },
  ]
  state.startEditing(state.threads.value[0])
  state.draftTitle.value = 'New title'
  await state.saveThreadTitle()
  assert.equal(state.threads.value[0].title, 'Keep me')
  assert.equal(state.editingThreadId.value, 'a')
  assert.equal(state.editTitleError.value, 'chat.threads.renameFailed')
})

test('delete failures keep the history entry and its confirmation target', async () => {
  const state = createChatState({
    deleteThreadAsync: async () => {
      throw new Error('persist failed')
    },
  })
  state.threads.value = [
    {
      id: 'a',
      title: 'Keep me',
      lastModelId: null,
      createdAt: 1_000,
      updatedAt: 2_000,
    },
  ]
  state.requestDelete(state.threads.value[0])
  await state.confirmDelete()
  assert.equal(state.threads.value[0].title, 'Keep me')
  assert.equal(state.deleteCandidate.value.id, 'a')
  assert.equal(state.deleteError.value, 'chat.threads.deleteFailed')
})

test('an accepted first send appears in the conversation list immediately', async () => {
  const state = createChatState()
  state.input.value = 'Hello'
  await state.send()
  assert.deepEqual(
    state.threads.value.map((thread) => thread.id),
    ['a'],
  )
  assert.equal(state.busy.value, true)
})

test('keeps a draft editable while model loading blocks sending', async () => {
  let sends = 0
  const state = createChatState({
    sendMessageAsync: async () => {
      sends++
      return { threadId: 'a', userMessageId: 'u', assistantMessageId: 'answer' }
    },
  })
  state.input.value = 'Hello while loading'
  state.activeModel.value = null
  state.modelLoadPending.value = true
  state.busy.value = true

  assert.equal(state.composerInputDisabled.value, false)
  assert.equal(state.sendDisabled.value, true)
  await state.send()
  assert.equal(sends, 0)
  assert.equal(state.input.value, 'Hello while loading')

  state.modelLoadPending.value = false
  state.busy.value = false
  state.activeModel.value = {
    modelId: 'model',
    name: 'Model',
    tokenizerRepo: 'tokenizer',
    contextWindow: null,
  }
  assert.equal(state.sendDisabled.value, false)
  await state.send()
  assert.equal(sends, 1)
})

test('tokens and retries stay with the generating thread after switching conversations', async () => {
  const state = createChatState()
  state.input.value = 'Hello'
  await state.send()
  await state.selectThread('b')
  state.handleToken({
    messageId: 'answer',
    delta: 'Failed attempt',
    reasoning: null,
  })
  assert.equal(state.messagesByThread.value.a[1].content, 'Failed attempt')
  state.handleRetry({ threadId: 'a', assistantMessageId: 'answer', attempt: 1 })
  assert.equal(state.messagesByThread.value.a[1].content, '')
  state.handleToken({
    messageId: 'answer',
    delta: 'Successful answer',
    reasoning: null,
  })
  assert.equal(state.messagesByThread.value.a[1].content, 'Successful answer')
  assert.deepEqual(state.messagesByThread.value.b, [])
})

test('turn completion restores persisted step order and removes cancelled approvals', async () => {
  const persisted = [
    { id: 'u', role: 'user', content: 'Inspect' },
    { id: 'interim', role: 'assistant', content: 'Inspecting' },
    { id: 'call', role: 'tool_call', content: '' },
    { id: 'result', role: 'tool_result', content: 'Found' },
    {
      id: 'answer',
      role: 'assistant',
      content: 'Done',
      finishReason: 'cancelled',
    },
  ]
  const state = createChatState({ listMessagesAsync: async () => persisted })
  state.input.value = 'Inspect'
  await state.send()
  state.handleToken({
    messageId: 'answer',
    delta: 'InspectingDone',
    reasoning: null,
  })
  state.handleToolPermissionRequest({
    threadId: 'a',
    requestId: 'approval',
    toolName: 'run_command',
  })
  await state.handleTurnComplete({
    threadId: 'a',
    assistantMessageId: 'answer',
    finishReason: 'cancelled',
  })
  assert.deepEqual(state.messagesByThread.value.a, persisted)
  assert.deepEqual(state.pendingApprovals.value, [])
  assert.equal(state.busy.value, false)
  assert.equal(state.streamingMessageId.value, null)
})

test('completion clears approvals queued on a background conversation', async () => {
  const state = createChatState()
  state.input.value = 'Inspect'
  await state.send()
  state.handleToolPermissionRequest({
    threadId: 'a',
    requestId: 'approval',
    toolName: 'run_command',
  })
  await state.selectThread('b')
  await state.handleTurnComplete({
    threadId: 'a',
    assistantMessageId: 'answer',
    finishReason: 'cancelled',
  })
  await state.selectThread('a')
  assert.deepEqual(state.pendingApprovals.value, [])
})

test('a terminal event before the invoke response cannot leave the composer busy', async () => {
  let acceptSend!: (value?: unknown) => void
  const state = createChatState({
    sendMessageAsync: () =>
      new Promise((resolve) => {
        acceptSend = resolve
      }),
  })
  state.input.value = 'Inspect'
  const sending = state.send()
  await state.handleTurnComplete({
    threadId: 'a',
    assistantMessageId: null,
    finishReason: 'error',
  })
  acceptSend({
    threadId: 'a',
    userMessageId: 'u',
    assistantMessageId: 'answer',
  })
  await sending
  assert.equal(state.busy.value, false)
  assert.deepEqual(state.messagesByThread.value.a, [])
})

test('history refresh failure surfaces the error and still releases the composer', async () => {
  const state = createChatState({
    listMessagesAsync: async () => {
      throw new Error('storage unavailable')
    },
  })
  state.input.value = 'Inspect'
  await state.send()
  await state.handleTurnComplete({
    threadId: 'a',
    assistantMessageId: 'answer',
    finishReason: 'cancelled',
  })
  assert.equal(state.busy.value, false)
  assert.match(state.lastError.value, /storage unavailable/)
  assert.equal(state.messagesByThread.value.a[1].finishReason, 'cancelled')
})

test('replaying a lost invoke response recovers a completed turn without new events or duplicate rows', async () => {
  let attempts = 0
  const requests: { idempotencyKey?: string }[] = []
  const persisted = [
    { id: 'u', role: 'user', content: 'Hello' },
    {
      id: 'answer',
      role: 'assistant',
      content: 'Already answered',
      finishReason: 'complete',
    },
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
  const state = createChatState({
    abortAsync: async () => {
      aborted++
    },
  })
  state.input.value = 'Inspect'
  await state.send()
  state.handleToolPermissionRequest({
    threadId: 'a',
    requestId: 'approval',
    toolName: 'run_command',
  })
  await state.unmount()
  assert.equal(aborted, 1)
})

test('leaving while the initial request starts aborts before response IDs exist', async () => {
  let aborted = 0
  let rejectSend
  const state = createChatState({
    abortAsync: async () => {
      aborted++
      rejectSend(new Error('cancelled'))
    },
    sendMessageAsync: () =>
      new Promise((_, reject) => {
        rejectSend = reject
      }),
  })
  state.input.value = 'Inspect'
  const sending = state.send()
  await state.unmount()
  await sending
  assert.equal(aborted, 1)
  assert.equal(state.busy.value, false)
})

test('listener registrations finishing after unmount are disposed without continuing initialization', async () => {
  let finishRegistration!: (value?: unknown) => void
  let deviceReads = 0
  const released: string[] = []
  const subscriptions = [
    'onToken',
    'onMessageComplete',
    'onMessageError',
    'onToolCall',
    'onToolResult',
    'onRetry',
    'onTurnComplete',
    'onToolPermissionRequest',
    'onModelLoadProgress',
    'onModelLoadStatus',
  ]
  const state = createChatState(
    {
      ...Object.fromEntries(
        subscriptions.map((name) => [
          name,
          async () => () => released.push(name),
        ]),
      ),
      onToken: () =>
        new Promise((resolve) => {
          finishRegistration = resolve
        }),
    },
    {},
    {
      useDevice: () => ({
        currentDeviceInfoAsync: async () => {
          deviceReads++
          return {}
        },
      }),
      useModels: () => ({
        onDownloadProgress: async () => () => released.push('download'),
      }),
    },
  )
  const mounting = state.mount()
  await nextTick()
  await state.unmount()
  finishRegistration(() => released.push('onToken'))
  await mounting
  assert.deepEqual(released.sort(), [...subscriptions, 'download'].sort())
  assert.equal(deviceReads, 0)
})

test('a failed permission save restores the persisted mode and prevents overlapping changes', async () => {
  let rejectSave!: (reason?: unknown) => void
  let writes = 0
  const state = createChatState(
    {},
    {
      setPrefAsync: () => {
        writes++
        return new Promise((_, reject) => {
          rejectSave = reject
        })
      },
    },
  )
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
