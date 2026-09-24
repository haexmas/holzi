import { ref } from 'vue'
import type { CloseGuardResult } from '~/lib/shell/types'

type PendingConfirmation = {
  results: CloseGuardResult[]
  resolve: (confirmed: boolean) => void
}

/** Module-level singleton: only one confirmation is ever pending at a time — closing is always a
 * single focused action, never several in flight together (spec 015-workspace-shell, T038). */
const pending = ref<PendingConfirmation | null>(null)

/** Requests confirmation for one or more close-guard results (FR-014); resolves once the user
 * decides. `ShellCloseConfirm.vue` (mounted once, in `ShellDesktop.vue`) renders whatever this
 * sets and calls `resolvePending`. */
export function requestConfirmation(
  results: CloseGuardResult[],
): Promise<boolean> {
  return new Promise((resolve) => {
    pending.value = { results, resolve }
  })
}

export function usePendingCloseConfirmation() {
  return pending
}

export function resolvePending(confirmed: boolean) {
  pending.value?.resolve(confirmed)
  pending.value = null
}
