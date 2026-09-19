// Run with `node scripts/check-chat-state.ts`. Replays the real page, its
// real composables (`useChat`, `useModels`, `useCatalog`, `useProviders`,
// `useInstance`, `usePreferences`, `useDevice`, `useAutoResizeTextarea`,
// `useErrorString`) and the real `useModelsStore` (a genuine Pinia instance,
// `createPinia`/`setActivePinia`, fresh per test) with Tauri replaced at its
// actual IPC boundary (`invoke`/`listen` from `@tauri-apps/api/core`/
// `event`). No browser or GPU is required.
//
// Maintainability exception (spaex 500-LoC rule): 25 replay tests plus the
// `createChatState` scaffold that boots the page's real `<script setup>`
// against a sandboxed `require` — a hand-rolled CommonJS loader (using the
// `typescript` package already a dependency here) that transpiles the page,
// the store and every composable either imports, then executes each in its
// own `new Function('exports', 'require', ...)` sandbox. Real composable
// imports used to be stripped and replaced with hand-written fakes matching
// their public method names; that made any logic later moved into a
// composable invisible to these tests. Now only three things are faked:
// Tauri's own `invoke`/`listen` (there is no backend here),
// `onMounted`/`onBeforeUnmount` (no real Vue component instance exists to
// register them against — every composable's calls are collected into one
// array `mount()`/`unmount()` drain), and `dompurify` (needs a real DOM).
//
// Concrete split plan, if this grows further: move `createChatState` into
// `scripts/lib/chat-state-harness.ts` and split the 25 cases by what they
// exercise — transcript/event ordering, thread sidebar, model lifecycle,
// and composer/permission state.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, resolve as resolvePath } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'
import ts from 'typescript'
import { nextTick } from 'vue'

/** Same shape the page's own `useI18n()` gets — see the bare-globals list below. */
const useI18nDouble = () => ({ t: (key: string) => key })

const __dirname = dirname(fileURLToPath(import.meta.url))
const repoRoot = resolvePath(__dirname, '..')
const nodeRequire = createRequire(import.meta.url)

const pageSource = readFileSync(
  resolvePath(repoRoot, 'src/pages/chat/[instance].vue'),
  'utf8',
)
const setupBlock = pageSource.match(
  /<script setup lang="ts">([\s\S]*?)<\/script>/,
)
if (!setupBlock) {
  throw new Error(
    'No <script setup lang="ts"> block found in the chat page. These tests ' +
      'replay that block verbatim, so a renamed or reformatted tag silently ' +
      'removes all coverage — fix the pattern above rather than this message.',
  )
}

const CJS_OPTIONS = {
  compilerOptions: {
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.CommonJS,
    esModuleInterop: true,
  },
}

const transpiledFileCache = new Map<string, string>()

function transpileFile(absPath: string): string {
  let cached = transpiledFileCache.get(absPath)
  if (cached === undefined) {
    cached = ts.transpileModule(
      readFileSync(absPath, 'utf8'),
      CJS_OPTIONS,
    ).outputText
    transpiledFileCache.set(absPath, cached)
  }
  return cached
}

const pageCode = ts.transpileModule(setupBlock[1], CJS_OPTIONS).outputText

function composablePath(name: string): string {
  return resolvePath(repoRoot, 'src/composables', `${name}.ts`)
}

/**
 * Executes one real composable file's transpiled CommonJS output in its own
 * sandbox and returns its exports. `req` is threaded through so a composable
 * that itself imports another `~/composables/*` module (none do today, but
 * a future page split may add one) resolves the same way as the page does.
 *
 * Returns whatever `Function`'s own call signature returns (loosely typed
 * by design, like the rest of this sandbox — see `noImplicitAny` above).
 */
function runComposable(
  absPath: string,
  req: (specifier: string) => unknown,
  autoImports: Record<string, unknown> = {},
) {
  const autoImportNames = Object.keys(autoImports)
  return new Function(
    'exports',
    'require',
    // `useModelsStore` (and `useErrorString`, which it depends on) call the
    // real ambient `useI18n()` — no `import` in their source, same as the
    // page's own macros — so it's injected here too, not just for the page.
    'useI18n',
    ...autoImportNames,
    `${transpileFile(absPath)}\nreturn exports;`,
  )({}, req, useI18nDouble, ...autoImportNames.map((name) => autoImports[name]))
}

type InvokeHandler = (args?: unknown) => unknown

