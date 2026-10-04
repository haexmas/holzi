import { onScopeDispose, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useColorScheme } from '~/composables/useColorScheme'
import { useExtensionPermissionsStore } from '~/stores/extensionPermissions'
import { useExtensionsStore } from '~/stores/extensions'
import { useWindowManagerStore } from '~/stores/windowManager'

/**
 * The extension host on the workspace page (spec 017): the extension list for the app list of the
 * window manager, and the color scheme and language holzi applies, which extensions read through
 * `extension_context_get` (contracts/tauri-commands.md `extension_host_context_set`).
 */
export function useExtensionHost() {
  const extensions = useExtensionsStore()
  const permissions = useExtensionPermissionsStore()
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

  // A click on a notification of an extension brings its tab forward (spec 017 US8, FR-052); the
  // extension itself hears the click through its frame.
  let unlistenClick: UnlistenFn | null = null
  onScopeDispose(() => unlistenClick?.())

  /** Loads the extension list; the session restore waits for it, so extension tabs survive. */
  async function startAsync(): Promise<void> {
    if (!unlistenClick) {
      const wm = useWindowManagerStore()
      unlistenClick = await listen<{ extensionId: string }>(
        'extension-notification-click',
        (event) => wm.showExtension(event.payload.extensionId),
      )
    }
    await Promise.all([extensions.startAsync(), permissions.startAsync()])
  }

  return { startAsync }
}
