<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, nextTick } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import DOMPurify from 'dompurify'
import { marked } from 'marked'
import {
  useChat,
  type LoadedModelInfo,
  type Message,
  type MessageCompleteEvent,
  type MessageErrorEvent,
  type ModelLoadErrorEvent,
  type ModelLoadPhase,
  type ModelLoadProgressEvent,
  type ModelLoadStatusPayload,
  type RetryEvent,
  type SendMessageArgs,
  type Thread,
  type TokenEvent,
  type ToolCallEvent,
  type ToolPermissionRequestEvent,
  type ToolResultEvent,
  type TurnCompleteEvent,
} from '~/composables/useChat'
import {
  parseModelIntegrityFailure,
  useModels,
  type InstalledModel,
  type ModelIntegrityFailure,
} from '~/composables/useModels'
import { hfErrorKey } from '~/composables/useHuggingFace'
import { useCatalog, type CatalogEntryWithFit } from '~/composables/useCatalog'
import {
  useProviders,
  type Provider,
  type ProviderModel,
} from '~/composables/useProviders'
import { useInstance } from '~/composables/useInstance'
import { usePreferences } from '~/composables/usePreferences'
import { useDevice } from '~/composables/useDevice'
import PermissionPrompt, {
  type PendingApproval,
} from '~/components/chat/PermissionPrompt.vue'
import ComposerSettingsPopover from '~/components/chat/ComposerSettingsPopover.vue'
import ReasoningAccordion from '~/components/chat/ReasoningAccordion.vue'
import { useAutoResizeTextarea } from '~/composables/useAutoResizeTextarea'

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
const { getPrefAsync, setPrefAsync } = usePreferences()
const { currentDeviceInfoAsync } = useDevice()
const store = useInstancesStore()

const PERMISSION_MODE_KEY = 'chat.permission_mode'
const permissionMode = ref<'manual' | 'auto' | 'plan'>('manual')
const pendingApprovals = ref<PendingApproval[]>([])
const deviceUuid = ref('')
const permissionModeSaving = ref(false)
const unlisteners: UnlistenFn[] = []
let unmounted = false

const instanceName = computed(() => String(route.params.instance ?? ''))

const threads = ref<Thread[]>([])
const messagesByThread = ref<Record<string, Message[]>>({})
const activeThreadId = ref<string | null>(null)
const durationNow = ref(Date.now())
let durationRefreshTimer: ReturnType<typeof setTimeout> | null = null
const editingThreadId = ref<string | null>(null)
const draftTitle = ref('')
const editTitleError = ref<string | null>(null)
const editingTitleInput = ref<HTMLInputElement | null>(null)
const renamingThreadId = ref<string | null>(null)
const deleteCandidate = ref<Thread | null>(null)
const deleteError = ref<string | null>(null)
const deletingThread = ref(false)
const activeModel = ref<LoadedModelInfo | null>(null)
const installedModels = ref<InstalledModel[]>([])
const catalogEntries = ref<CatalogEntryWithFit[]>([])
const providerList = ref<Provider[]>([])
const providerModels = ref<Record<string, ProviderModel[]>>({})

const streamingMessageId = ref<string | null>(null)
const streamingThreadId = ref<string | null>(null)
const streamingBuffer = ref<string>('')
const reasoningByMessage = ref<Record<string, string>>({})
// Set while an automatic LLM-request retry (spec.md FR-012) is between
// attempts for the currently-streaming message — cleared the moment new
// tokens arrive (the next attempt) or the turn ends. Never persisted;
// `chat-retry` carries no message row of its own.
const retryingMessageId = ref<string | null>(null)
const expandedReasoning = ref<Set<string>>(new Set())

type PendingStreamEvents = {
  tokens: string
  reasoning: string
  retryReset?: boolean
  complete?: MessageCompleteEvent
  error?: MessageErrorEvent
}

const pendingStreamEvents = new Map<string, PendingStreamEvents>()
const pendingToolEvents = new Map<
  string,
  Array<ToolCallEvent | ToolResultEvent>
>()
const pendingApprovalsByThread = new Map<string, PendingApproval[]>()
const pendingTurnCompletions = new Map<string, TurnCompleteEvent>()
const turnTerminalWaiters = new Map<string, Set<() => void>>()

const input = ref('')
const busy = ref(false)
const modelLoadPending = ref(false)
const effortLevel = ref<'low' | 'medium' | 'high'>('medium')
const effortTokens: Record<typeof effortLevel.value, number> = {
  low: 1024,
  medium: 4096,
  high: 8192,
}
const effortLabel = computed(() => t(`chat.effort.${effortLevel.value}`))
// True while `send()` has set `activeThreadId` but has not yet appended
// this turn's user/assistant placeholder rows — a `chat-tool-call`/
// `chat-tool-result` for that (already-active) thread can otherwise land
// before the messages it belongs after (backend events can arrive before
// `sendMessageAsync`'s own await resolves).
const turnSetupPending = ref(false)
const lastError = ref<string | null>(null)
const pendingSend = ref<SendMessageArgs | null>(null)

const { textareaRef, reset: resetTextarea } = useAutoResizeTextarea(input)

const downloadingId = ref<string | null>(null)
const downloadProgressBytes = ref<number>(0)
const downloadTotalBytes = ref<number | null>(null)

// Structured loading state driven by the `model-load-progress` event. The
// composer stays available for drafting while sending remains blocked until
// the load reaches `ready`.
const loadingPhase = ref<ModelLoadPhase | null>(null)
const loadingModelName = ref<string>('')
const loadingProviderName = ref<string | null>(null)
const loadErrorModelId = ref<string | null>(null)

const composerInputDisabled = computed(
  () =>
    busy.value && !modelLoadPending.value && streamingMessageId.value === null,
)
const sendDisabled = computed(
  () =>
    !activeModel.value ||
    busy.value ||
    modelLoadPending.value ||
    loadingPhase.value !== null,
)

const integrityDialog = ref<ModelIntegrityFailure | null>(null)
const integrityBusy = ref(false)
const integrityActionError = ref<string | null>(null)

const activeMessages = computed<Message[]>(() => {
  if (!activeThreadId.value) return []
  return messagesByThread.value[activeThreadId.value] ?? []
})

