import { ref, type Ref } from 'vue'
import type { Message, Thread, useChat } from '~/composables/useChat'
import type { useChatTranscript } from '~/composables/useChatTranscript'
import type { PendingApproval } from '~/components/chat/PermissionPrompt.vue'

type HistoryDurationUnit = 'min' | 'h' | 'd'

type HistoryDuration = {
  value: number
  unit: HistoryDurationUnit
}

/**
 * Thread list state and its title-editing/delete-confirmation/selection
 * flows — extracted from `src/pages/chat/[instance].vue` (2026-09-15
 * review, split step 3/4). See that file's own history for the rest of
 * the page's split plan.
 *
 * `confirmDelete` and `selectThread` reach into `chatTranscript` (the
 * `useChatTranscript` instance the page already built) for
 * `hasPendingTurn`/`waitForTurnTerminal` and the queued-tool-event replay
 * — a deliberate cross-composable call, not a layering slip, since those
 * belong to `turnTerminalWaiters`/`pendingToolEvents`, which travelled
 * with the transcript composable in the prior split step.
 */
export function useThreadSidebar(
  chat: ReturnType<typeof useChat>,
  chatTranscript: ReturnType<typeof useChatTranscript>,
  t: (key: string, params?: Record<string, unknown>) => string,
  messagesByThread: Ref<Record<string, Message[]>>,
  activeThreadId: Ref<string | null>,
  input: Ref<string>,
  streamingMessageId: Ref<string | null>,
  streamingThreadId: Ref<string | null>,
  streamingBuffer: Ref<string>,
  reasoningByMessage: Ref<Record<string, string>>,
  expandedReasoning: Ref<Set<string>>,
  pendingApprovals: Ref<PendingApproval[]>,
  pendingApprovalsByThread: Map<string, PendingApproval[]>,
  resetTextarea: () => Promise<void>,
  scrollToBottom: () => Promise<void>,
) {
  const {
    pendingStreamEvents,
    pendingToolEvents,
    pendingTurnCompletions,
    handleToolCall,
    handleToolResult,
    hasPendingTurn,
    waitForTurnTerminal,
  } = chatTranscript

  const threads = ref<Thread[]>([])
  const durationNow = ref(Date.now())
  let durationRefreshTimer: ReturnType<typeof setTimeout> | null = null
  const editingThreadId = ref<string | null>(null)
  const draftTitle = ref('')
  const editTitleError = ref<string | null>(null)
  const renamingThreadId = ref<string | null>(null)
  const deleteCandidate = ref<Thread | null>(null)
  const deleteError = ref<string | null>(null)
  const deletingThread = ref(false)

  /** Projects a persisted Unix-millisecond opening time into the compact UI form. */
  function historyDuration(
    createdAt: number,
    now = Date.now(),
  ): HistoryDuration {
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

  /** Cancels the pending duration-refresh timer, if any (page teardown). */
  function stopDurationRefresh() {
    if (durationRefreshTimer !== null) clearTimeout(durationRefreshTimer)
    durationRefreshTimer = null
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

  return {
    threads,
    durationNow,
    editingThreadId,
    draftTitle,
    editTitleError,
    renamingThreadId,
    deleteCandidate,
    deleteError,
    deletingThread,
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
    clearDeletedThreadState,
    confirmDelete,
    selectThread,
  }
}
