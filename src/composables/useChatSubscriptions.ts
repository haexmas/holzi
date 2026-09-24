import type { UnlistenFn } from '@tauri-apps/api/event'

/**
 * Awaits each `chat.on*` subscription promise and stores its unlisten
 * function — unless the component unmounted before the subscription
 * resolved (registration can finish after navigation already disposed the
 * page), in which case it unlistens immediately instead. Extracted from
 * `src/pages/chat/[instance].vue`'s `onMounted` (spec 015-workspace-shell,
 * T013, to keep the orchestrator page under the 500-line constitution
 * limit).
 */
export async function registerChatSubscriptions(
  subscriptions: Promise<UnlistenFn>[],
  unlisteners: UnlistenFn[],
  isUnmounted: () => boolean,
) {
  await Promise.all(
    subscriptions.map(async (subscription) => {
      const unlisten = await subscription
      if (isUnmounted()) unlisten()
      else unlisteners.push(unlisten)
    }),
  )
}
