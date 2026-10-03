import { watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useColorScheme } from '~/composables/useColorScheme'
import { useExtensionsStore } from '~/stores/extensions'

/**
 * The extension host on the workspace page (spec 017): the extension list for the app list of the
 * window manager, and the color scheme and language holzi applies, which extensions read through
 * `extension_context_get` (contracts/tauri-commands.md `extension_host_context_set`).
 */
export function useExtensionHost() {
  const extensions = useExtensionsStore()
  const { scheme } = useColorScheme()
  const { locale } = useI18n()

  watch(
    [scheme, locale],
    ([theme, language]) => {
      void invoke('extension_host_context_set', {
        theme,
        locale: language,
      }).catch((error: unknown) => {
        console.error('[extensions] reporting the context failed', error)
      })
    },
    { immediate: true },
  )

  /** Loads the extension list; the session restore waits for it, so extension tabs survive. */
  async function startAsync(): Promise<void> {
    await extensions.startAsync()
  }

  return { startAsync }
}
