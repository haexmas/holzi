import { getAppRoutes, type AppRouteRecord } from '~/components/shell/appRoutes'
import {
  requestCloseTab,
  requestCloseWindow,
  requestDeleteWorkspace,
} from '~/composables/useShellTab'
import { ALL_ACTIONS } from '~/lib/actions/catalog'
import { getAppDefinition, SHELL_APPS } from '~/lib/shell/apps'
import { clampGeometry } from '~/lib/shell/geometry'
import { formatLocation, currentLocation } from '~/lib/shell/navigation'
import type { ShellTab, ShellWindow } from '~/lib/shell/types'
import type { useShellStore } from '~/stores/shell'

type ShellStore = ReturnType<typeof useShellStore>
export type Translate = (
  key: string,
  params?: Record<string, unknown>,
) => string

/** A declined user confirmation fails the action (contracts/shell-actions.md §1). */
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
 * Global handlers of the Shell layout and read actions (spec
 * 020-tab-navigation, T047, `lib/actions/shellLayoutActions.ts`). Closing and
 * deleting go through the same confirmation helpers as the UI, so an agent
 * never closes a running reply without the user's consent.
 */
export function registerShellLayoutHandlers(
  shell: ShellStore,
  t: Translate,
): void {
  const on = shell.registerGlobalActionHandler
  const windowOfTab = (tabId: string) =>
    shell.windows.find((w) => w.tabs.some((tab) => tab.id === tabId))

  on('shell.tab.activate', ({ target }) => {
    const window = windowOfTab(target.tabId ?? '')
    if (!window) throw new Error('tab not found')
    shell.switchTab(window.id, target.tabId ?? '')
    shell.focusWindow(window.id)
    return { done: true }
  })
  on('shell.tab.close', async ({ target }) => {
    const window = windowOfTab(target.tabId ?? '')
    if (!window) throw new Error('tab not found')
    return confirmed(
      await requestCloseTab(shell, window.id, target.tabId ?? ''),
    )
  })
  on('shell.window.focus', ({ target }) => {
    shell.focusWindow(target.windowId ?? '')
    return { done: true }
  })
  on('shell.window.minimize', ({ target }) => {
    shell.minimizeWindow(target.windowId ?? '')
    return { done: true }
  })
  on('shell.window.toggleMaximize', ({ target }) => {
    shell.toggleMaximizeWindow(target.windowId ?? '')
    return { done: true }
  })
  on('shell.window.setGeometry', ({ target, input }) => {
    const window = shell.windows.find((w) => w.id === target.windowId)
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
    shell.updateWindowGeometry(
      window.id,
      clampGeometry(rect, minSize, shell.area),
    )
    return { done: true }
  })
  on('shell.window.close', async ({ target }) =>
    confirmed(await requestCloseWindow(shell, target.windowId ?? '')),
  )
  on('shell.window.moveToWorkspace', ({ target, input }) => {
    const to = String(input.toWorkspaceId)
    if (!shell.workspaces.some((w) => w.id === to))
      throw new Error(`no workspace ${to}`)
    shell.moveWindowToWorkspace(target.windowId ?? '', to)
    return { done: true }
  })
  on('shell.workspace.create', async () => {
    const workspace = await shell.createWorkspace()
    shell.switchWorkspace(workspace.id)
    return { workspaceId: workspace.id }
  })
  on('shell.workspace.switch', ({ target }) => {
    shell.switchWorkspace(target.workspaceId ?? '')
    return { done: true }
  })
  on('shell.workspace.delete', async ({ target }) => {
    if (shell.workspaces.length <= 1)
      throw new Error('the last workspace cannot be deleted')
    return confirmed(
      await requestDeleteWorkspace(shell, target.workspaceId ?? ''),
    )
  })
  on('shell.windows.overview', () => {
    shell.overlays.windows = true
    return { done: true }
  })
  on('shell.workspaces.overview', () => {
    shell.overlays.workspaces = true
    return { done: true }
  })
  on('shell.launcher.open', () => {
    shell.overlays.launcher = true
    return { done: true }
  })

  function tabTitle(tab: ShellTab): string {
    const info = shell.tabDisplayInfo(tab)
    return (
      info.titleOverride ??
      (info.titleKey ? t(info.titleKey, info.titleParams) : '')
    )
  }
  function describeWindow(window: ShellWindow) {
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
        const history = shell.historyOf(tab.id)
        return {
          tabId: tab.id,
          appId: tab.appId,
          location: history ? formatLocation(currentLocation(history)) : '/',
          title: tabTitle(tab),
          attention: shell.tabDisplayInfo(tab).hasAttention,
        }
      }),
    }
  }

  on('shell.state.get', () => ({
    activeWorkspaceId: shell.activeWorkspaceId,
    activeWindowId: shell.activeWindowId,
    compact: shell.compact,
    workspaces: shell.workspaces.map((workspace) => ({
      workspaceId: workspace.id,
      title: t('shell.workspaces.numbered', { number: workspace.position + 1 }),
      windowCount: shell.windows.filter((w) => w.workspaceId === workspace.id)
        .length,
    })),
    windows: shell.windows.map(describeWindow),
  }))
  on('shell.tab.history', ({ target }) => {
    const history = shell.historyOf(target.tabId ?? '')
    if (!history) throw new Error('tab not found')
    return {
      index: history.index,
      entries: history.entries.map((entry) => ({
        location: formatLocation(entry.location),
        title: entry.title,
      })),
    }
  })
  on('shell.apps.list', () => ({
    apps: SHELL_APPS.map((app) => ({
      appId: app.id,
      title: t(app.titleKey),
      multiInstance: app.multiInstance,
      locations: [...new Set(flattenRoutes(getAppRoutes(app.id) ?? []))],
    })),
  }))
  on('shell.actions.list', () => ({
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
