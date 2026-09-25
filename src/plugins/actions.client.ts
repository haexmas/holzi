import { onBackButtonPress } from '@tauri-apps/api/app'
import { registerShellActionHandlers } from '~/stores/shellActionHandlers'

/**
 * Registers the global action handlers once at startup (spec
 * 020-tab-navigation, research R19). Tab-bound handlers register themselves
 * from their mounted app via `useShellTab().registerActionHandler`.
 *
 * On Android, the platform back button/gesture triggers `shell.system.back`
 * (research R7) instead of the webview's own history, which holzi never uses
 * as navigation state (FR-035).
 */
export default defineNuxtPlugin(() => {
  const shell = useShellStore()
  registerShellActionHandlers(shell)

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
