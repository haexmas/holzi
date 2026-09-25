import { getAppDefinition } from '~/lib/shell/apps'
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
  shell.registerGlobalActionHandler('shell.system.back', () => ({
    outcome: shell.systemBack(),
  }))

  /** Unknown app ids fail loudly: `openApp` itself silently ignores them. */
  function knownApp(appId: unknown): string {
    const id = String(appId)
    if (!getAppDefinition(id)) throw new Error(`unknown app ${id}`)
    return id
  }
  function at(input: Record<string, unknown>): string | null {
    return typeof input.at === 'string' ? input.at : null
  }

  shell.registerGlobalActionHandler('shell.app.open', ({ input }) => {
    const appId = knownApp(input.appId)
    const before = new Set(
      shell.windows.flatMap((w) => w.tabs.map((t) => t.id)),
    )
    const tabId = shell.openApp(appId, at(input))
    return { tabId, created: tabId !== null && !before.has(tabId) }
  })
  shell.registerGlobalActionHandler('shell.tab.new', ({ input, target }) => {
    const appId = knownApp(input.appId)
    const before = new Set(
      shell.windows.flatMap((w) => w.tabs.map((t) => t.id)),
    )
    const tabId = shell.addTab(target.windowId ?? '', appId, at(input))
    return { tabId, created: tabId !== null && !before.has(tabId) }
  })
}
