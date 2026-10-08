import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import type {
  ChoiceAnswer,
  ChoiceRequestEvent,
  ToolPermissionRequestEvent,
  useChat,
} from '~/composables/useChat'
import type { WmTabApi } from '~/composables/useWmTab'

/** Listens for the agent's questions (`chat-choice-request`, spec 046). */
export function onChoiceRequest(
  handler: (e: ChoiceRequestEvent) => void,
): Promise<UnlistenFn> {
  return listen<ChoiceRequestEvent>('chat-choice-request', (ev) =>
    handler(ev.payload),
  )
}

/** Answers one open question of the running turn. */
export async function respondChoiceAsync(
  requestId: string,
  answer: ChoiceAnswer,
): Promise<void> {
  await invoke('respond_choice', { args: { requestId, answer } })
}

/** The two subscriptions that make a turn wait on the user, approvals and questions: each marks
 * the tab as waiting before it queues the prompt. */
export function promptSubscriptions(
  chat: Pick<ReturnType<typeof useChat>, 'onToolPermissionRequest'>,
  wmTab: Pick<WmTabApi, 'requestAttention'>,
  handlers: {
    handleToolPermissionRequest: (e: ToolPermissionRequestEvent) => void
    handleChoiceRequest: (e: ChoiceRequestEvent) => void
  },
): Promise<UnlistenFn>[] {
  return [
    chat.onToolPermissionRequest((e) => {
      wmTab.requestAttention()
      handlers.handleToolPermissionRequest(e)
    }),
    onChoiceRequest((e) => {
      wmTab.requestAttention()
      handlers.handleChoiceRequest(e)
    }),
  ]
}
