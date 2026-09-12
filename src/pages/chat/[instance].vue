<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, nextTick } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import {
  useChat,
  type LoadedModelInfo,
  type Message,
  type MessageCompleteEvent,
  type MessageErrorEvent,
  type ModelLoadPhase,
  type ModelLoadProgressEvent,
  type SendMessageArgs,
  type Thread,
  type TokenEvent,
  type ToolCallEvent,
  type ToolPermissionRequestEvent,
  type ToolResultEvent,
  type TurnCompleteEvent,
} from '~/composables/useChat'
import { useModels, type InstalledModel } from '~/composables/useModels'
import { useCatalog, type CatalogEntryWithFit } from '~/composables/useCatalog'
import {
  useProviders,
  type Provider,
  type ProviderModel,
} from '~/composables/useProviders'
import { useInstance } from '~/composables/useInstance'
import { usePreferences } from '~/composables/usePreferences'
import { useDevice } from '~/composables/useDevice'
import PermissionPrompt, { type PendingApproval } from '~/components/chat/PermissionPrompt.vue'

definePageMeta({
  middleware: ['onboarded'],
})

const route = useRoute()
const { t } = useI18n()
const chat = useChat()
const models = useModels()
const catalog = useCatalog()
const providers = useProviders()
const { closeAsync } = useInstance()
const { getPrefAsync, setPrefAsync, resolveDefaultModelAsync } = usePreferences()
const { currentDeviceInfoAsync } = useDevice()
const store = useInstancesStore()

const PERMISSION_MODE_KEY = 'chat.permission_mode'
const permissionMode = ref<'manual' | 'auto' | 'plan'>('manual')
const pendingApprovals = ref<PendingApproval[]>([])
let deviceUuid = ''
let unlistenToolPermissionRequest: UnlistenFn | null = null

const instanceName = computed(() => String(route.params.instance ?? ''))

const threads = ref<Thread[]>([])
const messagesByThread = ref<Record<string, Message[]>>({})
const activeThreadId = ref<string | null>(null)
const activeModel = ref<LoadedModelInfo | null>(null)
const installedModels = ref<InstalledModel[]>([])
const catalogEntries = ref<CatalogEntryWithFit[]>([])
const providerList = ref<Provider[]>([])
const providerModels = ref<Record<string, ProviderModel[]>>({})

const streamingMessageId = ref<string | null>(null)
const streamingBuffer = ref<string>('')
const reasoningByMessage = ref<Record<string, string>>({})
const expandedReasoning = ref<Set<string>>(new Set())

type PendingStreamEvents = {
  tokens: string
  reasoning: string
  complete?: MessageCompleteEvent
  error?: MessageErrorEvent
  turnComplete?: TurnCompleteEvent
}

const pendingStreamEvents = new Map<string, PendingStreamEvents>()
const pendingToolEvents = new Map<string, Array<ToolCallEvent | ToolResultEvent>>()
const pendingApprovalsByThread = new Map<string, PendingApproval[]>()

const input = ref('')
const busy = ref(false)
const reasoningMode = ref<'auto' | 'on' | 'off'>('auto')
const effortLevel = ref<'low' | 'medium' | 'high'>('medium')
const effortTokens: Record<typeof effortLevel.value, number> = {
  low: 1024,
  medium: 4096,
  high: 8192,
}
// True while `send()` has set `activeThreadId` but has not yet appended
// this turn's user/assistant placeholder rows — a `chat-tool-call`/
// `chat-tool-result` for that (already-active) thread can otherwise land
// before the messages it belongs after (backend events can arrive before
// `sendMessageAsync`'s own await resolves).
const turnSetupPending = ref(false)
const lastError = ref<string | null>(null)
const pendingSend = ref<SendMessageArgs | null>(null)

let unlistenToken: UnlistenFn | null = null
let unlistenComplete: UnlistenFn | null = null
let unlistenError: UnlistenFn | null = null
let unlistenToolCall: UnlistenFn | null = null
let unlistenToolResult: UnlistenFn | null = null
let unlistenTurnComplete: UnlistenFn | null = null
let unlistenDownloadProgress: UnlistenFn | null = null
let unlistenDownloadComplete: UnlistenFn | null = null

const downloadingId = ref<string | null>(null)
const downloadProgressBytes = ref<number>(0)
const downloadTotalBytes = ref<number | null>(null)

// Structured loading state driven by the `model-load-progress` event.
// Spec 002 §FR-015a: chat input stays disabled while `phase` is not
// `ready`. The label is translated in the template via
// `chat.loading.<phase>`.
const loadingPhase = ref<ModelLoadPhase | null>(null)
const loadingModelName = ref<string>('')
const loadingProviderName = ref<string | null>(null)
let unlistenLoadProgress: UnlistenFn | null = null

const activeMessages = computed<Message[]>(() => {
  if (!activeThreadId.value) return []
  return messagesByThread.value[activeThreadId.value] ?? []
})

const noModelsInstalled = computed(
  () =>
    installedModels.value.length === 0
    && Object.values(providerModels.value).every((list) => list.length === 0),
)

