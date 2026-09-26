import { readonly, ref } from 'vue'
import { useDevice } from '~/composables/useDevice'
import { usePreferences, type PrefScope } from '~/composables/usePreferences'
import {
  COLOR_SCHEME_KEY,
  colorSchemeState,
  isDark,
  parseColorScheme,
  type ColorScheme,
  type ColorSchemeState,
} from '~/lib/settings/colorScheme'

/** One state per process: a process holds one vault (spec 013). */
const state = ref<ColorSchemeState>(colorSchemeState(null, null))
let media: MediaQueryList | null = null

function apply() {
  document.documentElement.classList.toggle(
    'dark',
    isDark(state.value.effective, media?.matches ?? false),
  )
}

/** Follows the system's scheme; `apply` only changes something while `system` is effective. */
function watchSystem() {
  if (media) return
  media = window.matchMedia('(prefers-color-scheme: dark)')
  media.addEventListener('change', apply)
}

/**
 * The color scheme of the app (spec 023-settings-app, FR-013, FR-014, contracts §4): light, dark
 * or the system's, this device's value before the vault's. It sets the `dark` class of the
 * haex-ui theme on `<html>`; all windows share one webview, so that is the whole app. Before the
 * vault is open there is nothing to read and the system's scheme applies.
 */
export function useColorScheme() {
  const { getPrefAsync, setPrefAsync, clearPrefAsync } = usePreferences()
  const { currentDeviceInfoAsync } = useDevice()

  function startSystem() {
    watchSystem()
    apply()
  }

  async function deviceScope(): Promise<PrefScope> {
    const device = await currentDeviceInfoAsync()
    return { kind: 'device', uuid: device.vaultDeviceUuid }
  }

  /** Reads both values once the vault is open; a read error leaves the system's scheme. */
  async function loadAsync(): Promise<void> {
    watchSystem()
    const [device, vault] = await Promise.all([
      deviceScope().then((scope) => getPrefAsync(scope, COLOR_SCHEME_KEY)),
      getPrefAsync({ kind: 'vault' }, COLOR_SCHEME_KEY),
    ])
    state.value = colorSchemeState(
      parseColorScheme(device),
      parseColorScheme(vault),
    )
    apply()
  }

  /** Writes (or with `null` clears) one scope's value and applies the result at once (SC-005). */
  async function setAsync(
    scope: 'device' | 'vault',
    scheme: ColorScheme | null,
  ): Promise<ColorSchemeState> {
    const prefScope: PrefScope =
      scope === 'vault' ? { kind: 'vault' } : await deviceScope()
    if (scheme) await setPrefAsync(prefScope, COLOR_SCHEME_KEY, scheme)
    else await clearPrefAsync(prefScope, COLOR_SCHEME_KEY)
    const { device, vault } = state.value
    state.value =
      scope === 'vault'
        ? colorSchemeState(device, scheme)
        : colorSchemeState(scheme, vault)
    watchSystem()
    apply()
    return state.value
  }

  return { state: readonly(state), startSystem, loadAsync, setAsync }
}
