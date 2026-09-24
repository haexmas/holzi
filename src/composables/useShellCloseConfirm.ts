import { ref } from 'vue'
import type { CloseGuardResult } from '~/lib/shell/types'

/** A `CloseGuardResult` (its `reasonKey`) is a valid `ConfirmationReason` — this is the broader
 * shape so workspace deletion (T040) can add its own unconditional "N windows will close" reason,
 * which has no guard/`confirmAsync` of its own, alongside any real guard results. */
export type ConfirmationReason = {
  reasonKey: string
  params?: Record<string, unknown>
}

type PendingConfirmation = {
  reasons: ConfirmationReason[]
  resolve: (confirmed: boolean) => void
}

/** Module-level singleton: only one confirmation is ever pending at a time — closing is always a
 * single focused action, never several in flight together (spec 015-workspace-shell, T038). */
const pending = ref<PendingConfirmation | null>(null)

/** Requests confirmation for one or more reasons (FR-014/FR-021); resolves once the user decides.
 * `ShellCloseConfirm.vue` (mounted once, in `ShellDesktop.vue`) renders whatever this sets and
 * calls `resolvePending`. `CloseGuardResult[]` (its `confirmAsync` is simply not read here) is a
 * valid argument. */
export function requestConfirmation(
  reasons: ConfirmationReason[] | CloseGuardResult[],
): Promise<boolean> {
  return new Promise((resolve) => {
    pending.value = { reasons, resolve }
  })
}

export function usePendingCloseConfirmation() {
  return pending
}

export function resolvePending(confirmed: boolean) {
  pending.value?.resolve(confirmed)
  pending.value = null
}
