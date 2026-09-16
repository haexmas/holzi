<script setup lang="ts">
/*
 * Maintainability exception (spaex 500-LoC rule): the 2026-09-15/16
 * review's split is done — see `scripts/check-chat-state.ts`'s,
 * `useChatTranscript.ts`'s, `useThreadSidebar.ts`'s and
 * `useModelsStore`'s own history (the deferred Pinia migration flagged in
 * `ModelSelection.vue`'s prior header: activeModel, installedModels,
 * catalogEntries, providerList, providerModels and the
 * download/integrity/load-status state now live in `useModelsStore`,
 * along with `loadModel`, the integrity-dialog handlers,
 * `downloadCatalogEntry`, the refresh functions and the `onLoad*` status
 * handlers — `ModelSelection.vue` reads the store directly). What's left
 * on this page is the composer/send flow (`send`, `abort`, `newChat`,
 * `lock`, ~250 lines) plus the `onMounted` wiring that ties the
 * transcript, thread-sidebar and model-store together.
 *
 * Concrete split plan, if this grows further: extract the composer/send
 * flow (`send`, `abort`, `newChat`, `pendingSend`,
 * `effortLevel`/`effortTokens`, `input`, `busy`, `turnSetupPending`) into a
 * `useComposer` composable next to `useChatTranscript`/`useThreadSidebar`,
 * taking the same instance (`chat`, `chatTranscript`) as a dependency.
 */
import { computed, onMounted, onBeforeUnmount, ref, nextTick } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import DOMPurify from 'dompurify'
import { marked } from 'marked'
import {
  useChat,
  type Message,
  type SendMessageArgs,
} from '~/composables/useChat'
import { useChatTranscript } from '~/composables/useChatTranscript'
import { useThreadSidebar } from '~/composables/useThreadSidebar'
import { useErrorString } from '~/composables/useErrorString'
import { useInstance } from '~/composables/useInstance'
import { usePreferences } from '~/composables/usePreferences'
import { useDevice } from '~/composables/useDevice'
import PermissionPrompt, {
  type PendingApproval,
} from '~/components/chat/PermissionPrompt.vue'
import ComposerSettingsPopover from '~/components/chat/ComposerSettingsPopover.vue'
import ReasoningAccordion from '~/components/chat/ReasoningAccordion.vue'
import ModelSelection from '~/components/chat/ModelSelection.vue'
import { useAutoResizeTextarea } from '~/composables/useAutoResizeTextarea'

definePageMeta({
  middleware: ['onboarded'],
})

const route = useRoute()
const { t } = useI18n()
const chat = useChat()
const { closeAsync } = useInstance()
const { getPrefAsync, setPrefAsync } = usePreferences()
const { currentDeviceInfoAsync } = useDevice()
const { errString } = useErrorString()
const store = useInstancesStore()
const modelStore = useModelsStore()
const {
  activeModel,
  modelLoadPending,
  loadingPhase,
  loadingModelName,
  loadErrorModelId,
  loadingLabel,
  noModelsInstalled,
  activeModelId,
  modelGroups,
  providerList,
  integrityDialog,
  integrityBusy,
  integrityActionError,
  lastError: modelLastError,
} = storeToRefs(modelStore)
const {
  loadModel,
  retryModelLoad,
  onIntegrityLoadUntrusted,
  onIntegrityRepairSource,
  onIntegrityChooseOther,
  onIntegrityDialogOpenChange,
} = modelStore

const PERMISSION_MODE_KEY = 'chat.permission_mode'
const permissionMode = ref<'manual' | 'auto' | 'plan'>('manual')
const pendingApprovals = ref<PendingApproval[]>([])
const deviceUuid = ref('')
const permissionModeSaving = ref(false)
const unlisteners: UnlistenFn[] = []
let unmounted = false

const instanceName = computed(() => String(route.params.instance ?? ''))

const messagesByThread = ref<Record<string, Message[]>>({})
const activeThreadId = ref<string | null>(null)

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

const pendingApprovalsByThread = new Map<string, PendingApproval[]>()

const input = ref('')
const busy = ref(false)
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

// `chatTranscript.applyTurnComplete` needs `threadSidebar.refreshThreads`,
// and `threadSidebar.confirmDelete`/`selectThread` need
// `chatTranscript.hasPendingTurn`/`waitForTurnTerminal`/`handleToolCall`/
// `handleToolResult` — a genuine circular dependency between the two
// composables. Resolved with this indirection: `chatTranscript` is built
// first against a placeholder that's swapped for the real function the
// moment `threadSidebar` exists, before any asynchronous code can call it.
let refreshThreadsForTranscript = async () => {}
const chatTranscript = useChatTranscript(
  chat,
  messagesByThread,
  activeThreadId,
  streamingMessageId,
  streamingThreadId,
  streamingBuffer,
  reasoningByMessage,
  retryingMessageId,
  busy,
  turnSetupPending,
  lastError,
  pendingApprovals,
  pendingApprovalsByThread,
  () => refreshThreadsForTranscript(),
  scrollToBottom,
  errString,
)
const threadSidebar = useThreadSidebar(
  chat,
  chatTranscript,
  t,
  messagesByThread,
  activeThreadId,
  input,
  streamingMessageId,
  streamingThreadId,
  streamingBuffer,
  reasoningByMessage,
  expandedReasoning,
  pendingApprovals,
  pendingApprovalsByThread,
  resetTextarea,
  scrollToBottom,
)
refreshThreadsForTranscript = threadSidebar.refreshThreads

