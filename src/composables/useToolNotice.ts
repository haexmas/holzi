import { computed, ref, type Ref } from 'vue'
import type { ToolAvailabilityEvent } from '~/composables/useChat'
import { createToolNotices, noticeKey } from '~/lib/chat/toolNotice'

// One memory for the whole app session, shared by every chat tab (see `createToolNotices`).
const notices = createToolNotices()

/**
 * The tool-use notice of the open conversation: set by `chat-tool-availability` the first time
 * a state shows up for a conversation, hidden when another conversation is open, closed by
 * `dismiss`.
 */
export function useToolNotice(activeThreadId: Ref<string | null>) {
  const current = ref<{ threadId: string; key: string } | null>(null)

  function handleToolAvailability(e: ToolAvailabilityEvent) {
    const key = noticeKey(e.state)
    if (key && notices.show(e.threadId, e.state))
      current.value = { threadId: e.threadId, key }
  }

  /** The i18n key of the notice to show now, if any. */
  const toolNoticeKey = computed(() =>
    current.value && current.value.threadId === activeThreadId.value
      ? current.value.key
      : null,
  )

  function dismissToolNotice() {
    current.value = null
  }

  return { toolNoticeKey, handleToolAvailability, dismissToolNotice }
}
