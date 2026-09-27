import { getAppDefinition, resolveAppAlias } from '~/lib/wm/apps'
import type { useWindowManagerStore } from '~/stores/windowManager'

type WmStore = ReturnType<typeof useWindowManagerStore>

/**
 * Global handlers of the window manager actions (spec 020-tab-navigation,
 * contracts/wm-actions.md §1), registered once at startup by
 * `plugins/actions.client.ts`. The runner has already validated the input
 * and resolved the target, so handlers only perform the change and describe
 * the result.
 */
export function registerWmActionHandlers(wm: WmStore): void {
  wm.registerGlobalActionHandler('wm.tab.back', ({ target }) => ({
    moved: wm.goTab(target.tabId ?? '', -1),
  }))
  wm.registerGlobalActionHandler('wm.tab.forward', ({ target }) => ({
    moved: wm.goTab(target.tabId ?? '', 1),
  }))
  wm.registerGlobalActionHandler('wm.tab.go', ({ target, input }) => ({
    moved: wm.goTab(target.tabId ?? '', Number(input.steps)),
  }))
  wm.registerGlobalActionHandler('wm.tab.navigate', ({ target, input }) => ({
    moved: wm.navigate(target.tabId ?? '', String(input.to), {
      replace: input.replace === true,
    }),
  }))
  wm.registerGlobalActionHandler('wm.system.back', () => ({
    outcome: wm.systemBack(),
  }))

  /** The app and start location to open: a removed app resolves to its replacement first
   * (spec 023 research R11), an `at` from the input wins over the alias's. Unknown app ids fail
   * loudly: `openApp` itself silently ignores them. */
  function target(input: Record<string, unknown>): {
    appId: string
    at: string | null
  } {
    const alias = resolveAppAlias(String(input.appId))
    if (!getAppDefinition(alias.appId)) {
      throw new Error(`unknown app ${alias.appId}`)
    }
    return {
      appId: alias.appId,
      at: typeof input.at === 'string' ? input.at : alias.at,
    }
  }

  wm.registerGlobalActionHandler('wm.app.open', ({ input }) => {
    const { appId, at } = target(input)
    const before = new Set(wm.windows.flatMap((w) => w.tabs.map((t) => t.id)))
    const tabId = wm.openApp(appId, at)
    return { tabId, created: tabId !== null && !before.has(tabId) }
  })
  wm.registerGlobalActionHandler('wm.tab.new', ({ input, target: where }) => {
    const { appId, at } = target(input)
    const before = new Set(wm.windows.flatMap((w) => w.tabs.map((t) => t.id)))
    const tabId = wm.addTab(where.windowId ?? '', appId, at)
    return { tabId, created: tabId !== null && !before.has(tabId) }
  })
}
