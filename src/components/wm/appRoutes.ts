import { defineAsyncComponent, type Component } from 'vue'
import { settingsRoutePatterns } from '~/lib/settings/registry'
import { getAppDefinition } from '~/lib/wm/apps'
import { locationTitle, type RoutePattern } from '~/lib/wm/routeMatch'

/**
 * Per-app route tables for tab navigation (spec 020-tab-navigation, T020,
 * research R4, contracts/tab-navigation-contract.md §1) — replaces 015's
 * `appComponents.ts`. Kept separate from `lib/wm/apps.ts` so that pure
 * module stays free of Vue. Each component is built once at module load, so
 * Vue's async-component cache is keyed per app rather than reset per render.
 *
 * A record's `component` is optional below the root: an app can keep one
 * mounted root component and just read `useTabRouter().route` (the chat does,
 * so a conversation switch never remounts it and loses a running reply).
 */
export type AppRouteRecord = RoutePattern & {
  component?: Component
  children?: readonly AppRouteRecord[]
}

const ChatApp = defineAsyncComponent(
  () => import('~/components/apps/ChatApp.vue'),
)
const SettingsApp = defineAsyncComponent(
  () => import('~/components/apps/SettingsApp.vue'),
)

/** The view of each settings location (spec 023-settings-app, contracts §1). */
const SETTINGS_VIEWS: Record<string, Component> = {
  general: defineAsyncComponent(
    () => import('~/components/settings/GeneralView.vue'),
  ),
  appearance: defineAsyncComponent(
    () => import('~/components/settings/ColorSchemeSetting.vue'),
  ),
  models: defineAsyncComponent(
    () => import('~/components/settings/OverviewView.vue'),
  ),
  'models.default': defineAsyncComponent(
    () => import('~/components/settings/DefaultModelSetting.vue'),
  ),
  'models.installed': defineAsyncComponent(
    () => import('~/components/settings/InstalledModels.vue'),
  ),
  'models.download': defineAsyncComponent(
    () => import('~/components/settings/DownloadModels.vue'),
  ),
  'models.download.search': defineAsyncComponent(
    () => import('~/components/models/HuggingFaceSearch.vue'),
  ),
  'models.download.repo': defineAsyncComponent(
    () => import('~/components/models/HuggingFaceFilePicker.vue'),
  ),
  'models.speech': defineAsyncComponent(
    () => import('~/components/settings/SttModelSetting.vue'),
  ),
  agents: defineAsyncComponent(
    () => import('~/components/settings/OverviewView.vue'),
  ),
  'agents.providers': defineAsyncComponent(
    () => import('~/components/settings/ConnectDelegateProvider.vue'),
  ),
  'agents.autonomy': defineAsyncComponent(
    () => import('~/components/settings/AutonomyModeSetting.vue'),
  ),
  'agents.denyRules': defineAsyncComponent(
    () => import('~/components/settings/DelegateDenyRulesSetting.vue'),
  ),
  federation: defineAsyncComponent(
    () => import('~/components/settings/FederationView.vue'),
  ),
  'federation.link': defineAsyncComponent(
    () => import('~/components/settings/LinkDeviceView.vue'),
  ),
  'federation.remove': defineAsyncComponent(
    () => import('~/components/settings/RemoveDeviceView.vue'),
  ),
}

/** The settings routes from the registry: `SettingsApp` as the frame, one flat child per location. */
function settingsRoutes(): AppRouteRecord[] {
  return settingsRoutePatterns().map((root) => ({
    path: root.path,
    component: SettingsApp,
    children: (root.children ?? []).map((child) => ({
      path: child.path,
      titleKey: child.titleKey,
      component: SETTINGS_VIEWS[child.locationId ?? ''],
    })),
  }))
}

const APP_ROUTES: Record<string, readonly AppRouteRecord[]> = {
  // Chat (research R10): one mounted root; children only carry the location.
  'system.chat': [
    {
      path: '/',
      component: ChatApp,
      children: [
        { path: '' },
        { path: 'thread/:id', titleKey: 'wm.chat.thread' },
      ],
    },
  ],
  'system.settings': settingsRoutes(),
}

/** `undefined` for an `appId` no routes are registered for — `wm/TabPanel.vue` then renders
 * nothing, the same as an app the registry does not know (015 research R9). */
export function getAppRoutes(
  appId: string,
): readonly AppRouteRecord[] | undefined {
  return APP_ROUTES[appId]
}

/** The i18n key (and params) titling a location of an app (spec 020 research R9). */
export function titleForLocation(
  appId: string,
  path: string,
): { key: string | undefined; params: Record<string, string> } {
  return locationTitle(
    getAppRoutes(appId) ?? [],
    path,
    getAppDefinition(appId)?.titleKey,
  )
}
