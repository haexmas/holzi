import { registerShellActionHandlers } from '~/stores/shellActionHandlers'

/**
 * Registers the global action handlers once at startup (spec
 * 020-tab-navigation, research R19). Tab-bound handlers register themselves
 * from their mounted app via `useShellTab().registerActionHandler`.
 */
export default defineNuxtPlugin(() => {
  registerShellActionHandlers(useShellStore())
})
