import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import {
  listAgentActions,
  toOutcomeWire,
  type ActionLocale,
} from '~/lib/actions/agentTools'
import { ALL_ACTIONS } from '~/lib/actions/catalog'
import type { ActionOutcome } from '~/lib/actions/types'

/** `action-call-request` (contracts/tauri-commands.md): a model in the chat asks for an action. */
interface ActionCallRequest {
  requestId: string
  actionId: string
  input: Record<string, unknown>
}

/**
 * The built-in agent's access to the actions (spec 032, ADR-0006). Pushes the definitions the
 * built-in agent may call to Rust as tools (`set_agent_actions`) and runs the calls Rust sends back
 * (`action-call-request`) through the same runner a click uses, with caller `builtinAgent`.
 *
 * Call `startAsync()` once per vault session, from the workspace page: both Tauri commands need
 * an open vault, and the listener stays for the session like the sync listener. Definitions are
 * pushed again when the language changes, because they carry localized titles for the search.
 */
let started = false

export function useAgentActions() {
  const wm = useWindowManagerStore()
  const { t, locale } = useI18n()

  const titleOf = (key: string, language: ActionLocale) =>
    t(key, {}, { locale: language })

  async function pushDefinitionsAsync(): Promise<void> {
    await invoke('set_agent_actions', {
      args: { actions: listAgentActions(ALL_ACTIONS, titleOf) },
    })
  }

  async function runCallAsync(request: ActionCallRequest): Promise<void> {
    let outcome: ActionOutcome
    try {
      outcome = await wm.runAction(request.actionId, request.input, {
        kind: 'builtinAgent',
      })
    } catch (error) {
      console.error('[agent] running an action failed', error)
      outcome = {
        ok: false,
        code: 'failed',
        message: 'The action failed.',
      }
    }
    await invoke('respond_action_call', {
      args: { requestId: request.requestId, outcome: toOutcomeWire(outcome) },
    })
  }

  async function startAsync(): Promise<void> {
    if (started) return
    started = true
    await listen<ActionCallRequest>('action-call-request', (event) => {
      void runCallAsync(event.payload).catch((error: unknown) => {
        console.error('[agent] answering an action call failed', error)
      })
    })
    await pushDefinitionsAsync()
    watch(locale, () => {
      void pushDefinitionsAsync().catch((error: unknown) => {
        console.error('[agent] updating the action definitions failed', error)
      })
    })
  }

  return { startAsync }
}
