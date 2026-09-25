import type { useShellStore } from '~/stores/shell'

type ShellStore = ReturnType<typeof useShellStore>

/**
 * Global handlers of the Shell actions (spec 020-tab-navigation,
 * contracts/shell-actions.md §1), registered once at startup by
 * `plugins/actions.client.ts`. The runner has already validated the input
 * and resolved the target, so handlers only perform the change and describe
 * the result.
 */
export function registerShellActionHandlers(shell: ShellStore): void {
  shell.registerGlobalActionHandler('shell.tab.back', ({ target }) => ({
    moved: shell.goTab(target.tabId ?? '', -1),
  }))
  shell.registerGlobalActionHandler('shell.tab.forward', ({ target }) => ({
    moved: shell.goTab(target.tabId ?? '', 1),
  }))
  shell.registerGlobalActionHandler('shell.tab.go', ({ target, input }) => ({
    moved: shell.goTab(target.tabId ?? '', Number(input.steps)),
  }))
  shell.registerGlobalActionHandler(
    'shell.tab.navigate',
    ({ target, input }) => ({
      moved: shell.navigate(target.tabId ?? '', String(input.to), {
        replace: input.replace === true,
      }),
    }),
  )
}
