import { ref, type Ref } from 'vue'
import { isAutonomyMode } from '~/composables/usePreferences'
import type { usePreferences } from '~/composables/usePreferences'

const PERMISSION_MODE_KEY = 'chat.permission_mode'
const AUTONOMY_MODE_KEY = 'chat.autonomy_mode'

/**
 * Permission-mode (manual/auto/plan) and autonomy-mode preference state (both
 * vault-wide since spec 023, FR-024) — extracted from `src/pages/chat/[instance].vue` (spec
 * 015-workspace-shell, T013) to bring the orchestrator page under the
 * 500-line constitution limit. Autonomy mode is configured
 * on the Settings page only (see the page's own prior comment: a second,
 * delegate-only permission-style menu next to the composer's manual/
 * auto/plan control was confusing) — starts fail-closed until the
 * preference read confirms either the stored value or the unset default
 * ('ungated').
 */
export function useChatPermissionMode(
  getPrefAsync: ReturnType<typeof usePreferences>['getPrefAsync'],
  setPrefAsync: ReturnType<typeof usePreferences>['setPrefAsync'],
  errString: (e: unknown) => string,
  lastError: Ref<string | null>,
) {
  const permissionMode = ref<'manual' | 'auto' | 'plan'>('manual')
  const autonomyMode = ref<'standard' | 'ungated' | 'gated_permissive'>(
    'standard',
  )
  const autonomyPreferenceLoading = ref(true)
  const autonomyPreferenceError = ref<string | null>(null)
  const deviceUuid = ref('')
  const permissionModeSaving = ref(false)

  async function updatePermissionMode(mode: 'manual' | 'auto' | 'plan') {
    if (!deviceUuid.value || permissionModeSaving.value) return
    const previousMode = permissionMode.value
    permissionMode.value = mode
    permissionModeSaving.value = true
    try {
      await setPrefAsync({ kind: 'vault' }, PERMISSION_MODE_KEY, mode)
    } catch (e: unknown) {
      permissionMode.value = previousMode
      lastError.value = errString(e)
    } finally {
      permissionModeSaving.value = false
    }
  }

  async function reloadAutonomyMode() {
    autonomyPreferenceLoading.value = true
    autonomyPreferenceError.value = null
    try {
      const stored = await getPrefAsync({ kind: 'vault' }, AUTONOMY_MODE_KEY)
      autonomyMode.value = isAutonomyMode(stored) ? stored : 'ungated'
    } catch (e: unknown) {
      autonomyMode.value = 'standard'
      autonomyPreferenceError.value = errString(e)
    } finally {
      autonomyPreferenceLoading.value = false
    }
  }

  /** Reads the persisted permission mode and autonomy mode for `vaultDeviceUuid`, called once from
   * the page's `onMounted`. */
  async function initialize(vaultDeviceUuid: string) {
    const [permissionResult] = await Promise.allSettled([
      getPrefAsync({ kind: 'vault' }, PERMISSION_MODE_KEY),
    ])
    if (permissionResult.status === 'fulfilled') {
      const stored = permissionResult.value
      if (stored === 'manual' || stored === 'auto' || stored === 'plan') {
        permissionMode.value = stored
      }
    }
    deviceUuid.value = vaultDeviceUuid
    await reloadAutonomyMode()
  }

  return {
    permissionMode,
    autonomyMode,
    autonomyPreferenceLoading,
    autonomyPreferenceError,
    deviceUuid,
    permissionModeSaving,
    updatePermissionMode,
    reloadAutonomyMode,
    initialize,
  }
}
