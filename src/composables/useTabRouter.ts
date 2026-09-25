import { computed, reactive, type InjectionKey } from 'vue'
import { getAppRoutes } from '~/components/shell/appRoutes'
import { useShellTab } from '~/composables/useShellTab'
import {
  canGoBack,
  canGoForward,
  currentLocation,
  parseLocation,
  withQuery,
  type TabLocation,
} from '~/lib/shell/navigation'
import { matchRoute } from '~/lib/shell/routeMatch'

export type TabRoute = {
  path: string
  query: Record<string, string>
  params: Record<string, string>
  matched: readonly { path: string; titleKey?: string }[]
}

export type TabRouter = {
  readonly route: TabRoute
  readonly canGoBack: boolean
  readonly canGoForward: boolean
  push(to: string | TabLocation): void
  replace(to: string | TabLocation): void
  setQuery(
    patch: Record<string, string | null>,
    options?: { push?: boolean },
  ): void
  back(): void
  forward(): void
  /** Drops the current entry because its target no longer exists and moves on in the last travel
   * direction (FR-027); `false` if it was the only entry. */
  skipCurrent(): boolean
}

/** Nesting depth of `ShellRouterView` (like vue-router's `RouterView`): the root one in
 * `ShellTabPanel.vue` renders chain[0], a nested one chain[1], and so on (research R4). */
export const ROUTER_DEPTH_KEY: InjectionKey<number> = Symbol('shellRouterDepth')

const START_ROUTE: TabRoute = { path: '/', query: {}, params: {}, matched: [] }

/** Outside a Shell tab (e.g. the Node test harnesses) apps still mount: a router pinned to `/`. */
const INERT_ROUTER: TabRouter = {
  route: START_ROUTE,
  canGoBack: false,
  canGoForward: false,
  push() {},
  replace() {},
  setQuery() {},
  back() {},
  forward() {},
  skipCurrent: () => false,
}

/**
 * The app-facing router of one tab (spec 020-tab-navigation, T018,
 * contracts/tab-navigation-contract.md §2). Reads and changes only the own
 * tab's history in the Shell store (FR-008). `push` is for a new view,
 * `replace`/`setQuery` for the same view shown differently (FR-004, FR-005).
 */
export function useTabRouter(): TabRouter {
  const tab = useShellTab()
  if (!tab.tabId) return INERT_ROUTER
  const shell = useShellStore()
  const tabId = tab.tabId

  const history = computed(() => shell.historyOf(tabId))
  const route = computed<TabRoute>(() => {
    const current = history.value
    if (!current) return START_ROUTE
    const location = currentLocation(current)
    const match = matchRoute(getAppRoutes(tab.appId) ?? [], location.path)
    return {
      path: location.path,
      query: location.query,
      params: match?.params ?? {},
      matched: (match?.chain ?? []).map((record) => ({
        path: record.path,
        titleKey: record.titleKey,
      })),
    }
  })

  function setQuery(
    patch: Record<string, string | null>,
    options: { push?: boolean } = {},
  ) {
    shell.navigate(
      tabId,
      withQuery({ path: route.value.path, query: route.value.query }, patch),
      { replace: !options.push },
    )
  }

  return reactive({
    route,
    canGoBack: computed(() =>
      history.value ? canGoBack(history.value) : false,
    ),
    canGoForward: computed(() =>
      history.value ? canGoForward(history.value) : false,
    ),
    push: (to: string | TabLocation) => {
      shell.navigate(tabId, parseLocation(to))
    },
    replace: (to: string | TabLocation) => {
      shell.navigate(tabId, parseLocation(to), { replace: true })
    },
    setQuery,
    back: () => {
      shell.goTab(tabId, -1)
    },
    forward: () => {
      shell.goTab(tabId, 1)
    },
    skipCurrent: () => shell.skipCurrent(tabId),
  }) as TabRouter
}
