import { getAppDefinition } from '~/lib/wm/apps'
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

  /** Unknown app ids fail loudly: `openApp` itself silently ignores them. */
  function knownApp(appId: unknown): string {
    const id = String(appId)
    if (!getAppDefinition(id)) throw new Error(`unknown app ${id}`)
    return id
  }
  function at(input: Record<string, unknown>): string | null {
    return typeof input.at === 'string' ? input.at : null
  }

  wm.registerGlobalActionHandler('wm.app.open', ({ input }) => {
    const appId = knownApp(input.appId)
    const before = new Set(wm.windows.flatMap((w) => w.tabs.map((t) => t.id)))
    const tabId = wm.openApp(appId, at(input))
    return { tabId, created: tabId !== null && !before.has(tabId) }
  })
  wm.registerGlobalActionHandler('wm.tab.new', ({ input, target }) => {
    const appId = knownApp(input.appId)
    const before = new Set(wm.windows.flatMap((w) => w.tabs.map((t) => t.id)))
    const tabId = wm.addTab(target.windowId ?? '', appId, at(input))
    return { tabId, created: tabId !== null && !before.has(tabId) }
  })
}