/**
 * Minimal Tauri IPC double. `invoke` dispatches by command name to a
 * per-`createChatState` handler map (defaults below, overridable per test
 * via `overrides`/`preferenceOverrides`, which replace a composable's
 * *method* — the same shape as today's real `use*()` return value — not
 * the `invoke` layer itself). `listen` just records subscribers and never
 * fires them: every one of the 19 cases below drives page state by calling
 * its returned handlers directly (e.g. `state.handleToken(...)`), so this
 * only needs to resolve without hanging or throwing.
 */
function createTauriDouble(invokeHandlers: Record<string, InvokeHandler>) {
  async function invoke(cmd: string, args?: unknown) {
    const handler = invokeHandlers[cmd]
    if (!handler) {
      throw new Error(
        `check-chat-state harness: no invoke handler for '${cmd}'`,
      )
    }
    return handler(args)
  }
  async function listen(
    _event: string,
    _cb: (e: { payload: unknown }) => void,
  ) {
    return () => {}
  }
  return { invoke, listen }
}

/** Matches the page's default `createChatState()` mount flow — see the
 * one test that awaits `mount()` to completion. */
const DEFAULT_INVOKE_HANDLERS: Record<string, InvokeHandler> = {
  list_threads: () => [
    {
      id: 'a',
      title: 'New conversation',
      lastModelId: null,
      createdAt: Date.now(),
      updatedAt: Date.now(),
    },
  ],
  list_messages: () => [],
  rename_thread: (raw) => {
    const { args } = raw as { args: { threadId: string; title: string } }
    return {
      id: args.threadId,
      title: args.title,
      lastModelId: null,
      createdAt: Date.now(),
      updatedAt: Date.now(),
    }
  },
  delete_thread: () => {},
  send_message: () => ({
    threadId: 'a',
    userMessageId: 'u',
    assistantMessageId: 'answer',
  }),
  active_model_info: () => ({ modelId: 'model' }),
  model_load_status: () => ({ status: 'idle', vaultGeneration: 0 }),
  get_pref: () => null,
  list_installed_models: () => [],
  list_catalog: () => [],
  list_providers: () => [],
  current_device_info: () => ({ vaultDeviceUuid: 'device' }),
  // Inert by default: `active_model_info` above already returns a truthy
  // model, so `models.ts`'s `autoLoadFirstAvailableModel()` short-circuits
  // before ever calling this — only reached by a test that overrides
  // `active_model_info` to simulate nothing being active yet.
  resolve_default_model: () => ({ modelId: null, source: 'none' }),
}

const RETURN_STATEMENT = `
    return { send, selectThread, handleToken, handleRetry, handleTurnComplete,
      historyDuration, startEditing, saveThreadTitle, cancelEditing,
      requestDelete, confirmDelete,
      handleToolPermissionRequest, threads, messagesByThread, activeThreadId,
      input, busy, activeModel, loadingPhase, modelLoadPending, draftTitle,
      editingThreadId, editTitleError, deleteCandidate, deleteError,
      composerInputDisabled, sendDisabled, pendingApprovals, streamingMessageId,
      streamingThreadId, lastError,
      updatePermissionMode, permissionMode, permissionModeSaving };
`

/** Resolves a store module name to its source file in this checkout. */
function storePath(name: string): string {
  return resolvePath(repoRoot, 'src/stores', `${name}.ts`)
}

