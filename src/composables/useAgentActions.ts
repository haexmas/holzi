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

let listener: Promise<void> | null = null
let startup: Promise<void> | null = null
let localeWatchStarted = false

/**
 * The built-in agent's access to the actions (spec 032, ADR-0006). Pushes the definitions the
 * built-in agent may call to Rust as tools (`set_agent_actions`) and runs the calls Rust sends back
 * (`action-call-request`) through the same runner a click uses, with caller `builtinAgent`.
 *
 * Call `startAsync()` once per vault session, from the workspace page: both Tauri commands need
 * an open vault, and the listener stays for the session like the sync listener. Definitions are
 * pushed again when the language changes, because they carry localized titles for the search.
 */
export function useAgentActions() {
  const wm = useWindowManagerStore()
  const { t, locale, loadLocaleMessages } = useI18n()

  /** Translates an action title in the requested catalog language. */
  const titleOf = (key: string, language: ActionLocale) =>
    t(key, {}, { locale: language })

  /** Replaces Rust's action tools with the eligible catalog and its localized titles. */
  async function pushDefinitionsAsync(): Promise<void> {
    // Only the active language is loaded lazily; the titles need both catalogs.
    await Promise.all([loadLocaleMessages('de'), loadLocaleMessages('en')])
    await invoke('set_agent_actions', {
      args: { actions: listAgentActions(ALL_ACTIONS, titleOf) },
    })
  }

  /** Runs a request as the built-in agent and sends its outcome without raw errors to Rust. */
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

  /** Installs the listener once, publishes the catalog, and refreshes it on language changes. */
  function startAsync(): Promise<void> {
    if (startup) return startup

    const pending = (async () => {
      listener ??= listen<ActionCallRequest>('action-call-request', (event) => {
        void runCallAsync(event.payload).catch((error: unknown) => {
          console.error('[agent] answering an action call failed', error)
        })
      }).then(
        () => {},
        (error: unknown) => {
          listener = null
          throw error
        },
      )
      await listener
      await pushDefinitionsAsync()
      if (!localeWatchStarted) {
        localeWatchStarted = true
        watch(locale, () => {
          void pushDefinitionsAsync().catch((error: unknown) => {
            console.error(
              '[agent] updating the action definitions failed',
              error,
            )
          })
        })
      }
    })()

    startup = pending
    void pending.catch(() => {
      if (startup === pending) startup = null
    })
    return pending
  }

  return { startAsync }
}