/** Groups selectable models by provider for the picker's <optgroup>. */
type ModelGroup = { providerId: string, providerName: string, models: { id: string, name: string }[] }
const modelGroups = computed<ModelGroup[]>(() => {
  const localGroup: ModelGroup | null = installedModels.value.length > 0
    ? {
        providerId: 'local',
        providerName: t('chat.model.local'),
        models: installedModels.value.map((m) => ({ id: m.id, name: m.name })),
      }
    : null

  const remoteGroups = providerList.value
    .filter((p) => p.kind === 'api_key')
    .map<ModelGroup>((p) => ({
      providerId: p.id,
      providerName: p.name,
      models: (providerModels.value[p.id] ?? []).map((m) => ({ id: m.id, name: m.name })),
    }))
    .filter((g) => g.models.length > 0)

  return localGroup ? [localGroup, ...remoteGroups] : remoteGroups
})

/** Refreshes the installed models and their catalog metadata together. */
async function refreshInstalledAndCatalog() {
  installedModels.value = await models.listInstalledAsync()
  catalogEntries.value = await catalog.listAsync()
}

/** Refreshes the provider list and re-fetches api_key model caches. */
async function refreshProviders() {
  providerList.value = await providers.listAsync()
  const next: Record<string, ProviderModel[]> = {}
  await Promise.all(
    providerList.value
      .filter((p) => p.kind === 'api_key')
      .map(async (p) => {
        next[p.id] = await providers.listModelsAsync(p.id)
      }),
  )
  providerModels.value = next
}

/** Refreshes the thread list and selects the first thread when needed. */
async function refreshThreads() {
  threads.value = await chat.listThreadsAsync()
  const first = threads.value[0]
  if (!activeThreadId.value && first) {
    await selectThread(first.id)
  }
}

/** Selects a thread, loading its persisted messages on first access. */
async function selectThread(id: string) {
  const previousThreadId = activeThreadId.value
  if (previousThreadId && previousThreadId !== id && pendingApprovals.value.length > 0) {
    const queued = pendingApprovalsByThread.get(previousThreadId) ?? []
    pendingApprovalsByThread.set(previousThreadId, [...queued, ...pendingApprovals.value])
    pendingApprovals.value = []
  }
  activeThreadId.value = id
  const existing = messagesByThread.value[id]
  if (!existing) {
    messagesByThread.value[id] = await chat.listMessagesAsync(id)
  }
  const queuedApprovals = pendingApprovalsByThread.get(id)
  if (queuedApprovals) {
    // Merge, don't overwrite: a `tool-permission-request` for `id` may
    // have already landed directly in `pendingApprovals` during the
    // `listMessagesAsync` await above (its gate matches on
    // `activeThreadId`, which was set synchronously before that await).
    pendingApprovals.value = [...queuedApprovals, ...pendingApprovals.value]
    pendingApprovalsByThread.delete(id)
  }
  const queuedToolEvents = pendingToolEvents.get(id)
  if (queuedToolEvents) {
    pendingToolEvents.delete(id)
    for (const event of queuedToolEvents) {
      if ('toolName' in event) handleToolCall(event, true)
      else handleToolResult(event, true)
    }
  }
  await scrollToBottom()
}

/** Scrolls the message viewport to its newest item after rendering. */
async function scrollToBottom() {
  await nextTick()
  const el = document.querySelector('[data-messages-scroll]')
  if (el) el.scrollTop = el.scrollHeight
}

/** Loads the selected model (local or api_key composite id). */
async function loadModel(id: string) {
  lastError.value = null
  busy.value = true
  try {
    activeModel.value = await chat.loadModelAsync(id)
  }
  catch (e: unknown) {
    lastError.value = errString(e)
    loadingPhase.value = null
    activeModel.value = null
  }
  finally {
    busy.value = false
  }
}

/** Downloads a catalog model, refreshes the lists, and loads the result. */
async function downloadCatalogEntry(entry: CatalogEntryWithFit) {
  lastError.value = null
  downloadingId.value = entry.id
  downloadProgressBytes.value = 0
  downloadTotalBytes.value = entry.approx_size_bytes
  try {
    await models.downloadFromCatalogAsync(entry.id)
    await refreshInstalledAndCatalog()
    await loadModel(entry.id)
  }
  catch (e: unknown) {
    lastError.value = errString(e)
  }
  finally {
    downloadingId.value = null
  }
}