/** Boots the real chat page dependencies inside an isolated test sandbox. */
function createChatState(
  overrides: Record<string, unknown> = {},
  preferenceOverrides: Record<string, unknown> = {},
  dependencyOverrides: Record<string, (...args: unknown[]) => unknown> = {},
) {
  const tauri = createTauriDouble({ ...DEFAULT_INVOKE_HANDLERS })

  const mountHooks: Array<() => unknown> = []
  const unmountHooks: Array<() => unknown> = []
  const vueDouble = {
    ...(nodeRequire('vue') as object),
    onMounted: (hook: () => unknown) => mountHooks.push(hook),
    onBeforeUnmount: (hook: () => unknown) => unmountHooks.push(hook),
  }

  /**
   * Resolves imports for the sandbox while sharing overridable chat and
   * preference instances with the model store. This hoisted declaration is
   * called only after those instances have been initialized below.
   */
  function req(specifier: string) {
    if (specifier === 'vue') return vueDouble
    if (specifier === 'pinia') return nodeRequire('pinia')
    if (specifier === '@tauri-apps/api/core') return { invoke: tauri.invoke }
    if (specifier === '@tauri-apps/api/event') return { listen: tauri.listen }
    if (specifier === 'dompurify') {
      const identity = (html: string) => html
      return { default: { sanitize: identity }, sanitize: identity }
    }
    if (specifier === 'marked') return nodeRequire('marked')
    if (specifier === '~/composables/useChat') return { useChat: () => chat }
    if (specifier === '~/composables/usePreferences') {
      // `usePreferences()` itself is overridden below to share the one
      // `preferences` instance every consumer in this sandbox sees, but its
      // other named exports (`AUTONOMY_MODES`/`isAutonomyMode`) are plain,
      // side-effect-free values — running the real module for those costs
      // nothing and means the page's replayed autonomy-mode validation
      // exercises the actual shared helper instead of a hand-duplicated one.
      const real = runComposable(composablePath('usePreferences'), req)
      return { ...real, usePreferences: () => preferences }
    }
    if (specifier.startsWith('~/composables/')) {
      const name = specifier.slice('~/composables/'.length)
      const real = runComposable(
        composablePath(name),
        req,
        name === 'useErrorString'
          ? {
              hfErrorKey: req('~/composables/useHuggingFace').hfErrorKey,
            }
          : {},
      )
      const override = dependencyOverrides[name]
      return override ? { ...real, [name]: override } : real
    }
    // Only referenced as <template> tag names — never executed by the
    // replayed script-setup body — so a dead stub is enough.
    if (specifier.startsWith('~/components/')) return {}
    throw new Error(
      `check-chat-state harness: unexpected import '${specifier}'`,
    )
  }

  const chat = {
    ...runComposable(composablePath('useChat'), req).useChat(),
    ...overrides,
  }
  const preferences = {
    ...runComposable(composablePath('usePreferences'), req).usePreferences(),
    ...preferenceOverrides,
  }

  // A fresh Pinia per test, matching every other piece of state here.
  const pinia = nodeRequire('pinia')
  pinia.setActivePinia(pinia.createPinia())
  const modelStore = runComposable(storePath('models'), req, {
    useChat: () => chat,
    useModels: () => req('~/composables/useModels').useModels(),
    useCatalog: () => req('~/composables/useCatalog').useCatalog(),
    useProviders: () => req('~/composables/useProviders').useProviders(),
    useErrorString: () => req('~/composables/useErrorString').useErrorString(),
    usePreferences: () => preferences,
    parseModelIntegrityFailure: req('~/composables/useModels')
      .parseModelIntegrityFailure,
  }).useModelsStore()

  // These have no `import` statement in the page at all — real Nuxt
  // auto-imports/macros with no module backing here — so they must be
  // injected as bare names in scope rather than resolved through `require`.
  const state = new Function(
    'exports',
    'require',
    'definePageMeta',
    'useRoute',
    'useI18n',
    'useChat',
    'useInstance',
    'usePreferences',
    'useDevice',
    'useErrorString',
    'useAutoResizeTextarea',
    'useChatTranscript',
    'useThreadSidebar',
    'useInstancesStore',
    'useModelsStore',
    'storeToRefs',
    'document',
    `${pageCode}\n${RETURN_STATEMENT}`,
  )(
    {},
    req,
    () => {},
    () => ({ params: { instance: 'vault' } }),
    useI18nDouble,
    () => chat,
    req('~/composables/useInstance').useInstance,
    () => preferences,
    req('~/composables/useDevice').useDevice,
    req('~/composables/useErrorString').useErrorString,
    req('~/composables/useAutoResizeTextarea').useAutoResizeTextarea,
    req('~/composables/useChatTranscript').useChatTranscript,
    req('~/composables/useThreadSidebar').useThreadSidebar,
    () => ({}),
    () => modelStore,
    pinia.storeToRefs,
    { querySelector: () => null },
  )
  // Existing send-flow tests model a ready chat session unless they override it.
  state.activeModel.value = {
    modelId: 'model',
    name: 'Model',
    tokenizerRepo: 'tokenizer',
    contextWindow: null,
  }

  return {
    ...state,
    modelStore,
    mount: () => Promise.all(mountHooks.map((hook) => hook())),
    unmount: () => Promise.all(unmountHooks.map((hook) => hook())),
  }
}

