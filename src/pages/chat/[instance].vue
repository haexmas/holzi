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

const input = ref('')
const busy = ref(false)
const lastError = ref<string | null>(null)
const pendingSend = ref<{
  threadId: string | null
  content: string
  idempotencyKey: string
} | null>(null)

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
        providerName: 'Lokale Modelle',
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
  activeThreadId.value = id
  const existing = messagesByThread.value[id]
  if (!existing) {
    messagesByThread.value[id] = await chat.listMessagesAsync(id)
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
    idempotencyKey: crypto.randomUUID(),
  }
  pendingSend.value = null
  try {
    const result = await chat.sendMessageAsync(request)
    activeThreadId.value = result.threadId
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
    streamingMessageId.value = result.assistantMessageId
    streamingBuffer.value = ''
    reasoningByMessage.value = {
      ...reasoningByMessage.value,
      [result.assistantMessageId]: '',
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
  return f === 'fits'
    ? 'passt'
    : f === 'tight'
      ? 'passt knapp'
      : f === 'too_big'
        ? 'zu groß'
        : 'unbekannt'
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

/** Appends a `tool_call`/`tool_result` row. Only applied when the event's
 * own thread is the one currently open — unlike token/complete/error
 * events, these carry no pre-known placeholder to buffer against, and the
 * row is safely in `chat_messages` regardless; switching back to that
 * thread reloads it via `selectThread`. */
function handleToolCall(e: ToolCallEvent) {
  if (e.threadId !== activeThreadId.value) return
  const list = messagesByThread.value[e.threadId] ?? []
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

function handleToolResult(e: ToolResultEvent) {
  if (e.threadId !== activeThreadId.value) return
  const list = messagesByThread.value[e.threadId] ?? []
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
  if (e.threadId !== activeThreadId.value) return
  pendingApprovals.value = [
    ...pendingApprovals.value,
    {
      requestId: e.requestId,
      toolName: e.toolName,
      toolInput: e.toolInput,
      riskClass: e.riskClass,
    },
  ]
}

async function respondToApproval(requestId: string, decision: 'allow' | 'deny') {
  pendingApprovals.value = pendingApprovals.value.filter((a) => a.requestId !== requestId)
  try {
    await chat.respondToolPermissionAsync(requestId, decision)
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
  <main class="flex h-screen">
    <aside class="w-64 border-r border-border p-3 flex flex-col gap-2 overflow-y-auto">
      <div class="text-sm font-semibold truncate" :title="instanceName">
        {{ instanceName }}
      </div>
      <UiButton size="sm" variant="outline" @click="newChat">
        Neuer Chat
      </UiButton>
      <PermissionPrompt
        :mode="permissionMode"
        :pending-approvals="pendingApprovals"
        @update:mode="updatePermissionMode"
        @allow="respondToApproval($event, 'allow')"
        @deny="respondToApproval($event, 'deny')"
      />
      <div class="text-xs text-muted-foreground mt-2">
        Modell
      </div>
      <div v-if="activeModel" class="text-sm truncate" :title="activeModel.name">
        {{ activeModel.name }}
      </div>
      <div v-else class="text-xs text-muted-foreground italic">
        keins geladen
      </div>
      <select
        v-if="modelGroups.length > 0"
        class="text-sm bg-background border border-border rounded px-2 py-1"
        :value="activeModel?.modelId ?? ''"
        :disabled="busy"
        @change="(e) => loadModel((e.target as HTMLSelectElement).value)"
      >
        <option value="" disabled>
          Modell wählen …
        </option>
        <optgroup
          v-for="group in modelGroups"
          :key="group.providerId"
          :label="group.providerName"
        >
          <option
            v-for="m in group.models"
            :key="m.id"
            :value="m.id"
          >
            {{ m.name }}
          </option>
        </optgroup>
      </select>
      <div class="h-px bg-border my-2" />
      <div class="text-xs text-muted-foreground">
        Verläufe
      </div>
      <button
        v-for="t in threads"
        :key="t.id"
        class="text-left text-sm px-2 py-1 rounded hover:bg-accent truncate"
        :class="{ 'bg-accent': activeThreadId === t.id }"
        @click="selectThread(t.id)"
      >
        {{ t.title }}
      </button>
      <div class="flex-1" />
      <UiButton size="sm" variant="ghost" @click="lock">
        Sperren
      </UiButton>
    </aside>

    <section class="flex-1 flex flex-col">
      <div v-if="lastError" class="p-3 bg-destructive/10 text-destructive text-sm flex items-start justify-between gap-2">
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
            schließen
          </button>
        </span>
      </div>

      <div v-if="loadingLabel" class="p-3 bg-blue-500/10 text-blue-800 text-sm" role="status">
        {{ loadingLabel }}
      </div>

      <div v-if="noModelsInstalled" class="p-6 flex-1 overflow-y-auto">
        <h2 class="text-lg font-semibold mb-4">
          Erstes Modell einrichten
        </h2>
        <p class="text-sm text-muted-foreground mb-6">
          Lade eines der unten vorgeschlagenen Modelle herunter, um lokal zu chatten.
          Alternativ kannst du unter Einstellungen einen API-Anbieter (Anthropic …) hinterlegen — der Chat greift dann auf dessen Modelle zu.
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
                ~{{ humanBytes(e.approx_size_bytes) }} · Kontext {{ e.context_window.toLocaleString() }} · {{ e.license }} · {{ fitLabel(e.fit) }}
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
                Herunterladen
              </template>
            </UiButton>
          </div>
        </div>
      </div>

      <div v-else-if="!activeModel" class="p-6 flex-1 flex items-center justify-center text-muted-foreground">
        Wähle links ein Modell aus.
      </div>

      <div v-else class="flex-1 flex flex-col overflow-hidden">
        <div data-messages-scroll class="flex-1 overflow-y-auto p-4 space-y-4">
          <div
            v-for="m in activeMessages"
            :key="m.id"
            class="max-w-3xl mx-auto"
          >
            <div class="text-xs text-muted-foreground mb-1">
              <template v-if="m.role === 'tool_call'">
                {{ t('chat.tool.call', { name: m.toolName }) }}
              </template>
              <template v-else-if="m.role === 'tool_result'">
                {{ m.toolIsError ? t('chat.tool.resultError') : t('chat.tool.result') }}
              </template>
              <template v-else>
                {{ m.role === 'user' ? 'Du' : m.role === 'assistant' ? 'Assistent' : m.role }}
                <span v-if="m.role === 'assistant' && m.completionTokens" class="ml-2">
                  {{ m.completionTokens }} tokens
                </span>
                <span v-if="m.finishReason === 'error'" class="ml-2 text-destructive">
                  (Fehler)
                </span>
                <span v-if="m.finishReason === 'tool_limit_reached'" class="ml-2 text-amber-600">
                  {{ t('chat.tool.limitReached') }}
                </span>
              </template>
            </div>
            <div
              class="whitespace-pre-wrap text-sm rounded px-3 py-2"
              :class="{
                'bg-accent': m.role === 'user',
                'bg-muted/50': m.role === 'assistant' || m.role === 'system',
                'bg-muted/30 font-mono text-xs': m.role === 'tool_call' || (m.role === 'tool_result' && !m.toolIsError),
                'bg-destructive/10 text-destructive font-mono text-xs': m.role === 'tool_result' && m.toolIsError,
              }"
            >
              <template v-if="m.role === 'tool_call'">{{ m.toolInput }}</template>
              <template v-else>{{ m.content || (streamingMessageId === m.id ? '…' : '') }}</template>
            </div>
            <div
              v-if="m.role === 'assistant' && reasoningFor(m.id)"
              class="mt-1 text-xs"
            >
              <button
                type="button"
                class="text-muted-foreground hover:text-foreground underline"
                @click="toggleReasoning(m.id)"
              >
                {{ expandedReasoning.has(m.id) ? 'Denkschritte ausblenden' : 'Denkschritte anzeigen' }}
              </button>
              <div
                v-if="expandedReasoning.has(m.id)"
                class="mt-1 whitespace-pre-wrap text-muted-foreground bg-muted/30 rounded px-2 py-1"
              >
                {{ reasoningFor(m.id) }}
              </div>
            </div>
          </div>
        </div>

        <form
          class="p-3 border-t border-border flex gap-2"
          @submit.prevent="() => send()"
        >
          <input
            v-model="input"
            class="flex-1 bg-background border border-border rounded px-3 py-2 text-sm"
            placeholder="Nachricht schreiben …"
            :disabled="(busy && streamingMessageId === null) || loadingPhase !== null"
          >
          <UiButton
            v-if="streamingMessageId"
            variant="destructive"
            type="button"
            @click="abort"
          >
            Abbruch
          </UiButton>
          <UiButton
            v-else
            type="submit"
            :disabled="!input.trim() || busy || loadingPhase !== null"
          >
            Senden
          </UiButton>
        </form>
      </div>
    </section>
  </main>
</template>