const {
  pendingStreamEvents,
  pendingToolEvents,
  pendingTurnCompletions,
  turnTerminalWaiters,
  handleToken,
  handleRetry,
  handleComplete,
  handleError,
  handleToolCall,
  handleToolResult,
  handleToolPermissionRequest,
  handleTurnComplete,
} = chatTranscript

const {
  threads,
  editingThreadId,
  draftTitle,
  editTitleError,
  editingTitleInput,
  renamingThreadId,
  deleteCandidate,
  deleteError,
  deletingThread,
  // Not called directly on the page anymore (historyDurationLabel, its
  // only caller, moved into the composable too) — kept here only because
  // scripts/check-chat-state.ts replays it in isolation as a pure
  // date-math function.
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  historyDuration,
  historyDurationLabel,
  openingTimeLabel,
  stopDurationRefresh,
  refreshThreads,
  startEditing,
  cancelEditing,
  saveThreadTitle,
  requestDelete,
  closeDeleteDialog,
  confirmDelete,
  selectThread,
} = threadSidebar

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

// Composer/thread errors (`lastError`) and model-lifecycle errors
// (`modelStore.lastError`) are separate refs — Pinia setup stores can't
// take page-local refs as constructor params — merged into the one banner
// the template shows.
const displayedError = computed(() => lastError.value || modelLastError.value)

function dismissError() {
  lastError.value = null
  modelLastError.value = null
}

const activeMessages = computed<Message[]>(() => {
  if (!activeThreadId.value) return []
  return messagesByThread.value[activeThreadId.value] ?? []
})

/** Scrolls the message viewport to its newest item after rendering. */
async function scrollToBottom() {
  await nextTick()
  const el = document.querySelector('[data-messages-scroll]')
  if (el) el.scrollTop = el.scrollHeight
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
  if (busy.value || modelLoadPending.value) return
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

function setReasoningExpanded(messageId: string, expanded: boolean) {
  const next = new Set(expandedReasoning.value)
  if (expanded) next.add(messageId)
  else next.delete(messageId)
  expandedReasoning.value = next
}

function reasoningFor(messageId: string): string {
  return reasoningByMessage.value[messageId] ?? ''
}

/**
 * Localized "Answered by Claude Code"/"Answered by Codex" label for a
 * delegate-answered message, or `null` for any other backend (spec.md
 * FR-005) — derived from the message's existing `modelId`
 * (`<providerId>:claude`/`<providerId>:codex`, the real cached model row
 * `providers/mod.rs::compose_model_row` produces), not a new field.
 */
function delegateAnsweredByLabel(modelId: string | null): string | null {
  if (!modelId) return null
  const [providerId, remoteId] = modelId.split(':')
  if (remoteId !== 'claude' && remoteId !== 'codex') return null
  const provider = providerList.value.find(
    (p) => p.id === providerId && p.kind === 'cli_delegate',
  )
  if (!provider) return null
  return t('chat.model.answeredByDelegate', {
    name: t(`chat.model.delegate.${remoteId}`),
  })
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

function renderMarkdown(content: string): string {
  return DOMPurify.sanitize(
    marked.parse(content, { async: false, breaks: true }),
  )
}

onMounted(async () => {
  try {
    await Promise.all([
      Promise.all(
        [
          chat.onToken(handleToken),
          chat.onMessageComplete(handleComplete),
          chat.onMessageError(handleError),
          chat.onToolCall(handleToolCall),
          chat.onToolResult(handleToolResult),
          chat.onRetry(handleRetry),
          chat.onTurnComplete(handleTurnComplete),
          chat.onToolPermissionRequest(handleToolPermissionRequest),
        ].map(async (subscription) => {
          const unlisten = await subscription
          // Registration can finish after navigation already disposed this page.
          if (unmounted) unlisten()
          else unlisteners.push(unlisten)
        }),
      ),
      modelStore.startListening(),
    ])
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

    await modelStore.initialize()
    await refreshThreads()
  } catch (e: unknown) {
    if (!unmounted) lastError.value = errString(e)
  }
})

onBeforeUnmount(() => {
  unmounted = true
  modelStore.stopListening()
  stopDurationRefresh()
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
        v-if="displayedError"
        class="border-b border-destructive/20 bg-destructive/10 p-3 text-sm text-destructive flex items-start justify-between gap-2"
      >
        <span>{{ displayedError }}</span>
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
          <button class="text-xs underline" @click="dismissError">
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

      <ModelSelection
        v-if="
          noModelsInstalled ||
          (!activeModel && !modelLoadPending && !loadingPhase)
        "
        :busy="busy"
      />

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
                    v-if="
                      m.role === 'assistant' &&
                      delegateAnsweredByLabel(m.modelId)
                    "
                    class="ml-2"
                  >
                    {{ delegateAnsweredByLabel(m.modelId) }}
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