const noModelsInstalled = computed(
  () =>
    installedModels.value.length === 0 &&
    Object.values(providerModels.value).every((list) => list.length === 0),
)

type HistoryDurationUnit = 'min' | 'h' | 'd'

type HistoryDuration = {
  value: number
  unit: HistoryDurationUnit
}

/** Projects a persisted Unix-millisecond opening time into the compact UI form. */
function historyDuration(createdAt: number, now = Date.now()): HistoryDuration {
  if (!Number.isFinite(createdAt) || createdAt < 0) {
    return { value: 0, unit: 'min' }
  }
  const elapsedMs = Math.max(0, now - createdAt)
  const minutes = Math.floor(elapsedMs / 60_000)
  if (minutes < 60) return { value: minutes, unit: 'min' }
  const hours = Math.floor(elapsedMs / 3_600_000)
  if (hours < 24) return { value: hours, unit: 'h' }
  return { value: Math.floor(elapsedMs / 86_400_000), unit: 'd' }
}

function historyDurationLabel(createdAt: number): string {
  const duration = historyDuration(createdAt, durationNow.value)
  return `${duration.value}${duration.unit}`
}

function openingTimeLabel(createdAt: number): string {
  if (!Number.isFinite(createdAt) || createdAt < 0) {
    return t('chat.threads.openedAtUnknown')
  }
  const date = new Date(createdAt)
  if (Number.isNaN(date.getTime())) {
    return t('chat.threads.openedAtUnknown')
  }
  return t('chat.threads.openedAt', {
    date: date.toLocaleString(),
  })
}

function nextDurationBoundary(createdAt: number, now: number): number | null {
  if (!Number.isFinite(createdAt) || createdAt < 0) return null
  if (createdAt > now) return createdAt
  const elapsedMs = now - createdAt
  const unitMs =
    elapsedMs < 3_600_000
      ? 60_000
      : elapsedMs < 86_400_000
        ? 3_600_000
        : 86_400_000
  return createdAt + (Math.floor(elapsedMs / unitMs) + 1) * unitMs
}

function scheduleDurationRefresh() {
  if (durationRefreshTimer !== null) clearTimeout(durationRefreshTimer)
  const now = Date.now()
  const nextBoundary = threads.value
    .map((thread) => nextDurationBoundary(thread.createdAt, now))
    .filter((value): value is number => value !== null)
    .reduce((nearest, value) => Math.min(nearest, value), Infinity)
  if (!Number.isFinite(nextBoundary)) return
  durationRefreshTimer = setTimeout(
    () => {
      durationNow.value = Date.now()
      scheduleDurationRefresh()
    },
    Math.max(1_000, nextBoundary - now),
  )
  if (
    typeof durationRefreshTimer === 'object' &&
    durationRefreshTimer !== null &&
    'unref' in durationRefreshTimer &&
    typeof durationRefreshTimer.unref === 'function'
  ) {
    durationRefreshTimer.unref()
  }
}

// Read through a computed rather than `activeModel?.modelId` directly in
// the template — vue-tsc narrows `activeModel` to `never` at the model
// picker's `v-else-if="!activeModel"` (a chained-`v-if` control-flow
// quirk), which a plain computed's independent return type sidesteps.
const activeModelId = computed(() => activeModel.value?.modelId ?? '')

