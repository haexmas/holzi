import { computed, watch, type Ref } from 'vue'
import type { Thread } from '~/composables/useChat'
import type { TabRouter } from '~/composables/useTabRouter'

const THREAD_PREFIX = '/thread/'

/** The chat tab's location for a conversation (`/` = new conversation, the start location). */
export function chatPath(threadId: string | null): string {
  return threadId === null
    ? '/'
    : `${THREAD_PREFIX}${encodeURIComponent(threadId)}`
}

function threadIdOf(path: string): string | null {
  if (!path.startsWith(THREAD_PREFIX)) return null
  try {
    return decodeURIComponent(path.slice(THREAD_PREFIX.length))
  } catch {
    return null
  }
}

/**
 * Chat ↔ tab history (spec 020-tab-navigation, T025/T055, research R10). The
 * chat has two locations: `/` (new conversation, the start location — spec 004
 * still begins every chat entry there) and `/thread/<id>`. Kept in sync both
 * ways:
 *
 * - location → chat: reaching `/thread/<id>` runs the existing `selectThread`,
 *   reaching `/` runs `newChat`, so running replies and pending approvals keep
 *   following specs 003/006 (FR-027);
 * - chat → location: when the active conversation changes without a
 *   navigation (the first message creates the thread, the active thread is
 *   deleted, `newChat` refuses while a reply runs), the current entry is
 *   replaced, so back never lands on an empty new conversation.
 *
 * Takes the router as a parameter so the chat replay harness can pass its own.
 */
export function useChatNavigation(deps: {
  router: TabRouter
  activeThreadId: Ref<string | null>
  threads: Ref<Thread[]>
  /** Header/tab title of a conversation that has no title yet. */
  newChatLabel: () => string
  /** `useWmTab().setTitle`: the conversation title becomes the tab title (spec 020 US5). */
  setTitle: (title: string | null) => void
  selectThread: (id: string) => Promise<void> | void
  newChat: () => Promise<void> | void
}) {
  const { router, activeThreadId, threads } = deps

  const chatTitle = computed(
    () =>
      (activeThreadId.value &&
        threads.value.find((thread) => thread.id === activeThreadId.value)
          ?.title) ||
      deps.newChatLabel(),
  )

  function openConversation(threadId: string) {
    router.push(chatPath(threadId))
  }

  function startNewConversation() {
    router.push('/')
  }

  function alignLocation() {
    const expected = chatPath(activeThreadId.value)
    if (router.route.path !== expected) router.replace(expected)
  }

  let threadsLoaded = false
  async function applyLocation(path: string) {
    const threadId = threadIdOf(path)
    if (threadId === activeThreadId.value) return
    if (threadId !== null && !threads.value.some((t) => t.id === threadId)) {
      // The list is not loaded yet: decide once `syncFromLocation` runs.
      if (!threadsLoaded) return
      // FR-027: an entry of a deleted conversation is skipped in travel direction.
      if (!router.skipCurrent()) router.replace('/')
      return
    }
    if (threadId !== null) await deps.selectThread(threadId)
    else await deps.newChat()
    alignLocation()
  }
  watch(() => router.route.path, applyLocation)

  /** Call once the thread list has loaded: applies the location the tab started at. */
  function syncFromLocation() {
    threadsLoaded = true
    return applyLocation(router.route.path)
  }

  watch(activeThreadId, alignLocation)

  // The tab shows the open conversation's title; a new conversation keeps the app/route title.
  // The window manager resets this override on every navigation, so it is set again for the new view.
  watch(
    [activeThreadId, chatTitle, () => router.route.path],
    () => deps.setTitle(activeThreadId.value ? chatTitle.value : null),
    { immediate: true },
  )

  return {
    chatTitle,
    openConversation,
    startNewConversation,
    syncFromLocation,
  }
}
