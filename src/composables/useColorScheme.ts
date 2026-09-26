import { readonly, ref } from 'vue'
import { usePreferences } from '~/composables/usePreferences'
import {
  COLOR_SCHEME_KEY,
  isDark,
  parseColorScheme,
  type ColorScheme,
} from '~/lib/settings/colorScheme'

/** One state per process: a process holds one vault (spec 013). */
const scheme = ref<ColorScheme>('system')
let media: MediaQueryList | null = null

function apply() {
  const dark = isDark(scheme.value, media?.matches ?? false)
  const root = document.documentElement
  root.classList.toggle('dark', dark)
  // Native controls (radio buttons, scrollbars) follow `color-scheme`, not the class.
  root.style.colorScheme = dark ? 'dark' : 'light'
}

/** Follows the system's scheme; `apply` only changes something while `system` applies. */
function watchSystem() {
  if (media) return
  media = window.matchMedia('(prefers-color-scheme: dark)')
  media.addEventListener('change', apply)
}

/**
 * The color scheme of the app (spec 023-settings-app, FR-013, FR-014, FR-024, contracts §4):
 * light, dark or the system's, one value for the whole vault. It sets the `dark` class of the
 * haex-ui theme on `<html>`; all windows share one webview, so that is the whole app. Before the
 * vault is open there is nothing to read and the system's scheme applies.
 */
export function useColorScheme() {
  const { getPrefAsync, setPrefAsync } = usePreferences()

  function startSystem() {
    watchSystem()
    apply()
  }

  /** Reads the vault's value once it is open; a read error leaves the system's scheme. */
  async function loadAsync(): Promise<void> {
    watchSystem()
    const stored = await getPrefAsync({ kind: 'vault' }, COLOR_SCHEME_KEY)
    scheme.value = parseColorScheme(stored) ?? 'system'
    apply()
  }

  /** Writes the vault's value and applies it at once (SC-005). */
  async function setAsync(next: ColorScheme): Promise<ColorScheme> {
    await setPrefAsync({ kind: 'vault' }, COLOR_SCHEME_KEY, next)
    scheme.value = next
    watchSystem()
    apply()
    return next
  }

  return { scheme: readonly(scheme), startSystem, loadAsync, setAsync }
}