/** Groups selectable models by provider for the picker's <optgroup>. */
type ModelGroup = {
  providerId: string
  providerName: string
  models: { id: string; name: string }[]
}
const modelGroups = computed<ModelGroup[]>(() => {
  const localGroup: ModelGroup | null =
    installedModels.value.length > 0
      ? {
          providerId: 'local',
          providerName: t('chat.model.local'),
          models: installedModels.value.map((m) => ({
            id: m.id,
            name: m.name,
          })),
        }
      : null

  const remoteGroups = providerList.value
    .filter((p) => p.kind === 'api_key')
    .map<ModelGroup>((p) => ({
      providerId: p.id,
      providerName: p.name,
      models: (providerModels.value[p.id] ?? []).map((m) => ({
        id: m.id,
        name: m.name,
      })),
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

/** Refreshes the thread list without changing the active chat draft/thread. */
async function refreshThreads() {
  threads.value = await chat.listThreadsAsync()
  scheduleDurationRefresh()
}

function startEditing(thread: Thread) {
  if (deletingThread.value || renamingThreadId.value) return
  editingThreadId.value = thread.id
  draftTitle.value = thread.title
  editTitleError.value = null
  void nextTick(() => {
    editingTitleInput.value?.focus()
    editingTitleInput.value?.select()
  })
}

function cancelEditing() {
  editingThreadId.value = null
  draftTitle.value = ''
  editTitleError.value = null
}

async function saveThreadTitle() {
  const threadId = editingThreadId.value
  if (!threadId || renamingThreadId.value) return
  const title = draftTitle.value.trim()
  if (!title) {
    editTitleError.value = t('chat.threads.titleRequired')
    return
  }
  const titleLength = [
    ...new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(
      title,
    ),
  ].length
  if (titleLength > 120) {
    editTitleError.value = t('chat.threads.titleTooLong')
    return
  }
  renamingThreadId.value = threadId
  editTitleError.value = null
  try {
    const updated = await chat.renameThreadAsync(threadId, title)
    threads.value = threads.value.map((thread) =>
      thread.id === updated.id ? updated : thread,
    )
    cancelEditing()
  } catch {
    editTitleError.value = t('chat.threads.renameFailed')
  } finally {
    renamingThreadId.value = null
  }
}

function requestDelete(thread: Thread) {
  if (renamingThreadId.value || deletingThread.value) return
  deleteCandidate.value = thread
  deleteError.value = null
}

function closeDeleteDialog(open: boolean) {
  if (!open && !deletingThread.value) {
    deleteCandidate.value = null
    deleteError.value = null
  }
}

function hasPendingTurn(threadId: string): boolean {
  return (
    streamingThreadId.value === threadId ||
    (turnSetupPending.value && activeThreadId.value === threadId) ||
    pendingApprovalsByThread.has(threadId) ||
    (activeThreadId.value === threadId && pendingApprovals.value.length > 0)
  )
}

function waitForTurnTerminal(threadId: string): Promise<boolean> {
  if (!hasPendingTurn(threadId)) return Promise.resolve(true)
  return new Promise((resolve) => {
    const waiters = turnTerminalWaiters.get(threadId) ?? new Set<() => void>()
    const timeout = setTimeout(() => {
      waiters.delete(finish)
      if (waiters.size === 0) turnTerminalWaiters.delete(threadId)
      resolve(false)
    }, 10_000)
    const finish = () => {
      clearTimeout(timeout)
      waiters.delete(finish)
      if (waiters.size === 0) turnTerminalWaiters.delete(threadId)
      resolve(true)
    }
    waiters.add(finish)
    turnTerminalWaiters.set(threadId, waiters)
  })
}

function resolveTurnTerminal(threadId: string) {
  const waiters = turnTerminalWaiters.get(threadId)
  if (!waiters) return
  for (const waiter of [...waiters]) waiter()
}

function clearDeletedThreadState(threadId: string) {
  threads.value = threads.value.filter((thread) => thread.id !== threadId)
  messagesByThread.value = Object.fromEntries(
    Object.entries(messagesByThread.value).filter(([id]) => id !== threadId),
  )
  pendingApprovalsByThread.delete(threadId)
  pendingToolEvents.delete(threadId)
  pendingTurnCompletions.delete(threadId)
  if (editingThreadId.value === threadId) cancelEditing()
  if (activeThreadId.value !== threadId) return
  activeThreadId.value = null
  input.value = ''
  streamingMessageId.value = null
  streamingThreadId.value = null
  streamingBuffer.value = ''
  reasoningByMessage.value = {}
  expandedReasoning.value = new Set()
  pendingStreamEvents.clear()
  void resetTextarea()
}

async function confirmDelete() {
  const candidate = deleteCandidate.value
  if (!candidate || deletingThread.value) return
  deletingThread.value = true
  deleteError.value = null
  const needsAbort = hasPendingTurn(candidate.id)
  try {
    if (needsAbort) {
      const terminal = waitForTurnTerminal(candidate.id)
      try {
        await chat.abortAsync()
      } catch {
        deleteError.value = t('chat.threads.deleteCancelFailed')
        return
      }
      if (!(await terminal)) {
        deleteError.value = t('chat.threads.deleteCancelFailed')
        return
      }
    }
    await chat.deleteThreadAsync(candidate.id)
    clearDeletedThreadState(candidate.id)
    deleteCandidate.value = null
  } catch {
    deleteError.value = t('chat.threads.deleteFailed')
  } finally {
    deletingThread.value = false
  }
}

/** Selects a thread, loading its persisted messages on first access. */
async function selectThread(id: string) {
  const previousThreadId = activeThreadId.value
  if (
    previousThreadId &&
    previousThreadId !== id &&
    pendingApprovals.value.length > 0
  ) {
    const queued = pendingApprovalsByThread.get(previousThreadId) ?? []
    pendingApprovalsByThread.set(previousThreadId, [
      ...queued,
      ...pendingApprovals.value,
    ])
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
  loadErrorModelId.value = null
  modelLoadPending.value = true
  busy.value = true
  try {
    activeModel.value = await chat.loadModelAsync(id)
  } catch (e: unknown) {
    if (!openIntegrityDialog(id, e)) {
      lastError.value = errString(e)
    }
    loadingPhase.value = null
    activeModel.value = null
  } finally {
    busy.value = false
    modelLoadPending.value = false
  }
}

/**
 * Detects the three structured integrity error kinds `load_model` can
 * return (spec 005 §"load_model und lokale Integritätsprüfung") and opens
 * the decision dialog instead of showing a plain error string. Returns
 * `false` for every other error so the caller falls back to `errString`.
 */
function openIntegrityDialog(modelId: string, e: unknown): boolean {
  const failure = parseModelIntegrityFailure(modelId, e)
  if (!failure) return false
  integrityDialog.value = failure
  return true
}

/** "Trotzdem als unsicher laden" — bypasses the hash check for this load only. */
async function onIntegrityLoadUntrusted() {
  if (!integrityDialog.value) return
  modelLoadPending.value = true
  integrityBusy.value = true
  integrityActionError.value = null
  try {
    activeModel.value = await chat.loadModelWithIntegrityOverrideAsync(
      integrityDialog.value.modelId,
    )
    integrityDialog.value = null
    await refreshInstalledAndCatalog()
  } catch (e) {
    integrityActionError.value = errString(e)
  } finally {
    integrityBusy.value = false
    modelLoadPending.value = false
  }
}

/** "Erneut herunterladen / neu importieren" — re-installs from the model's stored HF source. */
async function onIntegrityRepairSource() {
  const dialog = integrityDialog.value
  if (!dialog) return
  const model = installedModels.value.find((m) => m.id === dialog.modelId)
  integrityBusy.value = true
  integrityActionError.value = null
  try {
    if (
      model?.sourceKind === 'huggingface' &&
      model.hfRepo &&
      model.hfFilename
    ) {
      await models.downloadFromHfAsync({
        repoId: model.hfRepo,
        filename: model.hfFilename,
        // Repair reinstalls the stored source: the tracked ref when there
        // is one, otherwise the pinned commit — never an implicit `main`.
        revision: model.hfRevisionRef ?? model.hfRevision ?? undefined,
        name: model.name,
        contextWindow: model.contextWindow,
        // Without this the backend's same-source short-circuit returns the
        // existing row and the corrupt file is never replaced.
        forceRepair: true,
      })
      integrityDialog.value = null
      await refreshInstalledAndCatalog()
    } else {
      integrityActionError.value = t('models.integrityDialog.actionFailed')
    }
  } catch (e) {
    integrityActionError.value = errString(e)
  } finally {
    integrityBusy.value = false
  }
}

/** "Anderes Modell auswählen" — just closes the dialog; the picker is already visible. */
function onIntegrityChooseOther() {
  integrityDialog.value = null
}

function onIntegrityDialogOpenChange(open: boolean) {
  if (!open) {
    integrityDialog.value = null
    integrityActionError.value = null
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
  } catch (e: unknown) {
    lastError.value = errString(e)
  } finally {
    downloadingId.value = null
  }
}

/** Sends the current input and seeds local message placeholders for streaming. */
async function send(retryPending = false) {
  const retry = retryPending ? pendingSend.value : null
  const content = retry?.content ?? input.value.trim()
  if (!content || sendDisabled.value) return
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
    if (retry) {
      // A replay returns the original IDs without emitting another turn.
      // Recover a completed answer when the initial invoke response was lost.
      const persisted = await chat.listMessagesAsync(result.threadId)
      messagesByThread.value[result.threadId] = persisted
      const completed = persisted.find(
        (message) => message.id === result.assistantMessageId,
      )
      if (completed?.finishReason) {
        streamingMessageId.value = result.assistantMessageId
        streamingThreadId.value = result.threadId
        turnSetupPending.value = false
        pendingStreamEvents.delete(result.assistantMessageId)
        pendingTurnCompletions.delete(result.threadId)
        await handleTurnComplete({
          threadId: result.threadId,
          assistantMessageId: result.assistantMessageId,
          finishReason: completed.finishReason,
        })
        return
      }
    }
    // Seed the assistant message placeholder so the UI can show
    // tokens as they stream in.
    const list = messagesByThread.value[result.threadId] ?? []
    if (!list.some((message) => message.id === result.userMessageId))
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
    if (!list.some((message) => message.id === result.assistantMessageId))
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
    streamingThreadId.value = result.threadId
    streamingBuffer.value = ''
    retryingMessageId.value = null
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
    if (pending?.retryReset) {
      streamingBuffer.value = ''
      reasoningByMessage.value = {
        ...reasoningByMessage.value,
        [result.assistantMessageId]: '',
      }
    }
    if (pending?.tokens || pending?.reasoning) {
      handleToken({
        messageId: result.assistantMessageId,
        delta: pending?.tokens ?? '',
        reasoning: pending?.reasoning ? pending.reasoning : null,
      })
    }
    if (pending?.error) {
      handleError(pending.error)
    } else if (pending?.complete) {
      handleComplete(pending.complete)
    }
    const completedTurn = pendingTurnCompletions.get(result.threadId)
    pendingTurnCompletions.delete(result.threadId)
    if (completedTurn) {
      await handleTurnComplete(completedTurn)
    } else {
      // A failed list refresh must not turn an accepted send into a retry.
      await refreshThreads().catch((e: unknown) => {
        lastError.value = errString(e)
      })
    }
    await scrollToBottom()
  } catch (e: unknown) {
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
  } catch (e: unknown) {
    lastError.value = errString(e)
  }
}

/** Clears the active conversation so the next send creates a new thread. */
async function newChat() {
  if (busy.value) return
  if (activeThreadId.value && pendingApprovals.value.length > 0) {
    const queued = pendingApprovalsByThread.get(activeThreadId.value) ?? []
    pendingApprovalsByThread.set(activeThreadId.value, [
      ...queued,
      ...pendingApprovals.value,
    ])
    pendingApprovals.value = []
  }
  activeThreadId.value = null
  input.value = ''
  streamingMessageId.value = null
  streamingThreadId.value = null
  streamingBuffer.value = ''
  reasoningByMessage.value = {}
  expandedReasoning.value = new Set()
  pendingStreamEvents.clear()
  void resetTextarea()
}

/** Closes the active instance and returns to the locked landing page. */
async function lock() {
  try {
    await closeAsync()
    store.setActiveInstance(null)
    await navigateTo('/')
  } catch (e: unknown) {
    lastError.value = errString(e)
  }
}

/** Converts backend and JavaScript failures into displayable text. */
const HF_ERROR_KINDS = new Set([
  'InvalidInput',
  'Network',
  'Timeout',
  'HttpStatus',
  'RateLimited',
  'UnsupportedFormat',
  'TokenizerRequired',
  'HardwareConfirmationRequired',
  'ModelRegistrationFailed',
  'ModelNotFound',
])

function errString(e: unknown): string {
  if (typeof e === 'string') return e
  if (e && typeof e === 'object' && 'kind' in e) {
    const kind = (e as { kind: unknown }).kind
    if (kind === 'InvalidIdempotencyKey')
      return t('errors.invalidIdempotencyKey')
    if (kind === 'IdempotencyKeyConflict')
      return t('errors.idempotencyKeyConflict')
    if (typeof kind === 'string' && HF_ERROR_KINDS.has(kind))
      return t(hfErrorKey(e))
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

function downloadProgressPercent(modelId: string): number | null {
  if (
    downloadingId.value !== modelId ||
    downloadTotalBytes.value === null ||
    downloadTotalBytes.value <= 0
  )
    return null
  return Math.min(
    100,
    Math.max(
      0,
      Math.round(
        (downloadProgressBytes.value / downloadTotalBytes.value) * 100,
      ),
    ),
  )
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
  if (retryingMessageId.value === e.messageId) retryingMessageId.value = null
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
  const threadId = streamingThreadId.value
  if (streamingMessageId.value !== e.messageId || !threadId) {
    const pending = pendingFor(e.messageId)
    if (e.delta) pending.tokens += e.delta
    if (e.reasoning) pending.reasoning += e.reasoning
    return
  }
  applyToken(e, threadId)
}

/** `chat-retry`: the attempt whose partial text was already shown just
 * got discarded — clear it and show "retrying…" instead (T039). Preserve
 * the reset when the event lands before `send()` installs its placeholder. */
function handleRetry(e: RetryEvent) {
  const threadId = streamingThreadId.value
  if (streamingMessageId.value !== e.assistantMessageId || !threadId) {
    const pending = pendingFor(e.assistantMessageId)
    pending.tokens = ''
    pending.reasoning = ''
    pending.retryReset = true
    return
  }
  streamingBuffer.value = ''
  reasoningByMessage.value = {
    ...reasoningByMessage.value,
    [e.assistantMessageId]: '',
  }
  const list = messagesByThread.value[threadId] ?? []
  const idx = list.findIndex((m) => m.id === e.assistantMessageId)
  const existing = list[idx]
  if (idx !== -1 && existing) {
    list[idx] = { ...existing, content: '' }
    messagesByThread.value[threadId] = list
  }
  retryingMessageId.value = e.assistantMessageId
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
  if (
    !restoring &&
    (e.threadId !== activeThreadId.value || turnSetupPending.value)
  ) {
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
  if (
    !restoring &&
    (e.threadId !== activeThreadId.value || turnSetupPending.value)
  ) {
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
async function applyTurnComplete(e: TurnCompleteEvent) {
  pendingApprovalsByThread.delete(e.threadId)
  if (activeThreadId.value === e.threadId) pendingApprovals.value = []
  const list = messagesByThread.value[e.threadId] ?? []
  const idx = list.findIndex((m) => m.id === e.assistantMessageId)
  const existing = list[idx]
  if (idx !== -1 && existing) {
    list[idx] = { ...existing, finishReason: e.finishReason }
    messagesByThread.value[e.threadId] = list
  }
  try {
    // All rounds stream through one placeholder, whereas persistence has
    // separate interim answers, tool rows, and the terminal answer.
    messagesByThread.value[e.threadId] = await chat.listMessagesAsync(
      e.threadId,
    )
    pendingToolEvents.delete(e.threadId)
    await refreshThreads()
    await scrollToBottom()
  } catch (error: unknown) {
    lastError.value = errString(error)
  } finally {
    streamingMessageId.value = null
    streamingThreadId.value = null
    streamingBuffer.value = ''
    retryingMessageId.value = null
    busy.value = false
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

async function respondToApproval(
  requestId: string,
  decision: 'allow' | 'deny',
) {
  try {
    await chat.respondToolPermissionAsync(requestId, decision)
    pendingApprovals.value = pendingApprovals.value.filter(
      (a) => a.requestId !== requestId,
    )
  } catch (e: unknown) {
    lastError.value = errString(e)
  }
}

async function updatePermissionMode(mode: 'manual' | 'auto' | 'plan') {
  if (!deviceUuid.value || permissionModeSaving.value) return
  const previousMode = permissionMode.value
  permissionMode.value = mode
  permissionModeSaving.value = true
  try {
    await setPrefAsync(
      { kind: 'device', uuid: deviceUuid.value },
      PERMISSION_MODE_KEY,
      mode,
    )
  } catch (e: unknown) {
    permissionMode.value = previousMode
    lastError.value = errString(e)
  } finally {
    permissionModeSaving.value = false
  }
}

function updateEffortLevel(level: string) {
  if (level === 'low' || level === 'medium' || level === 'high') {
    effortLevel.value = level
  }
}

async function handleTurnComplete(e: TurnCompleteEvent) {
  if (turnSetupPending.value) {
    pendingTurnCompletions.set(e.threadId, e)
    return
  }
  if (e.threadId !== streamingThreadId.value) return
  if (e.assistantMessageId && streamingMessageId.value !== e.assistantMessageId)
    return
  await applyTurnComplete(e)
  resolveTurnTerminal(e.threadId)
}

function setReasoningExpanded(messageId: string, expanded: boolean) {
  const next = new Set(expandedReasoning.value)
  if (expanded) next.add(messageId)
  else next.delete(messageId)
  expandedReasoning.value = next
}

function reasoningFor(messageId: string): string {
  return reasoningByMessage.value[messageId] ?? ''
}

function renderMarkdown(content: string): string {
  return DOMPurify.sanitize(
    marked.parse(content, { async: false, breaks: true }),
  )
}

function onLoadProgress(e: ModelLoadProgressEvent) {
  loadErrorModelId.value = null
  loadingPhase.value = e.phase
  loadingModelName.value = e.modelName
  loadingProviderName.value = e.providerName ?? null
  if (e.phase === 'ready') {
    loadingPhase.value = null
    void refreshActiveModel()
  }
}

async function refreshActiveModel() {
  try {
    activeModel.value = await chat.activeModelInfoAsync()
  } catch (e: unknown) {
    lastError.value = errString(e)
  }
}

function onLoadStatus(status: ModelLoadStatusPayload) {
  if (status.status === 'loading') {
    loadErrorModelId.value = null
    loadingPhase.value = status.phase
    loadingModelName.value = status.modelName
    loadingProviderName.value = status.providerName ?? null
  } else {
    loadingPhase.value = null
    loadingModelName.value =
      'modelName' in status ? (status.modelName ?? '') : ''
    loadingProviderName.value = null
    loadErrorModelId.value =
      status.status === 'error' ? (status.modelId ?? null) : null
    if (status.status === 'error') lastError.value = t('chat.loading.error')
    if (status.status === 'ready') void refreshActiveModel()
  }
}

function onLoadError(event: ModelLoadErrorEvent) {
  loadingPhase.value = null
  loadErrorModelId.value = event.modelId ?? null
  lastError.value = t('chat.loading.error')
}

async function retryModelLoad() {
  const modelId = loadErrorModelId.value
  if (!modelId) return
  await loadModel(modelId)
}

/** Localised label for the current loading phase, if any. */
const loadingLabel = computed<string | null>(() => {
  const phase = loadingPhase.value
  if (!phase || phase === 'ready') return null
  const key =
    phase === 'connecting'
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
  try {
    await Promise.all(
      [
        chat.onToken(handleToken),
        chat.onMessageComplete(handleComplete),
        chat.onMessageError(handleError),
        chat.onToolCall(handleToolCall),
        chat.onToolResult(handleToolResult),
        chat.onRetry(handleRetry),
        chat.onTurnComplete(handleTurnComplete),
        chat.onToolPermissionRequest(handleToolPermissionRequest),
        chat.onModelLoadProgress(onLoadProgress),
        chat.onModelLoadStatus(onLoadStatus),
        chat.onModelLoadError(onLoadError),
        models.onDownloadProgress((e) => {
          if (downloadingId.value === e.modelId) {
            downloadProgressBytes.value = e.bytesDownloaded
            downloadTotalBytes.value = e.bytesTotal
          }
        }),
      ].map(async (subscription) => {
        const unlisten = await subscription
        // Registration can finish after navigation already disposed this page.
        if (unmounted) unlisten()
        else unlisteners.push(unlisten)
      }),
    )
    if (unmounted) return

    const device = await currentDeviceInfoAsync()
    try {
      const stored = await getPrefAsync(
        { kind: 'device', uuid: device.vaultDeviceUuid },
        PERMISSION_MODE_KEY,
      )
      if (stored === 'manual' || stored === 'auto' || stored === 'plan') {
        permissionMode.value = stored
      }
    } catch {
      // Keep the default ('manual') if the read fails.
    }
    if (unmounted) return
    deviceUuid.value = device.vaultDeviceUuid

    const loadStatus = await chat.modelLoadStatusAsync()
    if (loadStatus) onLoadStatus(loadStatus)
    await refreshActiveModel()
    await refreshInstalledAndCatalog()
    await refreshProviders()
    await refreshThreads()
  } catch (e: unknown) {
    if (!unmounted) lastError.value = errString(e)
  }
})

onBeforeUnmount(() => {
  unmounted = true
  if (durationRefreshTimer !== null) clearTimeout(durationRefreshTimer)
  durationRefreshTimer = null
  turnTerminalWaiters.clear()
  // Approval requests cannot be reconstructed by a freshly mounted chat page.
  if (streamingMessageId.value || turnSetupPending.value) void abort()
  for (const unlisten of unlisteners.splice(0)) unlisten()
})
</script>

<template>
  <main class="flex h-screen min-h-0 bg-muted/20">
    <aside
      class="hidden md:flex w-64 shrink-0 border-r border-border bg-background p-4 flex-col gap-4 overflow-y-auto"
    >
      <div class="flex items-center gap-3 min-w-0">
        <div
          class="h-9 w-9 shrink-0 rounded-xl bg-foreground text-background flex items-center justify-center"
        >
          <Icon name="lucide:sparkles" class="h-4 w-4" />
        </div>
        <div class="min-w-0">
          <div class="text-sm font-semibold">Holzi</div>
          <div
            class="text-xs text-muted-foreground truncate"
            :title="instanceName"
          >
            {{ instanceName }}
          </div>
        </div>
      </div>

      <UiButton
        class="w-full justify-start gap-2"
        variant="outline"
        :disabled="busy"
        @click="newChat"
      >
        <Icon name="lucide:plus" class="h-4 w-4" />
        {{ t('chat.newChat') }}
      </UiButton>

      <div class="flex items-center justify-between px-1">
        <span class="text-xs font-medium text-muted-foreground">{{
          t('chat.threads.title')
        }}</span>
        <span class="text-[10px] text-muted-foreground">{{
          threads.length
        }}</span>
      </div>
      <div class="space-y-1">
        <div
          v-for="thread in threads"
          :key="thread.id"
          class="group flex w-full min-w-0 items-center gap-1 rounded-lg text-sm transition-colors hover:bg-accent focus-within:bg-accent"
          :class="{ 'bg-accent font-medium': activeThreadId === thread.id }"
        >
          <template v-if="editingThreadId === thread.id">
            <div class="min-w-0 flex-1 px-2 py-1.5">
              <input
                ref="editingTitleInput"
                v-model="draftTitle"
                class="w-full rounded border border-border bg-background px-2 py-1 text-sm outline-none focus:border-foreground/50"
                :aria-label="t('chat.threads.editTitle')"
                :disabled="renamingThreadId === thread.id"
                @keydown.enter.prevent="saveThreadTitle"
                @keydown.esc.prevent="cancelEditing"
              />
              <p
                v-if="editTitleError"
                class="mt-1 text-xs text-destructive"
                role="alert"
              >
                {{ editTitleError }}
              </p>
            </div>
            <button
              type="button"
              class="shrink-0 rounded p-1.5 text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              :aria-label="t('chat.threads.saveTitle')"
              :title="t('chat.threads.saveTitle')"
              :disabled="renamingThreadId === thread.id"
              @click.stop="saveThreadTitle"
            >
              <Icon name="lucide:check" class="h-4 w-4" />
            </button>
            <button
              type="button"
              class="mr-1 shrink-0 rounded p-1.5 text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              :aria-label="t('chat.threads.cancelEdit')"
              :title="t('chat.threads.cancelEdit')"
              :disabled="renamingThreadId === thread.id"
              @click.stop="cancelEditing"
            >
              <Icon name="lucide:x" class="h-4 w-4" />
            </button>
          </template>
          <template v-else>
            <button
              type="button"
              class="min-w-0 flex-1 truncate px-2 py-2 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
              @click="selectThread(thread.id)"
            >
              <span class="truncate">{{ thread.title }}</span>
            </button>
            <div
              class="flex max-w-0 shrink-0 items-center overflow-hidden opacity-0 transition-[max-width,opacity] duration-150 group-hover:max-w-14 group-hover:opacity-100 group-focus-within:max-w-14 group-focus-within:opacity-100"
            >
              <button
                type="button"
                class="rounded p-1.5 text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                :aria-label="t('chat.threads.editTitle')"
                :title="t('chat.threads.editTitle')"
                @click.stop="startEditing(thread)"
              >
                <Icon name="lucide:pencil" class="h-3.5 w-3.5" />
              </button>
              <button
                type="button"
                class="mr-1 rounded p-1.5 text-muted-foreground hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                :aria-label="t('chat.threads.deleteTitle')"
                :title="t('chat.threads.deleteTitle')"
                @click.stop="requestDelete(thread)"
              >
                <Icon name="lucide:trash-2" class="h-3.5 w-3.5" />
              </button>
            </div>
            <span
              class="w-8 shrink-0 rounded pr-1 text-right text-xs font-normal tabular-nums text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              tabindex="0"
              :title="openingTimeLabel(thread.createdAt)"
              :aria-label="`${t('chat.threads.duration', { duration: historyDurationLabel(thread.createdAt) })}, ${openingTimeLabel(thread.createdAt)}`"
            >
              {{ historyDurationLabel(thread.createdAt) }}
            </span>
          </template>
        </div>
        <div
          v-if="threads.length === 0"
          class="px-3 py-2 text-xs text-muted-foreground"
        >
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
      <UiButton
        class="justify-start gap-2"
        size="sm"
        variant="ghost"
        @click="lock"
      >
        <Icon name="lucide:lock-keyhole" class="h-4 w-4" />
        {{ t('chat.lock') }}
      </UiButton>
    </aside>

    <UiDrawerModal
      v-if="deleteCandidate"
      :open="deleteCandidate !== null"
      :title="t('chat.threads.deleteDialogTitle')"
      @update:open="closeDeleteDialog"
    >
      <template #content>
        <div class="space-y-3 px-6 py-2">
          <p class="text-sm">
            {{
              t('chat.threads.deleteConfirm', {
                title: deleteCandidate.title,
              })
            }}
          </p>
          <p v-if="deleteError" class="text-sm text-destructive" role="alert">
            {{ deleteError }}
          </p>
        </div>
      </template>
      <template #footer>
        <div class="flex justify-end gap-2">
          <UiButton
            type="button"
            variant="outline"
            :disabled="deletingThread"
            @click="closeDeleteDialog(false)"
          >
            {{ t('chat.cancel') }}
          </UiButton>
          <UiButton
            type="button"
            variant="destructive"
            :loading="deletingThread"
            @click="confirmDelete"
          >
            {{ t('chat.threads.deleteConfirmButton') }}
          </UiButton>
        </div>
      </template>
    </UiDrawerModal>

    <section class="min-w-0 flex-1 flex flex-col">
      <header
        class="flex items-center justify-between gap-3 border-b border-border bg-background/90 px-4 py-3 backdrop-blur md:px-6"
      >
        <div class="min-w-0">
          <div class="flex items-center gap-2">
            <div
              class="h-2 w-2 rounded-full"
              :class="activeModel ? 'bg-emerald-500' : 'bg-muted-foreground/40'"
            />
            <h1 class="truncate text-sm font-semibold">
              {{
                activeThreadId
                  ? threads.find((thread) => thread.id === activeThreadId)
                      ?.title || t('chat.newChat')
                  : t('chat.newChat')
              }}
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
          <UiButton
            class="gap-2"
            size="sm"
            variant="outline"
            :disabled="busy"
            @click="newChat"
          >
            <Icon name="lucide:plus" class="h-4 w-4" />
            {{ t('chat.newChatShort') }}
          </UiButton>
        </div>
      </header>

      <div
        v-if="lastError"
        class="border-b border-destructive/20 bg-destructive/10 p-3 text-sm text-destructive flex items-start justify-between gap-2"
      >
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
            v-if="loadErrorModelId"
            class="text-xs underline"
            @click="retryModelLoad"
          >
            {{ t('chat.loading.retry') }}
          </button>
          <button class="text-xs underline" @click="lastError = null">
            {{ t('chat.close') }}
          </button>
        </span>
      </div>

      <div
        v-if="loadingLabel"
        class="border-b border-blue-500/20 bg-blue-500/10 p-3 text-sm text-blue-800"
        role="status"
      >
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
            class="relative overflow-hidden border border-border rounded p-3 flex items-center justify-between gap-4"
          >
            <div
              v-if="downloadingId === e.id"
              class="pointer-events-none absolute inset-y-0 left-0 bg-blue-100/70 transition-[width] duration-150"
              :class="
                downloadProgressPercent(e.id) === null ? 'animate-pulse' : ''
              "
              :style="{ width: `${downloadProgressPercent(e.id) ?? 35}%` }"
              role="progressbar"
              :aria-valuenow="downloadProgressPercent(e.id) ?? undefined"
              aria-valuemin="0"
              aria-valuemax="100"
              :aria-label="`${humanBytes(downloadProgressBytes)} / ${humanBytes(downloadTotalBytes)}`"
            />
            <div class="relative z-10 flex-1 min-w-0">
              <div class="font-medium text-sm">
                {{ e.name }}
              </div>
              <div class="text-xs text-muted-foreground truncate">
                {{ e.hf_repo }}/{{ e.hf_filename }}
              </div>
              <div class="text-xs text-muted-foreground">
                {{
                  t('chat.catalog.meta', {
                    size: humanBytes(e.approx_size_bytes),
                    context: e.context_window.toLocaleString(),
                    license: e.license,
                    fit: fitLabel(e.fit),
                  })
                }}
              </div>
            </div>
            <div class="relative z-10">
              <UiButton
                size="sm"
                :disabled="downloadingId !== null"
                @click="downloadCatalogEntry(e)"
              >
                <template v-if="downloadingId === e.id">
                  {{ humanBytes(downloadProgressBytes) }} /
                  {{ humanBytes(downloadTotalBytes) }}
                </template>
                <template v-else>
                  {{ t('chat.download') }}
                </template>
              </UiButton>
            </div>
          </div>
        </div>
      </div>

      <div
        v-else-if="!activeModel && !modelLoadPending && !loadingPhase"
        class="flex-1 flex items-center justify-center p-6 text-muted-foreground"
      >
        <div class="flex w-full max-w-sm flex-col gap-3">
          <p>{{ t('chat.model.selectPrompt') }}</p>
          <label for="chat-model-empty" class="sr-only">{{
            t('chat.model.label')
          }}</label>
          <select
            id="chat-model-empty"
            class="rounded-lg border border-border bg-background px-3 py-2 text-sm"
            :value="activeModelId"
            :disabled="busy"
            @change="(e) => loadModel((e.target as HTMLSelectElement).value)"
          >
            <option value="" disabled>{{ t('chat.model.choose') }}</option>
            <optgroup
              v-for="group in modelGroups"
              :key="group.providerId"
              :label="group.providerName"
            >
              <option v-for="m in group.models" :key="m.id" :value="m.id">
                {{ m.name }}
              </option>
            </optgroup>
          </select>
        </div>
      </div>

      <div v-else class="flex-1 flex flex-col overflow-hidden">
        <div
          data-messages-scroll
          class="flex-1 min-h-0 overflow-y-auto px-4 py-6 md:px-8"
        >
          <div
            v-if="activeMessages.length === 0"
            class="mx-auto flex h-full max-w-3xl flex-col items-center justify-center text-center"
          >
            <div
              class="mb-4 flex h-12 w-12 items-center justify-center rounded-2xl bg-foreground text-background"
            >
              <Icon name="lucide:sparkles" class="h-5 w-5" />
            </div>
            <h2 class="text-xl font-semibold tracking-tight">
              {{ t('chat.empty.title') }}
            </h2>
            <p class="mt-2 max-w-md text-sm text-muted-foreground">
              {{
                t('chat.empty.description', {
                  modelName: activeModel?.name ?? loadingModelName,
                })
              }}
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
              <Icon
                :name="
                  m.role === 'assistant' ? 'lucide:sparkles' : 'lucide:wrench'
                "
                class="h-3.5 w-3.5"
              />
            </div>
            <div
              class="min-w-0 max-w-[min(90%,48rem)]"
              :class="m.role === 'user' ? 'order-first' : ''"
            >
              <div
                class="mb-1 flex items-center gap-2 text-xs text-muted-foreground"
              >
                <template v-if="m.role === 'tool_call'">
                  {{ t('chat.tool.call', { name: m.toolName }) }}
                </template>
                <template v-else-if="m.role === 'tool_result'">
                  {{
                    m.toolIsError
                      ? t('chat.tool.resultError')
                      : t('chat.tool.result')
                  }}
                </template>
                <template v-else>
                  {{
                    m.role === 'user'
                      ? t('chat.sender.user')
                      : m.role === 'assistant'
                        ? t('chat.sender.assistant')
                        : t('chat.sender.system')
                  }}
                  <span
                    v-if="m.role === 'assistant' && m.completionTokens"
                    class="ml-2"
                  >
                    {{ t('chat.tokens', { count: m.completionTokens }) }}
                  </span>
                  <span
                    v-if="m.finishReason === 'error'"
                    class="ml-2 text-destructive"
                  >
                    {{ t('chat.errorLabel') }}
                  </span>
                  <span
                    v-if="m.finishReason === 'cancelled'"
                    class="ml-2 text-muted-foreground"
                  >
                    {{ t('chat.cancelledLabel') }}
                  </span>
                  <span
                    v-if="m.finishReason === 'tool_limit_reached'"
                    class="ml-2 text-amber-600"
                  >
                    {{ t('chat.tool.limitReached') }}
                  </span>
                  <span
                    v-if="retryingMessageId === m.id"
                    class="ml-2 text-muted-foreground italic"
                  >
                    {{ t('chat.retrying') }}
                  </span>
                </template>
              </div>
              <div
                class="rounded-2xl px-4 py-3 text-sm leading-6 shadow-sm"
                :class="{
                  'whitespace-pre-wrap':
                    m.role === 'user' ||
                    m.role === 'tool_call' ||
                    m.role === 'tool_result',
                  'bg-foreground text-background': m.role === 'user',
                  'border border-border bg-background':
                    m.role === 'assistant' || m.role === 'system',
                  'rounded-lg bg-muted/30 font-mono text-xs leading-5':
                    m.role === 'tool_call' ||
                    (m.role === 'tool_result' && !m.toolIsError),
                  'rounded-lg bg-destructive/10 text-destructive font-mono text-xs leading-5':
                    m.role === 'tool_result' && m.toolIsError,
                }"
              >
                <template v-if="m.role === 'tool_call'">{{
                  m.toolInput
                }}</template>
                <!-- eslint-disable vue/no-v-html -->
                <div
                  v-else-if="m.role === 'assistant' || m.role === 'system'"
                  class="chat-markdown"
                  v-html="
                    renderMarkdown(
                      m.content || (streamingMessageId === m.id ? '…' : ''),
                    )
                  "
                />
                <!-- eslint-enable vue/no-v-html -->
                <template v-else>{{
                  m.content || (streamingMessageId === m.id ? '…' : '')
                }}</template>
              </div>
              <ReasoningAccordion
                v-if="m.role === 'assistant' && reasoningFor(m.id)"
                :reasoning="reasoningFor(m.id)"
                :label="t('chat.reasoning.title')"
                :expanded="expandedReasoning.has(m.id)"
                @update:expanded="setReasoningExpanded(m.id, $event)"
              />
            </div>
          </div>
        </div>

        <form
          class="border-t border-border bg-background/90 px-4 pb-4 pt-3 backdrop-blur md:px-8"
          @submit.prevent="() => send()"
        >
          <div class="mx-auto max-w-3xl">
            <div
              class="rounded-2xl border border-border bg-background shadow-sm transition-shadow focus-within:border-foreground/30 focus-within:shadow-md"
            >
              <textarea
                ref="textareaRef"
                v-model="input"
                rows="1"
                class="block w-full resize-none overflow-hidden bg-transparent px-4 pb-2 pt-3 text-sm leading-6 outline-none placeholder:text-muted-foreground"
                :placeholder="t('chat.composer.placeholder')"
                :disabled="composerInputDisabled"
                @keydown.enter.exact.prevent="send()"
              />
              <div
                class="flex min-w-0 items-center gap-2 overflow-x-auto px-1 pb-1"
                :aria-label="t('chat.composer.settingsLabel')"
              >
                <div
                  class="flex min-w-0 flex-1 items-center gap-1.5 overflow-x-auto text-xs"
                >
                  <ComposerSettingsPopover
                    :model-id="activeModelId"
                    :model-name="activeModel?.name"
                    :model-groups="modelGroups"
                    :effort-level="effortLevel"
                    :effort-label="effortLabel"
                    :disabled="busy"
                    :model-disabled="modelGroups.length === 0"
                    @update:model-id="loadModel"
                    @update:effort-level="updateEffortLevel"
                  />

                  <PermissionPrompt
                    :mode="permissionMode"
                    :pending-approvals="pendingApprovals"
                    :disabled="!deviceUuid || permissionModeSaving"
                    @update:mode="updatePermissionMode"
                    @allow="respondToApproval($event, 'allow')"
                    @deny="respondToApproval($event, 'deny')"
                    @cancel="abort"
                  />
                </div>
                <UiButton
                  v-if="streamingMessageId || turnSetupPending"
                  class="shrink-0 gap-2"
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
                  class="shrink-0"
                  size="icon-sm"
                  type="submit"
                  :disabled="!input.trim() || sendDisabled"
                  :aria-label="t('chat.send')"
                  :title="t('chat.send')"
                >
                  <Icon name="lucide:arrow-up" class="h-3.5 w-3.5" />
                </UiButton>
              </div>
            </div>
          </div>
        </form>
      </div>
    </section>

    <ModelsModelIntegrityDialog
      v-if="integrityDialog"
      :open="integrityDialog !== null"
      :error-kind="integrityDialog.errorKind"
      :expected-sha256="integrityDialog.expected"
      :actual-sha256="integrityDialog.actual"
      :busy="integrityBusy"
      :action-error="integrityActionError"
      @update:open="onIntegrityDialogOpenChange"
      @load-untrusted="onIntegrityLoadUntrusted"
      @repair-source="onIntegrityRepairSource"
      @choose-other="onIntegrityChooseOther"
    />
  </main>
</template>
