import type { ActionOutcome } from '~/lib/actions/types'

/**
 * The caller-`user` entry point for Vue components (spec 020-tab-navigation,
 * T017, contracts/wm-actions.md §2): every state-changing control triggers
 * its catalog action through this instead of calling the store directly
 * (FR-024), so the UI, shortcuts and — from spec 021 on — agents share one
 * code path. A failed outcome is logged in development; the controls that
 * need to react to it read the returned outcome.
 */
export function useAction(id: string) {
  const wm = useWindowManagerStore()
  return async (
    input: Record<string, unknown> = {},
  ): Promise<ActionOutcome> => {
    const outcome = await wm.runAction(id, input, { kind: 'user' })
    if (!outcome.ok && import.meta.dev)
      console.warn(`[actions] ${id} failed: ${outcome.code} ${outcome.message}`)
    return outcome
  }
}
