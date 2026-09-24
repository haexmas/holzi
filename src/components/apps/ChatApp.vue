<script setup lang="ts">
/*
 * Split across `useChatTranscript`/`useThreadSidebar`/`useComposer`/
 * `useComposerAttachments` (composables) and `ThreadSidebar`/`MessageList`/
 * `Composer` (presentational components, spec 015-workspace-shell T010-T013)
 * — see those files' own headers. What's left here is the orchestration:
 * page-level state shared across more than one of the above, model-store
 * wiring, permission-mode/autonomy-mode persistence and the `onMounted`
 * event-listener setup.
 *
 * Moved from `pages/chat/[instance].vue` into a Shell app (T021): the
 * instance name comes from `useInstancesStore()` rather than the route (this
 * component no longer owns one), and `lock()` flushes the Shell layout
 * first (FR-027). Onboarding enforcement (spec 002) now lives on the Shell
 * host page (`pages/workspace/[instance].vue`, T025), not here.
 */
import {
  computed,
  onMounted,
  onBeforeUnmount,
  ref,
  nextTick,
  useTemplateRef,
} from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { Message, SendMessageArgs } from '~/composables/useChat'
import type { PendingApproval } from '~/components/chat/PermissionPrompt.vue'

const instancesStore = useInstancesStore()
const shell = useShellStore()
const { t } = useI18n()
const chat = useChat()
const { closeAsync } = useInstance()
const { getPrefAsync, setPrefAsync } = usePreferences()
const { currentDeviceInfoAsync } = useDevice()
const { errString } = useErrorString()
const modelStore = useModelsStore()
const {
  activeModel,
  modelLoadPending,
  loadingPhase,
  loadingModelName,
  loadErrorModelId,
  loadingLabel,
  noModelsInstalled,
  displayModelId,
  effortLevel,
  providerList,
  integrityDialog,
  integrityBusy,
  integrityActionError,
  lastError: modelLastError,
} = storeToRefs(modelStore)
const {
  retryModelLoad,
  onIntegrityLoadUntrusted,
  onIntegrityRepairSource,
  onIntegrityChooseOther,
  onIntegrityDialogOpenChange,
} = modelStore

const pendingApprovals = ref<PendingApproval[]>([])
const unlisteners: UnlistenFn[] = []
let unmounted = false

const instanceName = computed(() => instancesStore.activeInstance ?? '')

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
/** True while the voice control is recording: it then provides the send button. */
const voiceRecording = ref(false)
const busy = ref(false)
// True while `send()` has set `activeThreadId` but has not yet appended
// this turn's user/assistant placeholder rows — a `chat-tool-call`/
// `chat-tool-result` for that (already-active) thread can otherwise land
// before the messages it belongs after (backend events can arrive before
// `sendMessageAsync`'s own await resolves).
const turnSetupPending = ref(false)
const lastError = ref<string | null>(null)
const pendingSend = ref<SendMessageArgs | null>(null)

const {
  permissionMode,
  autonomyMode,
  autonomyPreferenceLoading,
  autonomyPreferenceError,
  deviceUuid,
  permissionModeSaving,
  updatePermissionMode,
  reloadAutonomyMode,
  initialize: initializePermissionMode,
} = useChatPermissionMode(getPrefAsync, setPrefAsync, errString, lastError)

const { attachments, addAttachments, removeAttachment } =
  useComposerAttachments(chat, displayModelId, errString, lastError)

// `Composer.vue` owns the <textarea> and its auto-resize composable; this
// wrapper lets useComposer/useThreadSidebar keep a plain `() => Promise<void>`
// dependency for clearing the input from outside it (new chat, thread delete).
const composerRef = useTemplateRef<{ reset: () => Promise<void> } | null>(
  'composer',
)
async function resetTextarea() {
  await composerRef.value?.reset()
}

// chatTranscript/threadSidebar need each other's functions (refreshThreads /
// hasPendingTurn+co) — resolved by building chatTranscript against a
// placeholder swapped for the real function once threadSidebar exists.
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
  renamingThreadId,
  deleteCandidate,
  deleteError,
  deletingThread,
  // Unused here — kept because check-chat-state.ts replays it standalone.
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
    loadingPhase.value !== null ||
    (isDelegateModel.value &&
      (autonomyPreferenceLoading.value ||
        autonomyPreferenceError.value !== null)),
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

const chatTitle = computed(
  () =>
    (activeThreadId.value &&
      threads.value.find((thread) => thread.id === activeThreadId.value)
        ?.title) ||
    t('chat.newChat'),
)

// Only the message area falls back to the catalog-download state, and only
// when literally no model is installed/configured anywhere (spec 002
// §FR-014 covers picking among models that DO exist).
const showModelSelection = computed(() => noModelsInstalled.value)

/** Scrolls the message viewport to its newest item after rendering. */
async function scrollToBottom() {
  await nextTick()
  const el = document.querySelector('[data-messages-scroll]')
  if (el) el.scrollTop = el.scrollHeight
}

/**
 * Flushes the Shell layout (FR-027), then asks the backend to close the vault. It replaces this
 * page with a spinner and ends the process (spec 013), so nothing is navigated or cleared here and
 * a failed call has nothing to show.
 */
async function lock() {
  await shell.flushAsync()
  await closeAsync().catch(() => {})
}

function setReasoningExpanded(messageId: string, expanded: boolean) {
  const next = new Set(expandedReasoning.value)
  if (expanded) next.add(messageId)
  else next.delete(messageId)
  expandedReasoning.value = next
}

/** Whether the active model resolves to a `cli_delegate` provider — the
 * same provider lookup `MessageList.vue`'s `delegateAnsweredByLabel` does
 * (spec 009-autonomous-delegate-mode: the autonomy control only makes
 * sense for a delegate backend). */
