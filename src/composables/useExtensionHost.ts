import { onScopeDispose, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { toast } from 'vue-sonner'
import type { DevModeState } from '@bindings/DevModeState'
import { useColorScheme } from '~/composables/useColorScheme'
import { useExtensionPermissionsStore } from '~/stores/extensionPermissions'
import { useExtensionsStore } from '~/stores/extensions'
import { useWindowManagerStore } from '~/stores/windowManager'

/**
 * The extension host on the workspace page (spec 017): the extension list for the app list of the
 * window manager, and the color scheme and language holzi applies, which extensions read through
 * `extension_context_get` (contracts/tauri-commands.md `extension_host_context_set`).
 */
/** Set before the one reload for developer mode, so a document that still lacks the development
 * origins (in `tauri dev` holzi's hook does not run) does not reload again and again. */
const DEV_RELOAD_KEY = 'holzi.extensions.devReload'

/** Developer mode is on, but this document was loaded before holzi knew it (before unlocking): its
 * policy cannot frame development servers yet, so the window reloads once (spec 017, US12,
 * research R16). Returns whether it reloads. */
async function reloadForDevModeAsync(): Promise<boolean> {
  let state: DevModeState
  try {
    state = await invoke<DevModeState>('extension_dev_mode_get')
  } catch {
    return false
  }
  try {
    if (!state.enabled || state.framesAllowed) {
      sessionStorage.removeItem(DEV_RELOAD_KEY)
      return false
    }
    if (sessionStorage.getItem(DEV_RELOAD_KEY) !== null) return false
    sessionStorage.setItem(DEV_RELOAD_KEY, '1')
  } catch {
    // Without session storage no loop guard: no reload.
    return false
  }
  window.location.reload()
  return true
}

/** Brings this native window to the front. Mobile has no such window calls; their failure leaves
 * the window as it is. */
async function raiseWindowAsync(): Promise<void> {
  const window = getCurrentWindow()
  for (const step of [
    () => window.unminimize(),
    () => window.show(),
    () => window.setFocus(),
  ]) {
    try {
      await step()
    } catch {
      // Not offered on this platform.
    }
  }
}

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

  // A click on a notification of an extension brings its tab forward (spec 017 US8, FR-052), and
  // the native window that now shows the tab comes to the front, no other. The extension itself
  // hears the click through its frame. Registered once per scope; a listener that arrives after the
  // scope ended is dropped at once, and a failure does not stop the start.
  const wm = useWindowManagerStore()
  const { t } = useI18n()
  let disposed = false
  const unlisteners: UnlistenFn[] = []
  onScopeDispose(() => {
    disposed = true
    for (const unlisten of unlisteners) unlisten()
  })
  const keep = (listening: Promise<UnlistenFn>, what: string) => {
    listening
      .then((unlisten) => {
        if (disposed) unlisten()
        else unlisteners.push(unlisten)
      })
      .catch((error: unknown) => {
        console.error(`[extensions] listening to ${what} failed`, error)
      })
  }
  const showExtension = (extensionId: string) => {
    if (wm.showExtension(extensionId)) void raiseWindowAsync()
  }
  keep(
    listen<{ extensionId: string }>('extension-notification-click', (event) =>
      showExtension(event.payload.extensionId),
    ),
    'notification clicks',
  )
  // Where the system shows no notification of an extension (Android: the person refused the
  // permission, spec 043 FR-023), holzi shows it as a message; "open" brings the extension's tab.
  keep(
    listen<{ extensionId: string; title: string; body: string | null }>(
      'extension-notification-in-app',
      (event) => {
        const { extensionId, title, body } = event.payload
        toast(title, {
          description: body ?? undefined,
          action: {
            label: t('extensions.notification.open'),
            onClick: () => showExtension(extensionId),
          },
        })
      },
    ),
    'notifications shown in holzi',
  )

  /** Loads the extension list; the session restore waits for it, so extension tabs survive. */
  async function startAsync(): Promise<void> {
    if (await reloadForDevModeAsync()) return
    await Promise.all([extensions.startAsync(), permissions.startAsync()])
  }

  return { startAsync }
}
