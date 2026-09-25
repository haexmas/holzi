import { onBackButtonPress } from '@tauri-apps/api/app'
import { registerChatActionHandlers } from '~/stores/chatActionHandlers'
import { registerSettingsActionHandlers } from '~/stores/settingsActionHandlers'
import type { Translate } from '~/composables/useModelInventory'
import { registerShellActionHandlers } from '~/stores/shellActionHandlers'
import { registerShellLayoutHandlers } from '~/stores/shellLayoutHandlers'

/**
 * Registers the global action handlers once at startup (spec
 * 020-tab-navigation, research R19). Tab-bound handlers register themselves
 * from their mounted app via `useShellTab().registerActionHandler`.
 *
 * On Android, the platform back button/gesture triggers `shell.system.back`
 * (research R7) instead of the webview's own history, which holzi never uses
 * as navigation state (FR-035).
 */
export default defineNuxtPlugin((nuxtApp) => {
  const shell = useShellStore()
  const t = ((key, params) => nuxtApp.$i18n.t(key, params ?? {})) as Translate
  registerShellActionHandlers(shell)
  registerShellLayoutHandlers(shell, t)
  registerChatActionHandlers(shell)
  registerSettingsActionHandlers(shell)

  // ponytail: holzi has no Android target yet, so this hook is untested end to end; the decision
  // logic behind `shell.system.back` is covered by `pnpm check:shell-navigation`.
  if (/android/i.test(navigator.userAgent)) {
    onBackButtonPress(() => {
      void shell.runAction('shell.system.back')
    }).catch((error: unknown) => {
      console.error('[shell] onBackButtonPress unavailable', error)
    })
  }
})