/** Sends the current input and seeds local message placeholders for streaming. */
async function send(retryPending = false) {
  const retry = retryPending ? pendingSend.value : null
  const content = retry?.content ?? input.value.trim()
  if (!content || busy.value) return
  if (!retry) input.value = ''
  busy.value = true
  lastError.value = null
  const request = retry ?? {
    threadId: activeThreadId.value,
    content,
    maxNewTokens: effortTokens[effortLevel.value],
    idempotencyKey: crypto.randomUUID(),
  }
  pendingSend.value = null
  turnSetupPending.value = true
  try {
    const result = await chat.sendMessageAsync(request)
    activeThreadId.value = result.threadId
    const queuedApprovals = pendingApprovalsByThread.get(result.threadId)
    if (queuedApprovals) {
      // Merge, don't overwrite: see the matching comment in `selectThread`.
      pendingApprovals.value = [...queuedApprovals, ...pendingApprovals.value]
      pendingApprovalsByThread.delete(result.threadId)
    }
    // Seed the assistant message placeholder so the UI can show
    // tokens as they stream in.
    const list = messagesByThread.value[result.threadId] ?? []
    list.push({
      id: result.userMessageId,
      threadId: result.threadId,
      parentId: null,
      role: 'user',
      content,
      modelId: null,
      promptTokens: null,
      completionTokens: null,
      finishReason: 'complete',
      createdAt: Date.now(),
      toolName: null,
      toolCallId: null,
      toolInput: null,
      toolIsError: null,
      toolSource: null,
    })
    list.push({
      id: result.assistantMessageId,
      threadId: result.threadId,
      parentId: result.userMessageId,
      role: 'assistant',
      content: '',
      modelId: activeModel.value?.modelId ?? null,
      promptTokens: null,
      completionTokens: null,
      finishReason: null,
      createdAt: Date.now(),
      toolName: null,
      toolCallId: null,
      toolInput: null,
      toolIsError: null,
      toolSource: null,
    })
    messagesByThread.value[result.threadId] = list
    turnSetupPending.value = false
    streamingMessageId.value = result.assistantMessageId
    streamingBuffer.value = ''
    reasoningByMessage.value = {
      ...reasoningByMessage.value,
      [result.assistantMessageId]: '',
    }
    const queuedToolEvents = pendingToolEvents.get(result.threadId)
    if (queuedToolEvents) {
      pendingToolEvents.delete(result.threadId)
      for (const event of queuedToolEvents) {
        if ('toolName' in event) handleToolCall(event, true)
        else handleToolResult(event, true)
      }
    }
    const pending = pendingStreamEvents.get(result.assistantMessageId)
    pendingStreamEvents.delete(result.assistantMessageId)
    if (pending?.tokens || pending?.reasoning) {
      handleToken({
        messageId: result.assistantMessageId,
        delta: pending?.tokens ?? '',
        reasoning: pending?.reasoning ? pending.reasoning : null,
      })
    }
    if (pending?.error) {
      handleError(pending.error)
    }
    else if (pending?.complete) {
      handleComplete(pending.complete)
    }
    if (pending?.turnComplete) {
      handleTurnComplete(pending.turnComplete)
    }
    await scrollToBottom()
  }
  catch (e: unknown) {
    // The initial invoke may have been accepted even when its response was
    // lost. Keep the exact same key available for a safe retry; stream
    // errors are handled separately and deliberately do not retry.
    pendingSend.value = request
    lastError.value = errString(e)
    busy.value = false
    turnSetupPending.value = false
  }
}

/** Requests cancellation of the active generation. */
async function abort() {
  try {
    await chat.abortAsync()
  }
  catch {
    // ignore
  }
}

/** Clears the active conversation so the next send creates a new thread. */
async function newChat() {
  if (busy.value) return
  if (activeThreadId.value && pendingApprovals.value.length > 0) {
    const queued = pendingApprovalsByThread.get(activeThreadId.value) ?? []
    pendingApprovalsByThread.set(activeThreadId.value, [...queued, ...pendingApprovals.value])
    pendingApprovals.value = []
  }
  activeThreadId.value = null
  input.value = ''
  streamingMessageId.value = null
  streamingBuffer.value = ''
}

/** Closes the active instance and returns to the locked landing page. */
async function lock() {
  try {
    await closeAsync()
    store.setActiveInstance(null)
    await navigateTo('/')
  }
  catch (e: unknown) {
    lastError.value = errString(e)
  }
}

/** Converts backend and JavaScript failures into displayable text. */
function errString(e: unknown): string {
  if (typeof e === 'string') return e
  if (e && typeof e === 'object' && 'kind' in e) {
    const kind = (e as { kind: unknown }).kind
    if (kind === 'InvalidIdempotencyKey') return t('errors.invalidIdempotencyKey')
    if (kind === 'IdempotencyKeyConflict') return t('errors.idempotencyKeyConflict')
    return JSON.stringify(e)
  }
  return String(e)
}

/** Formats a byte count for the model download UI. */
function humanBytes(n: number | null): string {
  if (n === null) return '?'
  const kb = 1024
  const mb = kb * 1024
  const gb = mb * 1024
  if (n >= gb) return (n / gb).toFixed(1) + ' GB'
  if (n >= mb) return (n / mb).toFixed(0) + ' MB'
  return (n / kb).toFixed(0) + ' KB'
}

/** Maps a hardware-fit verdict to its localized display label. */
function fitLabel(f: CatalogEntryWithFit['fit']): string {
  return t(`chat.fit.${f}`)
}

function pendingFor(messageId: string): PendingStreamEvents {
  const existing = pendingStreamEvents.get(messageId)
  if (existing) return existing
  const pending: PendingStreamEvents = { tokens: '', reasoning: '' }
  pendingStreamEvents.set(messageId, pending)
  return pending
}

function applyToken(e: TokenEvent, threadId: string) {
  if (e.delta) {
    streamingBuffer.value += e.delta
    const list = messagesByThread.value[threadId] ?? []
    const idx = list.findIndex((m) => m.id === e.messageId)
    const existing = list[idx]
    if (idx !== -1 && existing) {
      list[idx] = { ...existing, content: existing.content + e.delta }
      messagesByThread.value[threadId] = list
    }
  }
  if (e.reasoning) {
    const prev = reasoningByMessage.value[e.messageId] ?? ''
    reasoningByMessage.value = {
      ...reasoningByMessage.value,
      [e.messageId]: prev + e.reasoning,
    }
  }
  scrollToBottom()
}

