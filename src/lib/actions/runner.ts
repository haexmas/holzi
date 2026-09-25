// The `runAction` pipeline (spec 020-tab-navigation, T013, data-model.md "Ablauf runAction"):
// look up → guardrail lock for agents → validate input → resolve target → wait for a tab-bound
// handler if needed → run. Pure: the store injects focus, target lookup and handler registries.
import { validate } from './schema.ts'
import type {
  ActionCaller,
  ActionOutcome,
  ActionTarget,
  ActionTargetIds,
  ShellActionDefinition,
} from './types.ts'

export type ActionHandlerContext = {
  input: Record<string, unknown>
  caller: ActionCaller
  target: ActionTargetIds
}

export type ActionHandler = (context: ActionHandlerContext) => unknown

export type ActionRunnerDeps = {
  catalog: readonly ShellActionDefinition[]
  globalHandler: (actionId: string) => ActionHandler | undefined
  /** The id the UI focus implies for this target kind, or `null` if there is none. */
  resolveFocus: (target: Exclude<ActionTarget, 'none'>) => string | null
  targetExists: (target: Exclude<ActionTarget, 'none'>, id: string) => boolean
  /** Opens/activates `appId` and resolves its tab-bound handler, or `null` after the timeout. */
  awaitTabHandler: (
    appId: string,
    actionId: string,
    timeoutMs: number,
  ) => Promise<ActionHandler | null>
}

export const TAB_HANDLER_TIMEOUT_MS = 5000

const TARGET_FIELD = {
  tab: 'tabId',
  window: 'windowId',
  workspace: 'workspaceId',
} as const

function failure(
  code: Exclude<ActionOutcome, { ok: true }>['code'],
  message: string,
  field?: string,
): ActionOutcome {
  return field === undefined
    ? { ok: false, code, message }
    : { ok: false, code, message, field }
}

export function createActionRunner(deps: ActionRunnerDeps) {
  async function runAction(
    id: string,
    input: Record<string, unknown> = {},
    caller: ActionCaller = { kind: 'user' },
  ): Promise<ActionOutcome> {
    const action = deps.catalog.find((a) => a.id === id)
    if (!action) return failure('unknown_action', `unknown action ${id}`)

    if (caller.kind !== 'user' && !action.agentCallable)
      return failure(
        'forbidden_for_agents',
        `${id} can only be triggered by the user`,
      )

    const validation = validate(action.input, input)
    if (!validation.ok)
      return failure('invalid_input', validation.message, validation.field)

    const target: ActionTargetIds = {}
    if (action.target !== 'none') {
      const field = TARGET_FIELD[action.target]
      const explicit = input[field]
      if (typeof explicit === 'string') {
        if (!deps.targetExists(action.target, explicit))
          return failure(
            'target_not_found',
            `no ${action.target} ${explicit}`,
            field,
          )
        target[field] = explicit
      } else if (caller.kind === 'user') {
        const focused = deps.resolveFocus(action.target)
        if (focused === null)
          return failure(
            'target_required',
            `no focused ${action.target}`,
            field,
          )
        target[field] = focused
      } else {
        return failure(
          'target_required',
          `agents must name the ${action.target}`,
          field,
        )
      }
    }

    let handler: ActionHandler | null | undefined
    if (action.binding === 'tab') {
      handler = await deps.awaitTabHandler(
        action.appId ?? '',
        id,
        TAB_HANDLER_TIMEOUT_MS,
      )
      if (!handler)
        return failure(
          'app_unavailable',
          `${action.appId ?? 'app'} did not become ready`,
        )
    } else {
      handler = deps.globalHandler(id)
      if (!handler) return failure('failed', `no handler registered for ${id}`)
    }

    try {
      const result = await handler({ input, caller, target })
      return { ok: true, result: result ?? null }
    } catch (error) {
      return failure(
        'failed',
        error instanceof Error ? error.message : String(error),
      )
    }
  }

  return { runAction }
}
