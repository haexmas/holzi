import { getAppRoutes, type AppRouteRecord } from '~/components/wm/appRoutes'
import {
  requestCloseTab,
  requestCloseWindow,
  requestDeleteWorkspace,
} from '~/composables/useWmTab'
import { ALL_ACTIONS } from '~/lib/actions/catalog'
import { getAppDefinition, WM_APPS } from '~/lib/wm/apps'
import { clampGeometry } from '~/lib/wm/geometry'
import { formatLocation, currentLocation } from '~/lib/wm/navigation'
import type { WmTab, WmWindow } from '~/lib/wm/types'
import type { Translate } from '~/composables/useModelInventory'
import type { useWindowManagerStore } from '~/stores/windowManager'

type WmStore = ReturnType<typeof useWindowManagerStore>

/** A declined user confirmation fails the action (contracts/wm-actions.md §1). */
function confirmed(ok: boolean): { done: true } {
  if (!ok) throw new Error('declined by user')
  return { done: true }
}

function flattenRoutes(
  records: readonly AppRouteRecord[],
  base = '',
): string[] {
  return records.flatMap((record) => {
    const path = [base, record.path].filter((p) => p && p !== '/').join('/')
    const full = `/${path}`.replace(/\/+/g, '/')
    return [full, ...flattenRoutes(record.children ?? [], path)]
  })
}

/**
 * Global handlers of the window manager layout and read actions (spec
 * 020-tab-navigation, T047, `lib/actions/wmLayoutActions.ts`). Closing and
 * deleting go through the same confirmation helpers as the UI, so an agent
 * never closes a running reply without the user's consent.
 */
export function registerWmLayoutHandlers(wm: WmStore, t: Translate): void {
  const on = wm.registerGlobalActionHandler
  const windowOfTab = (tabId: string) =>
    wm.windows.find((w) => w.tabs.some((tab) => tab.id === tabId))

  on('wm.tab.activate', ({ target }) => {
    const window = windowOfTab(target.tabId ?? '')
    if (!window) throw new Error('tab not found')
    wm.switchTab(window.id, target.tabId ?? '')
    wm.focusWindow(window.id)
    return { done: true }
  })
  on('wm.tab.close', async ({ target }) => {
    const window = windowOfTab(target.tabId ?? '')
    if (!window) throw new Error('tab not found')
    return confirmed(await requestCloseTab(wm, window.id, target.tabId ?? ''))
  })
  on('wm.window.focus', ({ target }) => {
    wm.focusWindow(target.windowId ?? '')
    return { done: true }
  })
  on('wm.window.minimize', ({ target }) => {
    wm.minimizeWindow(target.windowId ?? '')
    return { done: true }
  })
  on('wm.window.toggleMaximize', ({ target }) => {
    wm.toggleMaximizeWindow(target.windowId ?? '')
    return { done: true }
  })
  on('wm.window.setGeometry', ({ target, input }) => {
    const window = wm.windows.find((w) => w.id === target.windowId)
    if (!window) throw new Error('window not found')
    const minSize = window.tabs.reduce(
      (min, tab) => {
        const app = getAppDefinition(tab.appId)
        return {
          width: Math.max(min.width, app?.minSize.width ?? 0),
          height: Math.max(min.height, app?.minSize.height ?? 0),
        }
      },
      { width: 0, height: 0 },
    )
    const rect = {
      x: Number(input.x),
      y: Number(input.y),
      width: Number(input.width),
      height: Number(input.height),
    }
    wm.updateWindowGeometry(window.id, clampGeometry(rect, minSize, wm.area))
    return { done: true }
  })
  on('wm.window.close', async ({ target }) =>
    confirmed(await requestCloseWindow(wm, target.windowId ?? '')),
  )
  on('wm.window.moveToWorkspace', ({ target, input }) => {
    const to = String(input.toWorkspaceId)
    if (!wm.workspaces.some((w) => w.id === to))
      throw new Error(`no workspace ${to}`)
    wm.moveWindowToWorkspace(target.windowId ?? '', to)
    return { done: true }
  })
  on('wm.workspace.create', async () => {
    const workspace = await wm.createWorkspace()
    wm.switchWorkspace(workspace.id)
    return { workspaceId: workspace.id }
  })
  on('wm.workspace.switch', ({ target }) => {
    wm.switchWorkspace(target.workspaceId ?? '')
    return { done: true }
  })
  on('wm.workspace.delete', async ({ target }) => {
    if (wm.workspaces.length <= 1)
      throw new Error('the last workspace cannot be deleted')
    return confirmed(await requestDeleteWorkspace(wm, target.workspaceId ?? ''))
  })
  on('wm.windows.overview', () => {
    wm.overlays.windows = true
    return { done: true }
  })
  on('wm.workspaces.overview', () => {
    wm.overlays.workspaces = true
    return { done: true }
  })
  on('wm.launcher.open', () => {
    wm.overlays.launcher = true
    return { done: true }
  })

  function tabTitle(tab: WmTab): string {
    const info = wm.tabDisplayInfo(tab)
    return (
      info.titleOverride ??
      (info.titleKey ? t(info.titleKey, info.titleParams) : '')
    )
  }
  function describeWindow(window: WmWindow) {
    return {
      windowId: window.id,
      workspaceId: window.workspaceId,
      x: window.x,
      y: window.y,
      width: window.width,
      height: window.height,
      minimized: window.minimized,
      maximized: window.maximized,
      activeTabId: window.activeTabId,
      tabs: window.tabs.map((tab) => {
        const history = wm.historyOf(tab.id)
        return {
          tabId: tab.id,
          appId: tab.appId,
          location: history ? formatLocation(currentLocation(history)) : '/',
          title: tabTitle(tab),
          attention: wm.tabDisplayInfo(tab).hasAttention,
        }
      }),
    }
  }

  on('wm.state.get', () => ({
    activeWorkspaceId: wm.activeWorkspaceId,
    activeWindowId: wm.activeWindowId,
    compact: wm.compact,
    workspaces: wm.workspaces.map((workspace) => ({
      workspaceId: workspace.id,
      title: t('wm.workspaces.numbered', { number: workspace.position + 1 }),
      windowCount: wm.windows.filter((w) => w.workspaceId === workspace.id)
        .length,
    })),
    windows: wm.windows.map(describeWindow),
  }))
  on('wm.tab.history', ({ target }) => {
    const history = wm.historyOf(target.tabId ?? '')
    if (!history) throw new Error('tab not found')
    return {
      index: history.index,
      entries: history.entries.map((entry) => ({
        location: formatLocation(entry.location),
        title: entry.title,
      })),
    }
  })
  on('wm.apps.list', () => ({
    apps: WM_APPS.map((app) => ({
      appId: app.id,
      title: t(app.titleKey),
      multiInstance: app.multiInstance,
      locations: [...new Set(flattenRoutes(getAppRoutes(app.id) ?? []))],
    })),
  }))
  on('wm.actions.list', () => ({
    actions: ALL_ACTIONS.map((action) => ({
      id: action.id,
      title: t(action.titleKey),
      description: action.description,
      input: action.input,
      result: action.result,
      target: action.target,
      scope: action.scope,
      effect: action.effect,
      agentCallable: action.agentCallable,
    })),
  }))
}