function handleToken(e: TokenEvent) {
  const threadId = activeThreadId.value
  if (streamingMessageId.value !== e.messageId || !threadId) {
    const pending = pendingFor(e.messageId)
    if (e.delta) pending.tokens += e.delta
    if (e.reasoning) pending.reasoning += e.reasoning
    return
  }
  applyToken(e, threadId)
}

// `chat-message-complete` fires per step (a turn can have several); it no
// longer clears `streamingMessageId`/`busy` or sets a terminal
// `finishReason` itself — `chat-turn-complete` is the sole source for
// both, since only it knows whether the turn actually ended in
// `complete` vs. `tool_limit_reached` (contracts/tauri-commands.md).
function applyComplete(e: MessageCompleteEvent) {
  const list = messagesByThread.value[e.threadId] ?? []
  const idx = list.findIndex((m) => m.id === e.messageId)
  const existing = list[idx]
  if (idx !== -1 && existing) {
    list[idx] = {
      ...existing,
      promptTokens: e.promptTokens,
      completionTokens: e.completionTokens,
    }
    messagesByThread.value[e.threadId] = list
  }
}

function handleComplete(e: MessageCompleteEvent) {
  if (streamingMessageId.value !== e.messageId) {
    const pending = pendingFor(e.messageId)
    pending.complete = e
    delete pending.error
    return
  }
  applyComplete(e)
}

function applyError(e: MessageErrorEvent) {
  lastError.value = e.reason
  const list = messagesByThread.value[e.threadId] ?? []
  const idx = list.findIndex((m) => m.id === e.messageId)
  const existing = list[idx]
  if (idx !== -1 && existing) {
    list[idx] = { ...existing, finishReason: 'error' }
    messagesByThread.value[e.threadId] = list
  }
}

function handleError(e: MessageErrorEvent) {
  if (streamingMessageId.value !== e.messageId) {
    const pending = pendingFor(e.messageId)
    pending.error = e
    delete pending.complete
    return
  }
  applyError(e)
}

/** Appends a `tool_call`/`tool_result` row. Only applied once the event's
 * own thread is the one currently open AND that thread's turn placeholder
 * rows are in place — unlike token/complete/error events, these carry no
 * pre-known placeholder to buffer against, so while `send()` is still
 * setting one up (`turnSetupPending`) an event for the now-active thread
 * would otherwise render above the user message that triggered it. The
 * row is safely in `chat_messages` regardless; switching back to that
 * thread reloads it via `selectThread`. */
function handleToolCall(e: ToolCallEvent, restoring = false) {
  if (!restoring && (e.threadId !== activeThreadId.value || turnSetupPending.value)) {
    const queued = pendingToolEvents.get(e.threadId) ?? []
    pendingToolEvents.set(e.threadId, [...queued, e])
    return
  }
  const list = messagesByThread.value[e.threadId] ?? []
  if (list.some((message) => message.id === e.messageId)) return
  list.push({
    id: e.messageId,
    threadId: e.threadId,
    parentId: null,
    role: 'tool_call',
    content: '',
    modelId: null,
    promptTokens: null,
    completionTokens: null,
    finishReason: null,
    createdAt: Date.now(),
    toolName: e.toolName,
    toolCallId: null,
    toolInput: JSON.stringify(e.toolInput),
    toolIsError: null,
    toolSource: e.toolSource,
  })
  messagesByThread.value[e.threadId] = list
  scrollToBottom()
}

function handleToolResult(e: ToolResultEvent, restoring = false) {
  if (!restoring && (e.threadId !== activeThreadId.value || turnSetupPending.value)) {
    const queued = pendingToolEvents.get(e.threadId) ?? []
    pendingToolEvents.set(e.threadId, [...queued, e])
    return
  }
  const list = messagesByThread.value[e.threadId] ?? []
  if (list.some((message) => message.id === e.messageId)) return
  list.push({
    id: e.messageId,
    threadId: e.threadId,
    parentId: null,
    role: 'tool_result',
    content: e.content,
    modelId: null,
    promptTokens: null,
    completionTokens: null,
    finishReason: null,
    createdAt: Date.now(),
    toolName: null,
    toolCallId: e.toolCallId,
    toolInput: null,
    toolIsError: e.isError,
    toolSource: null,
  })
  messagesByThread.value[e.threadId] = list
  scrollToBottom()
}

// The sole place `streamingMessageId`/`busy` get cleared — a turn can
// span several steps, so only its one terminal event may signal "done"
// (contracts/tauri-commands.md).
function applyTurnComplete(e: TurnCompleteEvent) {
  streamingMessageId.value = null
  streamingBuffer.value = ''
  busy.value = false
  if (!e.assistantMessageId) return
  const list = messagesByThread.value[e.threadId] ?? []
  const idx = list.findIndex((m) => m.id === e.assistantMessageId)
  const existing = list[idx]
  if (idx !== -1 && existing) {
    list[idx] = { ...existing, finishReason: e.finishReason }
    messagesByThread.value[e.threadId] = list
  }
}

