import { onBackButtonPress } from '@tauri-apps/api/app'
import { registerChatActionHandlers } from '~/stores/chatActionHandlers'
import { registerFilesActionHandlers } from '~/stores/filesActionHandlers'
import { registerPasswordsActionHandlers } from '~/stores/passwordsActionHandlers'
import { registerSettingsActionHandlers } from '~/stores/settingsActionHandlers'
import type { Translate } from '~/composables/useModelInventory'
import { registerWmActionHandlers } from '~/stores/wmActionHandlers'
import { registerWmLayoutHandlers } from '~/stores/wmLayoutHandlers'

/**
 * Registers the global action handlers once at startup (spec
 * 020-tab-navigation, research R19). Tab-bound handlers register themselves
 * from their mounted app via `useWmTab().registerActionHandler`.
 *
 * Where the system has a back gesture (Android, spec 043 FR-013), it triggers
 * `wm.system.back` (research R7) instead of the webview's own history, which
 * holzi never uses as navigation state (FR-035). The device's capability table
 * decides, not the user agent.
 */
export default defineNuxtPlugin((nuxtApp) => {
  const wm = useWindowManagerStore()
  const t = ((key, params) => nuxtApp.$i18n.t(key, params ?? {})) as Translate
  registerWmActionHandlers(wm, t)
  registerWmLayoutHandlers(wm, t)
  registerChatActionHandlers(wm)
  registerSettingsActionHandlers(wm)
  registerPasswordsActionHandlers(wm)
  registerFilesActionHandlers(wm)

  // The decision logic behind `wm.system.back` is covered by `pnpm check:wm-navigation`, the
  // gesture itself by the e2e scenario `android-back-gesture`.
  void useDeviceCapabilities()
    .readyAsync()
    .then(async (table) => {
      if (!table?.backGesture) return
      await onBackButtonPress(() => {
        void wm.runAction('wm.system.back')
      })
    })
    .catch((error: unknown) => {
      console.error('[wm] onBackButtonPress unavailable', error)
    })
})
