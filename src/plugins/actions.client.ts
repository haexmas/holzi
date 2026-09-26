import { onBackButtonPress } from '@tauri-apps/api/app'
import { registerChatActionHandlers } from '~/stores/chatActionHandlers'
import { registerSettingsActionHandlers } from '~/stores/settingsActionHandlers'
import type { Translate } from '~/composables/useModelInventory'
import { registerWmActionHandlers } from '~/stores/wmActionHandlers'
import { registerWmLayoutHandlers } from '~/stores/wmLayoutHandlers'

/**
 * Registers the global action handlers once at startup (spec
 * 020-tab-navigation, research R19). Tab-bound handlers register themselves
 * from their mounted app via `useWmTab().registerActionHandler`.
 *
 * On Android, the platform back button/gesture triggers `wm.system.back`
 * (research R7) instead of the webview's own history, which holzi never uses
 * as navigation state (FR-035).
 */
export default defineNuxtPlugin((nuxtApp) => {
  const wm = useWindowManagerStore()
  const t = ((key, params) => nuxtApp.$i18n.t(key, params ?? {})) as Translate
  registerWmActionHandlers(wm)
  registerWmLayoutHandlers(wm, t)
  registerChatActionHandlers(wm)
  registerSettingsActionHandlers(wm)

  // ponytail: holzi has no Android target yet, so this hook is untested end to end; the decision
  // logic behind `wm.system.back` is covered by `pnpm check:wm-navigation`.
  if (/android/i.test(navigator.userAgent)) {
    onBackButtonPress(() => {
      void wm.runAction('wm.system.back')
    }).catch((error: unknown) => {
      console.error('[wm] onBackButtonPress unavailable', error)
    })
  }
})