function handleToolPermissionRequest(e: ToolPermissionRequestEvent) {
  const approval: PendingApproval = {
    requestId: e.requestId,
    toolName: e.toolName,
    toolInput: e.toolInput,
    riskClass: e.riskClass,
  }
  if (e.threadId !== activeThreadId.value) {
    const queued = pendingApprovalsByThread.get(e.threadId) ?? []
    pendingApprovalsByThread.set(e.threadId, [...queued, approval])
    return
  }
  if (!pendingApprovals.value.some((item) => item.requestId === e.requestId)) {
    pendingApprovals.value = [...pendingApprovals.value, approval]
  }
}

async function respondToApproval(requestId: string, decision: 'allow' | 'deny') {
  try {
    await chat.respondToolPermissionAsync(requestId, decision)
    pendingApprovals.value = pendingApprovals.value.filter((a) => a.requestId !== requestId)
  }
  catch (e: unknown) {
    lastError.value = errString(e)
  }
}

async function updatePermissionMode(mode: 'manual' | 'auto' | 'plan') {
  permissionMode.value = mode
  if (!deviceUuid) return
  try {
    await setPrefAsync({ kind: 'device', uuid: deviceUuid }, PERMISSION_MODE_KEY, mode)
  }
  catch (e: unknown) {
    lastError.value = errString(e)
  }
}

function handleTurnComplete(e: TurnCompleteEvent) {
  if (e.assistantMessageId && streamingMessageId.value !== e.assistantMessageId) {
    const pending = pendingFor(e.assistantMessageId)
    pending.turnComplete = e
    return
  }
  applyTurnComplete(e)
}

function toggleReasoning(messageId: string) {
  const next = new Set(expandedReasoning.value)
  if (next.has(messageId)) {
    next.delete(messageId)
  }
  else {
    next.add(messageId)
  }
  expandedReasoning.value = next
}

function reasoningFor(messageId: string): string {
  return reasoningByMessage.value[messageId] ?? ''
}

function onLoadProgress(e: ModelLoadProgressEvent) {
  loadingPhase.value = e.phase
  loadingModelName.value = e.modelName
  loadingProviderName.value = e.providerName ?? null
  if (e.phase === 'ready') {
    // Small delay so the "Ready" state is visible before it hides.
    // Kept synchronous — the user's next interaction shouldn't wait.
    loadingPhase.value = null
  }
}

/** Localised label for the current loading phase, if any. */
const loadingLabel = computed<string | null>(() => {
  const phase = loadingPhase.value
  if (!phase || phase === 'ready') return null
  const key
    = phase === 'connecting'
      ? 'chat.loading.connecting'
      : phase === 'cuda-jit-warmup'
        ? 'chat.loading.cudaJitWarmup'
        : 'chat.loading.loading'
  return t(key, {
    modelName: loadingModelName.value,
    providerName: loadingProviderName.value ?? '',
  })
})

onMounted(async () => {
  [
    unlistenToken,
    unlistenComplete,
    unlistenError,
    unlistenToolCall,
    unlistenToolResult,
    unlistenTurnComplete,
    unlistenToolPermissionRequest,
    unlistenLoadProgress,
  ] = await Promise.all([
    chat.onToken(handleToken),
    chat.onMessageComplete(handleComplete),
    chat.onMessageError(handleError),
    chat.onToolCall(handleToolCall),
    chat.onToolResult(handleToolResult),
    chat.onTurnComplete(handleTurnComplete),
    chat.onToolPermissionRequest(handleToolPermissionRequest),
    chat.onModelLoadProgress(onLoadProgress),
  ])

  const device = await currentDeviceInfoAsync()
  deviceUuid = device.vaultDeviceUuid
  try {
    const stored = await getPrefAsync({ kind: 'device', uuid: deviceUuid }, PERMISSION_MODE_KEY)
    if (stored === 'manual' || stored === 'auto' || stored === 'plan') {
      permissionMode.value = stored
    }
  }
  catch {
    // Keep the default ('manual') if the read fails.
  }

  activeModel.value = await chat.activeModelInfoAsync()
  await refreshInstalledAndCatalog()
  await refreshProviders()
  await refreshThreads()

  unlistenDownloadProgress = await models.onDownloadProgress((e) => {
    if (downloadingId.value === e.modelId) {
      downloadProgressBytes.value = e.bytesDownloaded
      downloadTotalBytes.value = e.bytesTotal
    }
  })
  unlistenDownloadComplete = await models.onDownloadComplete(() => {
    // Handled inline in downloadCatalogEntry
  })

  // Session-start resolver: pick the model per spec 002 §FR-014 and
  // auto-load it. `first_available` is a passive pick — it must not
  // echo back into last_active (the backend's load_model already
  // skips that; the frontend just calls loadModelAsync).
  if (!activeModel.value) {
    try {
      const resolved = await resolveDefaultModelAsync()
      if (resolved.modelId) {
        await loadModel(resolved.modelId)
      }
    }
    catch (e: unknown) {
      lastError.value = errString(e)
    }
  }
})

onBeforeUnmount(() => {
  unlistenToken?.()
  unlistenComplete?.()
  unlistenError?.()
  unlistenToolCall?.()
  unlistenToolResult?.()
  unlistenTurnComplete?.()
  unlistenToolPermissionRequest?.()
  unlistenLoadProgress?.()
  unlistenDownloadProgress?.()
  unlistenDownloadComplete?.()
})
</script>