const isDelegateModel = computed(() => {
  const modelId = activeModel.value?.modelId ?? null
  if (!modelId) return false
  const [providerId] = modelId.split(':')
  return providerList.value.some(
    (p) => p.id === providerId && p.kind === 'cli_delegate',
  )
})

const {
  send,
  abort,
  newChat,
  onVoiceTranscript,
  activeAgentCount,
  lastAgentBatchSize,
  handleAgentActivity,
  resetAgentActivity,
} = useComposer(
  chat,
  chatTranscript,
  refreshThreads,
  scrollToBottom,
  errString,
  resetTextarea,
  input,
  busy,
  pendingSend,
  turnSetupPending,
  lastError,
  activeModel,
  modelLoadPending,
  isDelegateModel,
  autonomyMode,
  effortLevel,
  sendDisabled,
  activeThreadId,
  messagesByThread,
  streamingMessageId,
  streamingThreadId,
  streamingBuffer,
  reasoningByMessage,
  retryingMessageId,
  expandedReasoning,
  attachments,
  pendingApprovals,
  pendingApprovalsByThread,
)

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

onMounted(async () => {
  try {
    await Promise.all([
      registerChatSubscriptions(
        [
          chat.onToken(handleToken),
          chat.onMessageComplete(handleComplete),
          chat.onMessageError(handleError),
          chat.onToolCall(handleToolCall),
          chat.onToolResult(handleToolResult),
          chat.onRetry(handleRetry),
          chat.onAgentActivity(handleAgentActivity),
          chat.onTurnComplete((e) => {
            // A turn ending — successfully, cancelled, or errored — is the
            // authoritative point to clear any lingering agent-activity
            // state (FR-011), the same signal that already clears
            // `streamingMessageId`/`busy`.
            resetAgentActivity()
            return handleTurnComplete(e)
          }),
          chat.onToolPermissionRequest(handleToolPermissionRequest),
        ],
        unlisteners,
        () => unmounted,
      ),
      modelStore.startListening(),
    ])
    if (unmounted) return

    const device = await currentDeviceInfoAsync()
    await initializePermissionMode(device.vaultDeviceUuid)
    if (unmounted) return

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
  <main class="flex h-full min-h-0 bg-muted/20">
    <ChatThreadSidebar
      :instance-name="instanceName"
      :busy="busy"
      :threads="threads"
      :active-thread-id="activeThreadId"
      :editing-thread-id="editingThreadId"
      :draft-title="draftTitle"
      :edit-title-error="editTitleError"
      :renaming-thread-id="renamingThreadId"
      :delete-candidate="deleteCandidate"
      :delete-error="deleteError"
      :deleting-thread="deletingThread"
      :history-duration-label="historyDurationLabel"
      :opening-time-label="openingTimeLabel"
      @update:draft-title="draftTitle = $event"
      @new-chat="newChat"
      @select-thread="selectThread"
      @start-editing="startEditing"
      @save-title="saveThreadTitle"
      @cancel-editing="cancelEditing"
      @request-delete="requestDelete"
      @close-delete-dialog="closeDeleteDialog"
      @confirm-delete="confirmDelete"
      @lock="lock"
      @open-settings="shell.openApp('system.settings')"
    />

    <section class="min-w-0 flex-1 flex flex-col">
      <ChatHeader
        :title="chatTitle"
        :model-loaded="!!activeModel"
        :model-name="activeModel?.name || t('chat.model.notLoaded')"
        :busy="busy"
        @lock="lock"
        @new-chat="newChat"
        @open-settings="shell.openApp('system.settings')"
      />

      <ChatStatusBanners
        :displayed-error="displayedError"
        :can-retry-send="!!pendingSend"
        :load-error-model-id="loadErrorModelId"
        :autonomy-preference-error="autonomyPreferenceError"
        :autonomy-preference-loading="autonomyPreferenceLoading"
        :loading-label="loadingLabel"
        @retry-send="send(true)"
        @retry-model-load="retryModelLoad"
        @dismiss-error="dismissError"
        @retry-autonomy-mode="reloadAutonomyMode(deviceUuid)"
      />

      <div class="flex-1 flex flex-col overflow-hidden">
        <div
          data-messages-scroll
          class="flex-1 min-h-0 overflow-y-auto px-4 py-6 md:px-8"
        >
          <ChatMessageList
            :active-messages="activeMessages"
            :show-model-selection="showModelSelection"
            :active-model-name="activeModel?.name"
            :loading-model-name="loadingModelName"
            :streaming-message-id="streamingMessageId"
            :retrying-message-id="retryingMessageId"
            :reasoning-by-message="reasoningByMessage"
            :expanded-reasoning="expandedReasoning"
            :provider-list="providerList"
            @toggle-reasoning="setReasoningExpanded"
          />
        </div>

        <ChatComposer
          ref="composer"
          v-model:input="input"
          v-model:voice-recording="voiceRecording"
          :composer-input-disabled="composerInputDisabled"
          :send-disabled="sendDisabled"
          :streaming-message-id="streamingMessageId"
          :turn-setup-pending="turnSetupPending"
          :busy="busy"
          :attachments="attachments"
          :active-agent-count="activeAgentCount"
          :last-agent-batch-size="lastAgentBatchSize"
          :permission-mode="permissionMode"
          :pending-approvals="pendingApprovals"
          :permission-disabled="!deviceUuid || permissionModeSaving"
          @send="send()"
          @abort="abort"
          @add-attachments="addAttachments"
          @remove-attachment="removeAttachment"
          @update-permission-mode="updatePermissionMode"
          @respond-approval="respondToApproval"
          @transcript="onVoiceTranscript"
        />
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