test('model download failures use the localized Hugging Face message', async () => {
  const state = createChatState(
    {},
    {},
    {
      useModels: () => ({
        listInstalledAsync: async () => [],
        onDownloadProgress: async () => () => {},
        downloadFromCatalogAsync: async () => {
          throw { kind: 'ModelDownload' }
        },
      }),
    },
  )

  await state.modelStore.downloadCatalogEntry({
    id: 'qwen3-0.6b',
    approx_size_bytes: 1,
  })

  assert.equal(state.modelStore.lastError, 'errors.hf.modelDownload')
})

test('model initialization clears stale transient UI errors', async () => {
  const state = createChatState()
  state.modelStore.lastError = 'stale error'
  state.modelStore.loadErrorModelId = 'stale-model'
  state.modelStore.downloadingId = 'download-in-progress'
  state.modelStore.integrityDialog = {
    modelId: 'stale-model',
    errorKind: 'HashMismatch',
    expected: 'expected',
    actual: 'actual',
  }
  state.modelStore.integrityActionError = 'stale action error'

  await state.modelStore.initialize()

  assert.equal(state.modelStore.lastError, null)
  assert.equal(state.modelStore.loadErrorModelId, null)
  assert.equal(state.modelStore.downloadingId, 'download-in-progress')
  assert.equal(state.modelStore.integrityDialog, null)
  assert.equal(state.modelStore.integrityActionError, null)
})

test('model initialization preserves an active integrity action', async () => {
  const state = createChatState()
  const dialog = {
    modelId: 'repairing-model',
    errorKind: 'HashMismatch',
    expected: 'expected',
    actual: 'actual',
  }
  state.modelStore.integrityDialog = dialog
  state.modelStore.integrityBusy = true
  state.modelStore.integrityActionError = 'action still running'

  await state.modelStore.initialize()

  assert.deepEqual(state.modelStore.integrityDialog, dialog)
  assert.equal(state.modelStore.integrityActionError, 'action still running')
})

test('model initialization auto-loads the first available model when nothing is active yet (spec 002 FR-014)', async () => {
  const state = createChatState(
    {
      activeModelInfoAsync: async () => null,
      loadModelAsync: async (modelId: string) => ({
        modelId,
        name: 'Auto Picked',
        tokenizerRepo: '',
        contextWindow: null,
      }),
    },
    {
      resolveDefaultModelAsync: async () => ({
        modelId: 'auto-picked',
        source: 'first_available',
      }),
    },
  )

  await state.modelStore.initialize()

  assert.equal(state.modelStore.activeModel?.modelId, 'auto-picked')
})

test('model initialization skips the auto-load fallback when a model is already active', async () => {
  let loadModelCalls = 0
  const state = createChatState({
    loadModelAsync: async (modelId: string) => {
      loadModelCalls++
      return { modelId, name: modelId, tokenizerRepo: '', contextWindow: null }
    },
  })

  await state.modelStore.initialize()

  assert.equal(state.modelStore.activeModel?.modelId, 'model')
  assert.equal(loadModelCalls, 0)
})

test('picking a model updates the composer display before load_model resolves', async () => {
  let resolveLoad: () => void = () => {}
  const state = createChatState({
    loadModelAsync: (modelId: string) =>
      new Promise((resolve) => {
        resolveLoad = () =>
          resolve({
            modelId,
            name: 'Qwen 3 4B',
            tokenizerRepo: '',
            contextWindow: null,
          })
      }),
  })
  state.modelStore.installedModels = [{ id: 'qwen3-4b', name: 'Qwen 3 4B' }]

  // `createChatState()` seeds a baseline active model ('model'/'Model') so
  // this starts from the realistic case: switching away from an already-
  // loaded model, not just picking a first one.
  const loadPromise = state.modelStore.loadModel('qwen3-4b')

  // Synchronous portion of `loadModel` (everything before its first
  // `await`) has already run by the time the call above returns — the
  // composer's display fields must reflect the pick immediately, without
  // waiting for `load_model`'s round trip. `activeModel` itself stays on
  // the previous model until the load actually resolves (`sendDisabled`
  // depends on that distinction).
  assert.equal(state.modelStore.displayModelId, 'qwen3-4b')
  assert.equal(state.modelStore.displayModelName, 'Qwen 3 4B')
  assert.equal(state.modelStore.activeModel?.modelId, 'model')

  resolveLoad()
  await loadPromise

  assert.equal(state.modelStore.activeModel?.modelId, 'qwen3-4b')
  assert.equal(state.modelStore.displayModelId, 'qwen3-4b')
  assert.equal(state.modelStore.displayModelName, 'Qwen 3 4B')
})

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