<template>
  <main class="flex h-screen min-h-0 bg-muted/20">
    <aside class="hidden md:flex w-64 shrink-0 border-r border-border bg-background p-4 flex-col gap-4 overflow-y-auto">
      <div class="flex items-center gap-3 min-w-0">
        <div class="h-9 w-9 shrink-0 rounded-xl bg-foreground text-background flex items-center justify-center">
          <Icon name="lucide:sparkles" class="h-4 w-4" />
        </div>
        <div class="min-w-0">
          <div class="text-sm font-semibold">Holzi</div>
          <div class="text-xs text-muted-foreground truncate" :title="instanceName">
            {{ instanceName }}
          </div>
        </div>
      </div>

      <UiButton class="w-full justify-start gap-2" variant="outline" :disabled="busy" @click="newChat">
        <Icon name="lucide:plus" class="h-4 w-4" />
        {{ t('chat.newChat') }}
      </UiButton>

      <div class="flex items-center justify-between px-1">
        <span class="text-xs font-medium text-muted-foreground">{{ t('chat.threads.title') }}</span>
        <span class="text-[10px] text-muted-foreground">{{ threads.length }}</span>
      </div>
      <div class="space-y-1">
        <button
          v-for="t in threads"
          :key="t.id"
          class="w-full text-left text-sm px-3 py-2 rounded-lg hover:bg-accent truncate transition-colors"
          :class="{ 'bg-accent font-medium': activeThreadId === t.id }"
          @click="selectThread(t.id)"
        >
          {{ t.title }}
        </button>
        <div v-if="threads.length === 0" class="px-3 py-2 text-xs text-muted-foreground">
          {{ t('chat.threads.empty') }}
        </div>
      </div>

      <div class="flex-1" />
      <NuxtLink
        :to="`/settings/${encodeURIComponent(instanceName)}`"
        class="flex items-center gap-2 rounded-lg px-3 py-2 text-sm text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
      >
        <Icon name="lucide:settings-2" class="h-4 w-4" />
        {{ t('chat.settings') }}
      </NuxtLink>
      <UiButton class="justify-start gap-2" size="sm" variant="ghost" @click="lock">
        <Icon name="lucide:lock-keyhole" class="h-4 w-4" />
        {{ t('chat.lock') }}
      </UiButton>
    </aside>

    <section class="min-w-0 flex-1 flex flex-col">
      <header class="flex items-center justify-between gap-3 border-b border-border bg-background/90 px-4 py-3 backdrop-blur md:px-6">
        <div class="min-w-0">
          <div class="flex items-center gap-2">
            <div class="h-2 w-2 rounded-full" :class="activeModel ? 'bg-emerald-500' : 'bg-muted-foreground/40'" />
            <h1 class="truncate text-sm font-semibold">
              {{ activeThreadId ? (threads.find((thread) => thread.id === activeThreadId)?.title || t('chat.newChat')) : t('chat.newChat') }}
            </h1>
          </div>
          <p class="mt-0.5 truncate text-xs text-muted-foreground">
            {{ activeModel?.name || t('chat.model.notLoaded') }}
          </p>
        </div>
        <div class="flex shrink-0 items-center gap-1 md:hidden">
          <NuxtLink
            :to="`/settings/${encodeURIComponent(instanceName)}`"
            class="rounded-lg p-2 text-muted-foreground hover:bg-accent hover:text-foreground"
            :aria-label="t('chat.settings')"
          >
            <Icon name="lucide:settings-2" class="h-4 w-4" />
          </NuxtLink>
          <UiButton
            size="sm"
            variant="ghost"
            :aria-label="t('chat.lock')"
            @click="lock"
          >
            <Icon name="lucide:lock-keyhole" class="h-4 w-4" />
          </UiButton>
          <UiButton class="gap-2" size="sm" variant="outline" :disabled="busy" @click="newChat">
            <Icon name="lucide:plus" class="h-4 w-4" />
            {{ t('chat.newChatShort') }}
          </UiButton>
        </div>
      </header>

      <div v-if="lastError" class="border-b border-destructive/20 bg-destructive/10 p-3 text-sm text-destructive flex items-start justify-between gap-2">
        <span>{{ lastError }}</span>
        <span class="flex gap-2 shrink-0">
          <button
            v-if="pendingSend"
            class="text-xs underline"
            @click="send(true)"
          >
            {{ t('chat.retry') }}
          </button>
          <button
            class="text-xs underline"
            @click="lastError = null"
          >
            {{ t('chat.close') }}
          </button>
        </span>
      </div>

      <div v-if="loadingLabel" class="border-b border-blue-500/20 bg-blue-500/10 p-3 text-sm text-blue-800" role="status">
        {{ loadingLabel }}
      </div>

      <div v-if="noModelsInstalled" class="flex-1 overflow-y-auto p-6">
        <h2 class="text-lg font-semibold mb-4">
          {{ t('chat.empty.noModelsTitle') }}
        </h2>
        <p class="text-sm text-muted-foreground mb-6">
          {{ t('chat.empty.noModelsDescription') }}
        </p>
        <div class="space-y-2">
          <div
            v-for="e in catalogEntries"
            :key="e.id"
            class="border border-border rounded p-3 flex items-center justify-between gap-4"
          >
            <div class="flex-1 min-w-0">
              <div class="font-medium text-sm">
                {{ e.name }}
              </div>
              <div class="text-xs text-muted-foreground truncate">
                {{ e.hf_repo }}/{{ e.hf_filename }}
              </div>
              <div class="text-xs text-muted-foreground">
                {{ t('chat.catalog.meta', { size: humanBytes(e.approx_size_bytes), context: e.context_window.toLocaleString(), license: e.license, fit: fitLabel(e.fit) }) }}
              </div>
            </div>
            <UiButton
              size="sm"
              :disabled="downloadingId !== null"
              @click="downloadCatalogEntry(e)"
            >
              <template v-if="downloadingId === e.id">
                {{ humanBytes(downloadProgressBytes) }} / {{ humanBytes(downloadTotalBytes) }}
              </template>
              <template v-else>
                {{ t('chat.download') }}
              </template>
            </UiButton>
          </div>
        </div>
      </div>

      <div v-else-if="!activeModel" class="flex-1 flex items-center justify-center p-6 text-muted-foreground">
        <div class="flex w-full max-w-sm flex-col gap-3">
          <p>{{ t('chat.model.selectPrompt') }}</p>
          <label for="chat-model-empty" class="sr-only">{{ t('chat.model.label') }}</label>
          <select
            id="chat-model-empty"
            class="rounded-lg border border-border bg-background px-3 py-2 text-sm"
            :value="activeModel?.modelId ?? ''"
            :disabled="busy"
            @change="(e) => loadModel((e.target as HTMLSelectElement).value)"
          >
            <option value="" disabled>{{ t('chat.model.choose') }}</option>
            <optgroup v-for="group in modelGroups" :key="group.providerId" :label="group.providerName">
              <option v-for="m in group.models" :key="m.id" :value="m.id">{{ m.name }}</option>
            </optgroup>
          </select>
        </div>
      </div>

      <div v-else class="flex-1 flex flex-col overflow-hidden">
        <div data-messages-scroll class="flex-1 min-h-0 overflow-y-auto px-4 py-6 md:px-8">
          <div v-if="activeMessages.length === 0" class="mx-auto flex h-full max-w-3xl flex-col items-center justify-center text-center">
            <div class="mb-4 flex h-12 w-12 items-center justify-center rounded-2xl bg-foreground text-background">
              <Icon name="lucide:sparkles" class="h-5 w-5" />
            </div>
            <h2 class="text-xl font-semibold tracking-tight">{{ t('chat.empty.title') }}</h2>
            <p class="mt-2 max-w-md text-sm text-muted-foreground">
              {{ t('chat.empty.description', { modelName: activeModel.name }) }}
            </p>
          </div>
          <div
            v-for="m in activeMessages"
            :key="m.id"
            class="mx-auto mb-6 flex max-w-3xl gap-3"
            :class="m.role === 'user' ? 'justify-end' : 'justify-start'"
          >
            <div
              v-if="m.role !== 'user'"
              class="mt-1 hidden h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-foreground text-background sm:flex"
            >
              <Icon :name="m.role === 'assistant' ? 'lucide:sparkles' : 'lucide:wrench'" class="h-3.5 w-3.5" />
            </div>
            <div class="min-w-0 max-w-[min(90%,48rem)]" :class="m.role === 'user' ? 'order-first' : ''">
              <div class="mb-1 flex items-center gap-2 text-xs text-muted-foreground">
              <template v-if="m.role === 'tool_call'">
                {{ t('chat.tool.call', { name: m.toolName }) }}
              </template>
              <template v-else-if="m.role === 'tool_result'">
                {{ m.toolIsError ? t('chat.tool.resultError') : t('chat.tool.result') }}
              </template>
              <template v-else>
                {{ m.role === 'user' ? t('chat.sender.user') : m.role === 'assistant' ? t('chat.sender.assistant') : t('chat.sender.system') }}
                <span v-if="m.role === 'assistant' && m.completionTokens" class="ml-2">
                  {{ t('chat.tokens', { count: m.completionTokens }) }}
                </span>
                <span v-if="m.finishReason === 'error'" class="ml-2 text-destructive">
                  {{ t('chat.errorLabel') }}
                </span>
                <span v-if="m.finishReason === 'tool_limit_reached'" class="ml-2 text-amber-600">
                  {{ t('chat.tool.limitReached') }}
                </span>
              </template>
              </div>
              <div
              class="whitespace-pre-wrap rounded-2xl px-4 py-3 text-sm leading-6 shadow-sm"
              :class="{
                'bg-foreground text-background': m.role === 'user',
                'border border-border bg-background': m.role === 'assistant' || m.role === 'system',
                'rounded-lg bg-muted/30 font-mono text-xs leading-5': m.role === 'tool_call' || (m.role === 'tool_result' && !m.toolIsError),
                'rounded-lg bg-destructive/10 text-destructive font-mono text-xs leading-5': m.role === 'tool_result' && m.toolIsError,
              }"
              >
              <template v-if="m.role === 'tool_call'">{{ m.toolInput }}</template>
              <template v-else>{{ m.content || (streamingMessageId === m.id ? '…' : '') }}</template>
              </div>
              <div
              v-if="reasoningMode !== 'off' && m.role === 'assistant' && reasoningFor(m.id)"
              class="mt-1 text-xs"
              >
              <button
                v-if="reasoningMode !== 'on'"
                type="button"
                class="text-muted-foreground hover:text-foreground underline"
                @click="toggleReasoning(m.id)"
              >
                {{ expandedReasoning.has(m.id) ? t('chat.reasoning.hide') : t('chat.reasoning.show') }}
              </button>
              <div
                v-if="reasoningMode === 'on' || expandedReasoning.has(m.id)"
                class="mt-1 whitespace-pre-wrap text-muted-foreground bg-muted/30 rounded px-2 py-1"
              >
                {{ reasoningFor(m.id) }}
              </div>
              </div>
            </div>
          </div>
        </div>

        <form
          class="border-t border-border bg-background/90 px-4 pb-4 pt-3 backdrop-blur md:px-8"
          @submit.prevent="() => send()"
        >
          <div class="mx-auto max-w-3xl">
            <div class="rounded-2xl border border-border bg-background shadow-sm transition-shadow focus-within:border-foreground/30 focus-within:shadow-md">
              <textarea
                v-model="input"
                rows="3"
                class="block w-full resize-none bg-transparent px-4 pb-2 pt-3 text-sm leading-6 outline-none placeholder:text-muted-foreground"
                :placeholder="t('chat.composer.placeholder')"
                :disabled="(busy && streamingMessageId === null) || loadingPhase !== null"
                @keydown.enter.exact.prevent="send()"
              />
              <div class="flex items-center justify-between gap-3 px-3 pb-3">
                <div class="flex items-center gap-1 text-xs text-muted-foreground">
                  <span class="hidden sm:inline">{{ t('chat.composer.newlineHint') }}</span>
                </div>
                <UiButton
                  v-if="streamingMessageId"
                  class="gap-2"
                  size="sm"
                  variant="destructive"
                  type="button"
                  @click="abort"
                >
                  <Icon name="lucide:square" class="h-3.5 w-3.5 fill-current" />
                  {{ t('chat.cancel') }}
                </UiButton>
                <UiButton
                  v-else
                  class="gap-2"
                  size="sm"
                  type="submit"
                  :disabled="!input.trim() || busy || loadingPhase !== null"
                >
                  {{ t('chat.send') }}
                  <Icon name="lucide:arrow-up" class="h-3.5 w-3.5" />
                </UiButton>
              </div>
            </div>

            <div class="flex flex-wrap items-center gap-2 pt-3 text-xs" :aria-label="t('chat.composer.settingsLabel')">
              <div class="flex items-center gap-2 rounded-lg border border-border bg-background px-2.5 py-1.5">
                <Icon name="lucide:cpu" class="h-3.5 w-3.5 text-muted-foreground" />
                <label for="chat-model" class="text-muted-foreground">{{ t('chat.model.label') }}</label>
                <select
                  id="chat-model"
                  v-if="modelGroups.length > 0"
                  class="max-w-40 bg-transparent font-medium outline-none"
                  :value="activeModel?.modelId ?? ''"
                  :disabled="busy"
                  @change="(e) => loadModel((e.target as HTMLSelectElement).value)"
                >
                  <option value="" disabled>{{ t('chat.model.choose') }}</option>
                  <optgroup v-for="group in modelGroups" :key="group.providerId" :label="group.providerName">
                    <option v-for="m in group.models" :key="m.id" :value="m.id">{{ m.name }}</option>
                  </optgroup>
                </select>
                <span v-else class="font-medium">{{ activeModel?.name || t('chat.model.none') }}</span>
              </div>

              <div class="flex items-center gap-2 rounded-lg border border-border bg-background px-2.5 py-1.5">
                <Icon name="lucide:brain" class="h-3.5 w-3.5 text-muted-foreground" />
                <label for="reasoning-mode" class="text-muted-foreground">{{ t('chat.reasoning.label') }}</label>
                <select id="reasoning-mode" v-model="reasoningMode" class="bg-transparent font-medium outline-none" :disabled="busy">
                  <option value="auto">{{ t('chat.reasoning.auto') }}</option>
                  <option value="on">{{ t('chat.reasoning.on') }}</option>
                  <option value="off">{{ t('chat.reasoning.off') }}</option>
                </select>
              </div>

              <div class="flex items-center gap-2 rounded-lg border border-border bg-background px-2.5 py-1.5">
                <Icon name="lucide:gauge" class="h-3.5 w-3.5 text-muted-foreground" />
                <label for="effort-level" class="text-muted-foreground">{{ t('chat.effort.label') }}</label>
                <select id="effort-level" v-model="effortLevel" class="bg-transparent font-medium outline-none" :disabled="busy">
                  <option value="low">{{ t('chat.effort.low') }}</option>
                  <option value="medium">{{ t('chat.effort.medium') }}</option>
                  <option value="high">{{ t('chat.effort.high') }}</option>
                </select>
              </div>

              <PermissionPrompt
                :mode="permissionMode"
                :pending-approvals="pendingApprovals"
                @update:mode="updatePermissionMode"
                @allow="respondToApproval($event, 'allow')"
                @deny="respondToApproval($event, 'deny')"
              />
            </div>
          </div>
        </form>
      </div>
    </section>
  </main>
</template>
